//! Pure projection of the immutable Markdown model into Kanban cards.
//!
//! This module deliberately owns no terminal or application state.  A caller can
//! rebuild a [`KanbanProjection`] after search, sort, grouping, or reload and use
//! the visual-order helpers to reconcile its selection.

use std::{cmp::Ordering, path::Path};

use crate::model::{Document, Section, Task, WorkspaceSnapshot};
use crate::tui::app::SortMode;

/// The three progress states used as Kanban columns.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CardStatus {
    NotStarted,
    Started,
    Finished,
}

impl CardStatus {
    pub const VISUAL_ORDER: [Self; 3] = [Self::NotStarted, Self::Started, Self::Finished];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::NotStarted => "Not Started",
            Self::Started => "Started",
            Self::Finished => "Finished",
        }
    }
}

/// Kanban and list views intentionally share one persisted sort choice.
pub type CardSort = SortMode;

/// Shallow statistics for the tasks directly owned by a card.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShallowProgress {
    pub total: usize,
    pub completed: usize,
}

impl ShallowProgress {
    #[must_use]
    pub const fn new(total: usize, completed: usize) -> Self {
        Self { total, completed }
    }

    #[must_use]
    pub const fn remaining(self) -> usize {
        self.total.saturating_sub(self.completed)
    }

    #[must_use]
    pub fn percent(self) -> usize {
        if self.total == 0 {
            0
        } else {
            // Add half of the denominator to round to the nearest integer.
            self.completed
                .saturating_mul(100)
                .saturating_add(self.total / 2)
                / self.total
        }
    }

    #[must_use]
    pub const fn status(self) -> CardStatus {
        if self.completed == 0 {
            CardStatus::NotStarted
        } else if self.completed < self.total {
            CardStatus::Started
        } else {
            CardStatus::Finished
        }
    }
}

/// File information carried by every card, even when boards are not grouped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardFile {
    pub key: String,
    pub relative_path: String,
    pub file_name: String,
}

/// The immutable task data needed to render a card.
///
/// During task-label search this list contains only matching tasks.  `progress`
/// on the enclosing card always describes every directly owned task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardTask {
    pub key: String,
    pub label: String,
    pub checked: bool,
    pub depth: usize,
    pub line_number: usize,
}

impl From<&Task> for CardTask {
    fn from(task: &Task) -> Self {
        Self {
            key: task.key().to_owned(),
            label: task.label().to_owned(),
            checked: task.checked(),
            depth: task.depth(),
            line_number: task.line_number(),
        }
    }
}

/// A section (or the document root) that directly owns at least one task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KanbanCard {
    pub key: String,
    pub title: String,
    /// Ancestor heading titles, excluding this card's own title.
    pub breadcrumbs: Vec<String>,
    pub file: CardFile,
    /// Tasks visible under the active search query, in source order.
    pub tasks: Vec<CardTask>,
    /// Full, unfiltered statistics for tasks directly owned by this section.
    pub progress: ShallowProgress,
    pub status: CardStatus,
    /// Stable traversal position, used only as a final comparison tie-breaker.
    pub source_order: usize,
}

/// One visual column in a board.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KanbanColumn {
    pub status: CardStatus,
    pub cards: Vec<KanbanCard>,
}

/// A global board (`file == None`) or a single-file board.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KanbanBoard {
    pub file: Option<CardFile>,
    pub columns: Vec<KanbanColumn>,
}

impl KanbanBoard {
    /// Return the requested column.
    ///
    /// # Panics
    ///
    /// Panics only if a board was constructed inside this module without one of
    /// the three invariant columns.
    #[must_use]
    pub fn column(&self, status: CardStatus) -> &KanbanColumn {
        self.columns
            .iter()
            .find(|column| column.status == status)
            .expect("every Kanban board has all three columns")
    }
}

