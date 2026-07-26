//! Deterministic, terminal-width-independent report rendering.
//!
//! The types in this module are deliberately a small projection of the scan
//! model. Keeping rendering pure over this projection makes it straightforward
//! for snapshot construction to provide a `From<&WorkspaceSnapshot>`
//! implementation without coupling output tests to filesystem traversal.

use std::fmt::{self, Write as _};

use crate::model::{Document, Section, Stats, Task, WorkspaceSnapshot};

/// Aggregate task counts used by workspaces, documents, and sections.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlainStats {
    pub total: usize,
    pub completed: usize,
}

impl PlainStats {
    #[must_use]
    pub const fn remaining(self) -> usize {
        self.total.saturating_sub(self.completed)
    }

    #[must_use]
    pub fn percent(self) -> usize {
        if self.total == 0 {
            0
        } else {
            // Integer arithmetic gives nearest-integer rounding without
            // floating-point platform differences.
            (self.completed.saturating_mul(100) + self.total / 2) / self.total
        }
    }
}

/// Counts collected while discovering Markdown files.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlainScanStats {
    pub markdown_files: usize,
    pub directories: usize,
    pub ignored_directories: usize,
    pub task_files: usize,
    pub complete_task_files: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainWarning {
    pub path: String,
    pub cause: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainTask {
    pub label: String,
    pub checked: bool,
    /// Source nesting depth relative to the task's owning section.
    pub depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainSection {
    /// `None` is the document's implicit root section.
    pub title: Option<String>,
    pub stats: PlainStats,
    pub tasks: Vec<PlainTask>,
    pub children: Vec<PlainSection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainDocument {
    pub relative_path: String,
    pub stats: PlainStats,
    pub root: PlainSection,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlainReport {
    pub aggregate: PlainStats,
    pub scan: PlainScanStats,
    pub warnings: Vec<PlainWarning>,
    pub documents: Vec<PlainDocument>,
}

impl From<Stats> for PlainStats {
    fn from(stats: Stats) -> Self {
        Self {
            total: stats.total(),
            completed: stats.completed(),
        }
    }
}

impl From<&Task> for PlainTask {
    fn from(task: &Task) -> Self {
        Self {
            label: task.label().to_owned(),
            checked: task.checked(),
            depth: task.depth(),
        }
    }
}

impl From<&Section> for PlainSection {
    fn from(section: &Section) -> Self {
        Self {
            title: section.title().map(ToOwned::to_owned),
            stats: section.stats().into(),
            tasks: section.tasks().iter().map(Self::task_from).collect(),
            children: section.children().iter().map(Self::from).collect(),
        }
    }
}

impl PlainSection {
    fn task_from(task: &Task) -> PlainTask {
        PlainTask::from(task)
    }
}

impl From<&Document> for PlainDocument {
    fn from(document: &Document) -> Self {
        Self {
            relative_path: document.relative_path().to_owned(),
            stats: document.stats().into(),
            root: PlainSection::from(document.root()),
        }
    }
}

impl From<&WorkspaceSnapshot> for PlainReport {
    fn from(snapshot: &WorkspaceSnapshot) -> Self {
        let scan = snapshot.scan_stats();
        Self {
            aggregate: snapshot.aggregate_stats().into(),
            scan: PlainScanStats {
                markdown_files: scan.markdown_files,
                directories: scan.directories,
                ignored_directories: scan.ignored_directories,
                task_files: scan.task_files,
                complete_task_files: scan.complete_files,
            },
            warnings: snapshot
                .warnings()
                .iter()
                .map(|warning| PlainWarning {
                    path: warning
                        .path()
                        .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
                    cause: warning.message().to_owned(),
                })
                .collect(),
            documents: snapshot
                .documents()
                .iter()
                .map(PlainDocument::from)
                .collect(),
        }
    }
}

/// The value accepted by `--color`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

/// Terminal facts used to resolve [`ColorMode::Auto`].
///
/// Callers obtain these facts at the process boundary. Tests can therefore
/// exercise color behavior without changing global environment variables.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ColorEnvironment {
    pub stdout_is_terminal: bool,
    pub color_supported: bool,
    pub no_color: bool,
}

impl ColorMode {
    #[must_use]
    pub const fn enabled(self, environment: ColorEnvironment) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                environment.stdout_is_terminal
                    && environment.color_supported
                    && !environment.no_color
            }
        }
    }
}

