//! Immutable data types shared by discovery, reporting, and the TUI.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// Aggregated task counts for a model node.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Stats {
    total: usize,
    completed: usize,
}

impl Stats {
    /// Construct validated counts.
    ///
    /// # Panics
    ///
    /// Panics when `completed` exceeds `total`.
    #[must_use]
    pub const fn new(total: usize, completed: usize) -> Self {
        assert!(
            completed <= total,
            "completed tasks cannot exceed total tasks"
        );
        Self { total, completed }
    }

    #[must_use]
    pub const fn total(self) -> usize {
        self.total
    }

    #[must_use]
    pub const fn completed(self) -> usize {
        self.completed
    }

    #[must_use]
    pub const fn remaining(self) -> usize {
        self.total - self.completed
    }

    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.total > 0 && self.completed == self.total
    }

    /// Rounded completion percentage, with an empty node reported as zero.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn percent(self) -> usize {
        // This value is bounded by 100 because completed <= total.
        match (self.completed as u128 * 100 + self.total as u128 / 2)
            .checked_div(self.total as u128)
        {
            Some(percent) => percent as usize,
            None => 0,
        }
    }

    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        Self {
            total: self.total + other.total,
            completed: self.completed + other.completed,
        }
    }
}

/// A parsed Markdown checkbox item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    key: String,
    label: String,
    checked: bool,
    depth: usize,
    line_number: usize,
}

impl Task {
    #[must_use]
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        checked: bool,
        depth: usize,
        line_number: usize,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            checked,
            depth,
            line_number,
        }
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn checked(&self) -> bool {
        self.checked
    }

    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }

    #[must_use]
    pub const fn line_number(&self) -> usize {
        self.line_number
    }
}

/// A heading and everything nested below it. The document's implicit root has
/// no title and level zero.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    key: String,
    title: Option<String>,
    level: u8,
    tasks: Vec<Task>,
    children: Vec<Self>,
    stats: Stats,
}

impl Section {
    #[must_use]
    pub fn new(
        key: impl Into<String>,
        title: Option<String>,
        level: u8,
        tasks: Vec<Task>,
        children: Vec<Self>,
    ) -> Self {
        let own = tasks.iter().fold(Stats::default(), |stats, task| {
            stats.combine(Stats::new(1, usize::from(task.checked())))
        });
        let stats = children
            .iter()
            .fold(own, |stats, child| stats.combine(child.stats()));
        Self {
            key: key.into(),
            title,
            level,
            tasks,
            children,
            stats,
        }
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub const fn level(&self) -> u8 {
        self.level
    }

    #[must_use]
    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    #[must_use]
    pub fn children(&self) -> &[Self] {
        &self.children
    }

    #[must_use]
    pub const fn stats(&self) -> Stats {
        self.stats
    }
}

/// A single parsed Markdown file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    key: String,
    absolute_path: PathBuf,
    relative_path: String,
    root: Section,
    stats: Stats,
}

impl Document {
    #[must_use]
    pub fn new(
        key: impl Into<String>,
        absolute_path: PathBuf,
        relative_path: impl Into<String>,
        root: Section,
    ) -> Self {
        let stats = root.stats();
        Self {
            key: key.into(),
            absolute_path,
            relative_path: relative_path.into(),
            root,
            stats,
        }
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn absolute_path(&self) -> &Path {
        &self.absolute_path
    }

    #[must_use]
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    #[must_use]
    pub const fn root(&self) -> &Section {
        &self.root
    }

    #[must_use]
    pub const fn stats(&self) -> Stats {
        self.stats
    }
}

/// Filesystem metadata collected while producing a workspace snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScanStats {
    pub directories: usize,
    pub ignored_directories: usize,
    pub markdown_files: usize,
    pub task_files: usize,
    pub complete_files: usize,
}

/// A recoverable scan or decode problem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Warning {
    path: Option<PathBuf>,
    message: String,
}

impl Warning {
    #[must_use]
    pub fn new(path: Option<PathBuf>, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// One complete, read-only view of a scanned target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSnapshot {
    root_path: PathBuf,
    documents: Vec<Document>,
    aggregate_stats: Stats,
    scan_stats: ScanStats,
    warnings: Vec<Warning>,
}

impl WorkspaceSnapshot {
    #[must_use]
    pub fn new(
        root_path: PathBuf,
        documents: Vec<Document>,
        scan_stats: ScanStats,
        warnings: Vec<Warning>,
    ) -> Self {
        let aggregate_stats = documents.iter().fold(Stats::default(), |stats, document| {
            stats.combine(document.stats())
        });
        Self {
            root_path,
            documents,
            aggregate_stats,
            scan_stats,
            warnings,
        }
    }

