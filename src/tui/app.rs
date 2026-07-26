//! Pure application state and list navigation.
//!
//! This module deliberately contains no terminal or rendering code.  The event
//! loop projects a snapshot, passes the resulting stable row identities here,
//! and applies the returned [`Command`]s at its boundary.

use std::collections::HashSet;

use super::projection::VisibleRow;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StatusFilter {
    #[default]
    All,
    Remaining,
    Done,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortMode {
    #[default]
    Progress,
    Name,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewMode {
    #[default]
    List,
    Kanban,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Focus {
    #[default]
    Content,
    Search,
    Help,
}

/// Effects which the pure reducer asks the outer event loop to perform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Quit,
    Refresh,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    pub key: String,
    pub index: usize,
}

/// All persistent and modal state for the interactive explorer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppState {
    pub filter: StatusFilter,
    pub sort: SortMode,
    pub view: ViewMode,
    pub group_kanban_by_file: bool,
    /// Selected Kanban lane, including empty lanes on narrow terminals.
    pub kanban_column: usize,
    pub focus: Focus,
    pub search: String,
    pub collapsed: HashSet<String>,
    pub selection: Option<Selection>,
    /// Index of the first visible item.
    pub viewport_offset: usize,
    /// Number of items which fit in the content viewport.
    pub viewport_height: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            filter: StatusFilter::All,
            sort: SortMode::Progress,
            view: ViewMode::List,
            group_kanban_by_file: false,
            kanban_column: 0,
            focus: Focus::Content,
            search: String::new(),
            collapsed: HashSet::new(),
            selection: None,
            viewport_offset: 0,
            viewport_height: 1,
        }
    }
}

impl AppState {
    #[must_use]
    pub fn selected_key(&self) -> Option<&str> {
        self.selection
            .as_ref()
            .map(|selection| selection.key.as_str())
    }

    pub fn set_viewport_height(&mut self, height: usize, row_count: usize) {
        self.viewport_height = height.max(1);
        self.keep_selection_visible(row_count);
    }

    /// Preserve stable identity across reprojection, falling back to the
    /// previous visual index if that identity disappeared.
    pub fn reconcile_selection(&mut self, rows: &[VisibleRow]) {
        if rows.is_empty() {
            self.selection = None;
            self.viewport_offset = 0;
            return;
        }

        let previous_index = self
            .selection
            .as_ref()
            .map_or(0, |selection| selection.index.min(rows.len() - 1));
        let index = self
            .selected_key()
            .and_then(|key| rows.iter().position(|row| row.key == key))
            .unwrap_or(previous_index);
        self.select(rows, index);
    }

    pub fn move_by(&mut self, rows: &[VisibleRow], delta: isize) {
        if rows.is_empty() {
            self.reconcile_selection(rows);
            return;
        }
        self.reconcile_selection(rows);
        let current = self
            .selection
            .as_ref()
            .map_or(0, |selection| selection.index);
        let index = current.saturating_add_signed(delta).min(rows.len() - 1);
        self.select(rows, index);
    }

    pub fn move_page(&mut self, rows: &[VisibleRow], pages: isize) {
        let distance = isize::try_from(self.viewport_height.max(1)).unwrap_or(isize::MAX);
        self.move_by(rows, pages.saturating_mul(distance));
    }

    pub fn move_first(&mut self, rows: &[VisibleRow]) {
        if rows.is_empty() {
            self.reconcile_selection(rows);
        } else {
            self.select(rows, 0);
        }
    }

    pub fn move_last(&mut self, rows: &[VisibleRow]) {
        if rows.is_empty() {
            self.reconcile_selection(rows);
        } else {
            self.select(rows, rows.len() - 1);
        }
    }

    /// Enter/Space semantics: toggle an expandable row. Tasks are deliberately
    /// inert (v1 is read-only).
    pub fn toggle_selected(&mut self, rows: &[VisibleRow]) {
        let Some(row) = self.selected_row(rows) else {
            return;
        };
        if row.expandable && !self.collapsed.insert(row.key.clone()) {
            self.collapsed.remove(&row.key);
        }
    }

    pub fn expand_selected(&mut self, rows: &[VisibleRow]) {
        let Some(row) = self.selected_row(rows) else {
            return;
        };
        if row.expandable {
            self.collapsed.remove(&row.key);
        }
    }

    /// Collapse the selected node if open.  If it is already collapsed or is a
    /// leaf, move to the nearest visible parent.
    pub fn collapse_or_parent(&mut self, rows: &[VisibleRow]) {
        self.reconcile_selection(rows);
        let Some(selection) = self.selection.clone() else {
            return;
        };
        let row = &rows[selection.index];
        if row.expandable && !self.collapsed.contains(&row.key) {
            self.collapsed.insert(row.key.clone());
            return;
        }
        let Some(parent_key) = row.parent_key.as_deref() else {
            return;
        };
        if let Some(parent_index) = rows
            .iter()
            .position(|candidate| candidate.key == parent_key)
        {
            self.select(rows, parent_index);
        }
    }

    /// Collapse all when any expandable node is open, otherwise expand all.
    pub fn toggle_collapse_all(&mut self, rows: &[VisibleRow]) {
        let expandable: Vec<_> = rows
            .iter()
            .filter(|row| row.expandable)
            .map(|row| row.key.clone())
            .collect();
        if expandable.is_empty() {
            return;
        }
        if expandable.iter().any(|key| !self.collapsed.contains(key)) {
            self.collapsed.extend(expandable);
        } else {
            self.collapsed.clear();
        }
    }

