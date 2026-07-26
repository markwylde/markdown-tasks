use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::theme::{Role, Theme};

const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 10;
const RECOMMENDED_WIDTH: u16 = 80;
const RECOMMENDED_HEIGHT: u16 = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LayoutClass {
    TooSmall,
    Narrow,
    Medium,
    Wide,
}

impl LayoutClass {
    #[must_use]
    pub(crate) const fn for_size(width: u16, height: u16) -> Self {
        if width < MIN_WIDTH || height < MIN_HEIGHT {
            Self::TooSmall
        } else if width < 60 {
            Self::Narrow
        } else if width < 100 {
            Self::Medium
        } else {
            Self::Wide
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct UiSummary {
    pub(crate) completed: usize,
    pub(crate) total: usize,
    pub(crate) complete_files: usize,
    pub(crate) task_files: usize,
    pub(crate) scanned_files: usize,
    pub(crate) scanned_directories: usize,
    pub(crate) ignored_directories: usize,
}

impl UiSummary {
    #[must_use]
    pub(crate) fn remaining(&self) -> usize {
        self.total.saturating_sub(self.completed)
    }

    #[must_use]
    pub(crate) fn percent(&self) -> usize {
        if self.total == 0 {
            0
        } else {
            ((self.completed.saturating_mul(100) + self.total / 2) / self.total).min(100)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // Populated by the app/projection adapter during integration.
pub(crate) enum UiRowKind {
    Document,
    Section,
    Task,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UiRow {
    pub(crate) kind: UiRowKind,
    pub(crate) label: String,
    pub(crate) depth: usize,
    pub(crate) completed: usize,
    pub(crate) total: usize,
    /// `Some` marks an expandable row and describes its current state.
    pub(crate) expanded: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct UiKanbanCard {
    pub(crate) title: String,
    pub(crate) context: String,
    pub(crate) completed: usize,
    pub(crate) total: usize,
    pub(crate) selected: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct UiKanbanColumn {
    pub(crate) title: String,
    pub(crate) groups: Vec<UiKanbanGroup>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct UiKanbanGroup {
    pub(crate) title: Option<String>,
    pub(crate) cards: Vec<UiKanbanCard>,
}

impl UiKanbanColumn {
    fn card_count(&self) -> usize {
        self.groups.iter().map(|group| group.cards.len()).sum()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct UiKanban {
    pub(crate) columns: Vec<UiKanbanColumn>,
    pub(crate) active_column: usize,
}

impl UiRow {
    #[must_use]
    pub(crate) fn complete(&self) -> bool {
        self.total > 0 && self.completed >= self.total
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)] // Populated by the app/projection adapter during integration.
pub(crate) enum UiState {
    Scanning,
    Refreshing,
    Ready,
    Empty,
    NoMatches(String),
    FatalError(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UiModel {
    pub(crate) title: String,
    pub(crate) root: String,
    pub(crate) status: String,
    pub(crate) summary: UiSummary,
    pub(crate) rows: Vec<UiRow>,
    pub(crate) kanban: Option<UiKanban>,
    pub(crate) selected: Option<usize>,
    pub(crate) scroll: usize,
    pub(crate) view: String,
    pub(crate) filter: String,
    pub(crate) sort: String,
    pub(crate) search: Option<String>,
    pub(crate) collapsed: bool,
    pub(crate) state: UiState,
    pub(crate) warning: Option<String>,
    pub(crate) last_refresh_error: Option<String>,
    pub(crate) help: bool,
}

impl Default for UiModel {
    fn default() -> Self {
        Self {
            title: "mdt".into(),
            root: ".".into(),
            status: "live".into(),
            summary: UiSummary::default(),
            rows: Vec::new(),
            kanban: None,
            selected: None,
            scroll: 0,
            view: "List".into(),
            filter: "All".into(),
            sort: "Progress".into(),
            search: None,
            collapsed: false,
            state: UiState::Scanning,
            warning: None,
            last_refresh_error: None,
            help: false,
        }
    }
}

pub(crate) fn render(frame: &mut Frame<'_>, model: &UiModel, theme: &Theme) {
    let area = frame.area();
    if area.is_empty() {
        return;
    }

    let class = LayoutClass::for_size(area.width, area.height);
    if class == LayoutClass::TooSmall {
        render_too_small(frame, area, theme);
        return;
    }

    let summary_height = match class {
        LayoutClass::Wide | LayoutClass::Medium => 3,
        LayoutClass::Narrow => 1,
        LayoutClass::TooSmall => 0,
    };
    let regions = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(summary_height),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    render_title(frame, regions[0], model, theme);
    render_summary(frame, regions[1], model, theme, class);
    render_toolbar(frame, regions[2], model, theme, class);
    render_content(frame, regions[3], model, theme, class);
    render_footer(frame, regions[4], model, theme, class);

    if model.help {
        render_help(frame, area, theme);
    }
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let text = vec![
        Line::from(Span::styled(
            "terminal too small",
            theme.style(Role::Warning),
        )),
        Line::from(format!(
            "{}x{} now; {}x{} recommended",
            area.width, area.height, RECOMMENDED_WIDTH, RECOMMENDED_HEIGHT
        )),
        Line::from(Span::styled(
            "resize or press q to quit",
            theme.style(Role::Muted),
        )),
    ];
    let height = u16::try_from(text.len()).unwrap_or(3).min(area.height);
    let box_area = centered(area, area.width.min(38), height);
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), box_area);
}

fn render_title(frame: &mut Frame<'_>, area: Rect, model: &UiModel, theme: &Theme) {
    if area.is_empty() {
        return;
    }
    let status = model.status.as_str();
    let right_width = UnicodeWidthStr::width(status);
    let fixed = UnicodeWidthStr::width(model.title.as_str()) + right_width + 4;
    let root_width = usize::from(area.width).saturating_sub(fixed);
    let root = middle_elide(&model.root, root_width, theme.glyphs.ellipsis);
    let left = format!(" {} {} {root}", model.title, theme.glyphs.bullet);
    let padding = usize::from(area.width)
        .saturating_sub(UnicodeWidthStr::width(left.as_str()) + right_width + 1);
    let line = Line::from(vec![
        Span::styled(left, theme.style(Role::Accent)),
        Span::raw(" ".repeat(padding)),
        Span::styled(status.to_owned(), theme.style(Role::Muted)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_summary(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &UiModel,
    theme: &Theme,
    class: LayoutClass,
) {
    if area.is_empty() {
        return;
    }
    let summary = &model.summary;
    if class == LayoutClass::Narrow {
        let text = format!(
            " {}%  {}/{} done  {} left",
            summary.percent(),
            summary.completed,
            summary.total,
            summary.remaining()
        );
        frame.render_widget(Paragraph::new(text), area);
        return;
    }

    let first = format!(
        " {}%  {} done  {} remaining  {}/{} files complete",
        summary.percent(),
        summary.completed,
        summary.remaining(),
        summary.complete_files,
        summary.task_files
    );
    let bar_width = usize::from(area.width).saturating_sub(3);
    let bar = progress_bar(summary.completed, summary.total, bar_width, theme);
    let metadata = if class == LayoutClass::Wide {
        format!(
            " {} markdown files {} {} folders watched {} {} ignored",
            summary.scanned_files,
            theme.glyphs.bullet,
            summary.scanned_directories,
            theme.glyphs.bullet,
            summary.ignored_directories
        )
    } else {
        format!(
            " {} files {} {} folders",
            summary.scanned_files, theme.glyphs.bullet, summary.scanned_directories
        )
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(first, theme.style(Role::Normal))),
            Line::from(vec![
                Span::raw(" "),
                Span::styled(bar, theme.style(Role::Accent)),
            ]),
            Line::from(Span::styled(metadata, theme.style(Role::Muted))),
        ]),
        area,
    );
}

fn render_toolbar(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &UiModel,
    theme: &Theme,
    class: LayoutClass,
) {
    if area.is_empty() {
        return;
    }
    let search = model
        .search
        .as_deref()
        .filter(|query| !query.is_empty())
        .map_or_else(|| "/ search".to_owned(), |query| format!("/{query}"));
    let collapse = if model.collapsed {
        "Expand"
    } else {
        "Collapse"
    };
    let filter = if model.kanban.is_some() {
        String::new()
    } else {
        format!(" Filter: {} {}", model.filter, theme.glyphs.bullet)
    };
    let compact_filter = if model.kanban.is_some() {
        String::new()
    } else {
        format!("{} {} ", model.filter, theme.glyphs.bullet)
    };
    let text = match class {
        LayoutClass::Wide => format!(
            " {} {} View: {} {}{filter}{} {} Sort: {} {} {collapse}",
            model.view,
            theme.glyphs.bullet,
            model.view,
            theme.glyphs.bullet,
            search,
            theme.glyphs.bullet,
            model.sort,
            theme.glyphs.bullet
        ),
        LayoutClass::Medium => format!(
            " {} {} {compact_filter}{} {} Sort: {}",
            model.view, theme.glyphs.bullet, search, theme.glyphs.bullet, model.sort
        ),
        LayoutClass::Narrow => format!(" {} {compact_filter}{} {}", model.view, search, model.sort),
        LayoutClass::TooSmall => String::new(),
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            end_elide(&text, usize::from(area.width), theme.glyphs.ellipsis),
            theme.style(Role::Muted),
        )),
        area,
    );
}

fn render_content(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &UiModel,
    theme: &Theme,
    class: LayoutClass,
) {
    if area.is_empty() {
        return;
    }

    let scanning = format!("Scanning markdown files{}", theme.glyphs.ellipsis);
    let refreshing = format!("Refreshing{}", theme.glyphs.ellipsis);
    let status = match &model.state {
        UiState::Scanning => Some((scanning.as_str(), Role::Accent)),
        UiState::Refreshing => Some((refreshing.as_str(), Role::Accent)),
        UiState::Empty => Some(("No tasks found", Role::Muted)),
        UiState::NoMatches(query) => {
            let message = format!("No tasks match \"{query}\"");
            render_centered_status(frame, area, &message, Role::Muted, theme);
            return;
        }
        UiState::FatalError(message) => {
            render_centered_status(
                frame,
                area,
                &format!("Scan failed: {message}"),
                Role::Error,
                theme,
            );
            return;
        }
        UiState::Ready => None,
    };
    if let Some((message, role)) = status {
        render_centered_status(frame, area, message, role, theme);
        return;
    }

    if let Some(kanban) = &model.kanban {
        render_kanban(frame, area, kanban, theme, class);
        return;
    }

    if model.rows.is_empty() {
        render_centered_status(frame, area, "No tasks found", Role::Muted, theme);
        return;
    }

    let height = usize::from(area.height);
    let start = model.scroll.min(model.rows.len().saturating_sub(1));
    let lines = model
        .rows
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(index, row)| {
            row_line(
                row,
                index == model.selected.unwrap_or(usize::MAX),
                area.width,
                class,
                theme,
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_kanban(
    frame: &mut Frame<'_>,
    area: Rect,
    kanban: &UiKanban,
    theme: &Theme,
    class: LayoutClass,
) {
    if kanban.columns.is_empty() {
        render_centered_status(frame, area, "No task groups to show", Role::Muted, theme);
        return;
    }

    if class == LayoutClass::Narrow {
        let index = kanban.active_column.min(kanban.columns.len() - 1);
        render_kanban_column(frame, area, &kanban.columns[index], theme);
        return;
    }

    let constraints = (0..kanban.columns.len())
        .map(|_| Constraint::Ratio(1, u32::try_from(kanban.columns.len()).unwrap_or(1)))
        .collect::<Vec<_>>();
    let columns = Layout::horizontal(constraints).split(area);
    for (column, column_area) in kanban.columns.iter().zip(columns.iter().copied()) {
        render_kanban_column(frame, column_area, column, theme);
    }
}

fn render_kanban_column(frame: &mut Frame<'_>, area: Rect, column: &UiKanbanColumn, theme: &Theme) {
    let width = usize::from(area.width.saturating_sub(4));
    let mut lines = Vec::new();
    let mut selected_line = None;
    for group in &column.groups {
        if let Some(title) = &group.title {
            lines.push(Line::from(Span::styled(
                end_elide(title, width, theme.glyphs.ellipsis),
                theme.style(Role::Accent),
            )));
        }
        for card in &group.cards {
            if !card.context.is_empty() {
                lines.push(Line::from(Span::styled(
                    end_elide(&card.context, width, theme.glyphs.ellipsis),
                    theme.style(Role::Muted),
                )));
            }
            let progress = format!("  {}/{}", card.completed, card.total);
            let title_width = width.saturating_sub(UnicodeWidthStr::width(progress.as_str()));
            let mut line = Line::from(vec![
                Span::styled(
                    end_elide(&card.title, title_width, theme.glyphs.ellipsis),
                    theme.style(if card.completed >= card.total && card.total > 0 {
                        Role::Success
                    } else {
                        Role::Normal
                    }),
                ),
                Span::styled(progress, theme.style(Role::Muted)),
            ]);
            if card.selected {
                selected_line = Some(lines.len());
                line = line.style(theme.style(Role::Selected));
            }
            lines.push(line);
            lines.push(Line::from(""));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No groups",
            theme.style(Role::Muted),
        )));
    }
    let visible_height = usize::from(
        area.height
            .saturating_sub(u16::from(theme.uses_unicode()) * 2),
    );
    let start = selected_line.map_or(0, |line| {
        line.saturating_sub(visible_height.saturating_sub(1))
    });
    let lines = lines
        .into_iter()
        .skip(start)
        .take(visible_height.max(1))
        .collect::<Vec<_>>();
    let title = format!(" {} ({}) ", column.title, column.card_count());
    let block = if theme.uses_unicode() {
        Block::bordered().title(title)
    } else {
        Block::default().title(title)
    };
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn row_line<'a>(
    row: &'a UiRow,
    selected: bool,
    width: u16,
    class: LayoutClass,
    theme: &Theme,
) -> Line<'a> {
    let indent = "  ".repeat(row.depth.min(20));
    let disclosure = row.expanded.map_or(" ", |expanded| {
        if expanded {
            theme.glyphs.expanded
        } else {
            theme.glyphs.collapsed
        }
    });
    let state = if row.complete() {
        theme.glyphs.completed
    } else {
        theme.glyphs.remaining
    };
    let prefix = format!("{indent}{disclosure} {state} ");
    let counts = format!("{}/{}", row.completed, row.total);
    let bar_width = match class {
        LayoutClass::Wide => 10,
        LayoutClass::Medium => 6,
        LayoutClass::Narrow | LayoutClass::TooSmall => 0,
    };
    let suffix = if bar_width == 0 {
        format!(" {counts}")
    } else {
        format!(
            " {counts} {}",
            progress_bar(row.completed, row.total, bar_width, theme)
        )
    };
    let label_width = usize::from(width).saturating_sub(
        UnicodeWidthStr::width(prefix.as_str()) + UnicodeWidthStr::width(suffix.as_str()),
    );
    let label = end_elide(&row.label, label_width, theme.glyphs.ellipsis);
    let used = UnicodeWidthStr::width(prefix.as_str())
        + UnicodeWidthStr::width(label.as_str())
        + UnicodeWidthStr::width(suffix.as_str());
    let gap = " ".repeat(usize::from(width).saturating_sub(used));
    let role = if row.complete() {
        Role::Success
    } else {
        match row.kind {
            UiRowKind::Task => Role::Normal,
            UiRowKind::Document | UiRowKind::Section => Role::Accent,
        }
    };
    let mut line = Line::from(vec![
        Span::styled(prefix, theme.style(role)),
        Span::styled(label, theme.style(role)),
        Span::raw(gap),
        Span::styled(
            suffix,
            theme.style(if row.complete() {
                Role::Success
            } else {
                Role::Muted
            }),
        ),
    ]);
    if selected {
        line = line.style(theme.style(Role::Selected));
    }
    line
}

fn render_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &UiModel,
    theme: &Theme,
    class: LayoutClass,
) {
    if area.is_empty() {
        return;
    }
    let base = match class {
        LayoutClass::Wide => " j/k move  enter expand  / search  v view  r refresh  ? help  q quit",
        LayoutClass::Medium => " j/k move  enter expand  / search  r refresh  ? help  q quit",
        LayoutClass::Narrow => " j/k move  / find  r scan  ? help  q quit",
        LayoutClass::TooSmall => "",
    };
    let alert = model
        .last_refresh_error
        .as_deref()
        .or(model.warning.as_deref());
    let (text, role) = alert.map_or_else(
        || (base.to_owned(), Role::Muted),
        |message| (format!(" ! {message}  {base}"), Role::Warning),
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            end_elide(&text, usize::from(area.width), theme.glyphs.ellipsis),
            theme.style(role),
        )),
        area,
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let width = area.width.saturating_sub(4).min(60);
    let height = area.height.saturating_sub(2).min(22);
    let popup = centered(area, width, height);
    frame.render_widget(Clear, popup);
    let lines = vec![
        Line::from(Span::styled("Keyboard help", theme.style(Role::Accent))),
        Line::from(""),
        Line::from("j/k, arrows    move selection"),
        Line::from("pgup/pgdn      move one page"),
        Line::from("g/G, home/end  first / last"),
        Line::from("enter/space    expand or collapse"),
        Line::from("left/right     tree parent / Kanban lane"),
        Line::from("z              collapse / expand all"),
        Line::from("/              search"),
        Line::from("esc            leave search, then clear it"),
        Line::from("1/2/3          all / remaining / done"),
        Line::from("v              change view"),
        Line::from("f              group Kanban by file"),
        Line::from("s              change sort"),
        Line::from("r              refresh scan"),
        Line::from("? / esc        close help"),
        Line::from("q / ctrl-c     quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Read-only: Markdown files are never modified.",
            theme.style(Role::Warning),
        )),
    ];
    let block = if theme.uses_unicode() {
        Block::bordered().title(" Help ")
    } else {
        Block::default().title(" Help ")
    };
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(block),
        popup,
    );
}

fn render_centered_status(
    frame: &mut Frame<'_>,
    area: Rect,
    message: &str,
    role: Role,
    theme: &Theme,
) {
    let width = usize::from(area.width).saturating_sub(2);
    let message = end_elide(message, width, theme.glyphs.ellipsis);
    frame.render_widget(
        Paragraph::new(Span::styled(message, theme.style(role))).alignment(Alignment::Center),
        centered(area, area.width, 1),
    );
}

fn progress_bar(completed: usize, total: usize, width: usize, theme: &Theme) -> String {
    if width == 0 {
        return String::new();
    }
    let filled = if total == 0 {
        0
    } else {
        completed.saturating_mul(width).saturating_add(total / 2) / total
    }
    .min(width);
    format!(
        "{}{}",
        theme.glyphs.progress_full.repeat(filled),
        theme.glyphs.progress_empty.repeat(width - filled)
    )
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn end_elide(value: &str, max_width: usize, ellipsis: &str) -> String {
    if UnicodeWidthStr::width(value) <= max_width {
        return value.to_owned();
    }
    let ellipsis_width = UnicodeWidthStr::width(ellipsis);
    if max_width <= ellipsis_width {
        return take_width(ellipsis, max_width);
    }
    let mut result = take_width(value, max_width - ellipsis_width);
    result.push_str(ellipsis);
    result
}

fn middle_elide(value: &str, max_width: usize, ellipsis: &str) -> String {
    if UnicodeWidthStr::width(value) <= max_width {
        return value.to_owned();
    }
    let ellipsis_width = UnicodeWidthStr::width(ellipsis);
    if max_width <= ellipsis_width {
        return take_width(ellipsis, max_width);
    }
    let available = max_width - ellipsis_width;
    let filename_width = value
        .rsplit(['/', '\\'])
        .next()
        .map_or(0, UnicodeWidthStr::width);
    let right_width = filename_width.min(available).max(available.div_ceil(2));
    let left_width = available - right_width;
    let left = take_width(value, left_width);
    let right = take_width_rev(value, right_width);
    format!("{left}{ellipsis}{right}")
}

fn take_width(value: &str, max_width: usize) -> String {
    let mut width = 0;
    value
        .chars()
        .take_while(|character| {
            let character_width = character.width().unwrap_or(0);
            if width + character_width > max_width {
                false
            } else {
                width += character_width;
                true
            }
        })
        .collect()
}

fn take_width_rev(value: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut characters = value
        .chars()
        .rev()
        .take_while(|character| {
            let character_width = character.width().unwrap_or(0);
            if width + character_width > max_width {
                false
            } else {
                width += character_width;
                true
            }
        })
        .collect::<Vec<_>>();
    characters.reverse();
    characters.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    fn rendered(width: u16, height: u16, model: &UiModel) -> Vec<String> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, model, &Theme::new(false, false)))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol().chars().next().unwrap_or(' '))
                    .collect()
            })
            .collect()
    }

    fn ready_model() -> UiModel {
        UiModel {
            root: "specs/tasks".into(),
            status: "live".into(),
            summary: UiSummary {
                completed: 7,
                total: 10,
                complete_files: 1,
                task_files: 2,
                scanned_files: 4,
                scanned_directories: 1,
                ignored_directories: 2,
            },
            rows: vec![
                UiRow {
                    kind: UiRowKind::Document,
                    label: "PLAN.md".into(),
                    depth: 0,
                    completed: 7,
                    total: 10,
                    expanded: Some(true),
                },
                UiRow {
                    kind: UiRowKind::Task,
                    label: "Implement responsive terminal rendering".into(),
                    depth: 1,
                    completed: 0,
                    total: 1,
                    expanded: None,
                },
            ],
            selected: Some(1),
            state: UiState::Ready,
            ..UiModel::default()
        }
    }

    #[test]
    fn classifies_all_responsive_widths() {
        assert_eq!(LayoutClass::for_size(39, 24), LayoutClass::TooSmall);
        assert_eq!(LayoutClass::for_size(40, 9), LayoutClass::TooSmall);
        assert_eq!(LayoutClass::for_size(40, 10), LayoutClass::Narrow);
        assert_eq!(LayoutClass::for_size(60, 10), LayoutClass::Medium);
        assert_eq!(LayoutClass::for_size(100, 10), LayoutClass::Wide);
    }

    #[test]
    fn renders_wide_medium_and_narrow_shells() {
        let model = ready_model();
        let wide = rendered(110, 16, &model).join("\n");
        let medium = rendered(80, 14, &model).join("\n");
        let narrow = rendered(45, 12, &model).join("\n");
        assert!(wide.contains("70%  7 done  3 remaining  1/2 files complete"));
        assert!(wide.contains("markdown files"));
        assert!(wide.contains("j/k move"));
        assert!(medium.contains("70%  7 done  3 remaining"));
        assert!(medium.contains("/ search"));
        assert!(narrow.contains("70%  7/10 done  3 left"));
        assert!(narrow.contains("0/1"));
        assert!(narrow.contains("? help"));
        assert!(!narrow.contains("######"));
    }

    #[test]
    fn renders_too_small_and_zero_area_without_panicking() {
        let model = ready_model();
        let small = rendered(39, 9, &model).join("\n");
        assert!(small.contains("terminal too small"));

        let backend = TestBackend::new(0, 0);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &model, &Theme::default()))
            .unwrap();
    }

