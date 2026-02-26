use std::path::PathBuf;
use std::time::Instant;

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
    pub focus_list: usize,
    pub focus_item: usize,
    pub focus_start: Option<Instant>,
    pub focus_accumulated: u64,
    pub filter_tags: Vec<String>,
    pub filter_available_tags: Vec<String>,
    pub filter_selected: Vec<bool>,
    pub filter_cursor: usize,
    pub autocomplete_suggestions: Vec<String>,
    pub autocomplete_cursor: usize,
    pub autocomplete_active: bool,
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
            focus_list: 0,
            focus_item: 0,
            focus_start: None,
            focus_accumulated: 0,
            filter_tags: Vec::new(),
            filter_available_tags: Vec::new(),
            filter_selected: Vec::new(),
            filter_cursor: 0,
            autocomplete_suggestions: Vec::new(),
            autocomplete_cursor: 0,
            autocomplete_active: false,
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
            focus_list: 0,
            focus_item: 0,
            focus_start: None,
            focus_accumulated: 0,
            filter_tags: Vec::new(),
            filter_available_tags: Vec::new(),
            filter_selected: Vec::new(),
            filter_cursor: 0,
            autocomplete_suggestions: Vec::new(),
            autocomplete_cursor: 0,
            autocomplete_active: false,
        }
    }

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

    pub fn current_list(&self) -> Option<&TodoList> {
        self.lists.get(self.selected_list_index)
    }

    pub fn current_list_mut(&mut self) -> Option<&mut TodoList> {
        self.lists.get_mut(self.selected_list_index)
    }

    /// Returns (real_index, &item) pairs, sorted with incomplete items first.
    /// Respects show_done and filter_tags settings.
    pub fn visible_items(&self) -> Vec<(usize, &TodoItem)> {
        self.current_list()
            .map(|list| {
                let mut items: Vec<(usize, &TodoItem)> = list
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| self.show_done || !item.done)
                    .filter(|(_, item)| {
                        self.filter_tags.is_empty()
                            || item.tags.iter().any(|t| self.filter_tags.contains(t))
                    })
                    .collect();
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
                    self.filter_tags.clear();
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
                    self.filter_tags.clear();
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
            time_secs: 0,
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

    pub fn start_focus(&mut self) {
        let visible = self.visible_items();
        if let Some(&(real_idx, _)) = visible.get(self.selected_item_index) {
            self.focus_list = self.selected_list_index;
            self.focus_item = real_idx;
            self.focus_accumulated = 0;
            self.focus_start = Some(Instant::now());
            self.input_mode = InputMode::Focused;
        }
    }

    pub fn stop_focus(&mut self) {
        let elapsed = self.focus_elapsed_secs();
        self.lists[self.focus_list].items[self.focus_item].time_secs += elapsed;
        self.focus_start = None;
        self.focus_accumulated = 0;
        self.input_mode = InputMode::Normal;
        self.save_current_list();
    }

    pub fn toggle_pause_focus(&mut self) {
        if let Some(start) = self.focus_start {
            self.focus_accumulated += start.elapsed().as_secs();
            self.focus_start = None;
        } else {
            self.focus_start = Some(Instant::now());
        }
    }

    pub fn focus_elapsed_secs(&self) -> u64 {
        let running = self.focus_start.map_or(0, |s| s.elapsed().as_secs());
        self.focus_accumulated + running
    }

    pub fn set_item_time(&mut self, secs: u64) {
        if let Some(real_idx) = self.selected_real_index() {
            self.push_undo();
            if let Some(list) = self.current_list_mut() {
                list.items[real_idx].time_secs = secs;
            }
            self.save_current_list();
        }
    }

    pub fn start_filter(&mut self) {
        let mut tags: Vec<String> = Vec::new();
        if let Some(list) = self.current_list() {
            for item in &list.items {
                for tag in &item.tags {
                    if !tags.contains(tag) {
                        tags.push(tag.clone());
                    }
                }
            }
        }
        tags.sort();

        if tags.is_empty() {
            return;
        }

        let selected: Vec<bool> = tags
            .iter()
            .map(|t| self.filter_tags.contains(t))
            .collect();

        self.filter_available_tags = tags;
        self.filter_selected = selected;
        self.filter_cursor = 0;
        self.input_mode = InputMode::FilteringTags;
    }

    pub fn confirm_filter(&mut self) {
        self.filter_tags = self
            .filter_available_tags
            .iter()
            .zip(self.filter_selected.iter())
            .filter(|(_, sel)| **sel)
            .map(|(tag, _)| tag.clone())
            .collect();
        self.input_mode = InputMode::Normal;
        self.filter_available_tags.clear();
        self.filter_selected.clear();
        self.filter_cursor = 0;
        self.clamp_selection();
    }

    pub fn cancel_filter(&mut self) {
        self.input_mode = InputMode::Normal;
        self.filter_available_tags.clear();
        self.filter_selected.clear();
        self.filter_cursor = 0;
    }

    pub fn toggle_filter_tag(&mut self) {
        if self.filter_cursor < self.filter_selected.len() {
            self.filter_selected[self.filter_cursor] = !self.filter_selected[self.filter_cursor];
        }
    }

    pub fn filter_move_up(&mut self) {
        if self.filter_cursor > 0 {
            self.filter_cursor -= 1;
        }
    }

    pub fn filter_move_down(&mut self) {
        if !self.filter_available_tags.is_empty()
            && self.filter_cursor + 1 < self.filter_available_tags.len()
        {
            self.filter_cursor += 1;
        }
    }

    pub fn collect_all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = Vec::new();
        for list in &self.lists {
            for item in &list.items {
                for tag in &item.tags {
                    if !tags.contains(tag) {
                        tags.push(tag.clone());
                    }
                }
            }
        }
        tags.sort();
        tags
    }

    pub fn update_autocomplete(&mut self) {
        let mode = self.input_mode;
        if !matches!(
            mode,
            InputMode::AddingItem | InputMode::EditingItem | InputMode::EditingTags
        ) {
            self.autocomplete_active = false;
            self.autocomplete_suggestions.clear();
            return;
        }

        let before_cursor = &self.input_buffer[..self.input_cursor];
        if let Some(at_pos) = before_cursor.rfind('@') {
            let at_starts_token = at_pos == 0 || before_cursor.as_bytes()[at_pos - 1] == b' ';
            if at_starts_token {
                let partial = &before_cursor[at_pos + 1..];
                if !partial.contains(' ') {
                    let all_tags = self.collect_all_tags();
                    let suggestions: Vec<String> = all_tags
                        .into_iter()
                        .filter(|t| t.to_lowercase().starts_with(&partial.to_lowercase()))
                        .collect();
                    if !suggestions.is_empty() {
                        self.autocomplete_suggestions = suggestions;
                        self.autocomplete_cursor = 0;
                        self.autocomplete_active = true;
                        return;
                    }
                }
            }
        }

        self.autocomplete_active = false;
        self.autocomplete_suggestions.clear();
    }

    pub fn accept_autocomplete(&mut self) {
        if !self.autocomplete_active || self.autocomplete_suggestions.is_empty() {
            return;
        }

        let suggestion = self.autocomplete_suggestions[self.autocomplete_cursor].clone();
        let before_cursor = &self.input_buffer[..self.input_cursor];

        if let Some(at_pos) = before_cursor.rfind('@') {
            let after_cursor = self.input_buffer[self.input_cursor..].to_string();
            let before_at = self.input_buffer[..at_pos].to_string();
            self.input_buffer = format!("{}@{}{}", before_at, suggestion, after_cursor);
            self.input_cursor = at_pos + 1 + suggestion.len();
        }

        self.autocomplete_active = false;
        self.autocomplete_suggestions.clear();
    }

    pub fn dismiss_autocomplete(&mut self) {
        self.autocomplete_active = false;
        self.autocomplete_suggestions.clear();
        self.autocomplete_cursor = 0;
    }

    pub fn autocomplete_move_up(&mut self) {
        if self.autocomplete_cursor > 0 {
            self.autocomplete_cursor -= 1;
        }
    }

    pub fn autocomplete_move_down(&mut self) {
        if !self.autocomplete_suggestions.is_empty()
            && self.autocomplete_cursor + 1 < self.autocomplete_suggestions.len()
        {
            self.autocomplete_cursor += 1;
        }
    }

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
        self.save_order();
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
            self.save_order();
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
        self.save_order();
    }

    pub fn move_list_up(&mut self) {
        if self.selected_list_index > 0 {
            self.push_undo();
            self.lists
                .swap(self.selected_list_index, self.selected_list_index - 1);
            self.selected_list_index -= 1;
            self.save_order();
        }
    }

    pub fn move_list_down(&mut self) {
        if self.selected_list_index + 1 < self.lists.len() {
            self.push_undo();
            self.lists
                .swap(self.selected_list_index, self.selected_list_index + 1);
            self.selected_list_index += 1;
            self.save_order();
        }
    }

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
            InputMode::EditingTime => {
                if let Some(secs) = storage::parse_time_str(&buffer) {
                    self.set_item_time(secs);
                }
            }
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

                    if self.lists[li].items[ii].done {
                        self.show_done = true;
                    }

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

    fn save_current_list(&self) {
        if let Some(list) = self.current_list() {
            let _ = storage::save_list(&self.data_dir, list);
        }
    }

    fn save_order(&self) {
        let _ = storage::save_order(&self.data_dir, &self.lists);
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
        assert!(!app.lists[0].items[0].done);

        app.toggle_done();
        assert!(app.lists[0].items[0].done);

        // After toggling, sort order changes so selected_item_index=0 now maps to Task C
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
        // Visible order (done sorted last): Task A (real 0), Task C (real 2), Task B (real 1)
        app.selected_item_index = 1; // Task C

        app.move_todo_up();
        assert_eq!(app.lists[0].items[0].title, "Task A");
        assert_eq!(app.lists[0].items[1].title, "Task C");
        assert_eq!(app.lists[0].items[2].title, "Task B");
        assert_eq!(app.selected_item_index, 0);

        app.move_todo_down();
        assert_eq!(app.lists[0].items[0].title, "Task C");
        assert_eq!(app.lists[0].items[1].title, "Task A");
        assert_eq!(app.lists[0].items[2].title, "Task B");
        assert_eq!(app.selected_item_index, 1);
    }

    #[test]
    fn test_move_list_up() {
        let mut app = App::with_lists(vec![
            TodoList::new("Alpha"),
            TodoList::new("Beta"),
            TodoList::new("Gamma"),
        ]);
        app.active_pane = Pane::Sidebar;
        app.selected_list_index = 1;

        app.move_list_up();
        assert_eq!(app.lists[0].name, "Beta");
        assert_eq!(app.lists[1].name, "Alpha");
        assert_eq!(app.selected_list_index, 0);

        app.move_list_up();
        assert_eq!(app.lists[0].name, "Beta");
        assert_eq!(app.selected_list_index, 0);
    }

    #[test]
    fn test_move_list_down() {
        let mut app = App::with_lists(vec![
            TodoList::new("Alpha"),
            TodoList::new("Beta"),
            TodoList::new("Gamma"),
        ]);
        app.active_pane = Pane::Sidebar;
        app.selected_list_index = 1;

        app.move_list_down();
        assert_eq!(app.lists[1].name, "Gamma");
        assert_eq!(app.lists[2].name, "Beta");
        assert_eq!(app.selected_list_index, 2);

        app.move_list_down();
        assert_eq!(app.lists[2].name, "Beta");
        assert_eq!(app.selected_list_index, 2);
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

    fn search_app() -> App {
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
            tags: vec!["urgent".to_string()],
            time_secs: 0,
        });
        let mut personal = TodoList::new("Personal");
        personal.items.push(TodoItem {
            title: "Buy groceries".to_string(),
            done: false,
            tags: vec!["shopping".to_string()],
            time_secs: 0,
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

    #[test]
    fn test_start_focus() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        app.start_focus();
        assert_eq!(app.input_mode, InputMode::Focused);
        assert!(app.focus_start.is_some());
        assert_eq!(app.focus_accumulated, 0);
    }

    #[test]
    fn test_stop_focus_accumulates_time() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        app.start_focus();
        app.focus_start = Some(std::time::Instant::now() - std::time::Duration::from_secs(10));
        app.stop_focus();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.lists[0].items[0].time_secs >= 9);
    }

    #[test]
    fn test_pause_resume_focus() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        app.start_focus();
        app.focus_start = Some(std::time::Instant::now() - std::time::Duration::from_secs(5));
        app.toggle_pause_focus();
        assert!(app.focus_start.is_none());
        assert!(app.focus_accumulated >= 4);

        app.toggle_pause_focus();
        assert!(app.focus_start.is_some());
    }

    #[test]
    fn test_edit_time() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        app.lists[0].items[0].time_secs = 3600;
        app.set_item_time(5400);
        assert_eq!(app.lists[0].items[0].time_secs, 5400);
    }

    #[test]
    fn test_start_focus_no_items() {
        let mut app = App::with_lists(vec![TodoList::new("Empty")]);
        app.active_pane = Pane::Main;
        app.start_focus();
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    fn tagged_app() -> App {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "Task A".to_string(),
            done: false,
            tags: vec!["code".to_string(), "urgent".to_string()],
            time_secs: 0,
        });
        work.items.push(TodoItem {
            title: "Task B".to_string(),
            done: false,
            tags: vec!["meeting".to_string()],
            time_secs: 0,
        });
        work.items.push(TodoItem {
            title: "Task C".to_string(),
            done: false,
            tags: vec!["code".to_string()],
            time_secs: 0,
        });
        work.items.push(TodoItem {
            title: "Task D".to_string(),
            done: false,
            tags: vec![],
            time_secs: 0,
        });
        let mut personal = TodoList::new("Personal");
        personal.items.push(TodoItem {
            title: "Buy stuff".to_string(),
            done: false,
            tags: vec!["shopping".to_string()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work, personal]);
        app.active_pane = Pane::Main;
        app
    }

    #[test]
    fn test_start_filter_collects_tags() {
        let mut app = tagged_app();
        app.start_filter();
        assert_eq!(app.input_mode, InputMode::FilteringTags);
        assert_eq!(app.filter_available_tags, vec!["code", "meeting", "urgent"]);
        assert_eq!(app.filter_selected, vec![false, false, false]);
    }

    #[test]
    fn test_start_filter_no_tags() {
        let mut app = App::with_lists(vec![TodoList::new("Empty")]);
        app.active_pane = Pane::Main;
        app.start_filter();
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_toggle_filter_tag() {
        let mut app = tagged_app();
        app.start_filter();
        app.toggle_filter_tag();
        assert_eq!(app.filter_selected, vec![true, false, false]);
        app.toggle_filter_tag();
        assert_eq!(app.filter_selected, vec![false, false, false]);
    }

    #[test]
    fn test_filter_navigation() {
        let mut app = tagged_app();
        app.start_filter();
        assert_eq!(app.filter_cursor, 0);
        app.filter_move_down();
        assert_eq!(app.filter_cursor, 1);
        app.filter_move_down();
        assert_eq!(app.filter_cursor, 2);
        app.filter_move_down();
        assert_eq!(app.filter_cursor, 2);
        app.filter_move_up();
        assert_eq!(app.filter_cursor, 1);
        app.filter_move_up();
        assert_eq!(app.filter_cursor, 0);
        app.filter_move_up();
        assert_eq!(app.filter_cursor, 0);
    }

    #[test]
    fn test_confirm_filter_applies_tags() {
        let mut app = tagged_app();
        app.start_filter();
        app.toggle_filter_tag();
        app.confirm_filter();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.filter_tags, vec!["code"]);
    }

    #[test]
    fn test_cancel_filter() {
        let mut app = tagged_app();
        app.start_filter();
        app.toggle_filter_tag();
        app.cancel_filter();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.filter_tags.is_empty());
    }

    #[test]
    fn test_visible_items_with_filter() {
        let mut app = tagged_app();
        assert_eq!(app.visible_items().len(), 4);

        app.filter_tags = vec!["code".to_string()];
        let visible = app.visible_items();
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].1.title, "Task A");
        assert_eq!(visible[1].1.title, "Task C");
    }

    #[test]
    fn test_visible_items_filter_or_logic() {
        let mut app = tagged_app();
        app.filter_tags = vec!["code".to_string(), "meeting".to_string()];
        let visible = app.visible_items();
        assert_eq!(visible.len(), 3);
    }

    #[test]
    fn test_filter_cleared_on_list_change() {
        let mut app = tagged_app();
        app.filter_tags = vec!["code".to_string()];
        app.active_pane = Pane::Sidebar;
        app.move_selection_down();
        assert!(app.filter_tags.is_empty());
    }

    #[test]
    fn test_start_filter_preserves_active_filter() {
        let mut app = tagged_app();
        app.filter_tags = vec!["code".to_string()];
        app.start_filter();
        assert_eq!(app.filter_selected, vec![true, false, false]);
    }

    #[test]
    fn test_collect_all_tags() {
        let app = tagged_app();
        let tags = app.collect_all_tags();
        assert_eq!(tags, vec!["code", "meeting", "shopping", "urgent"]);
    }

    #[test]
    fn test_autocomplete_activates_on_at() {
        let mut app = tagged_app();
        app.start_input(InputMode::AddingItem, "");
        app.input_insert_char('@');
        app.update_autocomplete();
        assert!(app.autocomplete_active);
        assert_eq!(app.autocomplete_suggestions.len(), 4);
    }

    #[test]
    fn test_autocomplete_filters_by_partial() {
        let mut app = tagged_app();
        app.start_input(InputMode::AddingItem, "");
        app.input_insert_char('@');
        app.input_insert_char('c');
        app.update_autocomplete();
        assert!(app.autocomplete_active);
        assert_eq!(app.autocomplete_suggestions, vec!["code"]);
    }

    #[test]
    fn test_autocomplete_no_match_dismisses() {
        let mut app = tagged_app();
        app.start_input(InputMode::AddingItem, "");
        app.input_insert_char('@');
        app.input_insert_char('z');
        app.input_insert_char('z');
        app.input_insert_char('z');
        app.update_autocomplete();
        assert!(!app.autocomplete_active);
        assert!(app.autocomplete_suggestions.is_empty());
    }

    #[test]
    fn test_accept_autocomplete() {
        let mut app = tagged_app();
        app.start_input(InputMode::AddingItem, "");
        app.input_insert_char('@');
        app.input_insert_char('c');
        app.update_autocomplete();
        assert!(app.autocomplete_active);

        app.accept_autocomplete();
        assert_eq!(app.input_buffer, "@code");
        assert_eq!(app.input_cursor, 5);
        assert!(!app.autocomplete_active);
    }

    #[test]
    fn test_autocomplete_with_preceding_text() {
        let mut app = tagged_app();
        app.start_input(InputMode::AddingItem, "");
        for c in "Do stuff ".chars() {
            app.input_insert_char(c);
        }
        app.input_insert_char('@');
        app.input_insert_char('u');
        app.update_autocomplete();
        assert!(app.autocomplete_active);
        assert_eq!(app.autocomplete_suggestions, vec!["urgent"]);

        app.accept_autocomplete();
        assert_eq!(app.input_buffer, "Do stuff @urgent");
    }

    #[test]
    fn test_autocomplete_navigation() {
        let mut app = tagged_app();
        app.start_input(InputMode::AddingItem, "");
        app.input_insert_char('@');
        app.update_autocomplete();
        assert_eq!(app.autocomplete_cursor, 0);

        app.autocomplete_move_down();
        assert_eq!(app.autocomplete_cursor, 1);

        app.autocomplete_move_up();
        assert_eq!(app.autocomplete_cursor, 0);

        app.autocomplete_move_up();
        assert_eq!(app.autocomplete_cursor, 0);
    }

    #[test]
    fn test_dismiss_autocomplete() {
        let mut app = tagged_app();
        app.start_input(InputMode::AddingItem, "");
        app.input_insert_char('@');
        app.update_autocomplete();
        assert!(app.autocomplete_active);

        app.dismiss_autocomplete();
        assert!(!app.autocomplete_active);
    }

    #[test]
    fn test_autocomplete_inactive_in_normal_mode() {
        let mut app = tagged_app();
        app.input_buffer = "@c".to_string();
        app.input_cursor = 2;
        app.update_autocomplete();
        assert!(!app.autocomplete_active);
    }

    #[test]
    fn test_autocomplete_works_in_editing_tags() {
        let mut app = tagged_app();
        app.start_input(InputMode::EditingTags, "");
        app.input_insert_char('@');
        app.update_autocomplete();
        assert!(app.autocomplete_active);
    }
}
