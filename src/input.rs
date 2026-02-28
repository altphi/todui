use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::model::{InputMode, Pane};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match app.input_mode {
        InputMode::Normal => handle_normal_mode(app, key),
        InputMode::ConfirmDelete => handle_confirm_delete(app, key),
        InputMode::Searching => handle_search_mode(app, key),
        InputMode::Focused => handle_focus_mode(app, key),
        InputMode::FilteringTags => handle_filter_mode(app, key),
        InputMode::MovingToList => handle_move_to_list_mode(app, key),
        _ => handle_input_mode(app, key),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    if app.pending_g {
        app.pending_g = false;
        if key.code == KeyCode::Char('g') && key.modifiers == KeyModifiers::NONE {
            app.jump_to_first();
            return;
        }
    }

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
        (KeyModifiers::NONE, KeyCode::Char('g')) => {
            app.pending_g = true;
            return;
        }
        (KeyModifiers::SHIFT, KeyCode::Char('G')) => {
            app.jump_to_last();
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
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            if let Ok((_, rows)) = crossterm::terminal::size() {
                let page_size = (rows as usize).saturating_sub(4);
                app.page_down(page_size);
            }
            return;
        }
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
            if let Ok((_, rows)) = crossterm::terminal::size() {
                let page_size = (rows as usize).saturating_sub(4);
                app.page_up(page_size);
            }
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
    let tag_view = app.is_tag_view();
    match (key.modifiers, key.code) {
        (_, KeyCode::Char(' ')) => {
            app.toggle_done_selected();
        }
        (KeyModifiers::NONE, KeyCode::Char('n')) => {
            if !tag_view {
                app.start_input(InputMode::AddingItem, "");
            }
        }
        (_, KeyCode::Enter) => {
            if let Some((li, ii)) = app.resolve_selected_item() {
                let title = app.lists[li].items[ii].title.clone();
                app.start_input(InputMode::EditingItem, &title);
            }
        }
        (KeyModifiers::NONE, KeyCode::Char('t')) => {
            if let Some((li, ii)) = app.resolve_selected_item() {
                let tags_str = app.lists[li].items[ii]
                    .tags
                    .iter()
                    .map(|t| format!("@{}", t))
                    .collect::<Vec<_>>()
                    .join(" ");
                app.start_input(InputMode::EditingTags, &tags_str);
            }
        }
        (KeyModifiers::NONE, KeyCode::Char('x')) => {
            if !tag_view {
                app.toggle_select_current();
            }
        }
        (_, KeyCode::Delete | KeyCode::Backspace) => {
            if tag_view {
                app.delete_todo();
            } else {
                app.delete_selected();
            }
        }
        (KeyModifiers::NONE, KeyCode::Char('m')) => {
            if !tag_view {
                app.start_move_to_list();
            }
        }
        (KeyModifiers::NONE, KeyCode::Esc) => {
            app.clear_selection();
        }
        (KeyModifiers::SHIFT, KeyCode::Up)
        | (KeyModifiers::SHIFT, KeyCode::Char('K'))
        | (KeyModifiers::ALT, KeyCode::Up) => {
            if !tag_view {
                app.move_todo_up();
            }
        }
        (KeyModifiers::SHIFT, KeyCode::Down)
        | (KeyModifiers::SHIFT, KeyCode::Char('J'))
        | (KeyModifiers::ALT, KeyCode::Down) => {
            if !tag_view {
                app.move_todo_down();
            }
        }
        (m, KeyCode::Up) if m == KeyModifiers::ALT | KeyModifiers::SUPER => {
            if !tag_view {
                app.move_todo_to_top();
            }
        }
        (m, KeyCode::Down) if m == KeyModifiers::ALT | KeyModifiers::SUPER => {
            if !tag_view {
                app.move_todo_to_bottom();
            }
        }
        (KeyModifiers::SHIFT, KeyCode::Char('D')) => {
            app.toggle_show_done();
        }
        (KeyModifiers::SHIFT, KeyCode::Char('F')) => {
            app.start_focus();
        }
        (KeyModifiers::NONE, KeyCode::Char('f')) => {
            if !tag_view {
                app.start_filter();
            }
        }
        (KeyModifiers::SHIFT, KeyCode::Char('T')) => {
            if let Some((li, ii)) = app.resolve_selected_item() {
                let time_str = crate::storage::format_time(app.lists[li].items[ii].time_secs);
                let prefill = if time_str.is_empty() {
                    "0m".to_string()
                } else {
                    time_str
                };
                app.start_input(InputMode::EditingTime, &prefill);
            }
        }
        _ => {}
    }
}