/// Render a complete report with exactly one trailing newline.
///
/// # Panics
///
/// Panics only if Rust's [`String`] implementation reports a formatting error;
/// writing formatted data to a `String` is infallible.
#[must_use]
pub fn render_report(
    report: &PlainReport,
    color_mode: ColorMode,
    environment: ColorEnvironment,
) -> String {
    let mut output = String::new();
    // Writing to String is infallible.
    render_into(&mut output, report, color_mode.enabled(environment))
        .expect("writing a report to String cannot fail");
    output
}

/// Render to any [`fmt::Write`] destination.
///
/// The generated report ends in exactly one newline and contains no cursor
/// control sequences. ANSI SGR sequences are used only when `color` is true.
///
/// # Errors
///
/// Returns the formatting error produced by `output`.
pub fn render_into(output: &mut impl fmt::Write, report: &PlainReport, color: bool) -> fmt::Result {
    let mut rendered = String::new();

    if report.scan.markdown_files == 0 {
        styled_line(
            &mut rendered,
            "No Markdown files found.",
            Style::Bold,
            color,
        )?;
        render_scan_metadata(&mut rendered, report.scan)?;
        render_warnings(&mut rendered, &report.warnings, color)?;
    } else if report.aggregate.total == 0 {
        styled_line(&mut rendered, "No tasks found.", Style::Bold, color)?;
        render_scan_metadata(&mut rendered, report.scan)?;
        render_warnings(&mut rendered, &report.warnings, color)?;
    } else {
        render_summary(&mut rendered, report, color)?;
        render_scan_metadata(&mut rendered, report.scan)?;
        render_warnings(&mut rendered, &report.warnings, color)?;

        for document in &report.documents {
            rendered.push('\n');
            render_document(&mut rendered, document, color)?;
        }
    }

    // Helpers always finish logical lines with '\n'. Normalize defensively so
    // future helpers cannot accidentally introduce extra trailing whitespace.
    while rendered.ends_with('\n') {
        rendered.pop();
    }
    rendered.push('\n');
    output.write_str(&rendered)
}

fn render_summary(output: &mut String, report: &PlainReport, color: bool) -> fmt::Result {
    let stats = report.aggregate;
    let percent = format!("{}% complete", stats.percent());
    write_styled(output, &percent, progress_style(stats), color)?;
    writeln!(
        output,
        "  |  {} done  |  {} remaining  |  {}/{} files complete",
        stats.completed,
        stats.remaining(),
        report.scan.complete_task_files,
        report.scan.task_files
    )
}

fn render_scan_metadata(output: &mut String, scan: PlainScanStats) -> fmt::Result {
    write!(
        output,
        "{} {} scanned in {} {}",
        scan.markdown_files,
        plural(scan.markdown_files, "markdown file", "markdown files"),
        scan.directories,
        plural(scan.directories, "directory", "directories")
    )?;
    if scan.ignored_directories > 0 {
        write!(output, "  |  {} ignored", scan.ignored_directories)?;
    }
    output.push('\n');
    Ok(())
}

fn render_warnings(output: &mut String, warnings: &[PlainWarning], color: bool) -> fmt::Result {
    if warnings.is_empty() {
        return Ok(());
    }

    output.push('\n');
    styled_line(output, "Warnings:", Style::Warning, color)?;
    for warning in warnings {
        write!(
            output,
            "  {}: ",
            single_line(&normalize_path(&warning.path), "(unknown path)")
        )?;
        write_styled(
            output,
            &single_line(&warning.cause, "(unknown warning)"),
            Style::Warning,
            color,
        )?;
        output.push('\n');
    }
    Ok(())
}