/// Complete result of a Kanban projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KanbanProjection {
    pub boards: Vec<KanbanBoard>,
    /// Card keys in keyboard-navigation order.
    pub visual_order: Vec<String>,
    /// Deliberate empty state for an active search.
    pub no_match_message: Option<String>,
}

impl KanbanProjection {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.visual_order.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KanbanOptions<'a> {
    pub query: &'a str,
    pub sort: CardSort,
    pub group_by_file: bool,
}

impl Default for KanbanOptions<'_> {
    fn default() -> Self {
        Self {
            query: "",
            sort: CardSort::Progress,
            group_by_file: false,
        }
    }
}

/// Project a scan snapshot without modifying it.
#[must_use]
pub fn project_kanban(
    snapshot: &WorkspaceSnapshot,
    options: KanbanOptions<'_>,
) -> KanbanProjection {
    project_documents(snapshot.documents(), options)
}

/// Project documents directly, useful for callers that already own a document
/// slice and for focused unit tests.
#[must_use]
pub fn project_documents(documents: &[Document], options: KanbanOptions<'_>) -> KanbanProjection {
    let query = options.query.trim().to_lowercase();
    let mut collected = Vec::new();

    for document in documents {
        let file = card_file(document);
        let path_matches = !query.is_empty() && file.relative_path.to_lowercase().contains(&query);
        let mut ancestry = Vec::new();
        collect_section_cards(
            document.root(),
            &file,
            &query,
            path_matches,
            &mut ancestry,
            &mut collected,
        );
    }

    let boards = if options.group_by_file {
        grouped_boards(documents, &collected, options.sort)
    } else {
        vec![make_board(None, &collected, options.sort)]
    };
    let visual_order = boards
        .iter()
        .flat_map(|board| {
            CardStatus::VISUAL_ORDER.into_iter().flat_map(|status| {
                board
                    .column(status)
                    .cards
                    .iter()
                    .map(|card| card.key.clone())
            })
        })
        .collect();
    let no_match_message = (!query.is_empty() && boards_are_empty(&boards))
        .then(|| format!("No tasks match \"{}\"", options.query.trim()));

    KanbanProjection {
        boards,
        visual_order,
        no_match_message,
    }
}

fn card_file(document: &Document) -> CardFile {
    let relative_path = document.relative_path().replace('\\', "/");
    let file_name = Path::new(document.relative_path()).file_name().map_or_else(
        || relative_path.clone(),
        |name| name.to_string_lossy().into_owned(),
    );
    CardFile {
        key: document.key().to_owned(),
        relative_path,
        file_name,
    }
}

fn collect_section_cards(
    section: &Section,
    file: &CardFile,
    query: &str,
    ancestor_matches: bool,
    ancestry: &mut Vec<String>,
    cards: &mut Vec<KanbanCard>,
) {
    let title = section.title();
    let heading_matches = title.is_some_and(|title| title.to_lowercase().contains(query));
    let context_matches = ancestor_matches || (!query.is_empty() && heading_matches);

    if !section.tasks().is_empty() {
        let progress = shallow_progress(section.tasks());
        let visible_tasks: Vec<_> = section
            .tasks()
            .iter()
            .filter(|task| {
                query.is_empty() || context_matches || task.label().to_lowercase().contains(query)
            })
            .map(CardTask::from)
            .collect();

        if !visible_tasks.is_empty() {
            let source_order = cards.len();
            cards.push(KanbanCard {
                key: section.key().to_owned(),
                title: title.unwrap_or("Ungrouped").to_owned(),
                breadcrumbs: ancestry.clone(),
                file: file.clone(),
                tasks: visible_tasks,
                progress,
                status: progress.status(),
                source_order,
            });
        }
    }

    if let Some(title) = title {
        ancestry.push(title.to_owned());
    }
    for child in section.children() {
        collect_section_cards(child, file, query, context_matches, ancestry, cards);
    }
    if title.is_some() {
        ancestry.pop();
    }
}

