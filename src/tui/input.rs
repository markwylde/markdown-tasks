//! Crossterm key translation for the pure [`AppState`](super::app::AppState).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::{
    app::{AppState, Command, Focus, SortMode, StatusFilter, ViewMode},
    projection::VisibleRow,
};

/// Apply one key event. Terminal lifecycle effects are returned to the caller;
/// all ordinary interaction is contained in `app`.
pub fn handle_key(app: &mut AppState, rows: &[VisibleRow], key: KeyEvent) -> Option<Command> {
    if matches!(key.kind, KeyEventKind::Release) {
        return None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Command::Quit);
    }

    match app.focus {
        Focus::Search => handle_search_key(app, key),
        Focus::Help => handle_help_key(app, key),
        Focus::Content => handle_content_key(app, rows, key),
    }
}

fn handle_search_key(app: &mut AppState, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Esc => app.escape(),
        KeyCode::Enter => app.focus = Focus::Content,
        KeyCode::Backspace => {
            app.search.pop();
        }
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            delete_previous_word(&mut app.search);
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.search.clear();
        }
        KeyCode::Char(character)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            app.search.push(character);
        }
        _ => {}
    }
    None
}

fn handle_help_key(app: &mut AppState, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') => app.focus = Focus::Content,
        KeyCode::Char('q') => return Some(Command::Quit),
        _ => {}
    }
    None
}

fn handle_content_key(app: &mut AppState, rows: &[VisibleRow], key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Char('q') => return Some(Command::Quit),
        KeyCode::Char('j') | KeyCode::Down => app.move_by(rows, 1),
        KeyCode::Char('k') | KeyCode::Up => app.move_by(rows, -1),
        KeyCode::PageDown => app.move_page(rows, 1),
        KeyCode::PageUp => app.move_page(rows, -1),
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_page(rows, 1);
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_page(rows, -1);
        }
        KeyCode::Char('g') | KeyCode::Home => app.move_first(rows),
        KeyCode::Char('G') | KeyCode::End => app.move_last(rows),
        KeyCode::Enter | KeyCode::Char(' ') if app.view == ViewMode::List => {
            app.toggle_selected(rows);
        }
        KeyCode::Right if app.view == ViewMode::List => app.expand_selected(rows),
        KeyCode::Left if app.view == ViewMode::List => app.collapse_or_parent(rows),
        KeyCode::Right if app.view == ViewMode::Kanban => {
            app.kanban_column = (app.kanban_column + 1).min(2);
            app.selection = None;
        }
        KeyCode::Left if app.view == ViewMode::Kanban => {
            app.kanban_column = app.kanban_column.saturating_sub(1);
            app.selection = None;
        }
        KeyCode::Char('z') if app.view == ViewMode::List => app.toggle_collapse_all(rows),
        KeyCode::Char('/') => app.enter_search(),
        KeyCode::Esc => app.escape(),
        KeyCode::Char('1') if app.view == ViewMode::List => app.filter = StatusFilter::All,
        KeyCode::Char('2') if app.view == ViewMode::List => app.filter = StatusFilter::Remaining,
        KeyCode::Char('3') if app.view == ViewMode::List => app.filter = StatusFilter::Done,
        KeyCode::Char('v') => {
            app.view = match app.view {
                ViewMode::List => ViewMode::Kanban,
                ViewMode::Kanban => ViewMode::List,
            };
        }
        KeyCode::Char('s') => {
            app.sort = match app.sort {
                SortMode::Progress => SortMode::Name,
                SortMode::Name => SortMode::Progress,
            };
        }
        KeyCode::Char('f') if app.view == ViewMode::Kanban => {
            app.group_kanban_by_file = !app.group_kanban_by_file;
        }
        KeyCode::Char('r') => return Some(Command::Refresh),
        KeyCode::Char('?') => app.focus = Focus::Help,
        _ => {}
    }
    None
}

