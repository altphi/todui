use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use automerge::AutoCommit;

use crate::config::KeyConfig;
use crate::crdt::{CrdtDocument, CrdtItem, CrdtList};
#[cfg(test)]
use crate::model::TodoList;
use crate::model::{AppSnapshot, InputMode, Pane, SearchResult, SidebarEntry};
use crate::storage;

pub struct App {
    auto_doc: AutoCommit,
    pub doc: CrdtDocument,
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
    pub focus_item_id: Option<String>,
    pub focus_start: Option<Instant>,
    pub focus_accumulated: u64,
    pub filter_tags: Vec<String>,
    pub filter_available_tags: Vec<String>,
    pub filter_selected: Vec<bool>,
    pub filter_cursor: usize,
    pub autocomplete_suggestions: Vec<String>,
    pub autocomplete_cursor: usize,
    pub autocomplete_active: bool,
    pub selected_items: HashSet<String>,
    pub move_to_list_cursor: usize,
    pub move_to_list_filter: String,
    pub sidebar_entries: Vec<SidebarEntry>,
    pub selected_sidebar_index: usize,
    pub current_context: String,
    pub context_cursor: usize,
    pub context_filter: String,
    pub key_config: KeyConfig,
    dirty: bool,
    needs_full_reconcile: bool,
    sync_handle: Option<crate::sync_transport::SyncHandle>,
    sync_event_tx: std::sync::mpsc::Sender<crate::sync_transport::SyncEvent>,
    pub sync_connected: bool,
    sync_backed_up: bool,
    pub pane_switch_at: Option<Instant>,
}

impl App {
    pub fn new(
        data_dir: PathBuf,
        context: String,
        ascii_mode: bool,
        key_config: KeyConfig,
        sync_event_tx: std::sync::mpsc::Sender<crate::sync_transport::SyncEvent>,
    ) -> std::io::Result<Self> {
        let context_dir = data_dir.join(&context);
        let (auto_doc, doc) = crate::crdt::load_context_document(&context_dir)?;
        let mut app = Self {
            auto_doc,
            doc,
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
            focus_item_id: None,
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
            current_context: context.clone(),
            context_cursor: 0,
            context_filter: String::new(),
            key_config,
            dirty: false,
            needs_full_reconcile: false,
            sync_handle: None,
            sync_event_tx,
            sync_connected: false,
            sync_backed_up: false,
            pane_switch_at: None,
        };

        if let Some(config) = crate::sync_auth::load_config() {
            let handle = crate::sync_transport::start_sync_thread(
                config.server_url,
                config.token,
                context.clone(),
                app.sync_event_tx.clone(),
            );
            app.sync_handle = Some(handle);
        }

        app.rebuild_sidebar_entries();
        Ok(app)
    }

