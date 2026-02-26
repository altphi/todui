#[derive(Debug, Clone, PartialEq)]
pub struct TodoItem {
    pub title: String,
    pub done: bool,
    pub tags: Vec<String>,
    pub time_secs: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TodoList {
    pub name: String,
    pub items: Vec<TodoItem>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pane {
    Sidebar,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    AddingItem,
    AddingList,
    RenamingList,
    EditingItem,
    EditingTags,
    ConfirmDelete,
    Searching,
    Focused,
    EditingTime,
    FilteringTags,
    MovingToList,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchResult {
    List(usize),
    Item(usize, usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppSnapshot {
    pub lists: Vec<TodoList>,
    pub selected_list_index: usize,
    pub selected_item_index: usize,
}

impl TodoItem {
    #[cfg(test)]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            done: false,
            tags: Vec::new(),
            time_secs: 0,
        }
    }
}

impl TodoList {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_item_new() {
        let item = TodoItem::new("Buy groceries");
        assert_eq!(item.title, "Buy groceries");
        assert!(!item.done);
        assert!(item.tags.is_empty());
    }

    #[test]
    fn test_todo_item_default_time() {
        let item = TodoItem::new("Test");
        assert_eq!(item.time_secs, 0);
    }

    #[test]
    fn test_todo_list_new() {
        let list = TodoList::new("Work");
        assert_eq!(list.name, "Work");
        assert!(list.items.is_empty());
    }
}
