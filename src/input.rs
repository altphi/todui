use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::model::{InputMode, Pane};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match app.input_mode {
        InputMode::Normal => handle_normal_mode(app, key),
        InputMode::ConfirmDelete => handle_confirm_delete(app, key),
        InputMode::Searching => handle_search_mode(app, key),
        _ => handle_input_mode(app, key),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        (_, KeyCode::Char('q')) => {
            app.quit();
            return;
        }
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            app.quit();
            return;
        }
        (_, KeyCode::Tab) => {
            app.toggle_pane();
            return;
        }
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k')) => {
            app.move_selection_up();
            return;
        }
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j')) => {
            app.move_selection_down();
            return;
        }
        (KeyModifiers::NONE, KeyCode::Char('u')) => {
            app.undo();
            return;
        }
        (KeyModifiers::CONTROL, KeyCode::Char('z')) => {
            app.undo();
            return;
        }
        (KeyModifiers::CONTROL, KeyCode::Char('y')) => {
            app.redo();
            return;
        }
        (KeyModifiers::NONE, KeyCode::Char('/')) => {
            app.start_search();
            return;
        }
        _ => {}
    }

    match app.active_pane {
        Pane::Main => handle_main_pane(app, key),
        Pane::Sidebar => handle_sidebar(app, key),
    }
}

fn handle_main_pane(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        (_, KeyCode::Char(' ')) => {
            app.toggle_done();
        }
        (KeyModifiers::NONE, KeyCode::Char('n')) => {
            app.start_input(InputMode::AddingItem, "");
        }
        (_, KeyCode::Enter) => {
            if let Some(list) = app.current_list() {
                let visible = app.visible_items();
                if let Some(&(real_idx, _)) = visible.get(app.selected_item_index) {
                    let title = list.items[real_idx].title.clone();
                    app.start_input(InputMode::EditingItem, &title);
                }
            }
        }
        (KeyModifiers::NONE, KeyCode::Char('t')) => {
            if let Some(list) = app.current_list() {
                let visible = app.visible_items();
                if let Some(&(real_idx, _)) = visible.get(app.selected_item_index) {
                    let tags_str = list.items[real_idx]
                        .tags
                        .iter()
                        .map(|t| format!("@{}", t))
                        .collect::<Vec<_>>()
                        .join(" ");
                    app.start_input(InputMode::EditingTags, &tags_str);
                }
            }
        }
        (_, KeyCode::Char('x') | KeyCode::Backspace) => {
            app.delete_todo();
        }
        (KeyModifiers::SHIFT, KeyCode::Up) | (KeyModifiers::SHIFT, KeyCode::Char('K')) => {
            app.move_todo_up();
        }
        (KeyModifiers::SHIFT, KeyCode::Down) | (KeyModifiers::SHIFT, KeyCode::Char('J')) => {
            app.move_todo_down();
        }
        (KeyModifiers::SHIFT, KeyCode::Char('D')) => {
            app.toggle_show_done();
        }
        _ => {}
    }
}

fn handle_sidebar(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char('n')) => {
            app.start_input(InputMode::AddingList, "");
        }
        (_, KeyCode::Enter) => {
            if let Some(list) = app.current_list() {
                let name = list.name.clone();
                app.start_input(InputMode::RenamingList, &name);
            }
        }
        (_, KeyCode::Char('x') | KeyCode::Backspace) => {
            if app.lists.len() > 1 {
                app.input_mode = InputMode::ConfirmDelete;
            }
        }
        _ => {}
    }
}

fn handle_search_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            app.select_search_result();
        }
        KeyCode::Esc => {
            app.cancel_input();
        }
        KeyCode::Backspace => {
            app.input_delete_char();
            app.update_search_results();
        }
        KeyCode::Up => {
            app.search_select_prev();
        }
        KeyCode::Down => {
            app.search_select_next();
        }
        KeyCode::Left => {
            app.input_move_cursor_left();
        }
        KeyCode::Right => {
            app.input_move_cursor_right();
        }
        KeyCode::Char(c) => {
            app.input_insert_char(c);
            app.update_search_results();
        }
        _ => {}
    }
}

fn handle_input_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            app.confirm_input();
        }
        KeyCode::Esc => {
            app.cancel_input();
        }
        KeyCode::Backspace => {
            app.input_delete_char();
        }
        KeyCode::Left => {
            app.input_move_cursor_left();
        }
        KeyCode::Right => {
            app.input_move_cursor_right();
        }
        KeyCode::Char(c) => {
            app.input_insert_char(c);
        }
        _ => {}
    }
}