fn render_document(output: &mut String, document: &PlainDocument, color: bool) -> fmt::Result {
    let path = single_line(
        &normalize_path(&document.relative_path),
        "(unknown document)",
    );
    write_styled(output, &path, Style::Bold, color)?;
    write!(
        output,
        "  {}/{}  ",
        document.stats.completed, document.stats.total
    )?;
    write_styled(
        output,
        &format!("{}%", document.stats.percent()),
        progress_style(document.stats),
        color,
    )?;
    output.push('\n');

    render_section_contents(output, &document.root, 1, color)
}

fn render_section_contents(
    output: &mut String,
    section: &PlainSection,
    depth: usize,
    color: bool,
) -> fmt::Result {
    let content_depth = if let Some(title) = &section.title {
        indent(output, depth);
        write_styled(
            output,
            &single_line(title, "(untitled section)"),
            Style::Bold,
            color,
        )?;
        writeln!(
            output,
            "  {}/{}",
            section.stats.completed, section.stats.total
        )?;
        depth + 1
    } else {
        depth
    };

    for task in &section.tasks {
        indent(output, content_depth.saturating_add(task.depth));
        let marker = if task.checked { "[x]" } else { "[ ]" };
        write_styled(
            output,
            marker,
            if task.checked {
                Style::Complete
            } else {
                Style::None
            },
            color,
        )?;
        writeln!(output, " {}", single_line(&task.label, "(untitled task)"))?;
    }

    for child in &section.children {
        render_section_contents(output, child, content_depth, color)?;
    }
    Ok(())
}

