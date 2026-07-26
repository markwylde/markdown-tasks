//! Pure list projection.
//!
//! The small `Projection*` input structs form the intentional adaptation seam
//! between the parser's immutable model and the TUI.  They also keep every
//! search/filter/sort/collapse combination independently testable.

use std::{cmp::Ordering, collections::HashSet};

use natord::compare;

use crate::model::{Document, Section, Stats, Task, WorkspaceSnapshot};

use super::app::{AppState, SortMode, StatusFilter};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RowStats {
    pub total: usize,
    pub completed: usize,
}

impl RowStats {
    #[must_use]
    pub const fn remaining(self) -> usize {
        self.total.saturating_sub(self.completed)
    }

    #[must_use]
    pub fn percent(self) -> usize {
        self.completed
            .saturating_mul(100)
            .saturating_add(self.total / 2)
            .checked_div(self.total)
            .unwrap_or(0)
    }
}

impl From<Stats> for RowStats {
    fn from(stats: Stats) -> Self {
        Self {
            total: stats.total(),
            completed: stats.completed(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionTask {
    pub key: String,
    pub label: String,
    pub checked: bool,
    pub depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionSection {
    pub key: String,
    pub title: Option<String>,
    pub stats: RowStats,
    pub tasks: Vec<ProjectionTask>,
    pub children: Vec<ProjectionSection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionDocument {
    pub key: String,
    pub path: String,
    pub stats: RowStats,
    pub root: ProjectionSection,
}

impl From<&Task> for ProjectionTask {
    fn from(task: &Task) -> Self {
        Self {
            key: task.key().to_owned(),
            label: task.label().to_owned(),
            checked: task.checked(),
            depth: task.depth(),
        }
    }
}

impl From<&Section> for ProjectionSection {
    fn from(section: &Section) -> Self {
        Self {
            key: section.key().to_owned(),
            title: section.title().map(str::to_owned),
            stats: section.stats().into(),
            tasks: section.tasks().iter().map(ProjectionTask::from).collect(),
            children: section
                .children()
                .iter()
                .map(ProjectionSection::from)
                .collect(),
        }
    }
}

impl From<&Document> for ProjectionDocument {
    fn from(document: &Document) -> Self {
        Self {
            key: document.key().to_owned(),
            path: document.relative_path().replace('\\', "/"),
            stats: document.stats().into(),
            root: ProjectionSection::from(document.root()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProjectionOptions<'a> {
    pub filter: StatusFilter,
    pub sort: SortMode,
    pub query: &'a str,
    pub collapsed: &'a HashSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowKind {
    Document,
    Section,
    Task,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleRow {
    pub key: String,
    pub parent_key: Option<String>,
    pub kind: RowKind,
    pub depth: usize,
    pub label: String,
    /// Complete unfiltered statistics for document/section rows.
    pub stats: Option<RowStats>,
    pub checked: Option<bool>,
    /// Additional checkbox indentation relative to the owning section.
    pub task_depth: usize,
    pub expandable: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ListProjection {
    pub rows: Vec<VisibleRow>,
    pub no_match_message: Option<String>,
}

/// Adapt a parser snapshot and project it without mutating either model or app
/// state.
#[must_use]
pub fn project_list(snapshot: &WorkspaceSnapshot, app: &AppState) -> ListProjection {
    project_snapshot(
        snapshot,
        ProjectionOptions {
            filter: app.filter,
            sort: app.sort,
            query: &app.search,
            collapsed: &app.collapsed,
        },
    )
}

/// Convenience reducer boundary for event loops: derive the new rows, then
/// restore exactly-one-visible selection and scrolling invariants.
pub fn project_and_reconcile(snapshot: &WorkspaceSnapshot, app: &mut AppState) -> ListProjection {
    let projection = project_list(snapshot, app);
    app.reconcile_selection(&projection.rows);
    projection
}

/// Project a parser snapshot with explicit options.
#[must_use]
pub fn project_snapshot(
    snapshot: &WorkspaceSnapshot,
    options: ProjectionOptions<'_>,
) -> ListProjection {
    let documents: Vec<_> = snapshot
        .documents()
        .iter()
        .map(ProjectionDocument::from)
        .collect();
    project_documents(&documents, options)
}

#[must_use]
pub fn project_documents(
    documents: &[ProjectionDocument],
    options: ProjectionOptions<'_>,
) -> ListProjection {
    let query = options.query.trim().to_lowercase();
    let searching = !query.is_empty();
    let mut ordered_documents: Vec<_> = documents.iter().collect();
    sort_nodes(
        &mut ordered_documents,
        options.sort,
        |document| document.path.as_str(),
        |document| document.stats,
    );

    let mut rows = Vec::new();
    for document in ordered_documents {
        let file_matches = searching && contains_case_folded(&document.path, &query);
        let mut descendant_rows = Vec::new();
        project_section_contents(
            &document.root,
            &document.key,
            1,
            file_matches,
            &query,
            options,
            &mut descendant_rows,
        );
        if descendant_rows.is_empty() {
            continue;
        }

        rows.push(VisibleRow {
            key: document.key.clone(),
            parent_key: None,
            kind: RowKind::Document,
            depth: 0,
            label: document.path.clone(),
            stats: Some(document.stats),
            checked: None,
            task_depth: 0,
            expandable: true,
        });
        if searching || !options.collapsed.contains(&document.key) {
            rows.extend(descendant_rows);
        }
    }

    let no_match_message = rows.is_empty().then(|| {
        if searching {
            format!("No tasks match \"{}\"", options.query.trim())
        } else {
            "No tasks match the active filter".to_owned()
        }
    });
    ListProjection {
        rows,
        no_match_message,
    }
}

#[allow(clippy::too_many_arguments)]
fn project_section_contents(
    section: &ProjectionSection,
    parent_key: &str,
    depth: usize,
    ancestor_matches: bool,
    query: &str,
    options: ProjectionOptions<'_>,
    output: &mut Vec<VisibleRow>,
) -> bool {
    let searching = !query.is_empty();
    let section_matches = searching
        && section
            .title
            .as_deref()
            .is_some_and(|title| contains_case_folded(title, query));
    let branch_matches = ancestor_matches || section_matches;

    let mut contents = Vec::new();
    for task in &section.tasks {
        if task_matches_filter(task, options.filter)
            && (!searching || branch_matches || contains_case_folded(&task.label, query))
        {
            contents.push(VisibleRow {
                key: task.key.clone(),
                parent_key: Some(section_parent_key(section, parent_key).to_owned()),
                kind: RowKind::Task,
                depth,
                label: display_task_label(&task.label).to_owned(),
                stats: None,
                checked: Some(task.checked),
                task_depth: task.depth,
                expandable: false,
            });
        }
    }

    let mut children: Vec<_> = section.children.iter().collect();
    sort_nodes(
        &mut children,
        options.sort,
        |child| child.title.as_deref().unwrap_or("Ungrouped"),
        |child| child.stats,
    );
    for child in children {
        let mut child_contents = Vec::new();
        if project_section_contents(
            child,
            &child.key,
            depth + 1,
            branch_matches,
            query,
            options,
            &mut child_contents,
        ) {
            contents.push(VisibleRow {
                key: child.key.clone(),
                parent_key: Some(section_parent_key(section, parent_key).to_owned()),
                kind: RowKind::Section,
                depth,
                label: child.title.as_deref().unwrap_or("Ungrouped").to_owned(),
                stats: Some(child.stats),
                checked: None,
                task_depth: 0,
                expandable: true,
            });
            if searching || !options.collapsed.contains(&child.key) {
                contents.extend(child_contents);
            }
        }
    }

    if contents.is_empty() {
        false
    } else {
        output.extend(contents);
        true
    }
}

fn section_parent_key<'a>(section: &'a ProjectionSection, fallback: &'a str) -> &'a str {
    // The implicit root is not rendered, so its tasks/children belong to the
    // document row. A named root is treated like any other section.
    if section.title.is_none() {
        fallback
    } else {
        &section.key
    }
}

fn task_matches_filter(task: &ProjectionTask, filter: StatusFilter) -> bool {
    match filter {
        StatusFilter::All => true,
        StatusFilter::Remaining => !task.checked,
        StatusFilter::Done => task.checked,
    }
}

fn display_task_label(label: &str) -> &str {
    if label.trim().is_empty() {
        "(untitled task)"
    } else {
        label
    }
}

fn contains_case_folded(value: &str, folded_query: &str) -> bool {
    value.to_lowercase().contains(folded_query)
}

fn sort_nodes<T>(
    nodes: &mut [&T],
    mode: SortMode,
    name: impl Fn(&T) -> &str,
    stats: impl Fn(&T) -> RowStats,
) {
    nodes.sort_by(|left, right| match mode {
        SortMode::Name => compare_folded(name(left), name(right)),
        SortMode::Progress => {
            let left_stats = stats(left);
            let right_stats = stats(right);
            right_stats
                .percent()
                .cmp(&left_stats.percent())
                .then_with(|| left_stats.remaining().cmp(&right_stats.remaining()))
                .then_with(|| compare_folded(name(left), name(right)))
        }
    });
}

fn compare_folded(left: &str, right: &str) -> Ordering {
    compare(&left.to_lowercase(), &right.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(key: &str, label: &str, checked: bool) -> ProjectionTask {
        ProjectionTask {
            key: key.into(),
            label: label.into(),
            checked,
            depth: 0,
        }
    }

    fn stats(tasks: &[ProjectionTask], children: &[ProjectionSection]) -> RowStats {
        let own_total = tasks.len();
        let own_completed = tasks.iter().filter(|task| task.checked).count();
        children.iter().fold(
            RowStats {
                total: own_total,
                completed: own_completed,
            },
            |stats, child| RowStats {
                total: stats.total + child.stats.total,
                completed: stats.completed + child.stats.completed,
            },
        )
    }

    fn section(
        key: &str,
        title: Option<&str>,
        tasks: Vec<ProjectionTask>,
        children: Vec<ProjectionSection>,
    ) -> ProjectionSection {
        let section_stats = stats(&tasks, &children);
        ProjectionSection {
            key: key.into(),
            title: title.map(str::to_owned),
            stats: section_stats,
            tasks,
            children,
        }
    }

    fn document(path: &str, root: ProjectionSection) -> ProjectionDocument {
        ProjectionDocument {
            key: format!("doc:{path}"),
            path: path.into(),
            stats: root.stats,
            root,
        }
    }

    fn options<'a>(
        filter: StatusFilter,
        sort: SortMode,
        query: &'a str,
        collapsed: &'a HashSet<String>,
    ) -> ProjectionOptions<'a> {
        ProjectionOptions {
            filter,
            sort,
            query,
            collapsed,
        }
    }

    #[test]
    fn status_filter_keeps_full_parent_stats() {
        let root = section(
            "root",
            None,
            vec![],
            vec![section(
                "section",
                Some("Work"),
                vec![task("todo", "Todo", false), task("done", "Done", true)],
                vec![],
            )],
        );
        let documents = [document("plan.md", root)];
        let collapsed = HashSet::new();
        let result = project_documents(
            &documents,
            options(StatusFilter::Remaining, SortMode::Name, "", &collapsed),
        );
        assert_eq!(
            result
                .rows
                .iter()
                .map(|row| row.key.as_str())
                .collect::<Vec<_>>(),
            ["doc:plan.md", "section", "todo"]
        );
        assert_eq!(
            result.rows[1].stats,
            Some(RowStats {
                total: 2,
                completed: 1
            })
        );
    }

    #[test]
    fn search_matches_path_heading_ancestry_and_task_label() {
        let root = section(
            "root",
            None,
            vec![],
            vec![
                section(
                    "alpha",
                    Some("Alpha"),
                    vec![task("a", "ordinary", false)],
                    vec![],
                ),
                section(
                    "beta",
                    Some("Beta"),
                    vec![task("b", "Needle", false), task("c", "other", false)],
                    vec![],
                ),
            ],
        );
        let documents = [document("PLAN.md", root)];
        let collapsed = HashSet::from(["alpha".to_owned(), "beta".to_owned()]);

        let heading = project_documents(
            &documents,
            options(StatusFilter::All, SortMode::Name, "ALPHA", &collapsed),
        );
        assert_eq!(
            heading
                .rows
                .iter()
                .map(|row| row.key.as_str())
                .collect::<Vec<_>>(),
            ["doc:PLAN.md", "alpha", "a"]
        );

        let label = project_documents(
            &documents,
            options(StatusFilter::All, SortMode::Name, "needle", &collapsed),
        );
        assert_eq!(
            label
                .rows
                .iter()
                .map(|row| row.key.as_str())
                .collect::<Vec<_>>(),
            ["doc:PLAN.md", "beta", "b"]
        );

        let path = project_documents(
            &documents,
            options(StatusFilter::All, SortMode::Name, "plan", &collapsed),
        );
        assert_eq!(path.rows.len(), 6);
        assert_eq!(
            collapsed.len(),
            2,
            "projection must not mutate collapse state"
        );
    }

    #[test]
    fn search_and_filter_compose() {
        let root = section(
            "root",
            None,
            vec![
                task("todo", "needle todo", false),
                task("done", "needle done", true),
            ],
            vec![],
        );
        let documents = [document("tasks.md", root)];
        let collapsed = HashSet::new();
        let result = project_documents(
            &documents,
            options(StatusFilter::Done, SortMode::Name, "needle", &collapsed),
        );
        assert_eq!(
            result
                .rows
                .iter()
                .map(|row| row.key.as_str())
                .collect::<Vec<_>>(),
            ["doc:tasks.md", "done"]
        );
    }

    #[test]
    fn collapse_hides_children_but_search_temporarily_reveals_them() {
        let root = section(
            "root",
            None,
            vec![],
            vec![section(
                "section",
                Some("Work"),
                vec![task("task", "Needle", false)],
                vec![],
            )],
        );
        let documents = [document("tasks.md", root)];
        let collapsed = HashSet::from(["section".to_owned()]);
        let normal = project_documents(
            &documents,
            options(StatusFilter::All, SortMode::Name, "", &collapsed),
        );
        assert_eq!(normal.rows.len(), 2);
        let searching = project_documents(
            &documents,
            options(StatusFilter::All, SortMode::Name, "needle", &collapsed),
        );
        assert_eq!(searching.rows.len(), 3);
    }

    #[test]
    fn progress_sort_is_recursive_and_tasks_stay_in_source_order() {
        let root = section(
            "root",
            None,
            vec![],
            vec![
                section(
                    "zero",
                    Some("10 Zero"),
                    vec![task("z1", "Z", false), task("z2", "A", false)],
                    vec![],
                ),
                section(
                    "done",
                    Some("2 Done"),
                    vec![task("d1", "Z", true), task("d2", "A", true)],
                    vec![],
                ),
            ],
        );
        let documents = [document("tasks.md", root)];
        let collapsed = HashSet::new();
        let result = project_documents(
            &documents,
            options(StatusFilter::All, SortMode::Progress, "", &collapsed),
        );
        assert_eq!(
            result
                .rows
                .iter()
                .map(|row| row.key.as_str())
                .collect::<Vec<_>>(),
            ["doc:tasks.md", "done", "d1", "d2", "zero", "z1", "z2"]
        );
    }

    #[test]
    fn empty_projection_has_explicit_message() {
        let root = section("root", None, vec![task("a", "alpha", false)], vec![]);
        let documents = [document("tasks.md", root)];
        let collapsed = HashSet::new();
        let result = project_documents(
            &documents,
            options(StatusFilter::All, SortMode::Name, "missing", &collapsed),
        );
        assert_eq!(
            result.no_match_message.as_deref(),
            Some("No tasks match \"missing\"")
        );
    }
}
