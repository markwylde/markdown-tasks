pub mod app;
pub mod input;
pub mod kanban;
pub mod projection;
pub mod theme;
pub mod ui;

use std::{
    env,
    io::{self, Write},
    panic,
    path::Path,
    sync::{
        Arc, Once,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, DisableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{
    cli::ColorWhen,
    discover::{DEFAULT_IGNORED_DIRECTORIES, DiscoverOptions},
    error::MdtError,
    model::{ScanStats, WorkspaceSnapshot},
    plain::single_line,
    snapshot::build_snapshot,
    watch::{
        EventFilter, NotifyWatcher, RefreshCoordinator, RefreshEvent, RefreshStatus, ScanError,
        ThreadScanWorker,
    },
};

use self::{
    app::{AppState, Command, Focus, SortMode, StatusFilter, ViewMode},
    input::handle_key,
    kanban::{CardStatus, KanbanOptions, KanbanProjection, project_kanban},
    projection::{ListProjection, RowKind, RowStats, VisibleRow, project_and_reconcile},
    theme::Theme,
    ui::{
        UiKanban, UiKanbanCard, UiKanbanColumn, UiKanbanGroup, UiModel, UiRow, UiRowKind, UiState,
        UiSummary,
    },
};

static PANIC_HOOK: Once = Once::new();
static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Restores every terminal feature `mdt` enables for interactive mode.
///
/// # Errors
///
/// Returns an I/O error when terminal control sequences cannot be written.
pub fn restore_terminal() -> io::Result<()> {
    let _ = disable_raw_mode();
    let mut output = io::stdout();
    execute!(output, DisableMouseCapture, Show, LeaveAlternateScreen)?;
    output.flush()
}

/// RAII owner for raw mode and the alternate screen.
#[derive(Debug)]
pub struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    /// Enter interactive terminal mode after all target validation succeeds.
    ///
    /// # Errors
    ///
    /// Returns a terminal error when raw mode or the alternate screen cannot be enabled.
    pub fn enter() -> Result<Self, MdtError> {
        install_panic_restore_hook();
        enable_raw_mode().map_err(|error| MdtError::Terminal(error.to_string()))?;

        let mut output = io::stdout();
        if let Err(error) = execute!(output, EnterAlternateScreen, Hide) {
            let _ = restore_terminal();
            return Err(MdtError::Terminal(error.to_string()));
        }
        if let Err(error) = output.flush() {
            let _ = restore_terminal();
            return Err(MdtError::Terminal(error.to_string()));
        }
        TERMINAL_ACTIVE.store(true, Ordering::Release);
        Ok(Self { active: true })
    }

    /// Restore early and make subsequent drops harmless.
    ///
    /// # Errors
    ///
    /// Returns a terminal error when restoration control sequences cannot be written.
    pub fn restore(&mut self) -> Result<(), MdtError> {
        if self.active {
            restore_terminal().map_err(|error| MdtError::Terminal(error.to_string()))?;
            self.active = false;
            TERMINAL_ACTIVE.store(false, Ordering::Release);
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = restore_terminal();
            self.active = false;
            TERMINAL_ACTIVE.store(false, Ordering::Release);
        }
    }
}

fn install_panic_restore_hook() {
    PANIC_HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |information| {
            if TERMINAL_ACTIVE.swap(false, Ordering::AcqRel) {
                let _ = restore_terminal();
            }
            previous(information);
        }));
    });
}