fn shallow_progress(tasks: &[Task]) -> ShallowProgress {
    ShallowProgress::new(
        tasks.len(),
        tasks.iter().filter(|task| task.checked()).count(),
    )
}

fn grouped_boards(
    documents: &[Document],
    cards: &[KanbanCard],
    sort: CardSort,
) -> Vec<KanbanBoard> {
    let mut document_order: Vec<_> = documents
        .iter()
        .enumerate()
        .map(|(source_order, document)| {
            (
                card_file(document),
                deep_progress(document.root()),
                source_order,
            )
        })
        .collect();
    document_order.sort_by(|left, right| {
        compare_item(
            sort,
            left.1,
            &left.0.relative_path,
            left.2,
            right.1,
            &right.0.relative_path,
            right.2,
        )
    });

    document_order
        .into_iter()
        .filter_map(|(file, _, _)| {
            let file_cards: Vec<_> = cards
                .iter()
                .filter(|card| card.file.key == file.key)
                .cloned()
                .collect();
            (!file_cards.is_empty()).then(|| make_board(Some(file), &file_cards, sort))
        })
        .collect()
}

fn make_board(file: Option<CardFile>, cards: &[KanbanCard], sort: CardSort) -> KanbanBoard {
    let columns = CardStatus::VISUAL_ORDER
        .into_iter()
        .map(|status| {
            let mut status_cards: Vec<_> = cards
                .iter()
                .filter(|card| card.status == status)
                .cloned()
                .collect();
            status_cards.sort_by(|left, right| compare_cards(sort, left, right));
            KanbanColumn {
                status,
                cards: status_cards,
            }
        })
        .collect();
    KanbanBoard { file, columns }
}

fn compare_cards(sort: CardSort, left: &KanbanCard, right: &KanbanCard) -> Ordering {
    compare_item(
        sort,
        left.progress,
        &left.title,
        left.source_order,
        right.progress,
        &right.title,
        right.source_order,
    )
    .then_with(|| natural_case_insensitive_cmp(&left.file.relative_path, &right.file.relative_path))
    .then_with(|| natural_case_insensitive_cmp(&left.key, &right.key))
}

#[allow(clippy::too_many_arguments)]
fn compare_item(
    sort: CardSort,
    left_progress: ShallowProgress,
    left_name: &str,
    left_source_order: usize,
    right_progress: ShallowProgress,
    right_name: &str,
    right_source_order: usize,
) -> Ordering {
    let name_order = || natural_case_insensitive_cmp(left_name, right_name);
    match sort {
        CardSort::Name => name_order().then_with(|| left_source_order.cmp(&right_source_order)),
        CardSort::Progress => right_progress
            .percent()
            .cmp(&left_progress.percent())
            .then_with(|| left_progress.remaining().cmp(&right_progress.remaining()))
            .then_with(name_order)
            .then_with(|| left_source_order.cmp(&right_source_order)),
    }
}

fn deep_progress(section: &Section) -> ShallowProgress {
    section
        .children()
        .iter()
        .fold(shallow_progress(section.tasks()), |progress, child| {
            let child_progress = deep_progress(child);
            ShallowProgress::new(
                progress.total + child_progress.total,
                progress.completed + child_progress.completed,
            )
        })
}

fn boards_are_empty(boards: &[KanbanBoard]) -> bool {
    boards
        .iter()
        .all(|board| board.columns.iter().all(|column| column.cards.is_empty()))
}