    #[test]
    fn renders_deliberate_empty_error_and_help_states() {
        let mut model = ready_model();
        model.state = UiState::Empty;
        assert!(
            rendered(80, 12, &model)
                .join("\n")
                .contains("No tasks found")
        );
        model.state = UiState::NoMatches("database".into());
        assert!(
            rendered(80, 12, &model)
                .join("\n")
                .contains("No tasks match \"database\"")
        );
        model.state = UiState::FatalError("permission denied".into());
        assert!(
            rendered(80, 12, &model)
                .join("\n")
                .contains("Scan failed: permission denied")
        );
        model.state = UiState::Ready;
        model.help = true;
        let help = rendered(80, 26, &model).join("\n");
        assert!(help.contains("Keyboard help"));
        assert!(help.contains("Read-only"));
    }

    #[test]
    fn renders_scan_refresh_warning_and_last_error_statuses() {
        let mut model = ready_model();
        model.state = UiState::Scanning;
        assert!(
            rendered(80, 12, &model)
                .join("\n")
                .contains("Scanning markdown files...")
        );

        model.state = UiState::Refreshing;
        assert!(
            rendered(80, 12, &model)
                .join("\n")
                .contains("Refreshing...")
        );

        model.state = UiState::Ready;
        model.warning = Some("1 file could not be read".into());
        let warning = rendered(100, 12, &model).join("\n");
        assert!(warning.contains("1 file could not be read"));
        assert!(warning.contains("j/k move"));

        model.last_refresh_error = Some("refresh failed; showing previous scan".into());
        let error = rendered(100, 12, &model).join("\n");
        assert!(error.contains("refresh failed; showing previous scan"));
    }