fn indent(output: &mut String, depth: usize) {
    for _ in 0..depth.saturating_mul(2) {
        output.push(' ');
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub(crate) fn single_line(value: &str, empty: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut replacing_control = false;

    for character in value.trim().chars() {
        if character.is_control() {
            if !result.is_empty() {
                replacing_control = true;
            }
        } else {
            if replacing_control && !character.is_whitespace() {
                result.push(' ');
            }
            replacing_control = false;
            result.push(character);
        }
    }

    let result = result.trim();
    if result.is_empty() {
        empty.to_owned()
    } else {
        result.to_owned()
    }
}

const fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

#[derive(Clone, Copy)]
enum Style {
    None,
    Bold,
    Complete,
    Progress,
    Warning,
    Muted,
}

fn progress_style(stats: PlainStats) -> Style {
    if stats.total > 0 && stats.completed == stats.total {
        Style::Complete
    } else if stats.completed > 0 {
        Style::Progress
    } else {
        Style::Muted
    }
}

fn styled_line(output: &mut String, value: &str, style: Style, color: bool) -> fmt::Result {
    write_styled(output, value, style, color)?;
    output.push('\n');
    Ok(())
}

fn write_styled(output: &mut String, value: &str, style: Style, color: bool) -> fmt::Result {
    let code = match style {
        Style::None => "",
        Style::Bold => "1",
        Style::Complete => "32",
        Style::Progress | Style::Warning => "33",
        Style::Muted => "2",
    };

    if color && !code.is_empty() {
        write!(output, "\x1b[{code}m{value}\x1b[0m")
    } else {
        output.push_str(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_COLOR: ColorEnvironment = ColorEnvironment {
        stdout_is_terminal: false,
        color_supported: false,
        no_color: false,
    };

    fn stats(completed: usize, total: usize) -> PlainStats {
        PlainStats { total, completed }
    }

    fn task(label: &str, checked: bool, depth: usize) -> PlainTask {
        PlainTask {
            label: label.to_owned(),
            checked,
            depth,
        }
    }

    fn section(
        title: Option<&str>,
        completed: usize,
        total: usize,
        tasks: Vec<PlainTask>,
        children: Vec<PlainSection>,
    ) -> PlainSection {
        PlainSection {
            title: title.map(str::to_owned),
            stats: stats(completed, total),
            tasks,
            children,
        }
    }

    #[test]
    fn renders_mixed_progress_hierarchy_and_root_tasks() {
        let report = PlainReport {
            aggregate: stats(6, 8),
            scan: PlainScanStats {
                markdown_files: 2,
                directories: 3,
                task_files: 2,
                complete_task_files: 1,
                ..PlainScanStats::default()
            },
            warnings: vec![],
            documents: vec![
                PlainDocument {
                    relative_path: "plan.md".to_owned(),
                    stats: stats(3, 4),
                    root: section(
                        None,
                        3,
                        4,
                        vec![],
                        vec![section(
                            Some("Project plan"),
                            3,
                            4,
                            vec![],
                            vec![
                                section(
                                    Some("Phase 1"),
                                    2,
                                    2,
                                    vec![
                                        task("Scaffold the CLI", true, 0),
                                        task("Parse checkboxes", true, 0),
                                    ],
                                    vec![],
                                ),
                                section(
                                    Some("Phase 2"),
                                    1,
                                    2,
                                    vec![
                                        task("Add static output", true, 0),
                                        task("Add the TUI", false, 0),
                                    ],
                                    vec![],
                                ),
                            ],
                        )],
                    ),
                },
                PlainDocument {
                    relative_path: r"nested\release.md".to_owned(),
                    stats: stats(3, 4),
                    root: section(
                        None,
                        3,
                        4,
                        vec![
                            task("Root task", true, 0),
                            task("Nested root task", false, 1),
                        ],
                        vec![section(
                            Some("Release 🚀"),
                            2,
                            2,
                            vec![
                                task("Publish café build", true, 0),
                                task("Announce", true, 0),
                            ],
                            vec![],
                        )],
                    ),
                },
            ],
        };

        assert_eq!(
            render_report(&report, ColorMode::Auto, NO_COLOR),
            "\
75% complete  |  6 done  |  2 remaining  |  1/2 files complete
2 markdown files scanned in 3 directories

plan.md  3/4  75%
  Project plan  3/4
    Phase 1  2/2
      [x] Scaffold the CLI
      [x] Parse checkboxes
    Phase 2  1/2
      [x] Add static output
      [ ] Add the TUI

nested/release.md  3/4  75%
  [x] Root task
    [ ] Nested root task
  Release 🚀  2/2
    [x] Publish café build
    [x] Announce
"
        );
    }

    #[test]
    fn renders_scan_warnings_with_normalized_paths() {
        let report = PlainReport {
            aggregate: stats(0, 1),
            scan: PlainScanStats {
                markdown_files: 1,
                directories: 1,
                ignored_directories: 2,
                task_files: 1,
                ..PlainScanStats::default()
            },
            warnings: vec![PlainWarning {
                path: r"nested\bad.md".to_owned(),
                cause: "invalid UTF-8\n(decoded lossily)".to_owned(),
            }],
            documents: vec![PlainDocument {
                relative_path: "todo.md".to_owned(),
                stats: stats(0, 1),
                root: section(None, 0, 1, vec![task("", false, 0)], vec![]),
            }],
        };

        assert_eq!(
            render_report(&report, ColorMode::Never, NO_COLOR),
            "\
0% complete  |  0 done  |  1 remaining  |  0/1 files complete
1 markdown file scanned in 1 directory  |  2 ignored

Warnings:
  nested/bad.md: invalid UTF-8 (decoded lossily)

todo.md  0/1  0%
  [ ] (untitled task)
"
        );
    }

    #[test]
    fn renders_no_files_and_no_tasks_deliberately() {
        let no_files = PlainReport {
            scan: PlainScanStats {
                directories: 4,
                ignored_directories: 1,
                ..PlainScanStats::default()
            },
            ..PlainReport::default()
        };
        assert_eq!(
            render_report(&no_files, ColorMode::Never, NO_COLOR),
            "No Markdown files found.\n0 markdown files scanned in 4 directories  |  1 ignored\n"
        );

        let no_tasks = PlainReport {
            scan: PlainScanStats {
                markdown_files: 3,
                directories: 1,
                ..PlainScanStats::default()
            },
            ..PlainReport::default()
        };
        assert_eq!(
            render_report(&no_tasks, ColorMode::Never, NO_COLOR),
            "No tasks found.\n3 markdown files scanned in 1 directory\n"
        );
    }

    #[test]
    fn all_complete_retains_tree_and_says_one_hundred_percent() {
        let report = PlainReport {
            aggregate: stats(1, 1),
            scan: PlainScanStats {
                markdown_files: 1,
                directories: 0,
                task_files: 1,
                complete_task_files: 1,
                ..PlainScanStats::default()
            },
            warnings: vec![],
            documents: vec![PlainDocument {
                relative_path: "done.md".to_owned(),
                stats: stats(1, 1),
                root: section(None, 1, 1, vec![task("Ship it", true, 0)], vec![]),
            }],
        };

        let output = render_report(&report, ColorMode::Never, NO_COLOR);
        assert!(
            output.starts_with("100% complete  |  1 done  |  0 remaining  |  1/1 files complete\n")
        );
        assert!(output.contains("done.md  1/1  100%\n  [x] Ship it\n"));
    }

    #[test]
    fn color_modes_are_explicit_and_auto_honors_terminal_facts() {
        let report = PlainReport {
            aggregate: stats(1, 1),
            scan: PlainScanStats {
                markdown_files: 1,
                directories: 0,
                task_files: 1,
                complete_task_files: 1,
                ..PlainScanStats::default()
            },
            documents: vec![PlainDocument {
                relative_path: "done.md".to_owned(),
                stats: stats(1, 1),
                root: section(None, 1, 1, vec![task("Done", true, 0)], vec![]),
            }],
            warnings: vec![],
        };
        let terminal = ColorEnvironment {
            stdout_is_terminal: true,
            color_supported: true,
            no_color: false,
        };
        let no_color_terminal = ColorEnvironment {
            no_color: true,
            ..terminal
        };

        assert!(!render_report(&report, ColorMode::Auto, NO_COLOR).contains('\x1b'));
        assert!(!render_report(&report, ColorMode::Never, terminal).contains('\x1b'));
        assert!(!render_report(&report, ColorMode::Auto, no_color_terminal).contains('\x1b'));

        let always = render_report(&report, ColorMode::Always, no_color_terminal);
        assert!(always.starts_with("\x1b[32m100% complete\x1b[0m"));
        assert!(always.contains("\x1b[1mdone.md\x1b[0m"));
        assert!(always.contains("\x1b[32m[x]\x1b[0m Done"));

        let auto = render_report(&report, ColorMode::Auto, terminal);
        assert_eq!(always, auto);
        assert!(!auto.contains("\x1b[2J"));
        assert!(!auto.contains("\x1b[?"));
    }

    #[test]
    fn stats_round_to_nearest_integer_and_never_underflow() {
        assert_eq!(stats(2, 3).percent(), 67);
        assert_eq!(stats(1, 2).percent(), 50);
        assert_eq!(stats(0, 0).percent(), 0);
        assert_eq!(stats(5, 3).remaining(), 0);
    }

    #[test]
    fn writes_exactly_one_final_newline() {
        let report = PlainReport::default();
        let output = render_report(&report, ColorMode::Never, NO_COLOR);
        assert!(output.ends_with('\n'));
        assert!(!output.ends_with("\n\n"));
    }

    #[test]
    fn source_control_bytes_cannot_inject_ansi_or_extra_lines() {
        let report = PlainReport {
            aggregate: stats(0, 1),
            scan: PlainScanStats {
                markdown_files: 1,
                task_files: 1,
                ..PlainScanStats::default()
            },
            warnings: vec![PlainWarning {
                path: "bad\u{1b}[2J.md".to_owned(),
                cause: "line one\r\nline two".to_owned(),
            }],
            documents: vec![PlainDocument {
                relative_path: "todo\u{1b}[?25l.md".to_owned(),
                stats: stats(0, 1),
                root: section(
                    None,
                    0,
                    1,
                    vec![task("first\tsecond\nthird", false, 0)],
                    vec![],
                ),
            }],
        };

        let output = render_report(&report, ColorMode::Never, NO_COLOR);
        assert!(!output.contains('\u{1b}'));
        assert!(output.contains("bad [2J.md: line one line two"));
        assert!(output.contains("todo [?25l.md  0/1  0%"));
        assert!(output.contains("[ ] first second third"));
    }
}