    pub fn enter_search(&mut self) {
        self.focus = Focus::Search;
    }

    /// Escape is deliberately two-stage: first leave search/help, then clear
    /// an active query from content focus.
    pub fn escape(&mut self) {
        match self.focus {
            Focus::Search | Focus::Help => self.focus = Focus::Content,
            Focus::Content if !self.search.is_empty() => self.search.clear(),
            Focus::Content => {}
        }
    }

    fn selected_row<'a>(&mut self, rows: &'a [VisibleRow]) -> Option<&'a VisibleRow> {
        self.reconcile_selection(rows);
        self.selection
            .as_ref()
            .and_then(|selection| rows.get(selection.index))
    }

    fn select(&mut self, rows: &[VisibleRow], index: usize) {
        let Some(row) = rows.get(index) else {
            self.selection = None;
            self.viewport_offset = 0;
            return;
        };
        self.selection = Some(Selection {
            key: row.key.clone(),
            index,
        });
        self.keep_selection_visible(rows.len());
    }

    fn keep_selection_visible(&mut self, row_count: usize) {
        let height = self.viewport_height.max(1);
        let max_offset = row_count.saturating_sub(height);
        let Some(index) = self.selection.as_ref().map(|selection| selection.index) else {
            self.viewport_offset = self.viewport_offset.min(max_offset);
            return;
        };
        if index < self.viewport_offset {
            self.viewport_offset = index;
        } else if index >= self.viewport_offset.saturating_add(height) {
            self.viewport_offset = index.saturating_add(1).saturating_sub(height);
        }
        self.viewport_offset = self.viewport_offset.min(max_offset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::projection::{RowKind, VisibleRow};

    fn row(key: &str, parent: Option<&str>, expandable: bool) -> VisibleRow {
        VisibleRow {
            key: key.into(),
            parent_key: parent.map(str::to_owned),
            kind: if expandable {
                RowKind::Section
            } else {
                RowKind::Task
            },
            depth: 0,
            label: key.into(),
            stats: None,
            checked: None,
            task_depth: 0,
            expandable,
        }
    }

    #[test]
    fn reconciliation_prefers_stable_key_then_nearest_index() {
        let mut app = AppState::default();
        let original = [row("a", None, false), row("b", None, false)];
        app.move_last(&original);
        let inserted = [
            row("x", None, false),
            row("a", None, false),
            row("b", None, false),
        ];
        app.reconcile_selection(&inserted);
        assert_eq!(
            app.selection,
            Some(Selection {
                key: "b".into(),
                index: 2
            })
        );

        let removed = [row("x", None, false), row("a", None, false)];
        app.reconcile_selection(&removed);
        assert_eq!(
            app.selection,
            Some(Selection {
                key: "a".into(),
                index: 1
            })
        );
    }

    #[test]
    fn navigation_clamps_and_scrolls_without_wrapping() {
        let rows: Vec<_> = (0..8)
            .map(|index| row(&index.to_string(), None, false))
            .collect();
        let mut app = AppState::default();
        app.set_viewport_height(3, rows.len());
        app.move_by(&rows, -1);
        assert_eq!(app.selection.as_ref().unwrap().index, 0);
        app.move_page(&rows, 1);
        assert_eq!(app.selection.as_ref().unwrap().index, 3);
        assert_eq!(app.viewport_offset, 1);
        app.move_by(&rows, 99);
        assert_eq!(app.selection.as_ref().unwrap().index, 7);
        assert_eq!(app.viewport_offset, 5);
    }

    #[test]
    fn left_collapses_then_moves_to_parent() {
        let rows = [
            row("parent", None, true),
            row("child", Some("parent"), false),
        ];
        let mut app = AppState::default();
        app.move_first(&rows);
        app.collapse_or_parent(&rows);
        assert!(app.collapsed.contains("parent"));
        app.move_last(&rows);
        app.collapse_or_parent(&rows);
        assert_eq!(app.selected_key(), Some("parent"));
    }

    #[test]
    fn task_activation_is_read_only() {
        let rows = [row("task", None, false)];
        let mut app = AppState::default();
        app.move_first(&rows);
        app.toggle_selected(&rows);
        assert!(app.collapsed.is_empty());
    }

    #[test]
    fn collapse_all_toggles_and_empty_results_preserve_state() {
        let rows = [row("file", None, true), row("section", Some("file"), true)];
        let mut app = AppState::default();
        app.toggle_collapse_all(&rows);
        assert_eq!(app.collapsed.len(), 2);
        app.toggle_collapse_all(&[]);
        assert_eq!(app.collapsed.len(), 2);
        app.toggle_collapse_all(&rows);
        assert!(app.collapsed.is_empty());
    }

    #[test]
    fn escape_is_two_stage() {
        let mut app = AppState {
            focus: Focus::Search,
            search: "needle".into(),
            ..AppState::default()
        };
        app.escape();
        assert_eq!(app.focus, Focus::Content);
        assert_eq!(app.search, "needle");
        app.escape();
        assert!(app.search.is_empty());
    }
}