/// Case-insensitive comparison with numeric runs compared by numeric magnitude.
///
/// Equal folded strings fall back to the original spelling so sorting remains a
/// total, deterministic ordering.
fn natural_case_insensitive_cmp(left: &str, right: &str) -> Ordering {
    let folded_left = left.to_lowercase();
    let folded_right = right.to_lowercase();
    let mut left_chars = folded_left.char_indices().peekable();
    let mut right_chars = folded_right.char_indices().peekable();

    loop {
        match (left_chars.peek().copied(), right_chars.peek().copied()) {
            (None, None) => return left.cmp(right),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some((_, left_char)), Some((_, right_char)))
                if left_char.is_ascii_digit() && right_char.is_ascii_digit() =>
            {
                let left_number = take_ascii_number(&folded_left, &mut left_chars);
                let right_number = take_ascii_number(&folded_right, &mut right_chars);
                let left_significant = left_number.trim_start_matches('0');
                let right_significant = right_number.trim_start_matches('0');
                let left_significant = if left_significant.is_empty() {
                    "0"
                } else {
                    left_significant
                };
                let right_significant = if right_significant.is_empty() {
                    "0"
                } else {
                    right_significant
                };
                let order = left_significant
                    .len()
                    .cmp(&right_significant.len())
                    .then_with(|| left_significant.cmp(right_significant))
                    .then_with(|| left_number.len().cmp(&right_number.len()));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some((_, left_char)), Some((_, right_char))) => {
                left_chars.next();
                right_chars.next();
                let order = left_char.cmp(&right_char);
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

fn take_ascii_number<'a>(
    value: &'a str,
    chars: &mut std::iter::Peekable<std::str::CharIndices<'a>>,
) -> &'a str {
    let start = chars.peek().map_or(value.len(), |(index, _)| *index);
    let mut end = start;
    while let Some((index, character)) = chars.peek().copied() {
        if !character.is_ascii_digit() {
            break;
        }
        chars.next();
        end = index + character.len_utf8();
    }
    &value[start..end]
}

/// Reconcile a selected key after reprojection, falling back to the nearest
/// surviving visual index.
#[must_use]
pub fn reconcile_visual_selection(
    visual_order: &[String],
    previous_key: Option<&str>,
    previous_index: usize,
) -> Option<usize> {
    if visual_order.is_empty() {
        None
    } else if let Some(index) =
        previous_key.and_then(|key| visual_order.iter().position(|candidate| candidate == key))
    {
        Some(index)
    } else {
        Some(previous_index.min(visual_order.len() - 1))
    }
}

/// Move without wrapping at either edge.
#[must_use]
pub fn move_visual_selection(current: Option<usize>, amount: isize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let current = current.unwrap_or(0).min(len - 1);
    if amount < 0 {
        Some(current.saturating_sub(amount.unsigned_abs()))
    } else {
        Some(current.saturating_add(amount.unsigned_abs()).min(len - 1))
    }
}

/// Return the selected stable key, if both projection and index are valid.
#[must_use]
pub fn visual_selection_key(visual_order: &[String], selected: Option<usize>) -> Option<&str> {
    selected.and_then(|index| visual_order.get(index).map(String::as_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, Section, Task};
    use std::path::PathBuf;

    fn task(key: &str, label: &str, checked: bool) -> Task {
        Task::new(key, label, checked, 0, 1)
    }

    fn section(
        key: &str,
        title: Option<&str>,
        tasks: Vec<Task>,
        children: Vec<Section>,
    ) -> Section {
        Section::new(key, title.map(str::to_owned), 1, tasks, children)
    }

    fn document(path: &str, root: Section) -> Document {
        Document::new(
            format!("doc:{path}"),
            PathBuf::from("/workspace").join(path),
            path,
            root,
        )
    }

    fn options(query: &str, sort: CardSort, group_by_file: bool) -> KanbanOptions<'_> {
        KanbanOptions {
            query,
            sort,
            group_by_file,
        }
    }

    #[test]
    fn classifies_exact_shallow_progress_boundaries() {
        assert_eq!(ShallowProgress::new(2, 0).status(), CardStatus::NotStarted);
        assert_eq!(ShallowProgress::new(2, 1).status(), CardStatus::Started);
        assert_eq!(ShallowProgress::new(2, 2).status(), CardStatus::Finished);
        assert_eq!(ShallowProgress::new(2, 3).status(), CardStatus::Finished);
    }

    #[test]
    fn rounds_percent_to_nearest_integer() {
        assert_eq!(ShallowProgress::new(0, 0).percent(), 0);
        assert_eq!(ShallowProgress::new(3, 1).percent(), 33);
        assert_eq!(ShallowProgress::new(3, 2).percent(), 67);
    }

    #[test]
    fn natural_name_order_is_case_insensitive_and_numeric_aware() {
        let mut names = ["Item 10", "item 2", "Item 1"];
        names.sort_by(|left, right| natural_case_insensitive_cmp(left, right));
        assert_eq!(names, ["Item 1", "item 2", "Item 10"]);
    }

    #[test]
    fn selection_reconciles_by_key_then_nearest_index() {
        let order = vec!["a".into(), "c".into()];
        assert_eq!(reconcile_visual_selection(&order, Some("c"), 0), Some(1));
        assert_eq!(reconcile_visual_selection(&order, Some("b"), 9), Some(1));
        assert_eq!(reconcile_visual_selection(&[], Some("b"), 1), None);
    }

    #[test]
    fn movement_is_non_wrapping() {
        assert_eq!(move_visual_selection(Some(0), -1, 3), Some(0));
        assert_eq!(move_visual_selection(Some(1), 8, 3), Some(2));
        assert_eq!(move_visual_selection(None, 1, 3), Some(1));
        assert_eq!(move_visual_selection(Some(0), 1, 0), None);
    }

    #[test]
    fn collects_root_and_every_direct_task_owner_with_breadcrumbs() {
        let root = section(
            "root",
            None,
            vec![task("root-task", "At root", false)],
            vec![section(
                "parent",
                Some("Parent"),
                vec![task("parent-task", "At parent", true)],
                vec![section(
                    "child",
                    Some("Child"),
                    vec![task("child-task", "At child", false)],
                    vec![],
                )],
            )],
        );
        let documents = [document("plan.md", root)];
        let result = project_documents(&documents, options("", CardSort::Name, false));
        let mut cards: Vec<_> = result.boards[0]
            .columns
            .iter()
            .flat_map(|column| &column.cards)
            .collect();
        cards.sort_by_key(|card| card.source_order);

        assert_eq!(
            cards
                .iter()
                .map(|card| card.key.as_str())
                .collect::<Vec<_>>(),
            ["root", "parent", "child"]
        );
        assert_eq!(cards[0].title, "Ungrouped");
        assert!(cards[0].breadcrumbs.is_empty());
        assert!(cards[1].breadcrumbs.is_empty());
        assert_eq!(cards[2].breadcrumbs, ["Parent"]);
        assert_eq!(cards[1].progress, ShallowProgress::new(1, 1));
        assert_eq!(
            cards[1].status,
            CardStatus::Finished,
            "child progress must not affect its parent card"
        );
    }

    #[test]
    fn duplicate_heading_titles_remain_distinct_by_stable_key() {
        let root = section(
            "root",
            None,
            vec![],
            vec![
                section(
                    "work#0",
                    Some("Work"),
                    vec![task("a", "First", false)],
                    vec![],
                ),
                section(
                    "work#1",
                    Some("Work"),
                    vec![task("b", "Second", false)],
                    vec![],
                ),
            ],
        );
        let documents = [document("plan.md", root)];
        let result = project_documents(&documents, options("", CardSort::Name, false));
        let cards = &result.boards[0].column(CardStatus::NotStarted).cards;

        assert_eq!(
            cards
                .iter()
                .map(|card| card.key.as_str())
                .collect::<Vec<_>>(),
            ["work#0", "work#1"]
        );
    }

    #[test]
    fn task_label_search_filters_tasks_without_changing_shallow_progress() {
        let root = section(
            "root",
            None,
            vec![],
            vec![section(
                "work",
                Some("Work"),
                vec![
                    task("a", "Needle", true),
                    task("b", "Other", false),
                    task("c", "Another needle", false),
                ],
                vec![],
            )],
        );
        let documents = [document("plan.md", root)];
        let result = project_documents(&documents, options("NEEDLE", CardSort::Name, false));
        let card = &result.boards[0].column(CardStatus::Started).cards[0];

        assert_eq!(card.progress, ShallowProgress::new(3, 1));
        assert_eq!(
            card.tasks
                .iter()
                .map(|task| task.key.as_str())
                .collect::<Vec<_>>(),
            ["a", "c"]
        );
    }

    #[test]
    fn file_and_heading_context_search_reveals_all_descendant_card_tasks() {
        let root = section(
            "root",
            None,
            vec![task("root", "Root task", false)],
            vec![section(
                "release",
                Some("Release 10"),
                vec![task("a", "First", false)],
                vec![section(
                    "child",
                    Some("Child"),
                    vec![task("b", "Second", false)],
                    vec![],
                )],
            )],
        );
        let documents = [document("Roadmap.md", root)];

        let path = project_documents(&documents, options("roadmap", CardSort::Name, false));
        assert_eq!(path.visual_order.len(), 3);
        let heading = project_documents(&documents, options("release", CardSort::Name, false));
        assert_eq!(
            heading.visual_order,
            ["child".to_owned(), "release".to_owned()],
            "a heading match reveals its own and descendant cards"
        );
    }

    #[test]
    fn progress_and_name_sort_cards_without_reordering_tasks() {
        let root = section(
            "root",
            None,
            vec![],
            vec![
                section(
                    "item10",
                    Some("Item 10"),
                    vec![task("ten-first", "Z", true), task("ten-second", "A", false)],
                    vec![],
                ),
                section(
                    "item2",
                    Some("Item 2"),
                    vec![
                        task("two-first", "Z", true),
                        task("two-second", "A", false),
                        task("two-third", "B", false),
                    ],
                    vec![],
                ),
            ],
        );
        let documents = [document("plan.md", root)];

        let by_progress = project_documents(&documents, options("", CardSort::Progress, false));
        let cards = &by_progress.boards[0].column(CardStatus::Started).cards;
        assert_eq!(
            cards
                .iter()
                .map(|card| card.key.as_str())
                .collect::<Vec<_>>(),
            ["item10", "item2"]
        );
        assert_eq!(
            cards[0]
                .tasks
                .iter()
                .map(|task| task.key.as_str())
                .collect::<Vec<_>>(),
            ["ten-first", "ten-second"]
        );

        let by_name = project_documents(&documents, options("", CardSort::Name, false));
        assert_eq!(
            by_name.boards[0]
                .column(CardStatus::Started)
                .cards
                .iter()
                .map(|card| card.key.as_str())
                .collect::<Vec<_>>(),
            ["item2", "item10"]
        );
    }

    #[test]
    fn groups_boards_by_sorted_file_and_builds_stable_visual_order() {
        let documents = [
            document(
                "file10.md",
                section("root10", None, vec![task("ten-done", "Done", true)], vec![]),
            ),
            document(
                "file2.md",
                section("root2", None, vec![task("two-open", "Open", false)], vec![]),
            ),
        ];
        let result = project_documents(&documents, options("", CardSort::Name, true));

        assert_eq!(
            result
                .boards
                .iter()
                .map(|board| board.file.as_ref().unwrap().file_name.as_str())
                .collect::<Vec<_>>(),
            ["file2.md", "file10.md"]
        );
        assert_eq!(result.visual_order, ["root2", "root10"]);
        assert_eq!(
            result.boards[0].column(CardStatus::Started).cards.len(),
            0,
            "every board retains deliberate empty columns"
        );
    }

    #[test]
    fn empty_search_has_deliberate_message() {
        let documents = [document(
            "plan.md",
            section("root", None, vec![task("a", "Present", false)], vec![]),
        )];
        let result = project_documents(&documents, options("missing", CardSort::Progress, false));
        assert!(result.is_empty());
        assert_eq!(
            result.no_match_message.as_deref(),
            Some("No tasks match \"missing\"")
        );
    }
}