    #[cfg(test)]
    pub fn with_lists(lists: Vec<TodoList>) -> Self {
        let doc = crate::crdt::migrate_from_lists(&lists);
        let data_dir = tempfile::tempdir().expect("failed to create temp dir");
        let mut app = Self {
            auto_doc: AutoCommit::new(),
            doc,
            active_pane: Pane::Sidebar,
            selected_list_index: 0,
            selected_item_index: 0,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            input_cursor: 0,
            show_done: true,
            running: true,
            data_dir: data_dir.keep(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            search_results: Vec::new(),
            search_selected: 0,
            ascii_mode: false,
            focus_item_id: None,
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
            dirty: false,
            needs_full_reconcile: false,
            sync_handle: None,
            sync_event_tx: std::sync::mpsc::channel().0,
            sync_connected: false,
            sync_backed_up: false,
            pane_switch_at: None,
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
        self.flush();
        self.shutdown_sync();
        self.sync_handle = None;
        self.sync_backed_up = false;

        self.current_context = name.to_string();
        let (auto_doc, doc) = crate::crdt::load_context_document(&self.context_dir())
            .unwrap_or_else(|_| {
                let mut doc = crate::crdt::CrdtDocument::default();
                let inbox = crate::crdt::CrdtList {
                    id: crate::crdt::new_id(),
                    name: "Inbox".to_string(),
                    list_type: "normal".to_string(),
                    last_reset: None,
                    position: 0.0,
                };
                doc.lists.insert(inbox.id.clone(), inbox);
                (AutoCommit::new(), doc)
            });
        self.auto_doc = auto_doc;
        self.doc = doc;
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

        if let Some(config) = crate::sync_auth::load_config() {
            self.sync_handle = Some(crate::sync_transport::start_sync_thread(
                config.server_url,
                config.token,
                name.to_string(),
                self.sync_event_tx.clone(),
            ));
        }
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
            doc: self.doc.clone(),
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
        self.doc = snap.doc;
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
            self.save_doc();
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            self.restore_snapshot(snap);
            self.selected_items.clear();
            self.save_doc();
        }
    }

    pub fn selected_list_id(&self) -> Option<String> {
        self.doc
            .ordered_lists()
            .get(self.selected_list_index)
            .map(|l| l.id.clone())
    }

    pub fn current_list(&self) -> Option<&CrdtList> {
        let lists = self.doc.ordered_lists();
        lists.get(self.selected_list_index).copied()
    }

    pub fn visible_items(&self) -> Vec<(&str, &CrdtItem)> {
        let list_id = match self.selected_list_id() {
            Some(id) => id,
            None => return vec![],
        };
        let items = self.doc.items_for_list(&list_id);
        let mut undone: Vec<(&str, &CrdtItem)> = Vec::new();
        let mut done: Vec<(&str, &CrdtItem)> = Vec::new();
        for item in items {
            if !self.filter_tags.is_empty()
                && !self.filter_tags.iter().any(|ft| item.tags.contains(ft))
            {
                continue;
            }
            if item.done {
                if self.show_done {
                    done.push((item.id.as_str(), item));
                }
            } else {
                undone.push((item.id.as_str(), item));
            }
        }
        undone.extend(done);
        undone
    }

    pub fn tag_visible_items(&self) -> Vec<(&str, &CrdtItem)> {
        let tag = match self.selected_tag_name() {
            Some(t) => t,
            None => return Vec::new(),
        };
        let items = self.doc.items_for_tag(tag);
        let mut undone: Vec<(&str, &CrdtItem)> = Vec::new();
        let mut done: Vec<(&str, &CrdtItem)> = Vec::new();
        for item in items {
            if item.done {
                if self.show_done {
                    done.push((item.id.as_str(), item));
                }
            } else {
                undone.push((item.id.as_str(), item));
            }
        }
        undone.extend(done);
        undone
    }

    pub fn unassigned_visible_items(&self) -> Vec<(&str, &CrdtItem)> {
        let items = self.doc.unassigned_items();
        let mut undone: Vec<(&str, &CrdtItem)> = Vec::new();
        let mut done: Vec<(&str, &CrdtItem)> = Vec::new();
        for item in items {
            if item.done {
                if self.show_done {
                    done.push((item.id.as_str(), item));
                }
            } else {
                undone.push((item.id.as_str(), item));
            }
        }
        undone.extend(done);
        undone
    }

    pub fn resolve_selected_item(&self) -> Option<String> {
        if self.is_tag_view() {
            self.tag_visible_items()
                .get(self.selected_item_index)
                .map(|(id, _)| id.to_string())
        } else if self.is_unassigned_view() {
            self.unassigned_visible_items()
                .get(self.selected_item_index)
                .map(|(id, _)| id.to_string())
        } else {
            self.visible_items()
                .get(self.selected_item_index)
                .map(|(id, _)| id.to_string())
        }
    }

    #[cfg(test)]
    pub fn items_for_nth_list(&self, n: usize) -> Vec<&CrdtItem> {
        if let Some(list) = self.doc.ordered_lists().get(n) {
            self.doc.items_for_list(&list.id)
        } else {
            vec![]
        }
    }

    #[cfg(test)]
    pub fn nth_list(&self, n: usize) -> Option<&CrdtList> {
        self.doc.ordered_lists().get(n).copied()
    }

    #[cfg(test)]
    pub fn num_lists(&self) -> usize {
        self.doc.ordered_lists().len()
    }

    #[cfg(test)]
    pub fn item_id_in_list(&self, list_n: usize, item_n: usize) -> String {
        self.items_for_nth_list(list_n)[item_n].id.clone()
    }

    #[cfg(test)]
    pub fn set_item_field(&mut self, list_n: usize, item_n: usize, f: impl FnOnce(&mut CrdtItem)) {
        let id = self.item_id_in_list(list_n, item_n);
        if let Some(item) = self.doc.items.get_mut(&id) {
            f(item);
        }
    }

    fn sync_sidebar_selection(&mut self) {
        match self.sidebar_entries.get(self.selected_sidebar_index) {
            Some(SidebarEntry::List(i)) => {
                self.selected_list_index = *i;
            }
            Some(SidebarEntry::Tag(_)) | Some(SidebarEntry::Unassigned) | None => {}
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
        self.pane_switch_at = Some(Instant::now());
    }

    pub fn switch_to_sidebar(&mut self) {
        if self.active_pane != Pane::Sidebar {
            self.active_pane = Pane::Sidebar;
            self.selected_items.clear();
            self.pane_switch_at = Some(Instant::now());
        }
    }

    pub fn switch_to_main(&mut self) {
        if self.active_pane != Pane::Main {
            self.active_pane = Pane::Main;
            self.selected_items.clear();
            self.pane_switch_at = Some(Instant::now());
        }
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
        let num_lists = self.doc.ordered_lists().len();
        if self.is_tag_view() {
            let tag_items = self.tag_visible_items();
            if tag_items.is_empty() {
                self.selected_item_index = 0;
            } else if self.selected_item_index >= tag_items.len() {
                self.selected_item_index = tag_items.len() - 1;
            }
        } else if self.is_unassigned_view() {
            let items = self.unassigned_visible_items();
            if items.is_empty() {
                self.selected_item_index = 0;
            } else if self.selected_item_index >= items.len() {
                self.selected_item_index = items.len() - 1;
            }
        } else {
            if num_lists == 0 {
                self.selected_list_index = 0;
                self.selected_item_index = 0;
                return;
            }
            if self.selected_list_index >= num_lists {
                self.selected_list_index = num_lists - 1;
            }
            let visible = self.visible_items();
            if visible.is_empty() {
                self.selected_item_index = 0;
            } else if self.selected_item_index >= visible.len() {
                self.selected_item_index = visible.len() - 1;
            }
        }
    }

    pub fn toggle_done(&mut self) {
        if let Some(id) = self.resolve_selected_item() {
            self.push_undo();
            if let Some(item) = self.doc.items.get_mut(&id) {
                item.done = !item.done;
            }
            self.save_doc();
        }
    }

    pub fn toggle_tag(&mut self, tag: &str) {
        if let Some(id) = self.resolve_selected_item() {
            self.push_undo();
            if let Some(item) = self.doc.items.get_mut(&id) {
                if let Some(pos) = item.tags.iter().position(|t| t == tag) {
                    item.tags.remove(pos);
                } else {
                    item.tags.push(tag.to_string());
                }
            }
            self.rebuild_sidebar_entries();
            self.save_doc();
        }
    }

    pub fn delete_todo(&mut self) {
        if let Some(id) = self.resolve_selected_item() {
            self.push_undo();
            self.doc.items.remove(&id);
            self.clamp_selection();
            self.rebuild_sidebar_entries();
            self.save_doc();
        }
    }

    pub fn add_todo(&mut self, title: String) {
        if title.trim().is_empty() {
            return;
        }
        self.push_undo();
        let (clean_title, mut tags) = storage::extract_tags_pub(&title);
        if let Some(tag) = self.selected_tag_name()
            && !tags.contains(&tag.to_string())
        {
            tags.push(tag.to_string());
        }
        let list_id = if self.is_tag_view() || self.is_unassigned_view() {
            None
        } else {
            self.selected_list_id()
        };
        let position = match &list_id {
            Some(lid) => {
                if let Some(sel_id) = self.resolve_selected_item() {
                    if let Some(sel_item) = self.doc.items.get(&sel_id) {
                        sel_item.position - 0.5
                    } else {
                        self.doc.next_position_for_list(lid)
                    }
                } else {
                    self.doc.next_position_for_list(lid)
                }
            }
            None => {
                if let Some(sel_id) = self.resolve_selected_item() {
                    if let Some(sel_item) = self.doc.items.get(&sel_id) {
                        sel_item.position - 0.5
                    } else {
                        self.doc.next_position_unassigned()
                    }
                } else {
                    self.doc.next_position_unassigned()
                }
            }
        };
        let inserted_above_selection = self.resolve_selected_item().is_some();
        let item = CrdtItem {
            id: crate::crdt::new_id(),
            title: clean_title,
            done: false,
            tags,
            time_secs: 0,
            list_id,
            position,
            archived: false,
        };
        self.doc.items.insert(item.id.clone(), item);
        if inserted_above_selection {
            self.selected_item_index += 1;
        }
        self.rebuild_sidebar_entries();
        self.save_doc();
    }

    pub fn edit_todo_title(&mut self, new_title: String) {
        if let Some(id) = self.resolve_selected_item() {
            self.push_undo();
            if let Some(item) = self.doc.items.get_mut(&id) {
                item.title = new_title;
            }
            self.save_doc();
        }
    }

    pub fn edit_todo_tags(&mut self, tags_str: String) {
        if let Some(id) = self.resolve_selected_item() {
            self.push_undo();
            let tags: Vec<String> = tags_str
                .split_whitespace()
                .map(|t| t.strip_prefix('@').unwrap_or(t).to_string())
                .filter(|t| !t.is_empty())
                .collect();
            if let Some(item) = self.doc.items.get_mut(&id) {
                item.tags = tags;
            }
            self.rebuild_sidebar_entries();
            self.save_doc();
        }
    }

    fn renumber_item_positions_for_list(&mut self, list_id: &str) {
        let ids: Vec<String> = self
            .doc
            .items_for_list(list_id)
            .iter()
            .map(|item| item.id.clone())
            .collect();
        for (i, id) in ids.iter().enumerate() {
            let new_pos = i as f64;
            if let Some(item) = self.doc.items.get_mut(id) {
                item.position = new_pos;
            }
            self.put_auto_doc_item_position(id, new_pos);
        }
    }

    pub fn move_todo_up(&mut self) {
        if self.is_virtual_view() {
            return;
        }
        let list_id = match self.selected_list_id() {
            Some(id) => id,
            None => return,
        };
        let visible = self.visible_items();
        let vi = self.selected_item_index;
        if vi > 0 && vi < visible.len() {
            let cur_id = visible[vi].0.to_string();
            let prev_id = visible[vi - 1].0.to_string();
            let cur_pos = match self.doc.items.get(&cur_id) {
                Some(item) => item.position,
                None => return,
            };
            let prev_pos = match self.doc.items.get(&prev_id) {
                Some(item) => item.position,
                None => return,
            };
            drop(visible);
            self.push_undo();
            if cur_pos == prev_pos {
                self.renumber_item_positions_for_list(&list_id);
            }
            let cur_pos = match self.doc.items.get(&cur_id) {
                Some(item) => item.position,
                None => return,
            };
            let prev_pos = match self.doc.items.get(&prev_id) {
                Some(item) => item.position,
                None => return,
            };
            if let Some(item) = self.doc.items.get_mut(&cur_id) {
                item.position = prev_pos;
            }
            if let Some(item) = self.doc.items.get_mut(&prev_id) {
                item.position = cur_pos;
            }
            self.put_auto_doc_item_position(&cur_id, prev_pos);
            self.put_auto_doc_item_position(&prev_id, cur_pos);
            self.selected_item_index -= 1;
            self.dirty = true;
        }
    }

    pub fn move_todo_down(&mut self) {
        if self.is_virtual_view() {
            return;
        }
        let list_id = match self.selected_list_id() {
            Some(id) => id,
            None => return,
        };
        let visible = self.visible_items();
        let vi = self.selected_item_index;
        if vi + 1 < visible.len() {
            let cur_id = visible[vi].0.to_string();
            let next_id = visible[vi + 1].0.to_string();
            let cur_pos = match self.doc.items.get(&cur_id) {
                Some(item) => item.position,
                None => return,
            };
            let next_pos = match self.doc.items.get(&next_id) {
                Some(item) => item.position,
                None => return,
            };
            drop(visible);
            self.push_undo();
            if cur_pos == next_pos {
                self.renumber_item_positions_for_list(&list_id);
            }
            let cur_pos = match self.doc.items.get(&cur_id) {
                Some(item) => item.position,
                None => return,
            };
            let next_pos = match self.doc.items.get(&next_id) {
                Some(item) => item.position,
                None => return,
            };
            if let Some(item) = self.doc.items.get_mut(&cur_id) {
                item.position = next_pos;
            }
            if let Some(item) = self.doc.items.get_mut(&next_id) {
                item.position = cur_pos;
            }
            self.put_auto_doc_item_position(&cur_id, next_pos);
            self.put_auto_doc_item_position(&next_id, cur_pos);
            self.selected_item_index += 1;
            self.dirty = true;
        }
    }

    pub fn move_todo_to_top(&mut self) {
        if self.is_virtual_view() {
            return;
        }
        let visible = self.visible_items();
        let vi = self.selected_item_index;
        if vi > 0 && vi < visible.len() {
            let cur_id = visible[vi].0.to_string();
            let min_pos = visible
                .iter()
                .map(|(_, item)| item.position)
                .fold(f64::INFINITY, f64::min);
            drop(visible);
            self.push_undo();
            let new_pos = min_pos - 1.0;
            if let Some(item) = self.doc.items.get_mut(&cur_id) {
                item.position = new_pos;
            } else {
                return;
            }
            self.put_auto_doc_item_position(&cur_id, new_pos);
            self.selected_item_index = 0;
            self.dirty = true;
        }
    }

    pub fn move_todo_to_bottom(&mut self) {
        if self.is_virtual_view() {
            return;
        }
        let visible = self.visible_items();
        let vi = self.selected_item_index;
        if vi < visible.len() {
            let undone_count = visible.iter().filter(|(_, item)| !item.done).count();
            if vi + 1 >= undone_count {
                return;
            }
            let cur_id = visible[vi].0.to_string();
            let max_pos = visible
                .iter()
                .filter(|(_, item)| !item.done)
                .map(|(_, item)| item.position)
                .fold(f64::NEG_INFINITY, f64::max);
            drop(visible);
            self.push_undo();
            let new_pos = max_pos + 1.0;
            if let Some(item) = self.doc.items.get_mut(&cur_id) {
                item.position = new_pos;
            } else {
                return;
            }
            self.put_auto_doc_item_position(&cur_id, new_pos);
            let new_visible = self.visible_items();
            if let Some(new_vi) = new_visible.iter().position(|(id, _)| *id == cur_id) {
                self.selected_item_index = new_vi;
            }
            self.dirty = true;
        }
    }

    pub fn toggle_show_done(&mut self) {
        self.show_done = !self.show_done;
        self.selected_items.clear();
        self.clamp_selection();
    }

    pub fn start_focus(&mut self) {
        if let Some(id) = self.resolve_selected_item() {
            self.focus_item_id = Some(id);
            self.focus_accumulated = 0;
            self.focus_start = Some(Instant::now());
            self.input_mode = InputMode::Focused;
        }
    }

    pub fn stop_focus(&mut self) {
        let elapsed = self.focus_elapsed_secs();
        if let Some(ref id) = self.focus_item_id
            && let Some(item) = self.doc.items.get_mut(id)
        {
            item.time_secs += elapsed;
        }
        self.focus_start = None;
        self.focus_accumulated = 0;
        self.input_mode = InputMode::Normal;
        self.save_doc();
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
        if let Some(id) = self.resolve_selected_item() {
            self.push_undo();
            if let Some(item) = self.doc.items.get_mut(&id) {
                item.time_secs = secs;
            }
            self.save_doc();
        }
    }

    pub fn start_filter(&mut self) {
        if self.is_tag_view() {
            return;
        }
        let mut tags: Vec<String> = Vec::new();
        if let Some(list_id) = self.selected_list_id() {
            for item in self.doc.items_for_list(&list_id) {
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
        self.doc.all_tags()
    }

    pub fn rebuild_sidebar_entries(&mut self) {
        self.sidebar_entries.clear();
        let lists = self.doc.ordered_lists();
        for i in 0..lists.len() {
            self.sidebar_entries.push(SidebarEntry::List(i));
        }
        if self.doc.items.values().any(|i| i.list_id.is_none()) {
            self.sidebar_entries.push(SidebarEntry::Unassigned);
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

    pub fn is_unassigned_view(&self) -> bool {
        matches!(
            self.selected_sidebar_entry(),
            Some(SidebarEntry::Unassigned)
        )
    }

    pub fn is_virtual_view(&self) -> bool {
        self.is_tag_view() || self.is_unassigned_view()
    }

    pub fn selected_tag_name(&self) -> Option<&str> {
        match self.selected_sidebar_entry() {
            Some(SidebarEntry::Tag(name)) => Some(name),
            _ => None,
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
        let list = CrdtList {
            id: crate::crdt::new_id(),
            name,
            list_type: "normal".to_string(),
            last_reset: None,
            position: self.doc.next_list_position(),
        };
        self.doc.lists.insert(list.id.clone(), list);
        self.selected_list_index = self.doc.ordered_lists().len() - 1;
        self.selected_item_index = 0;
        self.rebuild_sidebar_entries();
        self.save_doc();
    }

    pub fn rename_list(&mut self, new_name: String) {
        if new_name.trim().is_empty() {
            return;
        }
        if let Some(list_id) = self.selected_list_id() {
            self.push_undo();
            if let Some(list) = self.doc.lists.get_mut(&list_id) {
                list.name = new_name;
            }
            self.save_doc();
        }
    }

    pub fn delete_list(&mut self) {
        let num_lists = self.doc.ordered_lists().len();
        if num_lists <= 1 {
            return;
        }
        if let Some(list_id) = self.selected_list_id() {
            self.push_undo();
            for item in self.doc.items.values_mut() {
                if item.list_id.as_deref() == Some(&list_id) {
                    item.list_id = None;
                }
            }
            self.doc.lists.remove(&list_id);
            self.rebuild_sidebar_entries();
            self.clamp_selection();
            self.save_doc();
        }
    }

    fn renumber_list_positions(&mut self) {
        let ids: Vec<String> = self
            .doc
            .ordered_lists()
            .iter()
            .map(|list| list.id.clone())
            .collect();
        for (i, id) in ids.iter().enumerate() {
            let new_pos = i as f64;
            if let Some(list) = self.doc.lists.get_mut(id) {
                list.position = new_pos;
            }
            self.put_auto_doc_list_position(id, new_pos);
        }
    }

    pub fn move_list_up(&mut self) {
        let lists = self.doc.ordered_lists();
        if self.selected_list_index > 0 && self.selected_list_index < lists.len() {
            let cur_id = lists[self.selected_list_index].id.clone();
            let prev_id = lists[self.selected_list_index - 1].id.clone();
            let cur_pos = match self.doc.lists.get(&cur_id) {
                Some(l) => l.position,
                None => return,
            };
            let prev_pos = match self.doc.lists.get(&prev_id) {
                Some(l) => l.position,
                None => return,
            };
            drop(lists);
            self.push_undo();
            if cur_pos == prev_pos {
                self.renumber_list_positions();
            }
            let cur_pos = match self.doc.lists.get(&cur_id) {
                Some(l) => l.position,
                None => return,
            };
            let prev_pos = match self.doc.lists.get(&prev_id) {
                Some(l) => l.position,
                None => return,
            };
            if let Some(list) = self.doc.lists.get_mut(&cur_id) {
                list.position = prev_pos;
            }
            if let Some(list) = self.doc.lists.get_mut(&prev_id) {
                list.position = cur_pos;
            }
            self.put_auto_doc_list_position(&cur_id, prev_pos);
            self.put_auto_doc_list_position(&prev_id, cur_pos);
            self.selected_list_index -= 1;
            self.selected_sidebar_index = self.selected_list_index;
            self.rebuild_sidebar_entries();
            self.dirty = true;
        }
    }

    pub fn move_list_down(&mut self) {
        let lists = self.doc.ordered_lists();
        if self.selected_list_index + 1 < lists.len() {
            let cur_id = lists[self.selected_list_index].id.clone();
            let next_id = lists[self.selected_list_index + 1].id.clone();
            let cur_pos = match self.doc.lists.get(&cur_id) {
                Some(l) => l.position,
                None => return,
            };
            let next_pos = match self.doc.lists.get(&next_id) {
                Some(l) => l.position,
                None => return,
            };
            drop(lists);
            self.push_undo();
            if cur_pos == next_pos {
                self.renumber_list_positions();
            }
            let cur_pos = match self.doc.lists.get(&cur_id) {
                Some(l) => l.position,
                None => return,
            };
            let next_pos = match self.doc.lists.get(&next_id) {
                Some(l) => l.position,
                None => return,
            };
            if let Some(list) = self.doc.lists.get_mut(&cur_id) {
                list.position = next_pos;
            }
            if let Some(list) = self.doc.lists.get_mut(&next_id) {
                list.position = cur_pos;
            }
            self.put_auto_doc_list_position(&cur_id, next_pos);
            self.put_auto_doc_list_position(&next_id, cur_pos);
            self.selected_list_index += 1;
            self.selected_sidebar_index = self.selected_list_index;
            self.rebuild_sidebar_entries();
            self.dirty = true;
        }
    }

    pub fn move_list_to_top(&mut self) {
        let lists = self.doc.ordered_lists();
        if self.selected_list_index > 0 && self.selected_list_index < lists.len() {
            let cur_id = lists[self.selected_list_index].id.clone();
            let min_pos = lists
                .iter()
                .map(|l| l.position)
                .fold(f64::INFINITY, f64::min);
            drop(lists);
            self.push_undo();
            let new_pos = min_pos - 1.0;
            if let Some(list) = self.doc.lists.get_mut(&cur_id) {
                list.position = new_pos;
            } else {
                return;
            }
            self.put_auto_doc_list_position(&cur_id, new_pos);
            self.selected_list_index = 0;
            self.selected_sidebar_index = 0;
            self.rebuild_sidebar_entries();
            self.dirty = true;
        }
    }

    pub fn move_list_to_bottom(&mut self) {
        let lists = self.doc.ordered_lists();
        if self.selected_list_index + 1 < lists.len() {
            let cur_id = lists[self.selected_list_index].id.clone();
            let max_pos = lists
                .iter()
                .map(|l| l.position)
                .fold(f64::NEG_INFINITY, f64::max);
            drop(lists);
            self.push_undo();
            let new_pos = max_pos + 1.0;
            if let Some(list) = self.doc.lists.get_mut(&cur_id) {
                list.position = new_pos;
            } else {
                return;
            }
            self.put_auto_doc_list_position(&cur_id, new_pos);
            self.selected_list_index = self.doc.ordered_lists().len() - 1;
            self.selected_sidebar_index = self.selected_list_index;
            self.rebuild_sidebar_entries();
            self.dirty = true;
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

        let ordered_lists = self.doc.ordered_lists();
        for (li, list) in ordered_lists.iter().enumerate() {
            if list.name.to_lowercase().contains(&query) {
                list_matches.push(SearchResult::List(li));
            }
            for item in self.doc.items_for_list(&list.id) {
                let title_match = item.title.to_lowercase().contains(&query);
                let tag_match = item.tags.iter().any(|t| t.to_lowercase().contains(&query));
                if tag_match && !title_match {
                    tag_item_matches.push(SearchResult::Item(item.id.clone()));
                } else if title_match {
                    title_matches.push(SearchResult::Item(item.id.clone()));
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
                SearchResult::Item(ref item_id) => {
                    if let Some(item) = self.doc.items.get(item_id) {
                        if item.done {
                            self.show_done = true;
                        }
                        if let Some(ref list_id) = item.list_id {
                            let lists = self.doc.ordered_lists();
                            if let Some(li) = lists.iter().position(|l| l.id == *list_id) {
                                self.selected_list_index = li;
                                self.selected_sidebar_index = li;
                                self.active_pane = Pane::Main;
                                let visible = self.visible_items();
                                if let Some(vi) =
                                    visible.iter().position(|(id, _)| *id == item_id.as_str())
                                {
                                    self.selected_item_index = vi;
                                }
                            }
                        }
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
        if self.is_virtual_view() {
            return;
        }
        if let Some(id) = self.resolve_selected_item()
            && !self.selected_items.remove(&id)
        {
            self.selected_items.insert(id);
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
        let ids: Vec<String> = self.selected_items.drain().collect();
        for id in ids {
            self.doc.items.remove(&id);
        }
        self.rebuild_sidebar_entries();
        self.clamp_selection();
        self.save_doc();
    }

    pub fn toggle_done_selected(&mut self) {
        if self.selected_items.is_empty() {
            self.toggle_done();
            return;
        }
        self.push_undo();
        let ids: Vec<String> = self.selected_items.drain().collect();
        for id in &ids {
            if let Some(item) = self.doc.items.get_mut(id) {
                item.done = !item.done;
            }
        }
        self.rebuild_sidebar_entries();
        self.clamp_selection();
        self.save_doc();
    }

    pub fn done_count(&self) -> usize {
        if let Some(list_id) = self.selected_list_id() {
            self.doc
                .items_for_list(&list_id)
                .iter()
                .filter(|item| item.done)
                .count()
        } else {
            0
        }
    }

    pub fn start_archive(&mut self) {
        if self.done_count() == 0 {
            return;
        }
        self.input_mode = InputMode::ConfirmArchive;
    }

    pub fn archive_done_items(&mut self) {
        let list_id = match self.selected_list_id() {
            Some(id) => id,
            None => return,
        };
        let ids_to_archive: Vec<String> = self
            .doc
            .items
            .values()
            .filter(|item| item.list_id.as_deref() == Some(&list_id) && item.done)
            .map(|item| item.id.clone())
            .collect();
        if ids_to_archive.is_empty() {
            return;
        }
        self.push_undo();
        for id in &ids_to_archive {
            if let Some(item) = self.doc.items.get_mut(id) {
                item.archived = true;
            }
        }
        self.selected_items.clear();
        self.rebuild_sidebar_entries();
        self.clamp_selection();
        self.save_doc();
        self.input_mode = InputMode::Normal;
    }

    pub fn move_to_list_targets(&self) -> Vec<(usize, &str)> {
        let query = self.move_to_list_filter.to_lowercase();
        self.doc
            .ordered_lists()
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.selected_list_index)
            .filter(|(_, list)| query.is_empty() || list.name.to_lowercase().contains(&query))
            .map(|(i, list)| (i, list.name.as_str()))
            .collect()
    }

    pub fn start_move_to_list(&mut self) {
        if self.is_virtual_view() {
            return;
        }
        let num_lists = self.doc.ordered_lists().len();
        if num_lists < 2 {
            return;
        }
        let has_items = if self.selected_items.is_empty() {
            self.resolve_selected_item().is_some()
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
        let target_list_id = self
            .doc
            .ordered_lists()
            .get(target_list_idx)
            .map(|l| l.id.clone());
        let target_list_id = match target_list_id {
            Some(id) => id,
            None => return,
        };

        self.push_undo();

        let mut ids_to_move: Vec<String> = if self.selected_items.is_empty() {
            self.resolve_selected_item().into_iter().collect()
        } else {
            self.selected_items.iter().cloned().collect()
        };
        ids_to_move.sort_by(|a, b| {
            let pos_a = self.doc.items.get(a).map(|i| i.position).unwrap_or(0.0);
            let pos_b = self.doc.items.get(b).map(|i| i.position).unwrap_or(0.0);
            pos_a
                .partial_cmp(&pos_b)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(b))
        });

        let min_pos = self.doc.next_position_for_list(&target_list_id);
        for (offset, id) in ids_to_move.iter().enumerate() {
            if let Some(item) = self.doc.items.get_mut(id) {
                item.list_id = Some(target_list_id.clone());
                item.position = min_pos + offset as f64;
            }
        }

        self.selected_items.clear();
        self.move_to_list_filter.clear();
        self.input_mode = InputMode::Normal;
        self.rebuild_sidebar_entries();
        self.clamp_selection();
        self.save_doc();
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

    fn save_doc(&mut self) {
        self.dirty = true;
        self.needs_full_reconcile = true;
    }

    fn put_auto_doc_item_position(&mut self, item_id: &str, position: f64) {
        use automerge::{ROOT, ReadDoc, transaction::Transactable};
        if let Ok(Some((_, items_obj))) = self.auto_doc.get(ROOT, "items")
            && let Ok(Some((_, item_obj))) = self.auto_doc.get(&items_obj, item_id)
        {
            let _ = self.auto_doc.put(&item_obj, "position", position);
        }
    }

    fn put_auto_doc_list_position(&mut self, list_id: &str, position: f64) {
        use automerge::{ROOT, ReadDoc, transaction::Transactable};
        if let Ok(Some((_, lists_obj))) = self.auto_doc.get(ROOT, "lists")
            && let Ok(Some((_, list_obj))) = self.auto_doc.get(&lists_obj, list_id)
        {
            let _ = self.auto_doc.put(&list_obj, "position", position);
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn has_sync(&self) -> bool {
        self.sync_handle.is_some()
    }

    pub fn flush(&mut self) {
        if self.dirty {
            if self.needs_full_reconcile {
                let _ = autosurgeon::reconcile(&mut self.auto_doc, &self.doc);
                self.needs_full_reconcile = false;
            }
            let bytes = self.auto_doc.save();
            let automerge_path = self.context_dir().join("todui.automerge");
            let tmp_path = self.context_dir().join("todui.automerge.tmp");
            if std::fs::write(&tmp_path, &bytes).is_ok() {
                let _ = std::fs::rename(&tmp_path, &automerge_path);
            }
            self.dirty = false;
            if let Some(handle) = &self.sync_handle {
                let _ = handle
                    .command_tx
                    .send(crate::sync_transport::SyncCommand::SendMessage(bytes));
            }
        }
    }

    pub fn send_document(&mut self) {
        if let Some(handle) = &self.sync_handle {
            let bytes = self.auto_doc.save();
            let _ = handle
                .command_tx
                .send(crate::sync_transport::SyncCommand::SendMessage(bytes));
        }
    }

    pub fn process_sync_events(
        &mut self,
        events: Vec<crate::sync_transport::SyncEvent>,
    ) -> bool {
        let mut changed = false;

        if events.is_empty() {
            return false;
        }

        if self.needs_full_reconcile {
            let _ = autosurgeon::reconcile(&mut self.auto_doc, &self.doc);
            self.needs_full_reconcile = false;
        }

        for event in events {
            match event {
                crate::sync_transport::SyncEvent::MessageReceived(bytes) => {
                    if let Ok(mut server_doc) = AutoCommit::load(&bytes)
                        && server_doc.merge(&mut self.auto_doc).is_ok()
                    {
                        let our_heads = self.auto_doc.get_heads();
                        let merged_heads = server_doc.get_heads();
                        let has_new_changes = {
                            let ours: std::collections::HashSet<_> = our_heads.iter().collect();
                            merged_heads.iter().any(|h| !ours.contains(h))
                        };
                        if has_new_changes
                            && let Ok(new_doc) =
                                autosurgeon::hydrate::<_, crate::crdt::CrdtDocument>(&server_doc)
                        {
                            self.backup_before_sync();
                            self.auto_doc = server_doc;
                            self.doc = new_doc;
                            changed = true;
                        }
                    }
                }
                crate::sync_transport::SyncEvent::Connected => {
                    self.sync_connected = true;
                    self.send_document();
                }
                crate::sync_transport::SyncEvent::Disconnected => {
                    self.sync_connected = false;
                }
            }
        }

        if changed {
            self.rebuild_sidebar_entries();
            self.dirty = true;
        }

        changed
    }

    fn backup_before_sync(&mut self) {
        if self.sync_backed_up {
            return;
        }
        let source_path = self.context_dir().join("todui.automerge");
        if source_path.exists() {
            let backup_path = self.context_dir().join("todui.automerge.bak");
            let _ = std::fs::copy(&source_path, &backup_path);
        }
        self.sync_backed_up = true;
    }

    pub fn shutdown_sync(&mut self) {
        if let Some(handle) = &self.sync_handle {
            let _ = handle
                .command_tx
                .send(crate::sync_transport::SyncCommand::Shutdown);
        }
    }

    pub fn toggle_list_type(&mut self) {
        if let Some(list_id) = self.selected_list_id() {
            self.push_undo();
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            if let Some(list) = self.doc.lists.get_mut(&list_id) {
                if list.list_type == "normal" {
                    list.list_type = "daily".to_string();
                    list.last_reset = Some(today);
                } else {
                    list.list_type = "normal".to_string();
                    list.last_reset = None;
                }
            }
            self.save_doc();
        }
    }

    pub fn reset_daily_lists(&mut self) {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let list_ids: Vec<String> = self
            .doc
            .lists
            .values()
            .filter(|l| l.list_type == "daily")
            .map(|l| l.id.clone())
            .collect();
        for list_id in list_ids {
            let needs_reset = match self.doc.lists.get(&list_id) {
                Some(list) => match &list.last_reset {
                    None => true,
                    Some(date) => date.as_str() < today.as_str(),
                },
                None => continue,
            };
            if !needs_reset {
                continue;
            }
            let mut done_items: Vec<(String, f64)> = self
                .doc
                .items
                .values()
                .filter(|item| item.list_id.as_deref() == Some(&list_id) && item.done)
                .map(|item| (item.id.clone(), item.position))
                .collect();
            done_items.sort_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(&b.0))
            });
            let done_item_ids: Vec<String> = done_items.into_iter().map(|(id, _)| id).collect();
            if done_item_ids.is_empty() {
                if let Some(list) = self.doc.lists.get_mut(&list_id) {
                    list.last_reset = Some(today.clone());
                }
                continue;
            }
            let max_pos = self.doc.next_position_for_list(&list_id);
            for (offset, item_id) in done_item_ids.iter().enumerate() {
                if let Some(item) = self.doc.items.get_mut(item_id) {
                    item.done = false;
                    item.time_secs = 0;
                    item.position = max_pos + offset as f64;
                }
            }
            if let Some(list) = self.doc.lists.get_mut(&list_id) {
                list.last_reset = Some(today.clone());
            }
        }
        self.rebuild_sidebar_entries();
        self.save_doc();
    }

    pub fn quit(&mut self) {
        self.flush();
        self.shutdown_sync();
        self.running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ListType, TodoItem};

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
        assert!(!app.items_for_nth_list(0)[0].done);

        app.toggle_done();
        assert!(app.items_for_nth_list(0)[0].done);

        // After toggling, sort order changes so selected_item_index=0 now maps to Task C
        app.toggle_done();
        assert!(app.items_for_nth_list(0)[2].done);
    }

    #[test]
    fn test_delete_todo() {
        let mut app = sample_app();
        assert_eq!(app.items_for_nth_list(0).len(), 3);

        app.delete_todo();
        assert_eq!(app.items_for_nth_list(0).len(), 2);
        assert_eq!(app.items_for_nth_list(0)[0].title, "Task B");
        assert_eq!(app.items_for_nth_list(0)[1].title, "Task C");
    }

    #[test]
    fn test_add_todo() {
        let mut app = sample_app();
        assert_eq!(app.items_for_nth_list(0).len(), 3);

        app.add_todo("New task @urgent @work".to_string());
        assert_eq!(app.items_for_nth_list(0).len(), 4);
        let added = &app.items_for_nth_list(0)[0];
        assert_eq!(added.title, "New task");
        assert_eq!(added.tags, vec!["urgent", "work"]);
        assert!(!added.done);
    }

    #[test]
    fn test_add_todo_adjusts_selection_index() {
        let mut app = sample_app();
        app.selected_item_index = 1;
        let original_id = app.resolve_selected_item().unwrap();
        let original_title = app.doc.items.get(&original_id).unwrap().title.clone();

        app.add_todo("Inserted item".to_string());

        let after_id = app.resolve_selected_item().unwrap();
        let after_title = app.doc.items.get(&after_id).unwrap().title.clone();
        assert_eq!(
            original_title, after_title,
            "selection should stay on original item after add"
        );
        assert_eq!(app.selected_item_index, 2);
    }

    #[test]
    fn test_move_todo_up_down() {
        let mut app = sample_app();
        // Visible order (done sorted last): Task A (real 0), Task C (real 2), Task B (real 1)
        app.selected_item_index = 1; // Task C

        app.move_todo_up();
        // C swapped with A (visible neighbor), not B (real neighbor)
        assert_eq!(app.items_for_nth_list(0)[0].title, "Task C");
        assert_eq!(app.items_for_nth_list(0)[1].title, "Task B");
        assert_eq!(app.items_for_nth_list(0)[2].title, "Task A");
        assert_eq!(app.selected_item_index, 0);

        app.move_todo_down();
        // C swapped back with A
        assert_eq!(app.items_for_nth_list(0)[0].title, "Task A");
        assert_eq!(app.items_for_nth_list(0)[1].title, "Task B");
        assert_eq!(app.items_for_nth_list(0)[2].title, "Task C");
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
        assert_eq!(app.nth_list(0).unwrap().name, "Beta");
        assert_eq!(app.nth_list(1).unwrap().name, "Alpha");
        assert_eq!(app.selected_list_index, 0);
        assert_eq!(app.selected_sidebar_index, 0);

        app.move_list_up();
        assert_eq!(app.nth_list(0).unwrap().name, "Beta");
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
        assert_eq!(app.nth_list(1).unwrap().name, "Gamma");
        assert_eq!(app.nth_list(2).unwrap().name, "Beta");
        assert_eq!(app.selected_list_index, 2);
        assert_eq!(app.selected_sidebar_index, 2);

        app.move_list_down();
        assert_eq!(app.nth_list(2).unwrap().name, "Beta");
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
        assert_eq!(app.items_for_nth_list(0)[0].title, "C");
        assert_eq!(app.items_for_nth_list(0)[1].title, "A");
        assert_eq!(app.items_for_nth_list(0)[2].title, "B");
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
        assert_eq!(app.items_for_nth_list(0)[0].title, "A");
        assert_eq!(app.items_for_nth_list(0)[1].title, "B");
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
        assert_eq!(app.items_for_nth_list(0)[0].title, "B");
        assert_eq!(app.items_for_nth_list(0)[1].title, "C");
        assert_eq!(app.items_for_nth_list(0)[2].title, "A");
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
        assert_eq!(app.items_for_nth_list(0)[0].title, "A");
        assert_eq!(app.items_for_nth_list(0)[1].title, "B");
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
        assert_eq!(app.nth_list(0).unwrap().name, "Gamma");
        assert_eq!(app.nth_list(1).unwrap().name, "Alpha");
        assert_eq!(app.nth_list(2).unwrap().name, "Beta");
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
        assert_eq!(app.nth_list(0).unwrap().name, "Beta");
        assert_eq!(app.nth_list(1).unwrap().name, "Gamma");
        assert_eq!(app.nth_list(2).unwrap().name, "Alpha");
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
        assert_eq!(app.num_lists(), 2);

        app.add_list("Shopping".to_string());
        assert_eq!(app.num_lists(), 3);
        assert_eq!(app.nth_list(2).unwrap().name, "Shopping");
        assert_eq!(app.selected_list_index, 2);
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_delete_list() {
        let mut app = sample_app();
        assert_eq!(app.num_lists(), 2);

        app.delete_list();
        assert_eq!(app.num_lists(), 1);
        assert_eq!(app.nth_list(0).unwrap().name, "Personal");
    }

    #[test]
    fn test_delete_last_list_prevented() {
        let mut app = App::with_lists(vec![TodoList::new("Only")]);
        assert_eq!(app.num_lists(), 1);

        app.delete_list();
        assert_eq!(app.num_lists(), 1);
        assert_eq!(app.nth_list(0).unwrap().name, "Only");
    }

    #[test]
    fn test_undo_redo() {
        let mut app = sample_app();
        assert_eq!(app.items_for_nth_list(0).len(), 3);

        app.delete_todo();
        assert_eq!(app.items_for_nth_list(0).len(), 2);

        app.undo();
        assert_eq!(app.items_for_nth_list(0).len(), 3);
        assert_eq!(app.items_for_nth_list(0)[0].title, "Task A");

        app.redo();
        assert_eq!(app.items_for_nth_list(0).len(), 2);
        assert_eq!(app.items_for_nth_list(0)[0].title, "Task B");
    }

    #[test]
    fn test_undo_clears_redo_on_new_action() {
        let mut app = sample_app();

        app.delete_todo();
        assert_eq!(app.items_for_nth_list(0).len(), 2);

        app.undo();
        assert_eq!(app.items_for_nth_list(0).len(), 3);
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
        assert!(matches!(&app.search_results[0], SearchResult::Item(_)));
        assert!(matches!(&app.search_results[1], SearchResult::Item(_)));
    }

    #[test]
    fn test_search_results_matches_tags() {
        let mut app = search_app();
        app.start_search();
        app.input_buffer = "urgent".to_string();
        app.update_search_results();
        assert_eq!(app.search_results.len(), 2);
        assert_eq!(app.search_results[0], SearchResult::Tag("urgent".into()));
        assert!(matches!(&app.search_results[1], SearchResult::Item(_)));
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
        assert!(matches!(&app.search_results[0], SearchResult::Item(_)));

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
        assert!(matches!(&app.search_results[0], SearchResult::Item(_)));
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
        assert!(app.items_for_nth_list(0)[0].time_secs >= 9);
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
        app.set_item_field(0, 0, |item| item.time_secs = 3600);
        app.set_item_time(5400);
        assert_eq!(app.items_for_nth_list(0)[0].time_secs, 5400);
    }

    #[test]
    fn test_edit_time_clear() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        app.set_item_field(0, 0, |item| item.time_secs = 3600);
        app.input_mode = InputMode::EditingTime;
        app.input_buffer.clear();
        app.confirm_input();
        assert_eq!(app.items_for_nth_list(0)[0].time_secs, 0);
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
        assert_eq!(app.selected_items.len(), 1);
        app.toggle_select_current();
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_delete_selected_multiple() {
        let mut app = sample_app();
        // Visible order: Task A, Task C, Task B (done)
        let id_a = app.visible_items()[0].0.to_string();
        let id_c = app.visible_items()[1].0.to_string();
        app.selected_items.insert(id_a);
        app.selected_items.insert(id_c);
        app.delete_selected();
        assert_eq!(app.items_for_nth_list(0).len(), 1);
        assert_eq!(app.items_for_nth_list(0)[0].title, "Task B");
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_delete_selected_falls_back() {
        let mut app = sample_app();
        assert!(app.selected_items.is_empty());
        app.delete_selected();
        assert_eq!(app.items_for_nth_list(0).len(), 2);
    }

    #[test]
    fn test_toggle_done_selected_multiple() {
        let mut app = sample_app();
        // Visible: Task A (not done), Task C (not done), Task B (done)
        let id_a = app.visible_items()[0].0.to_string();
        let id_c = app.visible_items()[1].0.to_string();
        app.selected_items.insert(id_a.clone());
        app.selected_items.insert(id_c.clone());
        app.toggle_done_selected();
        assert!(app.doc.items[&id_a].done);
        assert!(app.doc.items[&id_c].done);
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_selection_cleared_on_pane_switch() {
        let mut app = sample_app();
        let id = app.visible_items()[0].0.to_string();
        app.selected_items.insert(id);
        app.toggle_pane();
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_selection_cleared_on_list_change() {
        let mut app = sample_app();
        app.active_pane = Pane::Sidebar;
        let id = app.visible_items()[0].0.to_string();
        app.selected_items.insert(id);
        app.move_selection_down();
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_selection_cleared_on_undo() {
        let mut app = sample_app();
        app.delete_todo();
        let id = app.visible_items()[0].0.to_string();
        app.selected_items.insert(id);
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
        assert_eq!(app.items_for_nth_list(0).len(), 2);
        assert_eq!(app.items_for_nth_list(1).len(), 1);
        assert_eq!(app.items_for_nth_list(1)[0].title, "Task A");
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_confirm_move_multi_selected() {
        let mut app = sample_app();
        let id_a = app.visible_items()[0].0.to_string(); // Task A
        let id_c = app.visible_items()[1].0.to_string(); // Task C
        app.selected_items.insert(id_a);
        app.selected_items.insert(id_c);
        app.start_move_to_list();
        app.confirm_move_to_list();
        assert_eq!(app.items_for_nth_list(0).len(), 1);
        assert_eq!(app.items_for_nth_list(0)[0].title, "Task B");
        assert_eq!(app.items_for_nth_list(1).len(), 2);
        assert_eq!(app.items_for_nth_list(1)[0].title, "Task A");
        assert_eq!(app.items_for_nth_list(1)[1].title, "Task C");
        assert!(app.selected_items.is_empty());
    }

    #[test]
    fn test_move_to_list_undoable() {
        let mut app = sample_app();
        app.selected_item_index = 0;
        app.start_move_to_list();
        app.confirm_move_to_list();
        assert_eq!(app.items_for_nth_list(0).len(), 2);
        assert_eq!(app.items_for_nth_list(1).len(), 1);
        app.undo();
        assert_eq!(app.items_for_nth_list(0).len(), 3);
        assert_eq!(app.items_for_nth_list(1).len(), 0);
    }

    #[test]
    fn test_cancel_move_to_list() {
        let mut app = sample_app();
        app.start_move_to_list();
        assert_eq!(app.input_mode, InputMode::MovingToList);
        app.cancel_move_to_list();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.items_for_nth_list(0).len(), 3);
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
        assert_eq!(app.nth_list(0).unwrap().list_type, "normal");

        app.toggle_list_type();
        assert_eq!(app.nth_list(0).unwrap().list_type, "daily");
        assert!(app.nth_list(0).unwrap().last_reset.is_some());

        app.toggle_list_type();
        assert_eq!(app.nth_list(0).unwrap().list_type, "normal");
        assert_eq!(app.nth_list(0).unwrap().last_reset, None);
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

        assert!(!app.items_for_nth_list(0)[0].done);
        assert!(!app.items_for_nth_list(0)[1].done);
        assert!(!app.items_for_nth_list(0)[2].done);
        // A stays at top, B and C moved to bottom
        assert_eq!(app.items_for_nth_list(0)[0].title, "A");
        assert_eq!(app.items_for_nth_list(0)[1].title, "B");
        assert_eq!(app.items_for_nth_list(0)[2].title, "C");
        // A (not done) keeps its timer, B and C (were done) get cleared
        assert_eq!(app.items_for_nth_list(0)[0].time_secs, 300);
        assert_eq!(app.items_for_nth_list(0)[1].time_secs, 0);
        assert_eq!(app.items_for_nth_list(0)[2].time_secs, 0);
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

        assert!(app.items_for_nth_list(0)[0].done);
    }

    #[test]
    fn test_reset_daily_lists_skips_normal() {
        let mut list = TodoList::new("Normal");
        list.last_reset = Some("2020-01-01".to_string());
        list.items.push(TodoItem::new("A"));
        list.items[0].done = true;

        let mut app = App::with_lists(vec![list]);
        app.reset_daily_lists();

        assert!(app.items_for_nth_list(0)[0].done);
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
        assert_eq!(app.items_for_nth_list(0)[0].title, "A");
        assert_eq!(app.items_for_nth_list(0)[1].title, "C");
        assert_eq!(app.items_for_nth_list(0)[2].title, "E");
        assert_eq!(app.items_for_nth_list(0)[3].title, "B");
        assert_eq!(app.items_for_nth_list(0)[4].title, "D");
        // All should be not done
        for item in app.items_for_nth_list(0) {
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
        assert_eq!(items[0].1.title, "A"); // Work/A (not done)
        assert_eq!(items[1].1.title, "C"); // Personal/C (done)
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
        assert_eq!(items[0].1.title, "A");
    }

    #[test]
    fn test_resolve_selected_item_list_mode() {
        let mut app = sample_app();
        app.active_pane = Pane::Main;
        app.selected_item_index = 0;
        let loc = app.resolve_selected_item();
        assert!(loc.is_some());
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
        // Find B's index in the tag view
        let tag_items = app.tag_visible_items();
        let b_index = tag_items
            .iter()
            .position(|(_, item)| item.title == "B")
            .unwrap();
        app.selected_item_index = b_index;
        let loc = app.resolve_selected_item();
        assert!(loc.is_some());
        let id = loc.unwrap();
        assert_eq!(app.doc.items.get(&id).unwrap().title, "B");
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
        app.selected_item_index = 0; // first item in tag view
        let tag_items = app.tag_visible_items();
        let first_title = tag_items[0].1.title.clone();
        drop(tag_items);
        app.toggle_done();
        // The toggled item should now be done
        let item = app
            .doc
            .items
            .values()
            .find(|i| i.title == first_title)
            .unwrap();
        assert!(item.done);
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
        assert_eq!(app.doc.items.len(), 2);
        app.active_pane = Pane::Main;
        app.selected_sidebar_index = 2;
        app.selected_item_index = 0;
        app.delete_todo();
        assert_eq!(app.doc.items.len(), 1);
    }

    #[test]
    fn test_add_todo_in_tag_view() {
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
        let unassigned: Vec<_> = app
            .doc
            .items
            .values()
            .filter(|i| i.list_id.is_none())
            .collect();
        assert_eq!(unassigned.len(), 1);
        assert_eq!(unassigned[0].title, "New item");
        assert!(unassigned[0].tags.contains(&"code".to_string()));
    }

    #[test]
    fn test_add_todo_in_tag_view_auto_tags() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec!["urgent".into()],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        app.selected_sidebar_index = 1; // Tag("urgent")
        app.add_todo("Fix bug @code".into());
        let unassigned: Vec<_> = app
            .doc
            .items
            .values()
            .filter(|i| i.list_id.is_none())
            .collect();
        assert_eq!(unassigned.len(), 1);
        assert_eq!(unassigned[0].title, "Fix bug");
        assert!(unassigned[0].tags.contains(&"code".to_string()));
        assert!(unassigned[0].tags.contains(&"urgent".to_string()));
    }

    #[test]
    fn test_add_todo_in_unassigned_view() {
        let mut work = TodoList::new("Work");
        work.items.push(TodoItem {
            title: "A".into(),
            done: false,
            tags: vec![],
            time_secs: 0,
        });
        let mut app = App::with_lists(vec![work]);
        // Add an unassigned item to create the Unassigned sidebar entry
        let item = CrdtItem {
            id: crate::crdt::new_id(),
            title: "Existing unassigned".into(),
            done: false,
            tags: vec![],
            time_secs: 0,
            list_id: None,
            position: 0.0,
            archived: false,
        };
        app.doc.items.insert(item.id.clone(), item);
        app.rebuild_sidebar_entries();
        // Navigate to Unassigned entry (after the list)
        let unassigned_idx = app
            .sidebar_entries
            .iter()
            .position(|e| matches!(e, SidebarEntry::Unassigned))
            .unwrap();
        app.selected_sidebar_index = unassigned_idx;
        app.add_todo("Loose task".into());
        let unassigned: Vec<_> = app
            .doc
            .items
            .values()
            .filter(|i| i.list_id.is_none())
            .collect();
        assert_eq!(unassigned.len(), 2);
        assert!(unassigned.iter().any(|i| i.title == "Loose task"));
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
        assert_eq!(app.items_for_nth_list(0)[0].title, "A");
        assert_eq!(app.items_for_nth_list(0)[1].title, "B");
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
    fn test_archive_done_items_archives_done() {
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

        assert_eq!(app.items_for_nth_list(0).len(), 1);
        assert_eq!(app.items_for_nth_list(0)[0].title, "Keep this");
        assert_eq!(app.doc.items.len(), 3);
        let archived: Vec<_> = app.doc.items.values().filter(|i| i.archived).collect();
        assert_eq!(archived.len(), 2);
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
        assert_eq!(app.items_for_nth_list(0).len(), 1);

        app.undo();
        assert_eq!(app.items_for_nth_list(0).len(), 2);
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
        assert_eq!(app.items_for_nth_list(0)[0].tags, vec!["focus"]);
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
        assert!(app.items_for_nth_list(0)[0].tags.is_empty());
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
        assert_eq!(app.items_for_nth_list(0)[0].tags, vec!["code", "focus"]);
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
        assert_eq!(app.items_for_nth_list(0)[0].tags, vec!["focus"]);
        app.undo();
        assert!(app.items_for_nth_list(0)[0].tags.is_empty());
    }

    #[test]
    fn test_switch_context_resets_sync_backed_up() {
        let mut app = App::with_lists(vec![]);
        app.sync_backed_up = true;
        app.switch_context("other");
        assert!(!app.sync_backed_up);
    }

    #[test]
    fn test_move_todo_with_duplicate_positions() {
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
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;

        // Give all three items the same position (simulates sync collision)
        app.set_item_field(0, 0, |item| item.position = 1.0);
        app.set_item_field(0, 1, |item| item.position = 1.0);
        app.set_item_field(0, 2, |item| item.position = 1.0);

        // Visible order is by position then id tiebreak
        let titles_before: Vec<_> = app
            .visible_items()
            .iter()
            .map(|(_, i)| i.title.clone())
            .collect();
        let first = titles_before[0].clone();
        let second = titles_before[1].clone();

        // Select second item and move it up
        app.selected_item_index = 1;
        app.move_todo_up();

        let titles_after: Vec<_> = app
            .visible_items()
            .iter()
            .map(|(_, i)| i.title.clone())
            .collect();
        // The first two items should have swapped
        assert_eq!(titles_after[0], second);
        assert_eq!(titles_after[1], first);
        assert_eq!(app.selected_item_index, 0);
    }

    #[test]
    fn test_move_todo_up_three_duplicate_positions_from_bottom() {
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
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;

        app.set_item_field(0, 0, |item| item.position = 1.0);
        app.set_item_field(0, 1, |item| item.position = 1.0);
        app.set_item_field(0, 2, |item| item.position = 1.0);

        let titles_before: Vec<_> = app
            .visible_items()
            .iter()
            .map(|(_, i)| i.title.clone())
            .collect();
        let first = titles_before[0].clone();
        let second = titles_before[1].clone();
        let third = titles_before[2].clone();

        app.selected_item_index = 2;
        app.move_todo_up();

        let titles_after: Vec<_> = app
            .visible_items()
            .iter()
            .map(|(_, i)| i.title.clone())
            .collect();
        assert_eq!(titles_after[0], first);
        assert_eq!(titles_after[1], third);
        assert_eq!(titles_after[2], second);
        assert_eq!(app.selected_item_index, 1);
    }

    #[test]
    fn test_move_todo_down_three_duplicate_positions_from_top() {
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
        let mut app = App::with_lists(vec![list]);
        app.active_pane = Pane::Main;

        app.set_item_field(0, 0, |item| item.position = 1.0);
        app.set_item_field(0, 1, |item| item.position = 1.0);
        app.set_item_field(0, 2, |item| item.position = 1.0);

        let titles_before: Vec<_> = app
            .visible_items()
            .iter()
            .map(|(_, i)| i.title.clone())
            .collect();
        let first = titles_before[0].clone();
        let second = titles_before[1].clone();
        let third = titles_before[2].clone();

        app.selected_item_index = 0;
        app.move_todo_down();

        let titles_after: Vec<_> = app
            .visible_items()
            .iter()
            .map(|(_, i)| i.title.clone())
            .collect();
        assert_eq!(titles_after[0], second);
        assert_eq!(titles_after[1], first);
        assert_eq!(titles_after[2], third);
        assert_eq!(app.selected_item_index, 1);
    }
}