    #[test]
    fn unicode_elision_respects_cell_width() {
        let elided = middle_elide("long/界界界/file.md", 12, "…");
        assert!(UnicodeWidthStr::width(elided.as_str()) <= 12);
        assert!(elided.ends_with("file.md"));

        let combining = end_elide("cafe\u{301}-very-long", 8, "…");
        assert!(UnicodeWidthStr::width(combining.as_str()) <= 8);
    }

    #[test]
    fn renders_wide_kanban_columns_and_narrow_selected_lane() {
        let mut model = ready_model();
        model.kanban = Some(UiKanban {
            columns: vec![
                UiKanbanColumn {
                    title: "Not Started".into(),
                    groups: vec![UiKanbanGroup {
                        title: None,
                        cards: vec![UiKanbanCard {
                            title: "Parser".into(),
                            context: "PLAN.md > Core".into(),
                            completed: 0,
                            total: 3,
                            selected: false,
                        }],
                    }],
                },
                UiKanbanColumn {
                    title: "Started".into(),
                    groups: vec![UiKanbanGroup {
                        title: None,
                        cards: vec![UiKanbanCard {
                            title: "TUI shell".into(),
                            context: "PLAN.md > UI".into(),
                            completed: 2,
                            total: 4,
                            selected: true,
                        }],
                    }],
                },
                UiKanbanColumn {
                    title: "Finished".into(),
                    groups: vec![],
                },
            ],
            active_column: 1,
        });

        let wide = rendered(120, 20, &model).join("\n");
        assert!(wide.contains("Not Started (1)"));
        assert!(wide.contains("Started (1)"));
        assert!(wide.contains("Finished (0)"));
        assert!(wide.contains("TUI shell"));
        assert!(wide.contains("2/4"));
        assert!(!wide.contains("Filter:"));

        let narrow = rendered(45, 16, &model).join("\n");
        assert!(narrow.contains("Started (1)"));
        assert!(narrow.contains("TUI shell"));
        assert!(!narrow.contains("Not Started (1)"));
        assert!(!narrow.contains("Finished (0)"));
    }
}