fn delete_previous_word(value: &mut String) {
    let trimmed_end = value.trim_end_matches(char::is_whitespace).len();
    value.truncate(trimmed_end);
    let word_start = value
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            character
                .is_whitespace()
                .then_some(index + character.len_utf8())
        })
        .unwrap_or(0);
    value.truncate(word_start);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::projection::{RowKind, VisibleRow};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn control(character: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL)
    }

    fn rows() -> Vec<VisibleRow> {
        ["a", "b", "c", "d"]
            .into_iter()
            .map(|key| VisibleRow {
                key: key.into(),
                parent_key: None,
                kind: RowKind::Task,
                depth: 0,
                label: key.into(),
                stats: None,
                checked: Some(false),
                task_depth: 0,
                expandable: false,
            })
            .collect()
    }

    #[test]
    fn every_movement_binding_is_non_wrapping() {
        let rows = rows();
        let mut app = AppState {
            viewport_height: 2,
            ..AppState::default()
        };
        for code in [KeyCode::Char('j'), KeyCode::Down] {
            handle_key(&mut app, &rows, key(code));
        }
        assert_eq!(app.selection.as_ref().unwrap().index, 2);
        handle_key(&mut app, &rows, key(KeyCode::PageDown));
        handle_key(&mut app, &rows, control('d'));
        assert_eq!(app.selection.as_ref().unwrap().index, 3);
        handle_key(&mut app, &rows, key(KeyCode::Char('k')));
        handle_key(&mut app, &rows, key(KeyCode::Up));
        handle_key(&mut app, &rows, control('u'));
        assert_eq!(app.selection.as_ref().unwrap().index, 0);
        handle_key(&mut app, &rows, key(KeyCode::End));
        assert_eq!(app.selection.as_ref().unwrap().index, 3);
        handle_key(&mut app, &rows, key(KeyCode::Char('g')));
        assert_eq!(app.selection.as_ref().unwrap().index, 0);
    }

    #[test]
    fn search_supports_unicode_editing_and_modal_q() {
        let mut app = AppState::default();
        handle_key(&mut app, &[], key(KeyCode::Char('/')));
        for character in "one café q".chars() {
            assert_eq!(
                handle_key(&mut app, &[], key(KeyCode::Char(character))),
                None
            );
        }
        assert_eq!(app.search, "one café q");
        handle_key(&mut app, &[], control('w'));
        assert_eq!(app.search, "one café ");
        handle_key(&mut app, &[], key(KeyCode::Backspace));
        assert_eq!(app.search, "one café");
        handle_key(&mut app, &[], control('u'));
        assert!(app.search.is_empty());
    }

    #[test]
    fn enter_retains_query_and_escape_is_two_stage() {
        let mut app = AppState {
            focus: Focus::Search,
            search: "query".into(),
            ..AppState::default()
        };
        handle_key(&mut app, &[], key(KeyCode::Enter));
        assert_eq!(app.focus, Focus::Content);
        assert_eq!(app.search, "query");
        handle_key(&mut app, &[], key(KeyCode::Esc));
        assert!(app.search.is_empty());
    }

    #[test]
    fn content_shortcuts_update_state_and_return_boundary_commands() {
        let mut app = AppState::default();
        handle_key(&mut app, &[], key(KeyCode::Char('2')));
        handle_key(&mut app, &[], key(KeyCode::Char('s')));
        handle_key(&mut app, &[], key(KeyCode::Char('v')));
        handle_key(&mut app, &[], key(KeyCode::Char('f')));
        assert_eq!(app.filter, StatusFilter::Remaining);
        assert_eq!(app.sort, SortMode::Name);
        assert_eq!(app.view, ViewMode::Kanban);
        assert!(app.group_kanban_by_file);
        handle_key(&mut app, &[], key(KeyCode::Char('3')));
        assert_eq!(
            app.filter,
            StatusFilter::Remaining,
            "hidden list filters must be retained in Kanban"
        );
        assert_eq!(
            handle_key(&mut app, &[], key(KeyCode::Char('r'))),
            Some(Command::Refresh)
        );
        assert_eq!(
            handle_key(&mut app, &[], key(KeyCode::Char('q'))),
            Some(Command::Quit)
        );
        assert_eq!(handle_key(&mut app, &[], control('c')), Some(Command::Quit));
    }

    #[test]
    fn filters_views_help_and_boundary_navigation_cover_aliases() {
        let rows = rows();
        let mut app = AppState::default();
        app.reconcile_selection(&rows);

        handle_key(&mut app, &rows, key(KeyCode::Char('3')));
        assert_eq!(app.filter, StatusFilter::Done);
        handle_key(&mut app, &rows, key(KeyCode::Char('1')));
        assert_eq!(app.filter, StatusFilter::All);

        handle_key(&mut app, &rows, key(KeyCode::Char('G')));
        assert_eq!(app.selection.as_ref().unwrap().index, 3);
        handle_key(&mut app, &rows, key(KeyCode::Home));
        assert_eq!(app.selection.as_ref().unwrap().index, 0);
        handle_key(&mut app, &rows, key(KeyCode::PageDown));
        handle_key(&mut app, &rows, key(KeyCode::PageUp));
        assert_eq!(app.selection.as_ref().unwrap().index, 0);

        handle_key(&mut app, &rows, key(KeyCode::Char('?')));
        assert_eq!(app.focus, Focus::Help);
        handle_key(&mut app, &rows, key(KeyCode::Char('?')));
        assert_eq!(app.focus, Focus::Content);
    }

    #[test]
    fn activation_keys_only_change_tree_state() {
        let rows = vec![
            VisibleRow {
                key: "file".into(),
                parent_key: None,
                kind: RowKind::Document,
                depth: 0,
                label: "file".into(),
                stats: None,
                checked: None,
                task_depth: 0,
                expandable: true,
            },
            VisibleRow {
                key: "task".into(),
                parent_key: Some("file".into()),
                kind: RowKind::Task,
                depth: 1,
                label: "task".into(),
                stats: None,
                checked: Some(false),
                task_depth: 0,
                expandable: false,
            },
        ];
        let mut app = AppState::default();
        app.reconcile_selection(&rows);
        handle_key(&mut app, &rows, key(KeyCode::Enter));
        assert!(app.collapsed.contains("file"));
        handle_key(&mut app, &rows, key(KeyCode::Char(' ')));
        assert!(!app.collapsed.contains("file"));
        handle_key(&mut app, &rows, key(KeyCode::Right));
        assert!(!app.collapsed.contains("file"));
        app.move_last(&rows);
        handle_key(&mut app, &rows, key(KeyCode::Left));
        assert_eq!(app.selected_key(), Some("file"));
        handle_key(&mut app, &rows, key(KeyCode::Char('z')));
        assert!(app.collapsed.contains("file"));
        assert_eq!(rows[1].checked, Some(false), "keys must never edit tasks");
    }

    #[test]
    fn release_events_are_ignored() {
        let mut app = AppState::default();
        let release = KeyEvent::new_with_kind(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        assert_eq!(handle_key(&mut app, &[], release), None);
    }
}