/// Run the interactive application.
///
/// The full event-loop integration is kept here so terminal ownership cannot
/// leak into the pure state and rendering modules.
///
/// # Errors
///
/// Returns terminal or signal setup errors. Scan failures after startup remain
/// visible in the UI while the last good snapshot is retained.
#[allow(clippy::too_many_lines)] // Terminal ownership is clearest in one scoped event loop.
pub fn run(
    target: &Path,
    display_target: &Path,
    options: &DiscoverOptions,
    color: ColorWhen,
) -> Result<(), MdtError> {
    let mut guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal =
        Terminal::new(backend).map_err(|error| MdtError::Terminal(error.to_string()))?;
    terminal
        .clear()
        .map_err(|error| MdtError::Terminal(error.to_string()))?;

    let scan_target = target.to_path_buf();
    let scan_options = options.clone();
    let worker = ThreadScanWorker::new(move || {
        build_snapshot(&scan_target, &scan_options).map_err(scan_error)
    });
    let mut refresh = RefreshCoordinator::new(worker);
    refresh.manual_refresh();
    let (mut watcher, mut watcher_error) = match NotifyWatcher::new(target) {
        Ok(watcher) => (Some(watcher), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let mut next_watcher_retry = Instant::now() + Duration::from_secs(2);
    let mut refresh_status = RefreshStatus::UpToDate;
    let filter = EventFilter::new(target.to_path_buf(), target.is_file())
        .with_ignored_directories(watch_ignore_names(options));
    let mut snapshot = WorkspaceSnapshot::new(
        target.to_path_buf(),
        Vec::new(),
        ScanStats::default(),
        Vec::new(),
    );
    let mut initial_scan = true;
    let mut initial_error = None;
    let mut app = AppState::default();
    let mut running = true;
    let termination = Arc::new(AtomicBool::new(false));
    #[cfg(unix)]
    install_signal_flags(&termination)?;

    while running && !termination.load(Ordering::Acquire) {
        let now = Instant::now();
        if watcher.is_none() && now >= next_watcher_retry {
            match NotifyWatcher::new(target) {
                Ok(updated) => {
                    watcher = Some(updated);
                    watcher_error = None;
                }
                Err(error) => watcher_error = Some(error.to_string()),
            }
            next_watcher_retry = now + Duration::from_secs(2);
        }
        if let Some(watcher) = &watcher {
            refresh.drain_watcher(watcher, &filter, now);
        }
        refresh.poll(now);
        let refresh_events = refresh.drain_events().collect::<Vec<_>>();
        for refresh_event in refresh_events {
            match refresh_event {
                RefreshEvent::Snapshot(updated) => {
                    snapshot = updated;
                    initial_scan = false;
                    initial_error = None;
                    match NotifyWatcher::new(target) {
                        Ok(updated) => {
                            watcher = Some(updated);
                            watcher_error = None;
                        }
                        Err(error) => {
                            watcher = None;
                            watcher_error = Some(error.to_string());
                            next_watcher_retry = now + Duration::from_secs(2);
                        }
                    }
                }
                RefreshEvent::Status(RefreshStatus::WatcherError(message)) => {
                    watcher = None;
                    watcher_error = Some(message);
                    next_watcher_retry = now + Duration::from_secs(2);
                }
                RefreshEvent::Status(RefreshStatus::RefreshError(message)) if initial_scan => {
                    initial_scan = false;
                    initial_error = Some(message);
                }
                RefreshEvent::Status(status) => refresh_status = status,
            }
        }

        let mut view = project_view(&snapshot, &app);
        app.reconcile_selection(view.rows());
        let terminal_size = terminal
            .size()
            .map_err(|error| MdtError::Terminal(error.to_string()))?;
        app.set_viewport_height(
            usize::from(terminal_size.height.saturating_sub(6)),
            view.rows().len(),
        );
        view = project_view(&snapshot, &app);
        app.reconcile_selection(view.rows());

        let model = build_ui_model(
            &snapshot,
            display_target,
            &app,
            &view,
            UiRuntimeStatus {
                refresh: &refresh_status,
                watcher_error: watcher_error.as_deref(),
                initial_scan,
                initial_error: initial_error.as_deref(),
            },
        );
        let colors = match color {
            ColorWhen::Always => true,
            ColorWhen::Never => false,
            ColorWhen::Auto => {
                env::var_os("NO_COLOR").is_none()
                    && env::var("TERM").map_or(true, |term| term != "dumb")
            }
        };
        let unicode = env::var("TERM").map_or(true, |term| term != "dumb");
        let light_background = env::var("COLORFGBG")
            .ok()
            .and_then(|value| value.rsplit(';').next()?.parse::<u8>().ok())
            .is_some_and(|background| background >= 7);
        let theme = Theme::new(colors, unicode).with_light_background(light_background);
        terminal
            .draw(|frame| ui::render(frame, &model, &theme))
            .map_err(|error| MdtError::Terminal(error.to_string()))?;

        if event::poll(Duration::from_millis(50))
            .map_err(|error| MdtError::Terminal(error.to_string()))?
        {
            match event::read().map_err(|error| MdtError::Terminal(error.to_string()))? {
                Event::Key(key) => match handle_key(&mut app, view.rows(), key) {
                    Some(Command::Quit) => running = false,
                    Some(Command::Refresh) => {
                        if initial_error.is_some() {
                            initial_scan = true;
                            initial_error = None;
                        }
                        if watcher.is_none() {
                            match NotifyWatcher::new(target) {
                                Ok(updated) => {
                                    watcher = Some(updated);
                                    watcher_error = None;
                                }
                                Err(error) => watcher_error = Some(error.to_string()),
                            }
                        }
                        refresh.manual_refresh();
                    }
                    None => {}
                },
                Event::Resize(_, _) => {
                    terminal
                        .autoresize()
                        .map_err(|error| MdtError::Terminal(error.to_string()))?;
                }
                Event::FocusGained | Event::FocusLost | Event::Mouse(_) | Event::Paste(_) => {}
            }
        }
    }

    refresh.shutdown();
    drop(terminal);
    guard.restore()
}

enum ProjectedView {
    List(ListProjection),
    Kanban {
        projection: KanbanProjection,
        rows: Vec<VisibleRow>,
    },
}

impl ProjectedView {
    fn rows(&self) -> &[VisibleRow] {
        match self {
            Self::List(projection) => &projection.rows,
            Self::Kanban { rows, .. } => rows,
        }
    }

    fn no_match_message(&self) -> Option<&str> {
        match self {
            Self::List(projection) => projection.no_match_message.as_deref(),
            Self::Kanban { projection, .. } => projection.no_match_message.as_deref(),
        }
    }
}

fn project_view(snapshot: &WorkspaceSnapshot, app: &AppState) -> ProjectedView {
    match app.view {
        ViewMode::List => {
            let mut state = app.clone();
            let projection = project_and_reconcile(snapshot, &mut state);
            ProjectedView::List(projection)
        }
        ViewMode::Kanban => {
            let projection = project_kanban(
                snapshot,
                KanbanOptions {
                    query: &app.search,
                    sort: app.sort,
                    group_by_file: app.group_kanban_by_file,
                },
            );
            let rows = projection
                .boards
                .iter()
                .flat_map(|board| {
                    let status = CardStatus::VISUAL_ORDER[app.kanban_column.min(2)];
                    board.column(status).cards.iter().map(|card| VisibleRow {
                        key: card.key.clone(),
                        parent_key: None,
                        kind: RowKind::Section,
                        depth: 0,
                        label: card.title.clone(),
                        stats: Some(RowStats {
                            total: card.progress.total,
                            completed: card.progress.completed,
                        }),
                        checked: None,
                        task_depth: 0,
                        expandable: false,
                    })
                })
                .collect();
            ProjectedView::Kanban { projection, rows }
        }
    }
}

#[derive(Clone, Copy)]
struct UiRuntimeStatus<'a> {
    refresh: &'a RefreshStatus,
    watcher_error: Option<&'a str>,
    initial_scan: bool,
    initial_error: Option<&'a str>,
}

#[allow(clippy::too_many_lines)] // This is a pure adapter from application state to UI roles.
fn build_ui_model(
    snapshot: &WorkspaceSnapshot,
    display_target: &Path,
    app: &AppState,
    view: &ProjectedView,
    runtime: UiRuntimeStatus<'_>,
) -> UiModel {
    let UiRuntimeStatus {
        refresh: refresh_status,
        watcher_error,
        initial_scan,
        initial_error,
    } = runtime;
    let stats = snapshot.aggregate_stats();
    let scan = snapshot.scan_stats();
    let no_match = view.no_match_message();
    let ui_state = if initial_scan {
        UiState::Scanning
    } else if let Some(message) = initial_error {
        UiState::FatalError(message.to_owned())
    } else if matches!(refresh_status, RefreshStatus::Refreshing) {
        UiState::Refreshing
    } else if let Some(message) = no_match {
        UiState::NoMatches(
            message
                .strip_prefix("No tasks match ")
                .unwrap_or(message)
                .trim_matches('"')
                .to_owned(),
        )
    } else if stats.total() == 0 {
        UiState::Empty
    } else {
        UiState::Ready
    };

    let warning = (!snapshot.warnings().is_empty()).then(|| {
        format!(
            "{} scan warning{}",
            snapshot.warnings().len(),
            if snapshot.warnings().len() == 1 {
                ""
            } else {
                "s"
            }
        )
    });
    let last_refresh_error = watcher_error
        .map(|message| {
            format!(
                "watch unavailable: {}; press r to retry",
                single_line(message, "unknown watcher error")
            )
        })
        .or_else(|| match refresh_status {
            RefreshStatus::WatcherError(message) | RefreshStatus::RefreshError(message) => {
                Some(single_line(message, "refresh failed"))
            }
            RefreshStatus::TargetMissing => Some("target missing; press r to retry".to_owned()),
            RefreshStatus::UpToDate
            | RefreshStatus::Refreshing
            | RefreshStatus::UpdatedJustNow
            | RefreshStatus::ShuttingDown => None,
        });

    let (rows, kanban) = match view {
        ProjectedView::List(projection) => (
            projection.rows.iter().map(|row| ui_row(row, app)).collect(),
            None,
        ),
        ProjectedView::Kanban { projection, .. } => (
            Vec::new(),
            Some(ui_kanban(projection, app.selected_key(), app.kanban_column)),
        ),
    };

    UiModel {
        title: "mdt".to_owned(),
        root: single_line(&display_path(display_target), "."),
        status: if watcher_error.is_some() {
            format!("manual refresh · {refresh_status}")
        } else {
            format!("live · {refresh_status}")
        },
        summary: UiSummary {
            completed: stats.completed(),
            total: stats.total(),
            complete_files: scan.complete_files,
            task_files: scan.task_files,
            scanned_files: scan.markdown_files,
            scanned_directories: scan.directories,
            ignored_directories: scan.ignored_directories,
        },
        rows,
        kanban,
        selected: app.selection.as_ref().map(|selection| selection.index),
        scroll: app.viewport_offset,
        view: match app.view {
            ViewMode::List => "List",
            ViewMode::Kanban => "Kanban",
        }
        .to_owned(),
        filter: match app.filter {
            StatusFilter::All => "All",
            StatusFilter::Remaining => "Remaining",
            StatusFilter::Done => "Done",
        }
        .to_owned(),
        sort: match app.sort {
            SortMode::Progress => "Progress",
            SortMode::Name => "Name",
        }
        .to_owned(),
        search: (!app.search.is_empty() || app.focus == Focus::Search).then(|| app.search.clone()),
        collapsed: !app.collapsed.is_empty(),
        state: ui_state,
        warning,
        last_refresh_error,
        help: app.focus == Focus::Help,
    }
}

fn ui_row(row: &VisibleRow, app: &AppState) -> UiRow {
    let (completed, total) = row.stats.map_or_else(
        || (usize::from(row.checked == Some(true)), 1),
        |stats| (stats.completed, stats.total),
    );
    UiRow {
        kind: match row.kind {
            RowKind::Document => UiRowKind::Document,
            RowKind::Section => UiRowKind::Section,
            RowKind::Task => UiRowKind::Task,
        },
        label: single_line(&row.label, "(untitled task)"),
        depth: row.depth + row.task_depth,
        completed,
        total,
        expanded: row
            .expandable
            .then(|| !app.collapsed.contains(&row.key) || !app.search.trim().is_empty()),
    }
}

fn ui_kanban(
    projection: &KanbanProjection,
    selected_key: Option<&str>,
    active_column: usize,
) -> UiKanban {
    let mut columns = Vec::new();
    for status in CardStatus::VISUAL_ORDER {
        let groups = projection
            .boards
            .iter()
            .map(|board| {
                let grouped = board.file.is_some();
                let cards = board
                    .column(status)
                    .cards
                    .iter()
                    .map(|card| {
                        let selected = selected_key == Some(card.key.as_str());
                        let mut context = Vec::new();
                        if !grouped {
                            context.push(single_line(&card.file.relative_path, "(unnamed file)"));
                        }
                        context.extend(
                            card.breadcrumbs
                                .iter()
                                .map(|value| single_line(value, "(untitled)")),
                        );
                        UiKanbanCard {
                            title: single_line(&card.title, "(untitled task group)"),
                            context: context.join(" › "),
                            completed: card.progress.completed,
                            total: card.progress.total,
                            selected,
                        }
                    })
                    .collect();
                UiKanbanGroup {
                    title: board
                        .file
                        .as_ref()
                        .map(|file| single_line(&file.relative_path, "(unnamed file)")),
                    cards,
                }
            })
            .collect();
        columns.push(UiKanbanColumn {
            title: status.title().to_owned(),
            groups,
        });
    }
    UiKanban {
        columns,
        active_column: active_column.min(2),
    }
}

fn watch_ignore_names(options: &DiscoverOptions) -> Vec<String> {
    let mut names = if options.no_default_ignore {
        Vec::new()
    } else {
        DEFAULT_IGNORED_DIRECTORIES
            .iter()
            .map(ToString::to_string)
            .collect()
    };
    names.extend(options.ignore.iter().cloned());
    names
}

fn scan_error(error: MdtError) -> ScanError {
    match error {
        MdtError::TargetMissing(_) => ScanError::TargetMissing,
        other => ScanError::Failed(other.to_string()),
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(unix)]
fn install_signal_flags(termination: &Arc<AtomicBool>) -> Result<(), MdtError> {
    use signal_hook::consts::signal::{SIGINT, SIGTERM};

    signal_hook::flag::register(SIGINT, Arc::clone(termination))
        .and_then(|_| signal_hook::flag::register(SIGTERM, Arc::clone(termination)))
        .map(|_| ())
        .map_err(|error| MdtError::Terminal(format!("cannot install signal handler: {error}")))
}