fn handle_confirm_delete(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') => {
            app.input_mode = InputMode::Normal;
            app.delete_list();
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{TodoItem, TodoList};

    fn sample_app() -> App {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "Task A".to_string(),
            done: false,
            tags: vec!["code".to_string()],
        });
        work.items.push(TodoItem {
            title: "Task B".to_string(),
            done: true,
            tags: vec![],
        });
        work.items.push(TodoItem {
            title: "Task C".to_string(),
            done: false,
            tags: vec![],
        });
        let personal = TodoList::new("Personal");

        let mut app = App::with_lists(vec![work, personal]);
        app.active_pane = Pane::Main;
        app
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn test_quit_q() {
        let mut app = sample_app();
        assert!(app.running);
        handle_key(&mut app, key(KeyCode::Char('q')));
        assert!(!app.running);
    }

    #[test]
    fn test_quit_ctrl_c() {
        let mut app = sample_app();
        assert!(app.running);
        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );
        assert!(!app.running);
    }

    #[test]
    fn test_toggle_pane() {
        let mut app = sample_app();
        assert_eq!(app.active_pane, Pane::Main);
        handle_key(&mut app, key(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::Sidebar);
        handle_key(&mut app, key(KeyCode::Tab));
        assert_eq!(app.active_pane, Pane::Main);
    }

    #[test]
    fn test_navigation_keys() {
        let mut app = sample_app();
        assert_eq!(app.selected_item_index, 0);

        handle_key(&mut app, key(KeyCode::Down));
        assert_eq!(app.selected_item_index, 1);

        handle_key(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.selected_item_index, 2);

        handle_key(&mut app, key(KeyCode::Up));
        assert_eq!(app.selected_item_index, 1);

        handle_key(&mut app, key(KeyCode::Char('k')));
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_toggle_done() {
        let mut app = sample_app();
        assert!(!app.lists[0].items[0].done);

        handle_key(&mut app, key(KeyCode::Char(' ')));
        assert!(app.lists[0].items[0].done);
    }

    #[test]
    fn test_add_item_flow() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('n')));
        assert_eq!(app.input_mode, InputMode::AddingItem);

        for c in "New task".chars() {
            handle_key(&mut app, key(KeyCode::Char(c)));
        }
        assert_eq!(app.input_buffer, "New task");

        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.lists[0].items.len(), 4);
        assert_eq!(app.lists[0].items[3].title, "New task");
    }

    #[test]
    fn test_edit_item_flow() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input_mode, InputMode::EditingItem);
        assert_eq!(app.input_buffer, "Task A");
    }

    #[test]
    fn test_edit_tags_flow() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('t')));
        assert_eq!(app.input_mode, InputMode::EditingTags);
        assert_eq!(app.input_buffer, "@code");
    }

    #[test]
    fn test_delete_todo() {
        let mut app = sample_app();
        assert_eq!(app.lists[0].items.len(), 3);
        handle_key(&mut app, key(KeyCode::Char('x')));
        assert_eq!(app.lists[0].items.len(), 2);
    }

    #[test]
    fn test_move_todo_shift_keys() {
        let mut app = sample_app();
        // Visible order after sort: Task A (real 0), Task C (real 2), Task B (real 1)
        // Select visible index 1 = Task C (real index 2)
        app.selected_item_index = 1;

        // Shift+Up: move Task C up, swaps items[2] and items[1] -> [A, C, B]
        handle_key(&mut app, key_with_mod(KeyCode::Up, KeyModifiers::SHIFT));
        assert_eq!(app.lists[0].items[1].title, "Task C");
        assert_eq!(app.selected_item_index, 0);

        // Shift+Down: move item at visible idx 0 (Task A, real 0) down
        // swaps items[0] and items[1] -> [C, A, B]
        handle_key(&mut app, key_with_mod(KeyCode::Down, KeyModifiers::SHIFT));
        assert_eq!(app.lists[0].items[0].title, "Task C");
        assert_eq!(app.selected_item_index, 1);
    }

    #[test]
    fn test_toggle_show_done() {
        let mut app = sample_app();
        assert!(app.show_done);
        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('D'), KeyModifiers::SHIFT),
        );
        assert!(!app.show_done);
    }

    #[test]
    fn test_undo_redo_keys() {
        let mut app = sample_app();
        assert_eq!(app.lists[0].items.len(), 3);

        handle_key(&mut app, key(KeyCode::Char('x')));
        assert_eq!(app.lists[0].items.len(), 2);

        handle_key(&mut app, key(KeyCode::Char('u')));
        assert_eq!(app.lists[0].items.len(), 3);

        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('y'), KeyModifiers::CONTROL),
        );
        assert_eq!(app.lists[0].items.len(), 2);

        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('z'), KeyModifiers::CONTROL),
        );
        assert_eq!(app.lists[0].items.len(), 3);
    }

    #[test]
    fn test_sidebar_add_list() {
        let mut app = sample_app();
        app.active_pane = Pane::Sidebar;
        handle_key(&mut app, key(KeyCode::Char('n')));
        assert_eq!(app.input_mode, InputMode::AddingList);
    }

    #[test]
    fn test_sidebar_rename_list() {
        let mut app = sample_app();
        app.active_pane = Pane::Sidebar;
        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input_mode, InputMode::RenamingList);
        assert_eq!(app.input_buffer, "Work");
    }

    #[test]
    fn test_sidebar_confirm_delete() {
        let mut app = sample_app();
        app.active_pane = Pane::Sidebar;
        assert_eq!(app.lists.len(), 2);

        handle_key(&mut app, key(KeyCode::Char('x')));
        assert_eq!(app.input_mode, InputMode::ConfirmDelete);

        handle_key(&mut app, key(KeyCode::Char('y')));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.lists.len(), 1);
    }

    #[test]
    fn test_sidebar_cancel_delete() {
        let mut app = sample_app();
        app.active_pane = Pane::Sidebar;
        assert_eq!(app.lists.len(), 2);

        handle_key(&mut app, key(KeyCode::Char('x')));
        assert_eq!(app.input_mode, InputMode::ConfirmDelete);

        handle_key(&mut app, key(KeyCode::Char('n')));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.lists.len(), 2);
    }

    #[test]
    fn test_sidebar_delete_prevented_single_list() {
        let mut app = App::with_lists(vec![TodoList::new("Only")]);
        app.active_pane = Pane::Sidebar;

        handle_key(&mut app, key(KeyCode::Char('x')));
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_input_mode_cancel() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('n')));
        assert_eq!(app.input_mode, InputMode::AddingItem);

        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_input_mode_backspace() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('n')));
        handle_key(&mut app, key(KeyCode::Char('a')));
        handle_key(&mut app, key(KeyCode::Char('b')));
        assert_eq!(app.input_buffer, "ab");

        handle_key(&mut app, key(KeyCode::Backspace));
        assert_eq!(app.input_buffer, "a");
    }

    #[test]
    fn test_input_mode_cursor_movement() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('n')));
        handle_key(&mut app, key(KeyCode::Char('a')));
        handle_key(&mut app, key(KeyCode::Char('b')));
        assert_eq!(app.input_cursor, 2);

        handle_key(&mut app, key(KeyCode::Left));
        assert_eq!(app.input_cursor, 1);

        handle_key(&mut app, key(KeyCode::Right));
        assert_eq!(app.input_cursor, 2);
    }

    // ---- Search mode tests ----

    #[test]
    fn test_search_mode_entry() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('/')));
        assert_eq!(app.input_mode, InputMode::Searching);
        assert_eq!(app.input_buffer, "");
    }

    #[test]
    fn test_search_mode_typing() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('/')));
        handle_key(&mut app, key(KeyCode::Char('T')));
        handle_key(&mut app, key(KeyCode::Char('a')));
        assert_eq!(app.input_buffer, "Ta");
        assert!(!app.search_results.is_empty());
    }

    #[test]
    fn test_search_mode_cancel() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('/')));
        handle_key(&mut app, key(KeyCode::Char('T')));
        assert_eq!(app.input_mode, InputMode::Searching);

        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.input_buffer, "");
        assert!(app.search_results.is_empty());
    }

    #[test]
    fn test_search_mode_select() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('/')));
        for c in "Personal".chars() {
            handle_key(&mut app, key(KeyCode::Char(c)));
        }
        assert!(!app.search_results.is_empty());

        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.selected_list_index, 1);
    }
}