    #[must_use]
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    #[must_use]
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }

    #[must_use]
    pub const fn aggregate_stats(&self) -> Stats {
        self.aggregate_stats
    }

    #[must_use]
    pub const fn scan_stats(&self) -> ScanStats {
        self.scan_stats
    }

    #[must_use]
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }
}

/// Compare names case-insensitively while treating ASCII digit runs as numbers.
#[must_use]
pub fn compare_names(left: &str, right: &str) -> Ordering {
    let left = left.to_lowercase();
    let right = right.to_lowercase();
    let mut left = left.chars().peekable();
    let mut right = right.chars().peekable();

    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) if a.is_ascii_digit() && b.is_ascii_digit() => {
                let mut a = String::new();
                while left.peek().is_some_and(char::is_ascii_digit) {
                    if let Some(character) = left.next() {
                        a.push(character);
                    }
                }
                let mut b = String::new();
                while right.peek().is_some_and(char::is_ascii_digit) {
                    if let Some(character) = right.next() {
                        b.push(character);
                    }
                }
                let a_value = a.trim_start_matches('0');
                let b_value = b.trim_start_matches('0');
                let a_value = if a_value.is_empty() { "0" } else { a_value };
                let b_value = if b_value.is_empty() { "0" } else { b_value };
                match a_value
                    .len()
                    .cmp(&b_value.len())
                    .then_with(|| a_value.cmp(b_value))
                {
                    Ordering::Equal => {}
                    ordering => return ordering,
                }
            }
            (Some(a), Some(b)) => {
                left.next();
                right.next();
                match a.cmp(&b) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                }
            }
        }
    }
}

/// Terminay progress order: percentage descending, remaining ascending, then
/// natural name ascending.
#[must_use]
pub fn compare_progress(
    left_stats: Stats,
    left_name: &str,
    right_stats: Stats,
    right_name: &str,
) -> Ordering {
    right_stats
        .percent()
        .cmp(&left_stats.percent())
        .then_with(|| left_stats.remaining().cmp(&right_stats.remaining()))
        .then_with(|| compare_names(left_name, right_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn stats_helpers_handle_empty_and_round_to_nearest_integer() {
        assert_eq!(Stats::default().percent(), 0);
        assert!(!Stats::default().is_complete());
        assert_eq!(Stats::new(3, 2).percent(), 67);
        assert_eq!(Stats::new(200, 1).percent(), 1);
        assert_eq!(Stats::new(usize::MAX, usize::MAX / 2).percent(), 50);
        assert!(Stats::new(2, 2).is_complete());
    }

    #[test]
    fn sections_and_snapshots_aggregate_recursively() {
        let child = Section::new(
            "child",
            Some("Child".into()),
            2,
            vec![Task::new("a", "A", true, 0, 1)],
            vec![],
        );
        let root = Section::new(
            "root",
            None,
            0,
            vec![Task::new("b", "B", false, 0, 2)],
            vec![child],
        );
        let document = Document::new("doc", "/tmp/a.md".into(), "a.md", root);
        let snapshot = WorkspaceSnapshot::new(
            "/tmp".into(),
            vec![document.clone(), document],
            ScanStats::default(),
            vec![],
        );
        assert_eq!(snapshot.aggregate_stats(), Stats::new(4, 2));
    }

    #[test]
    fn name_comparison_is_case_insensitive_and_numeric_aware() {
        assert_eq!(compare_names("FILE2.md", "file10.md"), Ordering::Less);
        assert_eq!(compare_names("alpha", "ALPHA"), Ordering::Equal);
        assert_eq!(compare_names("chapter9b", "chapter9A"), Ordering::Greater);
    }

    #[test]
    fn progress_comparison_applies_all_three_tiers() {
        assert_eq!(
            compare_progress(Stats::new(4, 3), "z", Stats::new(2, 1), "a"),
            Ordering::Less
        );
        assert_eq!(
            compare_progress(Stats::new(8, 4), "z", Stats::new(4, 2), "a"),
            Ordering::Greater
        );
        assert_eq!(
            compare_progress(Stats::new(2, 1), "file2", Stats::new(2, 1), "file10"),
            Ordering::Less
        );
    }

    proptest! {
        #[test]
        fn stats_invariants(total in 0usize..100_000, completed_seed in any::<usize>()) {
            let completed = if total == usize::MAX {
                completed_seed
            } else {
                completed_seed % (total + 1)
            };
            let stats = Stats::new(total, completed);
            prop_assert!(stats.completed() <= stats.total());
            prop_assert_eq!(stats.remaining(), total - completed);
            prop_assert!(stats.percent() <= 100);
            prop_assert_eq!(stats.is_complete(), total > 0 && completed == total);
        }
    }
}