fn handle_sidebar(app: &mut App, key: KeyEvent) {
    if app.is_tag_view() {
        return;
    }
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char('d')) => {
            app.toggle_list_type();
        }
        (KeyModifiers::NONE, KeyCode::Char('n')) => {
            app.start_input(InputMode::AddingList, "");
        }
        (_, KeyCode::Enter) => {
            if let Some(list) = app.current_list() {
                let name = list.name.clone();
                app.start_input(InputMode::RenamingList, &name);
            }
        }
        (_, KeyCode::Delete | KeyCode::Backspace) => {
            if app.lists.len() > 1 {
                app.input_mode = InputMode::ConfirmDelete;
            }
        }
        (KeyModifiers::SHIFT, KeyCode::Up)
        | (KeyModifiers::SHIFT, KeyCode::Char('K'))
        | (KeyModifiers::ALT, KeyCode::Up) => {
            app.move_list_up();
        }
        (KeyModifiers::SHIFT, KeyCode::Down)
        | (KeyModifiers::SHIFT, KeyCode::Char('J'))
        | (KeyModifiers::ALT, KeyCode::Down) => {
            app.move_list_down();
        }
        (m, KeyCode::Up) if m == KeyModifiers::ALT | KeyModifiers::SUPER => {
            app.move_list_to_top();
        }
        (m, KeyCode::Down) if m == KeyModifiers::ALT | KeyModifiers::SUPER => {
            app.move_list_to_bottom();
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
    if app.autocomplete_active {
        match key.code {
            KeyCode::Tab => {
                app.accept_autocomplete();
                return;
            }
            KeyCode::Up => {
                app.autocomplete_move_up();
                return;
            }
            KeyCode::Down => {
                app.autocomplete_move_down();
                return;
            }
            KeyCode::Esc => {
                app.dismiss_autocomplete();
                return;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Enter => {
            app.confirm_input();
        }
        KeyCode::Esc => {
            app.cancel_input();
        }
        KeyCode::Backspace => {
            app.input_delete_char();
            app.update_autocomplete();
        }
        KeyCode::Left => {
            app.input_move_cursor_left();
            app.update_autocomplete();
        }
        KeyCode::Right => {
            app.input_move_cursor_right();
            app.update_autocomplete();
        }
        KeyCode::Char(c) => {
            app.input_insert_char(c);
            app.update_autocomplete();
        }
        _ => {}
    }
}

fn handle_focus_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.stop_focus();
        }
        KeyCode::Char(' ') => {
            app.toggle_pause_focus();
        }
        _ => {}
    }
}

fn handle_filter_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            app.confirm_filter();
        }
        KeyCode::Esc => {
            app.cancel_filter();
        }
        KeyCode::Char(' ') => {
            app.toggle_filter_tag();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.filter_move_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.filter_move_down();
        }
        _ => {}
    }
}

