use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use crate::config::KeyConfig;
use crate::model::{
    AppSnapshot, InputMode, ListType, Pane, SearchResult, SidebarEntry, TodoItem, TodoList,
};
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
    pub selected_items: HashSet<usize>,
    pub move_to_list_cursor: usize,
    pub move_to_list_filter: String,
    pub sidebar_entries: Vec<SidebarEntry>,
    pub selected_sidebar_index: usize,
    pub current_context: String,
    pub context_cursor: usize,
    pub context_filter: String,
    pub key_config: KeyConfig,
}

impl App {
    pub fn new(
        data_dir: PathBuf,
        context: String,
        ascii_mode: bool,
        key_config: KeyConfig,
    ) -> std::io::Result<Self> {
        let context_dir = data_dir.join(&context);
        let lists = storage::load_lists(&context_dir)?;
        let mut app = Self {
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
            selected_items: HashSet::new(),
            move_to_list_cursor: 0,
            move_to_list_filter: String::new(),
            sidebar_entries: Vec::new(),
            selected_sidebar_index: 0,
            current_context: context,
            context_cursor: 0,
            context_filter: String::new(),
            key_config,
        };
        app.rebuild_sidebar_entries();
        Ok(app)
    }

    #[cfg(test)]
    pub fn with_lists(lists: Vec<TodoList>) -> Self {
        let mut app = Self {
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
            selected_items: HashSet::new(),
            move_to_list_cursor: 0,
            move_to_list_filter: String::new(),
            sidebar_entries: Vec::new(),
            selected_sidebar_index: 0,
            current_context: "test".into(),
            context_cursor: 0,
            context_filter: String::new(),
            key_config: KeyConfig::default(),
        };
        app.rebuild_sidebar_entries();
        app
    }

    pub fn context_dir(&self) -> PathBuf {
        self.data_dir.join(&self.current_context)
    }

    pub fn available_contexts(&self) -> Vec<String> {
        storage::list_contexts(&self.data_dir)
    }

    pub fn context_targets(&self) -> Vec<String> {
        let query = self.context_filter.to_lowercase();
        self.available_contexts()
            .into_iter()
            .filter(|c| c != &self.current_context)
            .filter(|c| query.is_empty() || c.to_lowercase().contains(&query))
            .collect()
    }

    pub fn start_switch_context(&mut self) {
        self.context_cursor = 0;
        self.context_filter.clear();
        self.input_mode = InputMode::SwitchingContext;
    }

    pub fn switch_context(&mut self, name: &str) {
        let _ = storage::save_all(&self.context_dir(), &self.lists);
        let _ = storage::save_order(&self.context_dir(), &self.lists);
        self.current_context = name.to_string();
        self.lists = storage::load_lists(&self.context_dir())
            .unwrap_or_else(|_| vec![TodoList::new("Inbox")]);
        self.selected_list_index = 0;
        self.selected_item_index = 0;
        self.selected_sidebar_index = 0;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.selected_items.clear();
        self.filter_tags.clear();
        self.rebuild_sidebar_entries();
        let _ = storage::save_last_context(&self.data_dir, name);
        self.input_mode = InputMode::Normal;
    }

    pub fn create_context(&mut self, name: String) {
        let slug = name.to_lowercase().replace(' ', "-");
        if slug.is_empty() {
            self.input_mode = InputMode::Normal;
            return;
        }
        let _ = storage::create_context_dir(&self.data_dir, &slug);
        self.switch_context(&slug);
    }

    pub fn context_move_up(&mut self) {
        if self.context_cursor > 0 {
            self.context_cursor -= 1;
        }
    }

    pub fn context_move_down(&mut self) {
        let targets = self.context_targets();
        let total = targets.len() + 1; // +1 for "new context" option
        if self.context_cursor + 1 < total {
            self.context_cursor += 1;
        }
    }

    pub fn context_insert_char(&mut self, c: char) {
        self.context_filter.push(c);
        self.context_cursor = 0;
    }

    pub fn context_delete_char(&mut self) {
        self.context_filter.pop();
        self.context_cursor = 0;
    }

    pub fn confirm_context_switch(&mut self) {
        let targets = self.context_targets();
        if self.context_cursor < targets.len() {
            let name = targets[self.context_cursor].clone();
            self.switch_context(&name);
        } else {
            self.context_filter.clear();
            self.input_mode = InputMode::CreatingContext;
            self.input_buffer.clear();
            self.input_cursor = 0;
        }
    }

    pub fn cancel_context_switch(&mut self) {
        self.context_filter.clear();
        self.input_mode = InputMode::Normal;
    }

    fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            lists: self.lists.clone(),
            selected_list_index: self.selected_list_index,
            selected_item_index: self.selected_item_index,
            selected_sidebar_index: self.selected_sidebar_index,
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
        self.selected_sidebar_index = snap.selected_sidebar_index;
        self.rebuild_sidebar_entries();
    }

    pub fn undo(&mut self) {
        if let Some(snap) = self.undo_stack.pop() {
            self.redo_stack.push(self.snapshot());
            self.restore_snapshot(snap);
            self.selected_items.clear();
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            self.restore_snapshot(snap);
            self.selected_items.clear();
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

    fn sync_sidebar_selection(&mut self) {
        match self.sidebar_entries.get(self.selected_sidebar_index) {
            Some(SidebarEntry::List(i)) => {
                self.selected_list_index = *i;
            }
            Some(SidebarEntry::Tag(_)) | None => {}
        }
        self.selected_item_index = 0;
        self.filter_tags.clear();
        self.selected_items.clear();
    }

    pub fn jump_to_first(&mut self) {
        match self.active_pane {
            Pane::Sidebar => {
                if self.selected_sidebar_index != 0 {
                    self.selected_sidebar_index = 0;
                    self.sync_sidebar_selection();
                }
            }
            Pane::Main => {
                self.selected_item_index = 0;
            }
        }
    }

    pub fn page_down(&mut self, page_size: usize) {
        match self.active_pane {
            Pane::Sidebar => {
                if !self.sidebar_entries.is_empty() {
                    let last = self.sidebar_entries.len() - 1;
                    let target = (self.selected_sidebar_index + page_size).min(last);
                    if target != self.selected_sidebar_index {
                        self.selected_sidebar_index = target;
                        self.sync_sidebar_selection();
                    }
                }
            }
            Pane::Main => {
                let count = if self.is_tag_view() {
                    self.tag_visible_items().len()
                } else {
                    self.visible_items().len()
                };
                if count > 0 {
                    let last = count - 1;
                    self.selected_item_index = (self.selected_item_index + page_size).min(last);
                }
            }
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        match self.active_pane {
            Pane::Sidebar => {
                let target = self.selected_sidebar_index.saturating_sub(page_size);
                if target != self.selected_sidebar_index {
                    self.selected_sidebar_index = target;
                    self.sync_sidebar_selection();
                }
            }
            Pane::Main => {
                self.selected_item_index = self.selected_item_index.saturating_sub(page_size);
            }
        }
    }

    pub fn jump_to_last(&mut self) {
        match self.active_pane {
            Pane::Sidebar => {
                if !self.sidebar_entries.is_empty() {
                    let last = self.sidebar_entries.len() - 1;
                    if self.selected_sidebar_index != last {
                        self.selected_sidebar_index = last;
                        self.sync_sidebar_selection();
                    }
                }
            }
            Pane::Main => {
                let count = if self.is_tag_view() {
                    self.tag_visible_items().len()
                } else {
                    self.visible_items().len()
                };
                if count > 0 {
                    self.selected_item_index = count - 1;
                }
            }
        }
    }

    pub fn toggle_pane(&mut self) {
        self.active_pane = match self.active_pane {
            Pane::Sidebar => Pane::Main,
            Pane::Main => Pane::Sidebar,
        };
        self.selected_items.clear();
    }

    pub fn move_selection_up(&mut self) {
        match self.active_pane {
            Pane::Sidebar => {
                if self.selected_sidebar_index > 0 {
                    self.selected_sidebar_index -= 1;
                    self.sync_sidebar_selection();
                }
            }
            Pane::Main => {
                if self.selected_item_index > 0 {
                    self.selected_item_index -= 1;
                }
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        match self.active_pane {
            Pane::Sidebar => {
                if self.selected_sidebar_index + 1 < self.sidebar_entries.len() {
                    self.selected_sidebar_index += 1;
                    self.sync_sidebar_selection();
                }
            }
            Pane::Main => {
                let count = if self.is_tag_view() {
                    self.tag_visible_items().len()
                } else {
                    self.visible_items().len()
                };
                if count > 0 && self.selected_item_index + 1 < count {
                    self.selected_item_index += 1;
                }
            }
        }
    }

    pub fn clamp_selection(&mut self) {
        if self.sidebar_entries.is_empty() {
            self.selected_sidebar_index = 0;
            self.selected_list_index = 0;
            self.selected_item_index = 0;
            return;
        }
        if self.selected_sidebar_index >= self.sidebar_entries.len() {
            self.selected_sidebar_index = self.sidebar_entries.len() - 1;
        }
        if let Some(SidebarEntry::List(i)) = self.sidebar_entries.get(self.selected_sidebar_index) {
            self.selected_list_index = *i;
        }
        if self.is_tag_view() {
            let tag_items = self.tag_visible_items();
            if tag_items.is_empty() {
                self.selected_item_index = 0;
            } else if self.selected_item_index >= tag_items.len() {
                self.selected_item_index = tag_items.len() - 1;
            }
        } else {
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
    }

    fn selected_real_index(&self) -> Option<usize> {
        let visible = self.visible_items();
        visible
            .get(self.selected_item_index)
            .map(|(real_idx, _)| *real_idx)
    }

    pub fn toggle_done(&mut self) {
        if let Some((li, ii)) = self.resolve_selected_item() {
            self.push_undo();
            self.lists[li].items[ii].done = !self.lists[li].items[ii].done;
            self.save_list_at(li);
        }
    }

    pub fn toggle_tag(&mut self, tag: &str) {
        if let Some((li, ii)) = self.resolve_selected_item() {
            self.push_undo();
            let tags = &mut self.lists[li].items[ii].tags;
            if let Some(pos) = tags.iter().position(|t| t == tag) {
                tags.remove(pos);
            } else {
                tags.push(tag.to_string());
            }
            self.save_list_at(li);
            self.rebuild_sidebar_entries();
        }
    }

    pub fn delete_todo(&mut self) {
        if let Some((li, ii)) = self.resolve_selected_item() {
            self.push_undo();
            self.lists[li].items.remove(ii);
            self.rebuild_sidebar_entries();
            self.clamp_selection();
            self.save_list_at(li);
        }
    }

    pub fn add_todo(&mut self, title: String) {
        if self.is_tag_view() {
            return;
        }
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
        let insert_idx = self.selected_real_index();
        if let Some(list) = self.current_list_mut() {
            match insert_idx {
                Some(idx) => list.items.insert(idx, item),
                None => list.items.push(item),
            }
        }
        self.rebuild_sidebar_entries();
        self.save_current_list();
    }

    pub fn edit_todo_title(&mut self, new_title: String) {
        if let Some((li, ii)) = self.resolve_selected_item() {
            self.push_undo();
            self.lists[li].items[ii].title = new_title;
            self.save_list_at(li);
        }
    }

    pub fn edit_todo_tags(&mut self, tags_str: String) {
        if let Some((li, ii)) = self.resolve_selected_item() {
            self.push_undo();
            let tags: Vec<String> = tags_str
                .split_whitespace()
                .map(|t| t.strip_prefix('@').unwrap_or(t).to_string())
                .filter(|t| !t.is_empty())
                .collect();
            self.lists[li].items[ii].tags = tags;
            self.rebuild_sidebar_entries();
            self.save_list_at(li);
        }
    }

    pub fn move_todo_up(&mut self) {
        if self.is_tag_view() {
            return;
        }
        let visible = self.visible_items();
        let vi = self.selected_item_index;
        if vi > 0 {
            let real_idx = visible[vi].0;
            let prev_real_idx = visible[vi - 1].0;
            self.push_undo();
            if let Some(list) = self.current_list_mut() {
                list.items.swap(real_idx, prev_real_idx);
            }
            self.selected_item_index -= 1;
            self.save_current_list();
        }
    }

    pub fn move_todo_down(&mut self) {
        if self.is_tag_view() {
            return;
        }
        let visible = self.visible_items();
        let vi = self.selected_item_index;
        if vi + 1 < visible.len() {
            let real_idx = visible[vi].0;
            let next_real_idx = visible[vi + 1].0;
            self.push_undo();
            if let Some(list) = self.current_list_mut() {
                list.items.swap(real_idx, next_real_idx);
            }
            self.selected_item_index += 1;
            self.save_current_list();
        }
    }

    pub fn move_todo_to_top(&mut self) {
        if self.is_tag_view() {
            return;
        }
        if let Some(real_idx) = self.selected_real_index()
            && real_idx > 0
        {
            self.push_undo();
            if let Some(list) = self.current_list_mut() {
                let item = list.items.remove(real_idx);
                list.items.insert(0, item);
            }
            let visible = self.visible_items();
            if let Some(vi) = visible.iter().position(|(ri, _)| *ri == 0) {
                self.selected_item_index = vi;
            }
            self.save_current_list();
        }
    }

    pub fn move_todo_to_bottom(&mut self) {
        if self.is_tag_view() {
            return;
        }
        if let Some(real_idx) = self.selected_real_index() {
            let len = self.current_list().map_or(0, |l| l.items.len());
            if real_idx + 1 < len {
                self.push_undo();
                if let Some(list) = self.current_list_mut() {
                    let item = list.items.remove(real_idx);
                    list.items.push(item);
                }
                let new_real_idx = self.current_list().map_or(0, |l| l.items.len()) - 1;
                let visible = self.visible_items();
                if let Some(vi) = visible.iter().position(|(ri, _)| *ri == new_real_idx) {
                    self.selected_item_index = vi;
                }
                self.save_current_list();
            }
        }
    }

    pub fn toggle_show_done(&mut self) {
        self.show_done = !self.show_done;
        self.selected_items.clear();
        self.clamp_selection();
    }

    pub fn start_focus(&mut self) {
        if let Some((li, ii)) = self.resolve_selected_item() {
            self.focus_list = li;
            self.focus_item = ii;
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
        if let Some((li, ii)) = self.resolve_selected_item() {
            self.push_undo();
            self.lists[li].items[ii].time_secs = secs;
            self.save_list_at(li);
        }
    }

    pub fn start_filter(&mut self) {
        if self.is_tag_view() {
            return;
        }
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

        let selected: Vec<bool> = tags.iter().map(|t| self.filter_tags.contains(t)).collect();

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
        self.selected_items.clear();
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

    pub fn rebuild_sidebar_entries(&mut self) {
        self.sidebar_entries.clear();
        for i in 0..self.lists.len() {
            self.sidebar_entries.push(SidebarEntry::List(i));
        }
        let tags = self.collect_all_tags();
        for tag in tags {
            self.sidebar_entries.push(SidebarEntry::Tag(tag));
        }
        if self.selected_sidebar_index >= self.sidebar_entries.len()
            && !self.sidebar_entries.is_empty()
        {
            self.selected_sidebar_index = self.sidebar_entries.len() - 1;
        }
    }

    pub fn selected_sidebar_entry(&self) -> Option<&SidebarEntry> {
        self.sidebar_entries.get(self.selected_sidebar_index)
    }

    pub fn is_tag_view(&self) -> bool {
        matches!(self.selected_sidebar_entry(), Some(SidebarEntry::Tag(_)))
    }

    pub fn selected_tag_name(&self) -> Option<&str> {
        match self.selected_sidebar_entry() {
            Some(SidebarEntry::Tag(name)) => Some(name),
            _ => None,
        }
    }

    pub fn tag_visible_items(&self) -> Vec<(usize, usize)> {
        let tag = match self.selected_tag_name() {
            Some(t) => t,
            None => return Vec::new(),
        };
        let mut undone: Vec<(usize, usize)> = Vec::new();
        let mut done: Vec<(usize, usize)> = Vec::new();
        for (li, list) in self.lists.iter().enumerate() {
            for (ii, item) in list.items.iter().enumerate() {
                if !item.tags.iter().any(|t| t == tag) {
                    continue;
                }
                if item.done {
                    if self.show_done {
                        done.push((li, ii));
                    }
                } else {
                    undone.push((li, ii));
                }
            }
        }
        undone.extend(done);
        undone
    }

    pub fn resolve_selected_item(&self) -> Option<(usize, usize)> {
        if self.is_tag_view() {
            self.tag_visible_items()
                .get(self.selected_item_index)
                .copied()
        } else {
            self.selected_real_index()
                .map(|ri| (self.selected_list_index, ri))
        }
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
        self.rebuild_sidebar_entries();
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
            let _ = storage::delete_list_file(&self.context_dir(), &old_name);
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
        let _ = storage::delete_list_file(&self.context_dir(), &list_name);
        self.lists.remove(self.selected_list_index);
        self.rebuild_sidebar_entries();
        self.clamp_selection();
        self.save_order();
    }

    pub fn move_list_up(&mut self) {
        if self.selected_list_index > 0 {
            self.push_undo();
            self.lists
                .swap(self.selected_list_index, self.selected_list_index - 1);
            self.selected_list_index -= 1;
            self.selected_sidebar_index = self.selected_list_index;
            self.rebuild_sidebar_entries();
            self.save_order();
        }
    }

    pub fn move_list_down(&mut self) {
        if self.selected_list_index + 1 < self.lists.len() {
            self.push_undo();
            self.lists
                .swap(self.selected_list_index, self.selected_list_index + 1);
            self.selected_list_index += 1;
            self.selected_sidebar_index = self.selected_list_index;
            self.rebuild_sidebar_entries();
            self.save_order();
        }
    }

    pub fn move_list_to_top(&mut self) {
        if self.selected_list_index > 0 {
            self.push_undo();
            let list = self.lists.remove(self.selected_list_index);
            self.lists.insert(0, list);
            self.selected_list_index = 0;
            self.selected_sidebar_index = 0;
            self.rebuild_sidebar_entries();
            self.save_order();
        }
    }

    pub fn move_list_to_bottom(&mut self) {
        if self.selected_list_index + 1 < self.lists.len() {
            self.push_undo();
            let list = self.lists.remove(self.selected_list_index);
            self.lists.push(list);
            self.selected_list_index = self.lists.len() - 1;
            self.selected_sidebar_index = self.selected_list_index;
            self.rebuild_sidebar_entries();
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
                let secs = storage::parse_time_str(&buffer).unwrap_or(0);
                self.set_item_time(secs);
            }
            InputMode::CreatingContext => {
                self.create_context(buffer);
                return;
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

    pub fn input_chars_before_cursor(&self) -> usize {
        self.input_buffer[..self.input_cursor].chars().count()
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

        let mut list_matches = Vec::new();
        let mut tag_result_matches = Vec::new();
        let mut tag_item_matches = Vec::new();
        let mut title_matches = Vec::new();

        for (li, list) in self.lists.iter().enumerate() {
            if list.name.to_lowercase().contains(&query) {
                list_matches.push(SearchResult::List(li));
            }
            for (ii, item) in list.items.iter().enumerate() {
                let title_match = item.title.to_lowercase().contains(&query);
                let tag_match = item.tags.iter().any(|t| t.to_lowercase().contains(&query));
                if tag_match && !title_match {
                    tag_item_matches.push(SearchResult::Item(li, ii));
                } else if title_match {
                    title_matches.push(SearchResult::Item(li, ii));
                }
            }
        }

        let all_tags = self.collect_all_tags();
        for tag in all_tags {
            if tag.to_lowercase().contains(&query) {
                tag_result_matches.push(SearchResult::Tag(tag));
            }
        }

        self.search_results.extend(list_matches);
        self.search_results.extend(tag_result_matches);
        self.search_results.extend(tag_item_matches);
        self.search_results.extend(title_matches);

        if self.search_results.is_empty() {
            self.search_selected = 0;
        } else if self.search_selected >= self.search_results.len() {
            self.search_selected = self.search_results.len() - 1;
        }
    }

    pub fn search_select_next(&mut self) {
        if !self.search_results.is_empty() && self.search_selected + 1 < self.search_results.len() {
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
                    self.selected_sidebar_index = li;
                    self.selected_item_index = 0;
                    self.active_pane = Pane::Main;
                }
                SearchResult::Item(li, ii) => {
                    self.selected_list_index = li;
                    self.selected_sidebar_index = li;
                    self.active_pane = Pane::Main;

                    if self.lists[li].items[ii].done {
                        self.show_done = true;
                    }

                    let visible = self.visible_items();
                    if let Some(vi) = visible.iter().position(|(real_idx, _)| *real_idx == ii) {
                        self.selected_item_index = vi;
                    }
                }
                SearchResult::Tag(ref name) => {
                    if let Some(idx) = self
                        .sidebar_entries
                        .iter()
                        .position(|e| matches!(e, SidebarEntry::Tag(t) if t == name))
                    {
                        self.selected_sidebar_index = idx;
                        self.selected_item_index = 0;
                        self.active_pane = Pane::Main;
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

    pub fn toggle_select_current(&mut self) {
        if self.is_tag_view() {
            return;
        }
        if let Some(real_idx) = self.selected_real_index()
            && !self.selected_items.remove(&real_idx)
        {
            self.selected_items.insert(real_idx);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_items.clear();
    }

    pub fn delete_selected(&mut self) {
        if self.selected_items.is_empty() {
            self.delete_todo();
            return;
        }
        self.push_undo();
        let mut indices: Vec<usize> = self.selected_items.iter().copied().collect();
        indices.sort_unstable_by(|a, b| b.cmp(a));
        if let Some(list) = self.current_list_mut() {
            for idx in indices {
                list.items.remove(idx);
            }
        }
        self.selected_items.clear();
        self.rebuild_sidebar_entries();
        self.clamp_selection();
        self.save_current_list();
    }

    pub fn toggle_done_selected(&mut self) {
        if self.selected_items.is_empty() {
            self.toggle_done();
            return;
        }
        self.push_undo();
        let indices: Vec<usize> = self.selected_items.iter().copied().collect();
        if let Some(list) = self.current_list_mut() {
            for idx in indices {
                list.items[idx].done = !list.items[idx].done;
            }
        }
        self.selected_items.clear();
        self.rebuild_sidebar_entries();
        self.clamp_selection();
        self.save_current_list();
    }

    pub fn done_count(&self) -> usize {
        self.current_list()
            .map(|list| list.items.iter().filter(|item| item.done).count())
            .unwrap_or(0)
    }

    pub fn start_archive(&mut self) {
        if self.done_count() == 0 {
            return;
        }
        self.input_mode = InputMode::ConfirmArchive;
    }

    pub fn archive_done_items(&mut self) {
        let Some(list) = self.current_list() else {
            return;
        };
        let done_items: Vec<_> = list
            .items
            .iter()
            .filter(|item| item.done)
            .cloned()
            .collect();
        if done_items.is_empty() {
            return;
        }
        let list_name = list.name.clone();
        self.push_undo();
        let _ = storage::append_to_archive(&self.context_dir(), &list_name, &done_items);
        if let Some(list) = self.current_list_mut() {
            list.items.retain(|item| !item.done);
        }
        self.selected_items.clear();
        self.rebuild_sidebar_entries();
        self.clamp_selection();
        self.save_current_list();
        self.input_mode = InputMode::Normal;
    }

    pub fn move_to_list_targets(&self) -> Vec<(usize, &str)> {
        let query = self.move_to_list_filter.to_lowercase();
        self.lists
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.selected_list_index)
            .filter(|(_, list)| query.is_empty() || list.name.to_lowercase().contains(&query))
            .map(|(i, list)| (i, list.name.as_str()))
            .collect()
    }

    pub fn start_move_to_list(&mut self) {
        if self.is_tag_view() {
            return;
        }
        if self.lists.len() < 2 {
            return;
        }
        let has_items = if self.selected_items.is_empty() {
            self.selected_real_index().is_some()
        } else {
            true
        };
        if !has_items {
            return;
        }
        self.move_to_list_cursor = 0;
        self.move_to_list_filter.clear();
        self.input_mode = InputMode::MovingToList;
    }

    pub fn move_to_list_move_up(&mut self) {
        if self.move_to_list_cursor > 0 {
            self.move_to_list_cursor -= 1;
        }
    }

    pub fn move_to_list_move_down(&mut self) {
        let targets = self.move_to_list_targets();
        if !targets.is_empty() && self.move_to_list_cursor + 1 < targets.len() {
            self.move_to_list_cursor += 1;
        }
    }

    pub fn confirm_move_to_list(&mut self) {
        let targets = self.move_to_list_targets();
        if self.move_to_list_cursor >= targets.len() {
            return;
        }
        let target_list_idx = targets[self.move_to_list_cursor].0;

        self.push_undo();

        let indices_to_move: Vec<usize> = if self.selected_items.is_empty() {
            self.selected_real_index().into_iter().collect()
        } else {
            let mut v: Vec<usize> = self.selected_items.iter().copied().collect();
            v.sort_unstable();
            v
        };

        let mut items_to_move: Vec<_> = indices_to_move
            .iter()
            .map(|&idx| self.lists[self.selected_list_index].items[idx].clone())
            .collect();

        // Remove from source in reverse order
        for &idx in indices_to_move.iter().rev() {
            self.lists[self.selected_list_index].items.remove(idx);
        }

        // Insert at top of target, preserving order
        items_to_move.reverse();
        for item in items_to_move {
            self.lists[target_list_idx].items.insert(0, item);
        }

        self.selected_items.clear();
        self.move_to_list_filter.clear();
        self.input_mode = InputMode::Normal;
        self.rebuild_sidebar_entries();
        self.clamp_selection();

        self.save_current_list();
        let _ = storage::save_list(&self.context_dir(), &self.lists[target_list_idx]);
    }

    pub fn move_to_list_insert_char(&mut self, c: char) {
        self.move_to_list_filter.push(c);
        self.move_to_list_cursor = 0;
    }

    pub fn move_to_list_delete_char(&mut self) {
        self.move_to_list_filter.pop();
        self.move_to_list_cursor = 0;
    }

    pub fn cancel_move_to_list(&mut self) {
        self.move_to_list_filter.clear();
        self.input_mode = InputMode::Normal;
    }

    fn save_current_list(&self) {
        if let Some(list) = self.current_list() {
            let _ = storage::save_list(&self.context_dir(), list);
        }
    }

    fn save_list_at(&self, index: usize) {
        if let Some(list) = self.lists.get(index) {
            let _ = storage::save_list(&self.context_dir(), list);
        }
    }

    fn save_order(&self) {
        let _ = storage::save_order(&self.context_dir(), &self.lists);
    }

    pub fn toggle_list_type(&mut self) {
        if let Some(list) = self.current_list() {
            let _ = list;
            self.push_undo();
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            if let Some(list) = self.current_list_mut() {
                match list.list_type {
                    ListType::Normal => {
                        list.list_type = ListType::Daily;
                        list.last_reset = Some(today);
                    }
                    ListType::Daily => {
                        list.list_type = ListType::Normal;
                        list.last_reset = None;
                    }
                }
            }
            self.save_current_list();
        }
    }

    pub fn reset_daily_lists(&mut self) {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        for i in 0..self.lists.len() {
            if self.lists[i].list_type != ListType::Daily {
                continue;
            }
            let needs_reset = match &self.lists[i].last_reset {
                None => true,
                Some(date) => date.as_str() < today.as_str(),
            };
            if !needs_reset {
                continue;
            }
            let done_indices: Vec<usize> = self.lists[i]
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.done)
                .map(|(idx, _)| idx)
                .collect();
            if done_indices.is_empty() {
                self.lists[i].last_reset = Some(today.clone());
                let _ = storage::save_list(&self.context_dir(), &self.lists[i]);
                continue;
            }
            let mut reset_items: Vec<TodoItem> = Vec::new();
            for &idx in &done_indices {
                self.lists[i].items[idx].done = false;
                self.lists[i].items[idx].time_secs = 0;
                reset_items.push(self.lists[i].items[idx].clone());
            }
            for &idx in done_indices.iter().rev() {
                self.lists[i].items.remove(idx);
            }
            self.lists[i].items.extend(reset_items);
            self.lists[i].last_reset = Some(today.clone());
            let _ = storage::save_list(&self.data_dir, &self.lists[i]);
        }
        self.rebuild_sidebar_entries();
    }

    pub fn quit(&mut self) {
        let _ = storage::save_all(&self.context_dir(), &self.lists);
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
        assert_eq!(app.selected_sidebar_index, 0);
        assert_eq!(app.selected_list_index, 0);

        app.move_selection_down();
        assert_eq!(app.selected_sidebar_index, 1);
        assert_eq!(app.selected_list_index, 1);
        assert_eq!(app.selected_item_index, 0);

        app.move_selection_down();
        assert_eq!(app.selected_sidebar_index, 2);
        assert!(app.is_tag_view());

        app.move_selection_down();
        assert_eq!(app.selected_sidebar_index, 2);

        app.move_selection_up();
        assert_eq!(app.selected_sidebar_index, 1);
        assert_eq!(app.selected_list_index, 1);
        assert_eq!(app.selected_item_index, 0);

        app.move_selection_up();
        assert_eq!(app.selected_sidebar_index, 0);
        assert_eq!(app.selected_list_index, 0);

        app.move_selection_up();
        assert_eq!(app.selected_sidebar_index, 0);
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
        let added = &app.lists[0].items[0];
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
        // C swapped with A (visible neighbor), not B (real neighbor)
        assert_eq!(app.lists[0].items[0].title, "Task C");
        assert_eq!(app.lists[0].items[1].title, "Task B");
        assert_eq!(app.lists[0].items[2].title, "Task A");
        assert_eq!(app.selected_item_index, 0);

        app.move_todo_down();
        // C swapped back with A
        assert_eq!(app.lists[0].items[0].title, "Task A");
        assert_eq!(app.lists[0].items[1].title, "Task B");
        assert_eq!(app.lists[0].items[2].title, "Task C");
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
        app.selected_sidebar_index = 1;

        app.move_list_up();
        assert_eq!(app.lists[0].name, "Beta");
        assert_eq!(app.lists[1].name, "Alpha");
        assert_eq!(app.selected_list_index, 0);
        assert_eq!(app.selected_sidebar_index, 0);

        app.move_list_up();
        assert_eq!(app.lists[0].name, "Beta");
        assert_eq!(app.selected_list_index, 0);
        assert_eq!(app.selected_sidebar_index, 0);
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
        app.selected_sidebar_index = 1;

        app.move_list_down();
        assert_eq!(app.lists[1].name, "Gamma");
        assert_eq!(app.lists[2].name, "Beta");
        assert_eq!(app.selected_list_index, 2);
        assert_eq!(app.selected_sidebar_index, 2);

        app.move_list_down();
        assert_eq!(app.lists[2].name, "Beta");
        assert_eq!(app.selected_list_index, 2);
        assert_eq!(app.selected_sidebar_index, 2);
    }

    #[test]
    fn test_move_todo_to_top() {
        let mut app = App::with_lists(vec![{
            let mut list = TodoList::new("Work");
            list.items.push(TodoItem {
                title: "A".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
            list.items.push(TodoItem {
                title: "B".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
            list.items.push(TodoItem {
                title: "C".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
            list
        }]);
        app.active_pane = Pane::Main;
        app.selected_item_index = 2;

        app.move_todo_to_top();
        assert_eq!(app.lists[0].items[0].title, "C");
        assert_eq!(app.lists[0].items[1].title, "A");
        assert_eq!(app.lists[0].items[2].title, "B");
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_move_todo_to_top_already_first() {
        let mut app = App::with_lists(vec![{
            let mut list = TodoList::new("Work");
            list.items.push(TodoItem {
                title: "A".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
            list.items.push(TodoItem {
                title: "B".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
            list
        }]);
        app.active_pane = Pane::Main;
        app.selected_item_index = 0;

        app.move_todo_to_top();
        assert_eq!(app.lists[0].items[0].title, "A");
        assert_eq!(app.lists[0].items[1].title, "B");
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_move_todo_to_bottom() {
        let mut app = App::with_lists(vec![{
            let mut list = TodoList::new("Work");
            list.items.push(TodoItem {
                title: "A".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
            list.items.push(TodoItem {
                title: "B".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
            list.items.push(TodoItem {
                title: "C".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
            list
        }]);
        app.active_pane = Pane::Main;
        app.selected_item_index = 0;

        app.move_todo_to_bottom();
        assert_eq!(app.lists[0].items[0].title, "B");
        assert_eq!(app.lists[0].items[1].title, "C");
        assert_eq!(app.lists[0].items[2].title, "A");
        assert_eq!(app.selected_item_index, 2);
    }

    #[test]
    fn test_move_todo_to_bottom_already_last() {
        let mut app = App::with_lists(vec![{
            let mut list = TodoList::new("Work");
            list.items.push(TodoItem {
                title: "A".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
            list.items.push(TodoItem {
                title: "B".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
            list
        }]);
        app.active_pane = Pane::Main;
        app.selected_item_index = 1;

        app.move_todo_to_bottom();
        assert_eq!(app.lists[0].items[0].title, "A");
        assert_eq!(app.lists[0].items[1].title, "B");
        assert_eq!(app.selected_item_index, 1);
    }

    #[test]
    fn test_move_list_to_top() {
        let mut app = App::with_lists(vec![
            TodoList::new("Alpha"),
            TodoList::new("Beta"),
            TodoList::new("Gamma"),
        ]);
        app.active_pane = Pane::Sidebar;
        app.selected_list_index = 2;
        app.selected_sidebar_index = 2;

        app.move_list_to_top();
        assert_eq!(app.lists[0].name, "Gamma");
        assert_eq!(app.lists[1].name, "Alpha");
        assert_eq!(app.lists[2].name, "Beta");
        assert_eq!(app.selected_list_index, 0);
        assert_eq!(app.selected_sidebar_index, 0);
    }

    #[test]
    fn test_move_list_to_bottom() {
        let mut app = App::with_lists(vec![
            TodoList::new("Alpha"),
            TodoList::new("Beta"),
            TodoList::new("Gamma"),
        ]);
        app.active_pane = Pane::Sidebar;
        app.selected_list_index = 0;
        app.selected_sidebar_index = 0;

        app.move_list_to_bottom();
        assert_eq!(app.lists[0].name, "Beta");
        assert_eq!(app.lists[1].name, "Gamma");
        assert_eq!(app.lists[2].name, "Alpha");
        assert_eq!(app.selected_list_index, 2);
        assert_eq!(app.selected_sidebar_index, 2);
    }

    #[test]
    fn test_jump_to_first_main() {
        let mut app = sample_app();
        app.selected_item_index = 2;
        app.jump_to_first();
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_jump_to_last_main() {
        let mut app = sample_app();
        assert_eq!(app.selected_item_index, 0);
        app.jump_to_last();
        assert_eq!(app.selected_item_index, 2);
    }

    #[test]
    fn test_jump_to_first_sidebar() {
        let mut app = App::with_lists(vec![
            TodoList::new("Alpha"),
            TodoList::new("Beta"),
            TodoList::new("Gamma"),
        ]);
        app.active_pane = Pane::Sidebar;
        app.selected_sidebar_index = 2;
        app.selected_list_index = 2;
        app.jump_to_first();
        assert_eq!(app.selected_sidebar_index, 0);
        assert_eq!(app.selected_list_index, 0);
    }

    #[test]
    fn test_jump_to_last_sidebar() {
        let mut app = App::with_lists(vec![
            TodoList::new("Alpha"),
            TodoList::new("Beta"),
            TodoList::new("Gamma"),
        ]);
        app.active_pane = Pane::Sidebar;
        assert_eq!(app.selected_list_index, 0);
        app.jump_to_last();
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

    #[test]
    fn test_input_chars_before_cursor() {
        let mut app = sample_app();
        app.start_input(InputMode::AddingItem, "");
        assert_eq!(app.input_chars_before_cursor(), 0);

        app.input_insert_char('a');
        app.input_insert_char('b');
        app.input_insert_char('c');
        assert_eq!(app.input_chars_before_cursor(), 3);

        app.input_move_cursor_left();
        assert_eq!(app.input_chars_before_cursor(), 2);

        app.input_move_cursor_left();
        app.input_move_cursor_left();
        assert_eq!(app.input_chars_before_cursor(), 0);
    }

    #[test]
    fn test_input_chars_before_cursor_multibyte() {
        let mut app = sample_app();
        app.start_input(InputMode::AddingItem, "");
        for c in "café".chars() {
            app.input_insert_char(c);
        }
        assert_eq!(app.input_chars_before_cursor(), 4);
        assert_eq!(app.input_cursor, "café".len());

        app.input_move_cursor_left();
        assert_eq!(app.input_chars_before_cursor(), 3);
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
        assert_eq!(app.search_results.len(), 2);
        assert_eq!(app.search_results[0], SearchResult::Tag("urgent".into()));
        assert_eq!(app.search_results[1], SearchResult::Item(0, 1));
    }

    #[test]
    fn test_search_results_matches_list_name() {
        let mut app = search_app();
        app.start_search();
        app.input_buffer = "Personal".to_string();
        app.update_search_results();
        assert!(
            app.search_results
                .iter()
                .any(|r| *r == SearchResult::List(1))
        );
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
        assert_eq!(app.selected_item_index, 0);
        assert_eq!(app.active_pane, Pane::Main);
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
    fn test_edit_time_clear() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        app.lists[0].items[0].time_secs = 3600;
        app.input_mode = InputMode::EditingTime;
        app.input_buffer.clear();
        app.confirm_input();
        assert_eq!(app.lists[0].items[0].time_secs, 0);
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

    fn many_items_app(count: usize) -> App {
        let mut list = TodoList::new("Work");
        for i in 0..count {
            list.items.push(TodoItem {
                title: format!("Task {}", i),
                done: false,
                tags: vec![],
                time_secs: 0,
            });
        }
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;
        app
    }

    #[test]
    fn test_page_down_main() {
        let mut app = many_items_app(20);
        assert_eq!(app.selected_item_index, 0);
        app.page_down(10);
        assert_eq!(app.selected_item_index, 10);
        app.page_down(10);
        assert_eq!(app.selected_item_index, 19);
    }

    #[test]
    fn test_page_up_main() {
        let mut app = many_items_app(20);
        app.selected_item_index = 19;
        app.page_up(10);
        assert_eq!(app.selected_item_index, 9);
        app.page_up(10);
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_page_down_clamps_to_last() {
        let mut app = many_items_app(5);
        app.selected_item_index = 3;
        app.page_down(10);
        assert_eq!(app.selected_item_index, 4);
    }

    #[test]
    fn test_page_up_clamps_to_zero() {
        let mut app = many_items_app(5);
        app.selected_item_index = 2;
        app.page_up(10);
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_page_down_sidebar() {
        let mut app = App::with_lists(vec![
            TodoList::new("A"),
            TodoList::new("B"),
            TodoList::new("C"),
            TodoList::new("D"),
            TodoList::new("E"),
        ]);
        app.active_pane = Pane::Sidebar;
        assert_eq!(app.selected_list_index, 0);
        app.page_down(3);
        assert_eq!(app.selected_list_index, 3);
        assert!(app.filter_tags.is_empty());
        app.page_down(3);
        assert_eq!(app.selected_list_index, 4);
    }

    #[test]
    fn test_page_up_sidebar() {
        let mut app = App::with_lists(vec![
            TodoList::new("A"),
            TodoList::new("B"),
            TodoList::new("C"),
            TodoList::new("D"),
            TodoList::new("E"),
        ]);
        app.active_pane = Pane::Sidebar;
        app.selected_sidebar_index = 4;
        app.selected_list_index = 4;
        app.page_up(3);
        assert_eq!(app.selected_sidebar_index, 1);
        assert_eq!(app.selected_list_index, 1);
        app.page_up(3);
        assert_eq!(app.selected_sidebar_index, 0);
        assert_eq!(app.selected_list_index, 0);
    }

    #[test]
    fn test_page_down_sidebar_clears_filter() {
        let mut app = App::with_lists(vec![
            TodoList::new("A"),
            TodoList::new("B"),
            TodoList::new("C"),
        ]);
        app.active_pane = Pane::Sidebar;
        app.filter_tags = vec!["tag".to_string()];
        app.page_down(1);
        assert!(app.filter_tags.is_empty());
    }

    #[test]
    fn test_page_up_sidebar_clears_filter() {
        let mut app = App::with_lists(vec![
            TodoList::new("A"),
            TodoList::new("B"),
            TodoList::new("C"),
        ]);
        app.active_pane = Pane::Sidebar;
        app.selected_sidebar_index = 2;
        app.selected_list_index = 2;
        app.filter_tags = vec!["tag".to_string()];
        app.page_up(1);
        assert!(app.filter_tags.is_empty());
    }

    #[test]
    fn test_page_down_empty_list() {
        let mut app = App::with_lists(vec![TodoList::new("Empty")]);
        app.active_pane = Pane::Main;
        app.page_down(10);
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_toggle_select_current() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        app.toggle_select_current();
        assert!(app.selected_items.contains(&0)); // real index of Task A
        app.toggle_select_current();
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_delete_selected_multiple() {
        let mut app = sample_app();
        // Visible order: Task A (real 0), Task C (real 2), Task B (real 1, done)
        // Select real indices 0 and 2
        app.selected_items.insert(0);
        app.selected_items.insert(2);
        app.delete_selected();
        assert_eq!(app.lists[0].items.len(), 1);
        assert_eq!(app.lists[0].items[0].title, "Task B");
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_delete_selected_falls_back() {
        let mut app = sample_app();
        assert!(app.selected_items.is_empty());
        app.delete_selected();
        assert_eq!(app.lists[0].items.len(), 2);
    }

    #[test]
    fn test_toggle_done_selected_multiple() {
        let mut app = sample_app();
        // Task A (real 0) not done, Task C (real 2) not done
        app.selected_items.insert(0);
        app.selected_items.insert(2);
        app.toggle_done_selected();
        assert!(app.lists[0].items[0].done);
        assert!(app.lists[0].items[2].done);
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_selection_cleared_on_pane_switch() {
        let mut app = sample_app();
        app.selected_items.insert(0);
        app.toggle_pane();
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_selection_cleared_on_list_change() {
        let mut app = sample_app();
        app.active_pane = Pane::Sidebar;
        app.selected_items.insert(0);
        app.move_selection_down();
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_selection_cleared_on_undo() {
        let mut app = sample_app();
        app.delete_todo();
        app.selected_items.insert(0);
        app.undo();
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_start_move_to_list() {
        let mut app = sample_app();
        app.start_move_to_list();
        assert_eq!(app.input_mode, InputMode::MovingToList);
        assert_eq!(app.move_to_list_cursor, 0);
    }

    #[test]
    fn test_start_move_to_list_single_list() {
        let mut list = TodoList::new("Only");
        list.items.push(TodoItem::new("Item"));
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;
        app.start_move_to_list();
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_move_to_list_targets() {
        let mut app = sample_app();
        let targets = app.move_to_list_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], (1, "Personal"));

        app.selected_list_index = 1;
        let targets = app.move_to_list_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], (0, "Work"));
    }

    #[test]
    fn test_confirm_move_single_item() {
        let mut app = sample_app();
        app.selected_item_index = 0; // Task A
        app.start_move_to_list();
        app.confirm_move_to_list();
        assert_eq!(app.lists[0].items.len(), 2);
        assert_eq!(app.lists[1].items.len(), 1);
        assert_eq!(app.lists[1].items[0].title, "Task A");
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_confirm_move_multi_selected() {
        let mut app = sample_app();
        app.selected_items.insert(0); // Task A
        app.selected_items.insert(2); // Task C
        app.start_move_to_list();
        app.confirm_move_to_list();
        assert_eq!(app.lists[0].items.len(), 1);
        assert_eq!(app.lists[0].items[0].title, "Task B");
        assert_eq!(app.lists[1].items.len(), 2);
        assert_eq!(app.lists[1].items[0].title, "Task A");
        assert_eq!(app.lists[1].items[1].title, "Task C");
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_move_to_list_undoable() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        app.start_move_to_list();
        app.confirm_move_to_list();
        assert_eq!(app.lists[0].items.len(), 2);
        assert_eq!(app.lists[1].items.len(), 1);
        app.undo();
        assert_eq!(app.lists[0].items.len(), 3);
        assert_eq!(app.lists[1].items.len(), 0);
    }

    #[test]
    fn test_cancel_move_to_list() {
        let mut app = sample_app();
        app.start_move_to_list();
        assert_eq!(app.input_mode, InputMode::MovingToList);
        app.cancel_move_to_list();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.lists[0].items.len(), 3);
    }

    #[test]
    fn test_move_to_list_navigation() {
        let mut app = App::with_lists(vec![
            {
                let mut l = TodoList::new("A");
                l.items.push(TodoItem::new("Item"));
                l
            },
            TodoList::new("B"),
            TodoList::new("C"),
        ]);
        app.active_pane = Pane::Main;
        app.start_move_to_list();
        assert_eq!(app.move_to_list_cursor, 0);
        app.move_to_list_move_down();
        assert_eq!(app.move_to_list_cursor, 1);
        app.move_to_list_move_down();
        assert_eq!(app.move_to_list_cursor, 1); // can't go past end
        app.move_to_list_move_up();
        assert_eq!(app.move_to_list_cursor, 0);
        app.move_to_list_move_up();
        assert_eq!(app.move_to_list_cursor, 0); // can't go past start
    }

    #[test]
    fn test_move_to_list_filter() {
        let mut app = App::with_lists(vec![
            {
                let mut l = TodoList::new("Work");
                l.items.push(TodoItem::new("Item"));
                l
            },
            TodoList::new("Personal"),
            TodoList::new("Projects"),
        ]);
        app.active_pane = Pane::Main;
        app.start_move_to_list();
        assert_eq!(app.move_to_list_targets().len(), 2);

        app.move_to_list_insert_char('p');
        let targets = app.move_to_list_targets();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].1, "Personal");
        assert_eq!(targets[1].1, "Projects");

        app.move_to_list_insert_char('r');
        let targets = app.move_to_list_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].1, "Projects");
        assert_eq!(app.move_to_list_cursor, 0);

        app.move_to_list_delete_char();
        assert_eq!(app.move_to_list_targets().len(), 2);

        app.move_to_list_insert_char('e');
        app.move_to_list_insert_char('r');
        let targets = app.move_to_list_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].1, "Personal");
    }

    #[test]
    fn test_move_to_list_filter_cleared_on_start() {
        let mut app = sample_app();
        app.move_to_list_filter = "stale".to_string();
        app.start_move_to_list();
        assert!(app.move_to_list_filter.is_empty());
    }

    #[test]
    fn test_toggle_list_type() {
        let mut app = App::with_lists(vec![TodoList::new("Habits")]);
        app.active_pane = Pane::Sidebar;
        assert_eq!(app.lists[0].list_type, ListType::Normal);

        app.toggle_list_type();
        assert_eq!(app.lists[0].list_type, ListType::Daily);
        assert!(app.lists[0].last_reset.is_some());

        app.toggle_list_type();
        assert_eq!(app.lists[0].list_type, ListType::Normal);
        assert_eq!(app.lists[0].last_reset, None);
    }

    #[test]
    fn test_reset_daily_lists() {
        let mut list = TodoList::new("Daily");
        list.list_type = ListType::Daily;
        list.last_reset = Some("2020-01-01".to_string());
        list.items.push(TodoItem::new("A"));
        list.items[0].time_secs = 300;
        list.items.push(TodoItem::new("B"));
        list.items[1].done = true;
        list.items[1].time_secs = 600;
        list.items.push(TodoItem::new("C"));
        list.items[2].done = true;
        list.items[2].time_secs = 900;

        let mut app = App::with_lists(vec![list]);
        app.reset_daily_lists();

        assert!(!app.lists[0].items[0].done);
        assert!(!app.lists[0].items[1].done);
        assert!(!app.lists[0].items[2].done);
        // A stays at top, B and C moved to bottom
        assert_eq!(app.lists[0].items[0].title, "A");
        assert_eq!(app.lists[0].items[1].title, "B");
        assert_eq!(app.lists[0].items[2].title, "C");
        // A (not done) keeps its timer, B and C (were done) get cleared
        assert_eq!(app.lists[0].items[0].time_secs, 300);
        assert_eq!(app.lists[0].items[1].time_secs, 0);
        assert_eq!(app.lists[0].items[2].time_secs, 0);
    }

    #[test]
    fn test_reset_daily_lists_same_day_no_op() {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut list = TodoList::new("Daily");
        list.list_type = ListType::Daily;
        list.last_reset = Some(today);
        list.items.push(TodoItem::new("A"));
        list.items[0].done = true;

        let mut app = App::with_lists(vec![list]);
        app.reset_daily_lists();

        assert!(app.lists[0].items[0].done);
    }

    #[test]
    fn test_reset_daily_lists_skips_normal() {
        let mut list = TodoList::new("Normal");
        list.last_reset = Some("2020-01-01".to_string());
        list.items.push(TodoItem::new("A"));
        list.items[0].done = true;

        let mut app = App::with_lists(vec![list]);
        app.reset_daily_lists();

        assert!(app.lists[0].items[0].done);
    }

    #[test]
    fn test_reset_daily_preserves_order() {
        let mut list = TodoList::new("Daily");
        list.list_type = ListType::Daily;
        list.last_reset = Some("2020-01-01".to_string());
        // Items: A(not done), B(done), C(not done), D(done), E(not done)
        list.items.push(TodoItem::new("A"));
        list.items.push(TodoItem::new("B"));
        list.items[1].done = true;
        list.items.push(TodoItem::new("C"));
        list.items.push(TodoItem::new("D"));
        list.items[3].done = true;
        list.items.push(TodoItem::new("E"));

        let mut app = App::with_lists(vec![list]);
        app.reset_daily_lists();

        // Not-done items stay in place, done items (B, D) move to end in order
        assert_eq!(app.lists[0].items[0].title, "A");
        assert_eq!(app.lists[0].items[1].title, "C");
        assert_eq!(app.lists[0].items[2].title, "E");
        assert_eq!(app.lists[0].items[3].title, "B");
        assert_eq!(app.lists[0].items[4].title, "D");
        // All should be not done
        for item in &app.lists[0].items {
            assert!(!item.done);
        }
    }

    #[test]
    fn test_move_todo_down_repeated_with_done_items() {
        let mut list = TodoList::new("Work");
        list.items.push(TodoItem {
            title: "A".to_string(),
            done: false,
            tags: vec![],
            time_secs: 0,
        });
        list.items.push(TodoItem {
            title: "B".to_string(),
            done: true,
            tags: vec![],
            time_secs: 0,
        });
        list.items.push(TodoItem {
            title: "C".to_string(),
            done: false,
            tags: vec![],
            time_secs: 0,
        });
        list.items.push(TodoItem {
            title: "D".to_string(),
            done: false,
            tags: vec![],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;
        // Visible order: A(real 0), C(real 2), D(real 3), B(real 1)
        // Cursor at visible[0] = A
        assert_eq!(app.selected_item_index, 0);

        // Move A down: should swap A with C (next visible), cursor follows A
        app.move_todo_down();
        let visible = app.visible_items();
        assert_eq!(visible[app.selected_item_index].1.title, "A");

        // Move A down again: should swap A with D (next visible), cursor follows A
        app.move_todo_down();
        let visible = app.visible_items();
        assert_eq!(visible[app.selected_item_index].1.title, "A");
    }

    #[test]
    fn test_move_todo_up_repeated_with_done_items() {
        let mut list = TodoList::new("Work");
        list.items.push(TodoItem {
            title: "A".to_string(),
            done: false,
            tags: vec![],
            time_secs: 0,
        });
        list.items.push(TodoItem {
            title: "B".to_string(),
            done: true,
            tags: vec![],
            time_secs: 0,
        });
        list.items.push(TodoItem {
            title: "C".to_string(),
            done: false,
            tags: vec![],
            time_secs: 0,
        });
        list.items.push(TodoItem {
            title: "D".to_string(),
            done: false,
            tags: vec![],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;
        // Visible order: A(real 0), C(real 2), D(real 3), B(real 1)
        // Start cursor at visible[2] = D
        app.selected_item_index = 2;

        // Move D up: should swap D with C (prev visible), cursor follows D
        app.move_todo_up();
        let visible = app.visible_items();
        assert_eq!(visible[app.selected_item_index].1.title, "D");

        // Move D up again: should swap D with A (prev visible), cursor follows D
        app.move_todo_up();
        let visible = app.visible_items();
        assert_eq!(visible[app.selected_item_index].1.title, "D");
    }

    #[test]
    fn test_rebuild_sidebar_entries_no_tags() {
        let app = App::with_lists(vec![TodoList::new("Work"), TodoList::new("Personal")]);
        assert_eq!(app.sidebar_entries.len(), 2);
        assert_eq!(app.sidebar_entries[0], SidebarEntry::List(0));
        assert_eq!(app.sidebar_entries[1], SidebarEntry::List(1));
    }

    #[test]
    fn test_rebuild_sidebar_entries_with_tags() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["urgent".into(), "code".into()],
            time_secs: 0,
        });
        let mut personal = TodoList::new("Personal");
        personal.items.push(TodoItem {
            title: "B".into(),
            done: false,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        let app = App::with_lists(vec![work, personal]);
        assert_eq!(app.sidebar_entries.len(), 4);
        assert_eq!(app.sidebar_entries[0], SidebarEntry::List(0));
        assert_eq!(app.sidebar_entries[1], SidebarEntry::List(1));
        assert_eq!(app.sidebar_entries[2], SidebarEntry::Tag("code".into()));
        assert_eq!(app.sidebar_entries[3], SidebarEntry::Tag("urgent".into()));
    }

    #[test]
    fn test_is_tag_view() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        assert!(!app.is_tag_view());
        app.selected_sidebar_index = 1;
        assert!(app.is_tag_view());
    }

    #[test]
    fn test_tag_visible_items() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        work.items.push(TodoItem {
            title: "B".into(),
            done: false,
            tags: vec![],
            time_secs: 0,
        });
        let mut personal = TodoList::new("Personal");
        personal.items.push(TodoItem {
            title: "C".into(),
            done: true,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work, personal]);
        app.selected_sidebar_index = 2; // Tag("urgent")
        let items = app.tag_visible_items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], (0, 0)); // Work/A (not done)
        assert_eq!(items[1], (1, 0)); // Personal/C (done)
    }

    #[test]
    fn test_tag_visible_items_respects_show_done() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        let mut personal = TodoList::new("Personal");
        personal.items.push(TodoItem {
            title: "C".into(),
            done: true,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work, personal]);
        app.selected_sidebar_index = 2;
        app.show_done = false;
        let items = app.tag_visible_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], (0, 0));
    }

    #[test]
    fn test_resolve_selected_item_list_mode() {
        let mut app = sample_app();
        app.active_pane = Pane::Main;
        app.selected_item_index = 0;
        let loc = app.resolve_selected_item();
        assert_eq!(loc, Some((0, 0)));
    }

    #[test]
    fn test_resolve_selected_item_tag_mode() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut personal = TodoList::new("Personal");
        personal.items.push(TodoItem {
            title: "B".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work, personal]);
        app.selected_sidebar_index = 2; // Tag("code")
        app.selected_item_index = 1; // second item in tag view
        let loc = app.resolve_selected_item();
        assert_eq!(loc, Some((1, 0))); // Personal, item 0
    }

    #[test]
    fn test_sidebar_navigation_through_tags() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work, TodoList::new("Personal")]);
        app.active_pane = Pane::Sidebar;
        // sidebar_entries: [List(0), List(1), Tag("code")]
        assert_eq!(app.selected_sidebar_index, 0);

        app.move_selection_down();
        assert_eq!(app.selected_sidebar_index, 1);
        assert_eq!(app.selected_list_index, 1);
        assert!(!app.is_tag_view());

        app.move_selection_down();
        assert_eq!(app.selected_sidebar_index, 2);
        assert!(app.is_tag_view());

        app.move_selection_down(); // at end, shouldn't move
        assert_eq!(app.selected_sidebar_index, 2);

        app.move_selection_up();
        assert_eq!(app.selected_sidebar_index, 1);
        assert!(!app.is_tag_view());
        assert_eq!(app.selected_list_index, 1);
    }

    #[test]
    fn test_sidebar_jump_to_last_includes_tags() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.active_pane = Pane::Sidebar;
        app.jump_to_last();
        assert_eq!(app.selected_sidebar_index, 1); // Tag("code")
        assert!(app.is_tag_view());
    }

    #[test]
    fn test_sidebar_jump_to_first_from_tag() {
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
        app.jump_to_first();
        assert_eq!(app.selected_sidebar_index, 0);
        assert!(!app.is_tag_view());
    }

    #[test]
    fn test_toggle_done_in_tag_view() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        let mut personal = TodoList::new("Personal");
        personal.items.push(TodoItem {
            title: "B".into(),
            done: false,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work, personal]);
        app.active_pane = Pane::Main;
        app.selected_sidebar_index = 2; // Tag("urgent")
        app.selected_item_index = 1; // Personal/B
        app.toggle_done();
        assert!(app.lists[1].items[0].done);
    }

    #[test]
    fn test_delete_in_tag_view() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        let mut personal = TodoList::new("Personal");
        personal.items.push(TodoItem {
            title: "B".into(),
            done: false,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work, personal]);
        app.active_pane = Pane::Main;
        app.selected_sidebar_index = 2;
        app.selected_item_index = 0; // Work/A
        app.delete_todo();
        assert_eq!(app.lists[0].items.len(), 0);
        assert_eq!(app.lists[1].items.len(), 1);
    }

    #[test]
    fn test_add_todo_disabled_in_tag_view() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.selected_sidebar_index = 1; // Tag("code")
        app.add_todo("New item".into());
        assert_eq!(app.lists[0].items.len(), 1); // unchanged
    }

    #[test]
    fn test_move_todo_disabled_in_tag_view() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        work.items.push(TodoItem {
            title: "B".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.active_pane = Pane::Main;
        app.selected_sidebar_index = 1;
        app.selected_item_index = 0;
        app.move_todo_down();
        assert_eq!(app.lists[0].items[0].title, "A");
        assert_eq!(app.lists[0].items[1].title, "B");
    }

    #[test]
    fn test_search_finds_tags() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["urgent".into(), "code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.start_search();
        app.input_buffer = "urg".into();
        app.input_cursor = 3;
        app.update_search_results();
        assert!(app.search_results.len() >= 2);
        assert_eq!(app.search_results[0], SearchResult::Tag("urgent".into()));
    }

    #[test]
    fn test_search_tag_result_no_duplicates() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        work.items.push(TodoItem {
            title: "B".into(),
            done: false,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.start_search();
        app.input_buffer = "urgent".into();
        app.input_cursor = 6;
        app.update_search_results();
        let tag_results: Vec<_> = app
            .search_results
            .iter()
            .filter(|r| matches!(r, SearchResult::Tag(_)))
            .collect();
        assert_eq!(tag_results.len(), 1);
    }

    #[test]
    fn test_select_search_result_tag() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["code".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.start_search();
        app.input_buffer = "code".into();
        app.input_cursor = 4;
        app.update_search_results();
        app.select_search_result();
        assert!(app.is_tag_view());
        assert_eq!(app.selected_tag_name(), Some("code"));
        assert_eq!(app.active_pane, Pane::Main);
    }

    #[test]
    fn test_start_switch_context() {
        let mut app = sample_app();
        app.start_switch_context();
        assert_eq!(app.input_mode, InputMode::SwitchingContext);
        assert_eq!(app.context_cursor, 0);
        assert!(app.context_filter.is_empty());
    }

    #[test]
    fn test_cancel_context_switch() {
        let mut app = sample_app();
        app.start_switch_context();
        app.context_filter = "test".into();
        app.cancel_context_switch();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.context_filter.is_empty());
    }

    #[test]
    fn test_context_targets_excludes_current() {
        let mut app = sample_app();
        app.current_context = "work".into();
        app.data_dir = PathBuf::from("/tmp/todui-ctx-test");
        let targets = app.context_targets();
        assert!(!targets.contains(&"work".to_string()));
    }

    #[test]
    fn test_context_filter_insert_delete() {
        let mut app = sample_app();
        app.start_switch_context();
        app.context_insert_char('w');
        assert_eq!(app.context_filter, "w");
        app.context_insert_char('o');
        assert_eq!(app.context_filter, "wo");
        app.context_delete_char();
        assert_eq!(app.context_filter, "w");
    }

    #[test]
    fn test_context_move_up_down() {
        let mut app = sample_app();
        app.start_switch_context();
        assert_eq!(app.context_cursor, 0);
        // With no real contexts, only "new context" option exists at index 0
        // Can't move past it
        app.context_move_down();
        assert_eq!(app.context_cursor, 0);
        app.context_move_up();
        assert_eq!(app.context_cursor, 0);
    }

    #[test]
    fn test_confirm_context_switch_new_context() {
        let mut app = sample_app();
        app.start_switch_context();
        // With no available contexts, cursor 0 = "new context" option
        let targets = app.context_targets();
        app.context_cursor = targets.len(); // "new context" is last
        app.confirm_context_switch();
        assert_eq!(app.input_mode, InputMode::CreatingContext);
    }

    #[test]
    fn test_start_archive_sets_confirm_mode() {
        let mut list = TodoList::new("Work");
        list.items.push(TodoItem::new("Task A"));
        let mut done_item = TodoItem::new("Task B");
        done_item.done = true;
        list.items.push(done_item);
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;

        app.start_archive();

        assert_eq!(app.input_mode, InputMode::ConfirmArchive);
    }

    #[test]
    fn test_start_archive_noop_when_no_done_items() {
        let mut list = TodoList::new("Work");
        list.items.push(TodoItem::new("Task A"));
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;

        app.start_archive();

        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_archive_done_items_removes_done() {
        let mut list = TodoList::new("Work");
        list.items.push(TodoItem::new("Keep this"));
        let mut done1 = TodoItem::new("Done 1");
        done1.done = true;
        list.items.push(done1);
        let mut done2 = TodoItem::new("Done 2");
        done2.done = true;
        list.items.push(done2);
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;
        app.selected_list_index = 0;

        app.archive_done_items();

        assert_eq!(app.lists[0].items.len(), 1);
        assert_eq!(app.lists[0].items[0].title, "Keep this");
    }

    #[test]
    fn test_archive_done_items_supports_undo() {
        let mut list = TodoList::new("Work");
        list.items.push(TodoItem::new("Keep"));
        let mut done = TodoItem::new("Archive me");
        done.done = true;
        list.items.push(done);
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;
        app.selected_list_index = 0;

        app.archive_done_items();
        assert_eq!(app.lists[0].items.len(), 1);

        app.undo();
        assert_eq!(app.lists[0].items.len(), 2);
    }

    #[test]
    fn test_archive_count_done() {
        let mut list = TodoList::new("Work");
        list.items.push(TodoItem::new("Not done"));
        let mut d1 = TodoItem::new("Done 1");
        d1.done = true;
        list.items.push(d1);
        let mut d2 = TodoItem::new("Done 2");
        d2.done = true;
        list.items.push(d2);
        let app = App::with_lists(vec![list]);

        assert_eq!(app.done_count(), 2);
    }

    #[test]
    fn test_toggle_tag_adds_tag() {
        let mut app = App::with_lists(vec![{
            let mut list = TodoList::new("Work");
            list.items.push(TodoItem::new("Task A"));
            list
        }]);
        app.active_pane = Pane::Main;
        app.toggle_tag("focus");
        assert_eq!(app.lists[0].items[0].tags, vec!["focus"]);
    }

    #[test]
    fn test_toggle_tag_removes_tag() {
        let mut app = App::with_lists(vec![{
            let mut list = TodoList::new("Work");
            list.items.push(TodoItem {
                title: "Task A".into(),
                done: false,
                tags: vec!["focus".into()],
                time_secs: 0,
            });
            list
        }]);
        app.active_pane = Pane::Main;
        app.toggle_tag("focus");
        assert!(app.lists[0].items[0].tags.is_empty());
    }

    #[test]
    fn test_toggle_tag_preserves_other_tags() {
        let mut app = App::with_lists(vec![{
            let mut list = TodoList::new("Work");
            list.items.push(TodoItem {
                title: "Task A".into(),
                done: false,
                tags: vec!["code".into()],
                time_secs: 0,
            });
            list
        }]);
        app.active_pane = Pane::Main;
        app.toggle_tag("focus");
        assert_eq!(app.lists[0].items[0].tags, vec!["code", "focus"]);
    }

    #[test]
    fn test_toggle_tag_supports_undo() {
        let mut app = App::with_lists(vec![{
            let mut list = TodoList::new("Work");
            list.items.push(TodoItem::new("Task A"));
            list
        }]);
        app.active_pane = Pane::Main;
        app.toggle_tag("focus");
        assert_eq!(app.lists[0].items[0].tags, vec!["focus"]);
        app.undo();
        assert!(app.lists[0].items[0].tags.is_empty());
    }
}
