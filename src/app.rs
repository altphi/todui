use std::path::PathBuf;

use crate::model::{AppSnapshot, InputMode, Pane, SearchResult, TodoItem, TodoList};
use crate::storage;

pub struct App {
    pub lists: Vec<TodoList>,
    pub active_pane: Pane,
    pub selected_list_index: usize,
    pub selected_item_index: usize,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub input_cursor: usize,
    pub show_done: bool,
    pub running: bool,
    pub data_dir: PathBuf,
    pub undo_stack: Vec<AppSnapshot>,
    pub redo_stack: Vec<AppSnapshot>,
    pub search_results: Vec<SearchResult>,
    pub search_selected: usize,
    pub ascii_mode: bool,
}

impl App {
    pub fn new(data_dir: PathBuf, ascii_mode: bool) -> std::io::Result<Self> {
        let lists = storage::load_lists(&data_dir)?;
        Ok(Self {
            lists,
            active_pane: Pane::Sidebar,
            selected_list_index: 0,
            selected_item_index: 0,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            input_cursor: 0,
            show_done: true,
            running: true,
            data_dir,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            search_results: Vec::new(),
            search_selected: 0,
            ascii_mode,
        })
    }

    #[cfg(test)]
    pub fn with_lists(lists: Vec<TodoList>) -> Self {
        Self {
            lists,
            active_pane: Pane::Sidebar,
            selected_list_index: 0,
            selected_item_index: 0,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            input_cursor: 0,
            show_done: true,
            running: true,
            data_dir: PathBuf::from("/tmp/todui-test"),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            search_results: Vec::new(),
            search_selected: 0,
            ascii_mode: false,
        }
    }

    // ---- Snapshot / Undo / Redo ----

    fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            lists: self.lists.clone(),
            selected_list_index: self.selected_list_index,
            selected_item_index: self.selected_item_index,
        }
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.snapshot());
        self.redo_stack.clear();
    }

    fn restore_snapshot(&mut self, snap: AppSnapshot) {
        self.lists = snap.lists;
        self.selected_list_index = snap.selected_list_index;
        self.selected_item_index = snap.selected_item_index;
    }

    pub fn undo(&mut self) {
        if let Some(snap) = self.undo_stack.pop() {
            self.redo_stack.push(self.snapshot());
            self.restore_snapshot(snap);
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            self.restore_snapshot(snap);
        }
    }

    // ---- Navigation ----

    pub fn current_list(&self) -> Option<&TodoList> {
        self.lists.get(self.selected_list_index)
    }

    pub fn current_list_mut(&mut self) -> Option<&mut TodoList> {
        self.lists.get_mut(self.selected_list_index)
    }

    /// Returns (real_index, &item) pairs. Completed items are sorted to the bottom.
    pub fn visible_items(&self) -> Vec<(usize, &TodoItem)> {
        self.current_list()
            .map(|list| {
                let mut items: Vec<(usize, &TodoItem)> = list
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| self.show_done || !item.done)
                    .collect();
                // Sort: incomplete first, then completed
                items.sort_by_key(|(_, item)| item.done);
                items
            })
            .unwrap_or_default()
    }

    pub fn toggle_pane(&mut self) {
        self.active_pane = match self.active_pane {
            Pane::Sidebar => Pane::Main,
            Pane::Main => Pane::Sidebar,
        };
    }

    pub fn move_selection_up(&mut self) {
        match self.active_pane {
            Pane::Sidebar => {
                if self.selected_list_index > 0 {
                    self.selected_list_index -= 1;
                    self.selected_item_index = 0;
                }
            }
            Pane::Main => {
                if self.selected_item_index > 0 {
                    let visible = self.visible_items();
                    if !visible.is_empty() && self.selected_item_index > 0 {
                        self.selected_item_index -= 1;
                    }
                }
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        match self.active_pane {
            Pane::Sidebar => {
                if self.selected_list_index + 1 < self.lists.len() {
                    self.selected_list_index += 1;
                    self.selected_item_index = 0;
                }
            }
            Pane::Main => {
                let visible = self.visible_items();
                if !visible.is_empty() && self.selected_item_index + 1 < visible.len() {
                    self.selected_item_index += 1;
                }
            }
        }
    }

    pub fn clamp_selection(&mut self) {
        if self.lists.is_empty() {
            self.selected_list_index = 0;
            self.selected_item_index = 0;
            return;
        }
        if self.selected_list_index >= self.lists.len() {
            self.selected_list_index = self.lists.len() - 1;
        }
        let visible = self.visible_items();
        if visible.is_empty() {
            self.selected_item_index = 0;
        } else if self.selected_item_index >= visible.len() {
            self.selected_item_index = visible.len() - 1;
        }
    }

    // ---- Todo mutations ----

    fn selected_real_index(&self) -> Option<usize> {
        let visible = self.visible_items();
        visible
            .get(self.selected_item_index)
            .map(|(real_idx, _)| *real_idx)
    }

    pub fn toggle_done(&mut self) {
        if let Some(real_idx) = self.selected_real_index() {
            self.push_undo();
            if let Some(list) = self.current_list_mut() {
                list.items[real_idx].done = !list.items[real_idx].done;
            }
            self.save_current_list();
        }
    }

    pub fn delete_todo(&mut self) {
        if let Some(real_idx) = self.selected_real_index() {
            self.push_undo();
            if let Some(list) = self.current_list_mut() {
                list.items.remove(real_idx);
            }
            self.clamp_selection();
            self.save_current_list();
        }
    }

    pub fn add_todo(&mut self, title: String) {
        if title.trim().is_empty() {
            return;
        }
        self.push_undo();
        let (clean_title, tags) = storage::extract_tags_pub(&title);
        let item = TodoItem {
            title: clean_title,
            done: false,
            tags,
        };
        if let Some(list) = self.current_list_mut() {
            list.items.push(item);
        }
        self.save_current_list();
    }

    pub fn edit_todo_title(&mut self, new_title: String) {
        if let Some(real_idx) = self.selected_real_index() {
            self.push_undo();
            if let Some(list) = self.current_list_mut() {
                list.items[real_idx].title = new_title;
            }
            self.save_current_list();
        }
    }

    pub fn edit_todo_tags(&mut self, tags_str: String) {
        if let Some(real_idx) = self.selected_real_index() {
            self.push_undo();
            let tags: Vec<String> = tags_str
                .split_whitespace()
                .map(|t| t.strip_prefix('@').unwrap_or(t).to_string())
                .filter(|t| !t.is_empty())
                .collect();
            if let Some(list) = self.current_list_mut() {
                list.items[real_idx].tags = tags;
            }
            self.save_current_list();
        }
    }

    pub fn move_todo_up(&mut self) {
        if let Some(real_idx) = self.selected_real_index()
            && real_idx > 0
        {
            self.push_undo();
            if let Some(list) = self.current_list_mut() {
                list.items.swap(real_idx, real_idx - 1);
            }
            if self.selected_item_index > 0 {
                self.selected_item_index -= 1;
            }
            self.save_current_list();
        }
    }

    pub fn move_todo_down(&mut self) {
        if let Some(real_idx) = self.selected_real_index() {
            let len = self.current_list().map_or(0, |l| l.items.len());
            if real_idx + 1 < len {
                self.push_undo();
                if let Some(list) = self.current_list_mut() {
                    list.items.swap(real_idx, real_idx + 1);
                }
                let visible_len = self.visible_items().len();
                if self.selected_item_index + 1 < visible_len {
                    self.selected_item_index += 1;
                }
                self.save_current_list();
            }
        }
    }

    pub fn toggle_show_done(&mut self) {
        self.show_done = !self.show_done;
        self.clamp_selection();
    }

    // ---- List mutations ----

    pub fn add_list(&mut self, name: String) {
        if name.trim().is_empty() {
            return;
        }
        self.push_undo();
        let list = TodoList::new(name);
        self.lists.push(list);
        self.selected_list_index = self.lists.len() - 1;
        self.selected_item_index = 0;
        self.save_current_list();
    }

    pub fn rename_list(&mut self, new_name: String) {
        if new_name.trim().is_empty() {
            return;
        }
        if let Some(list) = self.current_list() {
            let old_name = list.name.clone();
            self.push_undo();
            let _ = storage::delete_list_file(&self.data_dir, &old_name);
            if let Some(list) = self.current_list_mut() {
                list.name = new_name;
            }
            self.save_current_list();
        }
    }

    pub fn delete_list(&mut self) {
        if self.lists.len() <= 1 {
            return;
        }
        self.push_undo();
        let list_name = self.lists[self.selected_list_index].name.clone();
        let _ = storage::delete_list_file(&self.data_dir, &list_name);
        self.lists.remove(self.selected_list_index);
        self.clamp_selection();
    }

    // ---- Input mode helpers ----

    pub fn start_input(&mut self, mode: InputMode, prefill: &str) {
        self.input_mode = mode;
        self.input_buffer = prefill.to_string();
        self.input_cursor = prefill.len();
    }

    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.search_results.clear();
        self.search_selected = 0;
    }

    pub fn confirm_input(&mut self) {
        let buffer = self.input_buffer.clone();
        match self.input_mode {
            InputMode::AddingItem => self.add_todo(buffer),
            InputMode::AddingList => self.add_list(buffer),
            InputMode::RenamingList => self.rename_list(buffer),
            InputMode::EditingItem => self.edit_todo_title(buffer),
            InputMode::EditingTags => self.edit_todo_tags(buffer),
            _ => {}
        }
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.input_cursor = 0;
    }

    pub fn input_insert_char(&mut self, c: char) {
        self.input_buffer.insert(self.input_cursor, c);
        self.input_cursor += c.len_utf8();
    }

    pub fn input_delete_char(&mut self) {
        if self.input_cursor > 0 {
            let prev = self.input_buffer[..self.input_cursor]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.input_cursor -= prev;
            self.input_buffer.remove(self.input_cursor);
        }
    }

    pub fn input_move_cursor_left(&mut self) {
        if self.input_cursor > 0 {
            let prev = self.input_buffer[..self.input_cursor]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.input_cursor -= prev;
        }
    }

    pub fn input_move_cursor_right(&mut self) {
        if self.input_cursor < self.input_buffer.len() {
            let next = self.input_buffer[self.input_cursor..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.input_cursor += next;
        }
    }

    // ---- Search ----

    pub fn start_search(&mut self) {
        self.input_mode = InputMode::Searching;
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.search_results.clear();
        self.search_selected = 0;
    }

    pub fn update_search_results(&mut self) {
        self.search_results.clear();

        let query = self.input_buffer.to_lowercase();
        if query.is_empty() {
            self.search_selected = 0;
            return;
        }

        for (li, list) in self.lists.iter().enumerate() {
            if list.name.to_lowercase().contains(&query) {
                self.search_results.push(SearchResult::List(li));
            }
            for (ii, item) in list.items.iter().enumerate() {
                let title_match = item.title.to_lowercase().contains(&query);
                let tag_match = item
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&query));
                if title_match || tag_match {
                    self.search_results.push(SearchResult::Item(li, ii));
                }
            }
        }

        if self.search_results.is_empty() {
            self.search_selected = 0;
        } else if self.search_selected >= self.search_results.len() {
            self.search_selected = self.search_results.len() - 1;
        }
    }

    pub fn search_select_next(&mut self) {
        if !self.search_results.is_empty() && self.search_selected + 1 < self.search_results.len()
        {
            self.search_selected += 1;
        }
    }

    pub fn search_select_prev(&mut self) {
        if self.search_selected > 0 {
            self.search_selected -= 1;
        }
    }

    pub fn select_search_result(&mut self) {
        if let Some(result) = self.search_results.get(self.search_selected).cloned() {
            match result {
                SearchResult::List(li) => {
                    self.selected_list_index = li;
                    self.active_pane = Pane::Sidebar;
                }
                SearchResult::Item(li, ii) => {
                    self.selected_list_index = li;
                    self.active_pane = Pane::Main;

                    // Enable show_done if the target item is done
                    if self.lists[li].items[ii].done {
                        self.show_done = true;
                    }

                    // Find the visible index for the real item index
                    let visible = self.visible_items();
                    if let Some(vi) = visible.iter().position(|(real_idx, _)| *real_idx == ii) {
                        self.selected_item_index = vi;
                    }
                }
            }

            self.input_mode = InputMode::Normal;
            self.input_buffer.clear();
            self.input_cursor = 0;
            self.search_results.clear();
            self.search_selected = 0;
        }
    }

    // ---- Persistence ----

    fn save_current_list(&self) {
        if let Some(list) = self.current_list() {
            let _ = storage::save_list(&self.data_dir, list);
        }
    }

    pub fn quit(&mut self) {
        let _ = storage::save_all(&self.data_dir, &self.lists);
        self.running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_toggle_pane() {
        let mut app = sample_app();
        assert_eq!(app.active_pane, Pane::Main);
        app.toggle_pane();
        assert_eq!(app.active_pane, Pane::Sidebar);
        app.toggle_pane();
        assert_eq!(app.active_pane, Pane::Main);
    }

    #[test]
    fn test_navigation_sidebar() {
        let mut app = sample_app();
        app.active_pane = Pane::Sidebar;
        assert_eq!(app.selected_list_index, 0);

        app.move_selection_down();
        assert_eq!(app.selected_list_index, 1);
        assert_eq!(app.selected_item_index, 0);

        app.move_selection_down();
        assert_eq!(app.selected_list_index, 1);

        app.move_selection_up();
        assert_eq!(app.selected_list_index, 0);
        assert_eq!(app.selected_item_index, 0);

        app.move_selection_up();
        assert_eq!(app.selected_list_index, 0);
    }

    #[test]
    fn test_navigation_main() {
        let mut app = sample_app();
        assert_eq!(app.selected_item_index, 0);

        app.move_selection_down();
        assert_eq!(app.selected_item_index, 1);

        app.move_selection_down();
        assert_eq!(app.selected_item_index, 2);

        app.move_selection_down();
        assert_eq!(app.selected_item_index, 2);

        app.move_selection_up();
        assert_eq!(app.selected_item_index, 1);

        app.move_selection_up();
        assert_eq!(app.selected_item_index, 0);

        app.move_selection_up();
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_toggle_done() {
        let mut app = sample_app();
        // Visible order after sort: Task A (idx 0), Task C (idx 1), Task B (idx 2)
        // selected_item_index=0 maps to Task A (real index 0)
        assert!(!app.lists[0].items[0].done);

        app.toggle_done();
        assert!(app.lists[0].items[0].done);

        // After sorting, visible order changes: Task C, Task A, Task B
        // selected_item_index=0 now maps to Task C (real index 2)
        app.toggle_done();
        assert!(app.lists[0].items[2].done);
    }

    #[test]
    fn test_delete_todo() {
        let mut app = sample_app();
        assert_eq!(app.lists[0].items.len(), 3);

        app.delete_todo();
        assert_eq!(app.lists[0].items.len(), 2);
        assert_eq!(app.lists[0].items[0].title, "Task B");
        assert_eq!(app.lists[0].items[1].title, "Task C");
    }

    #[test]
    fn test_add_todo() {
        let mut app = sample_app();
        assert_eq!(app.lists[0].items.len(), 3);

        app.add_todo("New task @urgent @work".to_string());
        assert_eq!(app.lists[0].items.len(), 4);
        let added = &app.lists[0].items[3];
        assert_eq!(added.title, "New task");
        assert_eq!(added.tags, vec!["urgent", "work"]);
        assert!(!added.done);
    }

    #[test]
    fn test_move_todo_up_down() {
        let mut app = sample_app();
        // Underlying items: Task A (not done), Task B (done), Task C (not done)
        // Visible order after sort: Task A (real 0), Task C (real 2), Task B (real 1)
        // Select visible index 1 = Task C (real index 2)
        app.selected_item_index = 1;

        // Move Task C up: swaps items[2] and items[1] -> underlying: [A, C, B]
        app.move_todo_up();
        assert_eq!(app.lists[0].items[0].title, "Task A");
        assert_eq!(app.lists[0].items[1].title, "Task C");
        assert_eq!(app.lists[0].items[2].title, "Task B");
        assert_eq!(app.selected_item_index, 0);

        // Move Task A down (now at visible idx 0, real idx 0): swaps items[0] and items[1]
        // -> underlying: [C, A, B]
        app.move_todo_down();
        assert_eq!(app.lists[0].items[0].title, "Task C");
        assert_eq!(app.lists[0].items[1].title, "Task A");
        assert_eq!(app.lists[0].items[2].title, "Task B");
        assert_eq!(app.selected_item_index, 1);
    }

    #[test]
    fn test_add_list() {
        let mut app = sample_app();
        assert_eq!(app.lists.len(), 2);

        app.add_list("Shopping".to_string());
        assert_eq!(app.lists.len(), 3);
        assert_eq!(app.lists[2].name, "Shopping");
        assert_eq!(app.selected_list_index, 2);
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_delete_list() {
        let mut app = sample_app();
        assert_eq!(app.lists.len(), 2);

        app.delete_list();
        assert_eq!(app.lists.len(), 1);
        assert_eq!(app.lists[0].name, "Personal");
    }

    #[test]
    fn test_delete_last_list_prevented() {
        let mut app = App::with_lists(vec![TodoList::new("Only")]);
        assert_eq!(app.lists.len(), 1);

        app.delete_list();
        assert_eq!(app.lists.len(), 1);
        assert_eq!(app.lists[0].name, "Only");
    }

    #[test]
    fn test_undo_redo() {
        let mut app = sample_app();
        assert_eq!(app.lists[0].items.len(), 3);

        app.delete_todo();
        assert_eq!(app.lists[0].items.len(), 2);

        app.undo();
        assert_eq!(app.lists[0].items.len(), 3);
        assert_eq!(app.lists[0].items[0].title, "Task A");

        app.redo();
        assert_eq!(app.lists[0].items.len(), 2);
        assert_eq!(app.lists[0].items[0].title, "Task B");
    }

    #[test]
    fn test_undo_clears_redo_on_new_action() {
        let mut app = sample_app();

        app.delete_todo();
        assert_eq!(app.lists[0].items.len(), 2);

        app.undo();
        assert_eq!(app.lists[0].items.len(), 3);
        assert!(!app.redo_stack.is_empty());

        app.add_todo("Brand new".to_string());
        assert!(app.redo_stack.is_empty());
    }

    #[test]
    fn test_toggle_show_done() {
        let mut app = sample_app();
        assert_eq!(app.visible_items().len(), 3);

        app.toggle_show_done();
        let visible = app.visible_items();
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].1.title, "Task A");
        assert_eq!(visible[1].1.title, "Task C");

        app.toggle_show_done();
        assert_eq!(app.visible_items().len(), 3);
    }

    #[test]
    fn test_input_buffer() {
        let mut app = sample_app();
        app.start_input(InputMode::AddingItem, "");
        assert_eq!(app.input_mode, InputMode::AddingItem);
        assert_eq!(app.input_buffer, "");
        assert_eq!(app.input_cursor, 0);

        app.input_insert_char('H');
        app.input_insert_char('i');
        assert_eq!(app.input_buffer, "Hi");
        assert_eq!(app.input_cursor, 2);

        app.input_move_cursor_left();
        assert_eq!(app.input_cursor, 1);

        app.input_insert_char('x');
        assert_eq!(app.input_buffer, "Hxi");
        assert_eq!(app.input_cursor, 2);

        app.input_delete_char();
        assert_eq!(app.input_buffer, "Hi");
        assert_eq!(app.input_cursor, 1);

        app.input_move_cursor_right();
        assert_eq!(app.input_cursor, 2);

        app.cancel_input();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.input_buffer, "");
        assert_eq!(app.input_cursor, 0);
    }

    // ---- Search tests ----

    fn search_app() -> App {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "Task A".to_string(),
            done: false,
            tags: vec!["code".to_string()],
        });
        work.items.push(TodoItem {
            title: "Task B".to_string(),
            done: true,
            tags: vec!["urgent".to_string()],
        });
        let mut personal = TodoList::new("Personal");
        personal.items.push(TodoItem {
            title: "Buy groceries".to_string(),
            done: false,
            tags: vec!["shopping".to_string()],
        });
        App::with_lists(vec![work, personal])
    }

    #[test]
    fn test_search_results_matches_title() {
        let mut app = search_app();
        app.start_search();
        app.input_buffer = "Task".to_string();
        app.update_search_results();
        assert_eq!(app.search_results.len(), 2);
        assert_eq!(app.search_results[0], SearchResult::Item(0, 0));
        assert_eq!(app.search_results[1], SearchResult::Item(0, 1));
    }

    #[test]
    fn test_search_results_matches_tags() {
        let mut app = search_app();
        app.start_search();
        app.input_buffer = "urgent".to_string();
        app.update_search_results();
        assert_eq!(app.search_results.len(), 1);
        assert_eq!(app.search_results[0], SearchResult::Item(0, 1));
    }

    #[test]
    fn test_search_results_matches_list_name() {
        let mut app = search_app();
        app.start_search();
        app.input_buffer = "Personal".to_string();
        app.update_search_results();
        assert!(app
            .search_results
            .iter()
            .any(|r| *r == SearchResult::List(1)));
    }

    #[test]
    fn test_search_results_case_insensitive() {
        let mut app = search_app();
        app.start_search();
        app.input_buffer = "TASK".to_string();
        app.update_search_results();
        assert_eq!(app.search_results.len(), 2);
    }

    #[test]
    fn test_search_results_empty_query() {
        let mut app = search_app();
        app.start_search();
        app.input_buffer.clear();
        app.update_search_results();
        assert!(app.search_results.is_empty());
    }

    #[test]
    fn test_search_select_next_prev() {
        let mut app = search_app();
        app.start_search();
        app.input_buffer = "Task".to_string();
        app.update_search_results();
        assert_eq!(app.search_selected, 0);

        app.search_select_next();
        assert_eq!(app.search_selected, 1);

        app.search_select_next();
        assert_eq!(app.search_selected, 1);

        app.search_select_prev();
        assert_eq!(app.search_selected, 0);

        app.search_select_prev();
        assert_eq!(app.search_selected, 0);
    }

    #[test]
    fn test_select_search_result_list() {
        let mut app = search_app();
        app.start_search();
        app.input_buffer = "Personal".to_string();
        app.update_search_results();
        let list_pos = app
            .search_results
            .iter()
            .position(|r| matches!(r, SearchResult::List(1)))
            .unwrap();
        app.search_selected = list_pos;
        app.select_search_result();

        assert_eq!(app.selected_list_index, 1);
        assert_eq!(app.active_pane, Pane::Sidebar);
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.search_results.is_empty());
    }

    #[test]
    fn test_select_search_result_item() {
        let mut app = search_app();
        app.start_search();
        app.input_buffer = "groceries".to_string();
        app.update_search_results();
        assert_eq!(app.search_results.len(), 1);
        assert_eq!(app.search_results[0], SearchResult::Item(1, 0));

        app.select_search_result();
        assert_eq!(app.selected_list_index, 1);
        assert_eq!(app.active_pane, Pane::Main);
        assert_eq!(app.selected_item_index, 0);
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_select_search_result_done_item_enables_show_done() {
        let mut app = search_app();
        app.show_done = false;
        app.start_search();
        app.input_buffer = "Task B".to_string();
        app.update_search_results();
        assert_eq!(app.search_results[0], SearchResult::Item(0, 1));
        app.select_search_result();
        assert!(app.show_done);
        assert_eq!(app.selected_list_index, 0);
        assert_eq!(app.active_pane, Pane::Main);
    }
}