fn handle_move_to_list_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            app.confirm_move_to_list();
        }
        KeyCode::Esc => {
            app.cancel_move_to_list();
        }
        KeyCode::Up => {
            app.move_to_list_move_up();
        }
        KeyCode::Down => {
            app.move_to_list_move_down();
        }
        KeyCode::Backspace => {
            app.move_to_list_delete_char();
        }
        KeyCode::Char(c) => {
            app.move_to_list_insert_char(c);
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
            time_secs: 0,
        });
        work.items.push(TodoItem {
            title: "Task B".to_string(),
            done: true,
            tags: vec![],
            time_secs: 0,
        });
        work.items.push(TodoItem {
            title: "Task C".to_string(),
            done: false,
            tags: vec![],
            time_secs: 0,
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
        assert_eq!(app.lists[0].items[0].title, "New task");
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
        handle_key(&mut app, key(KeyCode::Delete));
        assert_eq!(app.lists[0].items.len(), 2);
    }

    #[test]
    fn test_move_todo_shift_keys() {
        let mut app = sample_app();
        // Visible order (done sorted last): Task A (real 0), Task C (real 2), Task B (real 1)
        app.selected_item_index = 1; // Task C

        handle_key(&mut app, key_with_mod(KeyCode::Up, KeyModifiers::SHIFT));
        assert_eq!(app.lists[0].items[0].title, "Task C");
        assert_eq!(app.lists[0].items[2].title, "Task A");
        assert_eq!(app.selected_item_index, 0);

        handle_key(&mut app, key_with_mod(KeyCode::Down, KeyModifiers::SHIFT));
        assert_eq!(app.lists[0].items[0].title, "Task A");
        assert_eq!(app.lists[0].items[2].title, "Task C");
        assert_eq!(app.selected_item_index, 1);
    }

    #[test]
    fn test_move_todo_alt_keys() {
        let mut app = sample_app();
        app.selected_item_index = 1;

        handle_key(&mut app, key_with_mod(KeyCode::Up, KeyModifiers::ALT));
        assert_eq!(app.selected_item_index, 0);

        handle_key(&mut app, key_with_mod(KeyCode::Down, KeyModifiers::ALT));
        assert_eq!(app.selected_item_index, 1);
    }

    #[test]
    fn test_move_todo_alt_shift_to_top() {
        let mut app = sample_app();
        app.selected_item_index = 2; // Task B (done, last visible)

        handle_key(
            &mut app,
            key_with_mod(KeyCode::Up, KeyModifiers::ALT | KeyModifiers::SUPER),
        );
        assert_eq!(app.lists[0].items[0].title, "Task B");
    }

    #[test]
    fn test_move_todo_alt_shift_to_bottom() {
        let mut app = sample_app();
        app.selected_item_index = 0; // Task A

        handle_key(
            &mut app,
            key_with_mod(KeyCode::Down, KeyModifiers::ALT | KeyModifiers::SUPER),
        );
        assert_eq!(app.lists[0].items[2].title, "Task A");
    }

    #[test]
    fn test_move_list_alt_keys() {
        let mut app = App::with_lists(vec![TodoList::new("Alpha"), TodoList::new("Beta")]);
        app.active_pane = Pane::Sidebar;
        app.selected_list_index = 0;

        handle_key(&mut app, key_with_mod(KeyCode::Down, KeyModifiers::ALT));
        assert_eq!(app.lists[0].name, "Beta");
        assert_eq!(app.lists[1].name, "Alpha");
        assert_eq!(app.selected_list_index, 1);

        handle_key(&mut app, key_with_mod(KeyCode::Up, KeyModifiers::ALT));
        assert_eq!(app.lists[0].name, "Alpha");
        assert_eq!(app.selected_list_index, 0);
    }

    #[test]
    fn test_move_list_alt_shift_to_top_bottom() {
        let mut app = App::with_lists(vec![
            TodoList::new("Alpha"),
            TodoList::new("Beta"),
            TodoList::new("Gamma"),
        ]);
        app.active_pane = Pane::Sidebar;
        app.selected_list_index = 0;

        handle_key(
            &mut app,
            key_with_mod(KeyCode::Down, KeyModifiers::ALT | KeyModifiers::SUPER),
        );
        assert_eq!(app.lists[0].name, "Beta");
        assert_eq!(app.lists[1].name, "Gamma");
        assert_eq!(app.lists[2].name, "Alpha");
        assert_eq!(app.selected_list_index, 2);

        handle_key(
            &mut app,
            key_with_mod(KeyCode::Up, KeyModifiers::ALT | KeyModifiers::SUPER),
        );
        assert_eq!(app.lists[0].name, "Alpha");
        assert_eq!(app.selected_list_index, 0);
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

        handle_key(&mut app, key(KeyCode::Delete));
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

        handle_key(&mut app, key(KeyCode::Delete));
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

        handle_key(&mut app, key(KeyCode::Delete));
        assert_eq!(app.input_mode, InputMode::ConfirmDelete);

        handle_key(&mut app, key(KeyCode::Char('n')));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.lists.len(), 2);
    }

    #[test]
    fn test_sidebar_move_list_up() {
        let mut app = App::with_lists(vec![TodoList::new("Alpha"), TodoList::new("Beta")]);
        app.active_pane = Pane::Sidebar;
        app.selected_list_index = 1;

        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('K'), KeyModifiers::SHIFT),
        );
        assert_eq!(app.lists[0].name, "Beta");
        assert_eq!(app.lists[1].name, "Alpha");
        assert_eq!(app.selected_list_index, 0);
    }

    #[test]
    fn test_sidebar_move_list_down() {
        let mut app = App::with_lists(vec![TodoList::new("Alpha"), TodoList::new("Beta")]);
        app.active_pane = Pane::Sidebar;
        app.selected_list_index = 0;

        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('J'), KeyModifiers::SHIFT),
        );
        assert_eq!(app.lists[0].name, "Beta");
        assert_eq!(app.lists[1].name, "Alpha");
        assert_eq!(app.selected_list_index, 1);
    }

    #[test]
    fn test_sidebar_delete_prevented_single_list() {
        let mut app = App::with_lists(vec![TodoList::new("Only")]);
        app.active_pane = Pane::Sidebar;

        handle_key(&mut app, key(KeyCode::Delete));
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

    #[test]
    fn test_focus_mode_entry() {
        let mut app = sample_app();
        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('F'), KeyModifiers::SHIFT),
        );
        assert_eq!(app.input_mode, InputMode::Focused);
    }

    #[test]
    fn test_focus_mode_stop() {
        let mut app = sample_app();
        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('F'), KeyModifiers::SHIFT),
        );
        assert_eq!(app.input_mode, InputMode::Focused);
        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_focus_mode_pause() {
        let mut app = sample_app();
        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('F'), KeyModifiers::SHIFT),
        );
        assert!(app.focus_start.is_some());
        handle_key(&mut app, key(KeyCode::Char(' ')));
        assert!(app.focus_start.is_none());
        handle_key(&mut app, key(KeyCode::Char(' ')));
        assert!(app.focus_start.is_some());
    }

    #[test]
    fn test_edit_time_entry() {
        let mut app = sample_app();
        app.lists[0].items[0].time_secs = 3600;
        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('T'), KeyModifiers::SHIFT),
        );
        assert_eq!(app.input_mode, InputMode::EditingTime);
        assert_eq!(app.input_buffer, "1h");
    }

    #[test]
    fn test_filter_mode_entry() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('f')));
        assert_eq!(app.input_mode, InputMode::FilteringTags);
    }

    #[test]
    fn test_filter_mode_toggle_and_confirm() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('f')));
        assert_eq!(app.input_mode, InputMode::FilteringTags);

        handle_key(&mut app, key(KeyCode::Char(' ')));
        assert_eq!(app.filter_selected, vec![true]);

        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.filter_tags, vec!["code"]);
    }

    #[test]
    fn test_filter_mode_cancel() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('f')));
        handle_key(&mut app, key(KeyCode::Char(' ')));
        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.filter_tags.is_empty());
    }

    #[test]
    fn test_filter_mode_navigation() {
        let mut app = sample_app();
        app.lists[0].items[1].tags = vec!["review".to_string()];
        handle_key(&mut app, key(KeyCode::Char('f')));
        assert_eq!(app.filter_cursor, 0);

        handle_key(&mut app, key(KeyCode::Char('j')));
        assert_eq!(app.filter_cursor, 1);

        handle_key(&mut app, key(KeyCode::Char('k')));
        assert_eq!(app.filter_cursor, 0);
    }

    #[test]
    fn test_autocomplete_tab_accepts() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('n')));
        handle_key(&mut app, key(KeyCode::Char('@')));
        assert!(app.autocomplete_active);

        handle_key(&mut app, key(KeyCode::Tab));
        assert!(!app.autocomplete_active);
        assert!(app.input_buffer.starts_with("@code"));
    }

    #[test]
    fn test_autocomplete_esc_dismisses() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('n')));
        handle_key(&mut app, key(KeyCode::Char('@')));
        assert!(app.autocomplete_active);

        handle_key(&mut app, key(KeyCode::Esc));
        assert!(!app.autocomplete_active);
        assert_eq!(app.input_mode, InputMode::AddingItem);
    }

    #[test]
    fn test_gg_jumps_to_first_main() {
        let mut app = sample_app();
        app.selected_item_index = 2;
        handle_key(&mut app, key(KeyCode::Char('g')));
        assert!(app.pending_g);
        handle_key(&mut app, key(KeyCode::Char('g')));
        assert!(!app.pending_g);
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_shift_g_jumps_to_last_main() {
        let mut app = sample_app();
        assert_eq!(app.selected_item_index, 0);
        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('G'), KeyModifiers::SHIFT),
        );
        assert_eq!(app.selected_item_index, 2);
    }

    #[test]
    fn test_gg_jumps_to_first_sidebar() {
        let mut app = App::with_lists(vec![
            TodoList::new("Alpha"),
            TodoList::new("Beta"),
            TodoList::new("Gamma"),
        ]);
        app.active_pane = Pane::Sidebar;
        app.selected_sidebar_index = 2;
        app.selected_list_index = 2;
        handle_key(&mut app, key(KeyCode::Char('g')));
        handle_key(&mut app, key(KeyCode::Char('g')));
        assert_eq!(app.selected_sidebar_index, 0);
        assert_eq!(app.selected_list_index, 0);
    }

    #[test]
    fn test_shift_g_jumps_to_last_sidebar() {
        let mut app = App::with_lists(vec![
            TodoList::new("Alpha"),
            TodoList::new("Beta"),
            TodoList::new("Gamma"),
        ]);
        app.active_pane = Pane::Sidebar;
        assert_eq!(app.selected_list_index, 0);
        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('G'), KeyModifiers::SHIFT),
        );
        assert_eq!(app.selected_list_index, 2);
    }

    #[test]
    fn test_pending_g_cleared_by_other_key() {
        let mut app = sample_app();
        app.selected_item_index = 2;
        handle_key(&mut app, key(KeyCode::Char('g')));
        assert!(app.pending_g);
        handle_key(&mut app, key(KeyCode::Char('k')));
        assert!(!app.pending_g);
        assert_eq!(app.selected_item_index, 1);
    }

    #[test]
    fn test_ctrl_d_page_down() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        app.page_down(2);
        assert_eq!(app.selected_item_index, 2);
    }

    #[test]
    fn test_ctrl_u_page_up() {
        let mut app = sample_app();
        app.selected_item_index = 2;
        app.page_up(2);
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_ctrl_d_dispatches() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('d'), KeyModifiers::CONTROL),
        );
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_ctrl_u_dispatches() {
        let mut app = sample_app();
        app.selected_item_index = 2;
        handle_key(
            &mut app,
            key_with_mod(KeyCode::Char('u'), KeyModifiers::CONTROL),
        );
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_autocomplete_up_down_navigates() {
        let mut app = sample_app();
        app.lists[0].items[1].tags = vec!["cooking".to_string()];
        handle_key(&mut app, key(KeyCode::Char('n')));
        handle_key(&mut app, key(KeyCode::Char('@')));
        handle_key(&mut app, key(KeyCode::Char('c')));
        assert!(app.autocomplete_active);
        assert_eq!(app.autocomplete_cursor, 0);

        handle_key(&mut app, key(KeyCode::Down));
        assert_eq!(app.autocomplete_cursor, 1);

        handle_key(&mut app, key(KeyCode::Up));
        assert_eq!(app.autocomplete_cursor, 0);
    }

    #[test]
    fn test_x_toggles_selection_in_main() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('x')));
        assert!(!app.selected_items.is_empty());
        handle_key(&mut app, key(KeyCode::Char('x')));
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_delete_key_deletes_in_main() {
        let mut app = sample_app();
        assert_eq!(app.lists[0].items.len(), 3);
        handle_key(&mut app, key(KeyCode::Delete));
        assert_eq!(app.lists[0].items.len(), 2);
    }

    #[test]
    fn test_delete_key_in_sidebar_deletes_list() {
        let mut app = sample_app();
        app.active_pane = Pane::Sidebar;
        assert_eq!(app.lists.len(), 2);
        handle_key(&mut app, key(KeyCode::Delete));
        assert_eq!(app.input_mode, InputMode::ConfirmDelete);
    }

    #[test]
    fn test_m_starts_move_to_list() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('m')));
        assert_eq!(app.input_mode, InputMode::MovingToList);
    }

    #[test]
    fn test_move_to_list_mode_enter_confirms() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('m')));
        assert_eq!(app.input_mode, InputMode::MovingToList);
        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.lists[1].items.len(), 1);
    }

    #[test]
    fn test_move_to_list_mode_esc_cancels() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('m')));
        assert_eq!(app.input_mode, InputMode::MovingToList);
        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.lists[0].items.len(), 3);
    }

    #[test]
    fn test_esc_clears_selection_in_main() {
        let mut app = sample_app();
        handle_key(&mut app, key(KeyCode::Char('x')));
        assert!(!app.selected_items.is_empty());
        handle_key(&mut app, key(KeyCode::Esc));
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_sidebar_d_toggles_list_type() {
        let mut app = sample_app();
        app.active_pane = Pane::Sidebar;
        assert_eq!(app.lists[0].list_type, crate::model::ListType::Normal);
        handle_key(&mut app, key(KeyCode::Char('d')));
        assert_eq!(app.lists[0].list_type, crate::model::ListType::Daily);
        handle_key(&mut app, key(KeyCode::Char('d')));
        assert_eq!(app.lists[0].list_type, crate::model::ListType::Normal);
    }

    #[test]
    fn test_sidebar_no_rename_on_tag() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.active_pane = Pane::Sidebar;
        app.selected_sidebar_index = 1;
        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_sidebar_no_delete_on_tag() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.active_pane = Pane::Sidebar;
        app.selected_sidebar_index = 1;
        handle_key(&mut app, key(KeyCode::Delete));
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_main_no_add_in_tag_view() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.active_pane = Pane::Main;
        app.selected_sidebar_index = 1;
        handle_key(&mut app, key(KeyCode::Char('n')));
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_main_toggle_done_works_in_tag_view() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.active_pane = Pane::Main;
        app.selected_sidebar_index = 1;
        handle_key(&mut app, key(KeyCode::Char(' ')));
        assert!(app.lists[0].items[0].done);
    }

    #[test]
    fn test_main_edit_works_in_tag_view() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.active_pane = Pane::Main;
        app.selected_sidebar_index = 1;
        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.input_mode, InputMode::EditingItem);
        assert_eq!(app.input_buffer, "A");
    }

    #[test]
    fn test_main_delete_works_in_tag_view() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.active_pane = Pane::Main;
        app.selected_sidebar_index = 1;
        handle_key(&mut app, key(KeyCode::Delete));
        assert_eq!(app.lists[0].items.len(), 0);
    }
}
