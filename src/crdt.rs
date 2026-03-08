use automerge::AutoCommit;
use autosurgeon::{Hydrate, Reconcile, hydrate, reconcile};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
pub struct CrdtItem {
    pub id: String,
    pub title: String,
    pub done: bool,
    pub tags: Vec<String>,
    pub time_secs: u64,
    pub list_id: Option<String>,
    pub position: f64,
}

#[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
pub struct CrdtList {
    pub id: String,
    pub name: String,
    pub list_type: String,
    pub last_reset: Option<String>,
    pub position: f64,
}

#[derive(Debug, Clone, PartialEq, Default, Reconcile, Hydrate)]
pub struct CrdtDocument {
    pub items: HashMap<String, CrdtItem>,
    pub lists: HashMap<String, CrdtList>,
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn save_document(
    path: &Path,
    auto_doc: &mut AutoCommit,
    data: &CrdtDocument,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    reconcile(auto_doc, data).map_err(|e| io::Error::other(e.to_string()))?;
    let bytes = auto_doc.save();
    fs::write(path, bytes)
}

#[cfg(test)]
pub fn save_document_fresh(path: &Path, data: &CrdtDocument) -> io::Result<()> {
    let mut auto_doc = AutoCommit::new();
    save_document(path, &mut auto_doc, data)
}

use crate::model::{ListType, TodoList};

pub fn migrate_from_lists(lists: &[TodoList]) -> CrdtDocument {
    let mut doc = CrdtDocument::default();

    for (list_idx, list) in lists.iter().enumerate() {
        let list_id = new_id();
        let list_type = match list.list_type {
            ListType::Normal => "normal",
            ListType::Daily => "daily",
        };
        let crdt_list = CrdtList {
            id: list_id.clone(),
            name: list.name.clone(),
            list_type: list_type.to_string(),
            last_reset: list.last_reset.clone(),
            position: list_idx as f64,
        };
        doc.lists.insert(list_id.clone(), crdt_list);

        for (item_idx, item) in list.items.iter().enumerate() {
            let item_id = new_id();
            let crdt_item = CrdtItem {
                id: item_id.clone(),
                title: item.title.clone(),
                done: item.done,
                tags: item.tags.clone(),
                time_secs: item.time_secs,
                list_id: Some(list_id.clone()),
                position: item_idx as f64,
            };
            doc.items.insert(item_id, crdt_item);
        }
    }

    doc
}

impl CrdtDocument {
    pub fn items_for_list(&self, list_id: &str) -> Vec<&CrdtItem> {
        let mut items: Vec<&CrdtItem> = self
            .items
            .values()
            .filter(|i| i.list_id.as_deref() == Some(list_id))
            .collect();
        items.sort_by(|a, b| {
            a.position
                .partial_cmp(&b.position)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });
        items
    }

    pub fn items_for_tag(&self, tag: &str) -> Vec<&CrdtItem> {
        let mut items: Vec<&CrdtItem> = self
            .items
            .values()
            .filter(|i| i.tags.iter().any(|t| t == tag))
            .collect();
        items.sort_by(|a, b| {
            a.position
                .partial_cmp(&b.position)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });
        items
    }

    pub fn unassigned_items(&self) -> Vec<&CrdtItem> {
        let mut items: Vec<&CrdtItem> = self
            .items
            .values()
            .filter(|i| i.list_id.is_none())
            .collect();
        items.sort_by(|a, b| {
            a.position
                .partial_cmp(&b.position)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });
        items
    }

    pub fn ordered_lists(&self) -> Vec<&CrdtList> {
        let mut lists: Vec<&CrdtList> = self.lists.values().collect();
        lists.sort_by(|a, b| {
            a.position
                .partial_cmp(&b.position)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });
        lists
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .items
            .values()
            .flat_map(|i| i.tags.iter().cloned())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        tags.sort();
        tags
    }

    pub fn next_position_for_list(&self, list_id: &str) -> f64 {
        let max = self
            .items
            .values()
            .filter(|i| i.list_id.as_deref() == Some(list_id))
            .map(|i| i.position)
            .fold(f64::NEG_INFINITY, f64::max);
        if max == f64::NEG_INFINITY {
            0.0
        } else {
            max + 1.0
        }
    }

    pub fn next_position_unassigned(&self) -> f64 {
        let max = self
            .items
            .values()
            .filter(|i| i.list_id.is_none())
            .map(|i| i.position)
            .fold(f64::NEG_INFINITY, f64::max);
        if max == f64::NEG_INFINITY {
            0.0
        } else {
            max + 1.0
        }
    }

    pub fn next_list_position(&self) -> f64 {
        let max = self
            .lists
            .values()
            .map(|l| l.position)
            .fold(f64::NEG_INFINITY, f64::max);
        if max == f64::NEG_INFINITY {
            0.0
        } else {
            max + 1.0
        }
    }
}

pub fn load_context_document(context_dir: &Path) -> io::Result<(AutoCommit, CrdtDocument)> {
    let automerge_path = context_dir.join("todui.automerge");
    if automerge_path.exists() {
        return load_document(&automerge_path);
    }

    let md_files_exist = fs::read_dir(context_dir)?
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"));

    if md_files_exist {
        let lists = crate::storage::load_lists(context_dir)?;
        let data = migrate_from_lists(&lists);
        let mut auto_doc = AutoCommit::new();
        save_document(&automerge_path, &mut auto_doc, &data)?;
        let backup_dir = context_dir.join(".md-backup");
        fs::create_dir_all(&backup_dir)?;
        for entry in fs::read_dir(context_dir)?.filter_map(|e| e.ok()) {
            if entry.path().extension().and_then(|x| x.to_str()) == Some("md") {
                let dest = backup_dir.join(entry.file_name());
                fs::rename(entry.path(), dest)?;
            }
        }
        let order_path = context_dir.join(".order");
        if order_path.exists() {
            fs::rename(&order_path, backup_dir.join(".order"))?;
        }
        return Ok((auto_doc, data));
    }

    let mut data = CrdtDocument::default();
    let inbox = CrdtList {
        id: new_id(),
        name: "Inbox".to_string(),
        list_type: "normal".to_string(),
        last_reset: None,
        position: 0.0,
    };
    data.lists.insert(inbox.id.clone(), inbox);
    let mut auto_doc = AutoCommit::new();
    save_document(&automerge_path, &mut auto_doc, &data)?;
    Ok((auto_doc, data))
}

pub fn save_context_document(
    context_dir: &Path,
    auto_doc: &mut AutoCommit,
    data: &CrdtDocument,
) -> io::Result<()> {
    let automerge_path = context_dir.join("todui.automerge");
    save_document(&automerge_path, auto_doc, data)
}

pub fn load_document(path: &Path) -> io::Result<(AutoCommit, CrdtDocument)> {
    if !path.exists() {
        return Ok((AutoCommit::new(), CrdtDocument::default()));
    }
    let bytes = fs::read(path)?;
    let auto_doc = AutoCommit::load(&bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let data: CrdtDocument = hydrate(&auto_doc)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    Ok((auto_doc, data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use automerge::AutoCommit;
    use autosurgeon::{hydrate, reconcile};

    #[test]
    fn test_roundtrip_item() {
        let item = CrdtItem {
            id: "test-id".to_string(),
            title: "Buy milk".to_string(),
            done: false,
            tags: vec!["errands".to_string()],
            time_secs: 0,
            list_id: None,
            position: 1.0,
        };
        let mut doc = AutoCommit::new();
        reconcile(&mut doc, &item).unwrap();
        let hydrated: CrdtItem = hydrate(&doc).unwrap();
        assert_eq!(hydrated.id, "test-id");
        assert_eq!(hydrated.title, "Buy milk");
        assert!(!hydrated.done);
        assert_eq!(hydrated.tags, vec!["errands".to_string()]);
        assert_eq!(hydrated.time_secs, 0);
        assert!(hydrated.list_id.is_none());
    }

    #[test]
    fn test_roundtrip_list() {
        let list = CrdtList {
            id: "list-1".to_string(),
            name: "Work".to_string(),
            list_type: "normal".to_string(),
            last_reset: None,
            position: 0.0,
        };
        let mut doc = AutoCommit::new();
        reconcile(&mut doc, &list).unwrap();
        let hydrated: CrdtList = hydrate(&doc).unwrap();
        assert_eq!(hydrated.id, "list-1");
        assert_eq!(hydrated.name, "Work");
        assert_eq!(hydrated.list_type, "normal");
    }

    #[test]
    fn test_roundtrip_document() {
        let mut doc_data = CrdtDocument::default();
        let item = CrdtItem {
            id: "item-1".to_string(),
            title: "Task A".to_string(),
            done: false,
            tags: vec!["work".to_string()],
            time_secs: 0,
            list_id: Some("list-1".to_string()),
            position: 1.0,
        };
        let list = CrdtList {
            id: "list-1".to_string(),
            name: "Work".to_string(),
            list_type: "normal".to_string(),
            last_reset: None,
            position: 0.0,
        };
        doc_data.items.insert(item.id.clone(), item);
        doc_data.lists.insert(list.id.clone(), list);

        let mut doc = AutoCommit::new();
        reconcile(&mut doc, &doc_data).unwrap();
        let hydrated: CrdtDocument = hydrate(&doc).unwrap();
        assert_eq!(hydrated.items.len(), 1);
        assert_eq!(hydrated.lists.len(), 1);
        assert_eq!(hydrated.items["item-1"].title, "Task A");
        assert_eq!(hydrated.lists["list-1"].name, "Work");
    }

    #[test]
    fn test_item_with_list_id() {
        let item = CrdtItem {
            id: "i1".to_string(),
            title: "Assigned".to_string(),
            done: false,
            tags: vec![],
            time_secs: 0,
            list_id: Some("list-1".to_string()),
            position: 1.0,
        };
        let mut doc = AutoCommit::new();
        reconcile(&mut doc, &item).unwrap();
        let hydrated: CrdtItem = hydrate(&doc).unwrap();
        assert_eq!(hydrated.list_id, Some("list-1".to_string()));
    }

    #[test]
    fn test_save_and_load_document() {
        let mut doc_data = CrdtDocument::default();
        doc_data.items.insert(
            "i1".to_string(),
            CrdtItem {
                id: "i1".to_string(),
                title: "Test".to_string(),
                done: false,
                tags: vec!["tag1".to_string()],
                time_secs: 60,
                list_id: None,
                position: 1.0,
            },
        );

        let mut doc = AutoCommit::new();
        reconcile(&mut doc, &doc_data).unwrap();
        let bytes = doc.save();

        let loaded_doc = AutoCommit::load(&bytes).unwrap();
        let loaded_data: CrdtDocument = hydrate(&loaded_doc).unwrap();
        assert_eq!(loaded_data.items.len(), 1);
        assert_eq!(loaded_data.items["i1"].title, "Test");
        assert_eq!(loaded_data.items["i1"].time_secs, 60);
    }

    #[test]
    fn test_save_and_load_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.automerge");

        let mut doc_data = CrdtDocument::default();
        doc_data.lists.insert(
            "l1".to_string(),
            CrdtList {
                id: "l1".to_string(),
                name: "Inbox".to_string(),
                list_type: "normal".to_string(),
                last_reset: None,
                position: 0.0,
            },
        );
        doc_data.items.insert(
            "i1".to_string(),
            CrdtItem {
                id: "i1".to_string(),
                title: "Task".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
                list_id: Some("l1".to_string()),
                position: 1.0,
            },
        );

        save_document_fresh(&path, &doc_data).unwrap();
        assert!(path.exists());

        let (_, loaded) = load_document(&path).unwrap();
        assert_eq!(loaded.lists.len(), 1);
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items["i1"].title, "Task");
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("missing.automerge");
        let (_, loaded) = load_document(&path).unwrap();
        assert!(loaded.items.is_empty());
        assert!(loaded.lists.is_empty());
    }

    #[test]
    fn test_new_document_id() {
        let id1 = new_id();
        let id2 = new_id();
        assert_ne!(id1, id2);
        assert!(!id1.is_empty());
    }

    // Task 4: Migration tests

    use crate::model::{ListType, TodoItem, TodoList};

    #[test]
    fn test_migrate_from_lists() {
        let mut list1 = TodoList::new("Work");
        list1.items = vec![
            TodoItem {
                title: "Task A".to_string(),
                done: false,
                tags: vec!["urgent".to_string()],
                time_secs: 2700,
            },
            TodoItem {
                title: "Task B".to_string(),
                done: true,
                tags: vec![],
                time_secs: 0,
            },
        ];
        let mut list2 = TodoList::new("Personal");
        list2.list_type = ListType::Daily;
        list2.last_reset = Some("2026-03-07".to_string());
        list2.items = vec![TodoItem {
            title: "Exercise".to_string(),
            done: false,
            tags: vec!["health".to_string()],
            time_secs: 0,
        }];

        let doc = migrate_from_lists(&[list1, list2]);
        assert_eq!(doc.lists.len(), 2);
        assert_eq!(doc.items.len(), 3);

        let work_list = doc.lists.values().find(|l| l.name == "Work").unwrap();
        assert_eq!(work_list.list_type, "normal");

        let personal_list = doc.lists.values().find(|l| l.name == "Personal").unwrap();
        assert_eq!(personal_list.list_type, "daily");
        assert_eq!(personal_list.last_reset, Some("2026-03-07".to_string()));

        let task_a = doc.items.values().find(|i| i.title == "Task A").unwrap();
        assert_eq!(task_a.tags, vec!["urgent".to_string()]);
        assert_eq!(task_a.time_secs, 2700);
        assert_eq!(task_a.list_id.as_ref(), Some(&work_list.id));

        let exercise = doc.items.values().find(|i| i.title == "Exercise").unwrap();
        assert_eq!(exercise.list_id.as_ref(), Some(&personal_list.id));
    }

    #[test]
    fn test_migrate_empty_lists() {
        let doc = migrate_from_lists(&[]);
        assert!(doc.items.is_empty());
        assert!(doc.lists.is_empty());
    }

    #[test]
    fn test_migrate_preserves_item_order_via_position() {
        let mut list = TodoList::new("Work");
        list.items = vec![
            TodoItem {
                title: "First".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            },
            TodoItem {
                title: "Second".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            },
            TodoItem {
                title: "Third".to_string(),
                done: false,
                tags: vec![],
                time_secs: 0,
            },
        ];

        let doc = migrate_from_lists(&[list]);
        let list_id = doc.lists.values().next().unwrap().id.clone();
        let mut items: Vec<&CrdtItem> = doc
            .items
            .values()
            .filter(|i| i.list_id.as_ref() == Some(&list_id))
            .collect();
        items.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
        assert_eq!(items[0].title, "First");
        assert_eq!(items[1].title, "Second");
        assert_eq!(items[2].title, "Third");
    }

    // Task 5: Query method tests

    fn sample_document() -> CrdtDocument {
        let mut doc = CrdtDocument::default();
        doc.lists.insert(
            "l1".to_string(),
            CrdtList {
                id: "l1".to_string(),
                name: "Work".to_string(),
                list_type: "normal".to_string(),
                last_reset: None,
                position: 0.0,
            },
        );
        doc.lists.insert(
            "l2".to_string(),
            CrdtList {
                id: "l2".to_string(),
                name: "Personal".to_string(),
                list_type: "daily".to_string(),
                last_reset: Some("2026-03-07".to_string()),
                position: 1.0,
            },
        );
        doc.items.insert(
            "i1".to_string(),
            CrdtItem {
                id: "i1".to_string(),
                title: "Send email".to_string(),
                done: false,
                tags: vec!["urgent".to_string()],
                time_secs: 0,
                list_id: Some("l1".to_string()),
                position: 0.0,
            },
        );
        doc.items.insert(
            "i2".to_string(),
            CrdtItem {
                id: "i2".to_string(),
                title: "Review PR".to_string(),
                done: true,
                tags: vec!["code".to_string()],
                time_secs: 2700,
                list_id: Some("l1".to_string()),
                position: 1.0,
            },
        );
        doc.items.insert(
            "i3".to_string(),
            CrdtItem {
                id: "i3".to_string(),
                title: "Exercise".to_string(),
                done: false,
                tags: vec!["health".to_string(), "urgent".to_string()],
                time_secs: 0,
                list_id: Some("l2".to_string()),
                position: 0.0,
            },
        );
        doc.items.insert(
            "i4".to_string(),
            CrdtItem {
                id: "i4".to_string(),
                title: "Unassigned task".to_string(),
                done: false,
                tags: vec!["urgent".to_string()],
                time_secs: 0,
                list_id: None,
                position: 0.0,
            },
        );
        doc
    }

    #[test]
    fn test_items_for_list() {
        let doc = sample_document();
        let items = doc.items_for_list("l1");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Send email");
        assert_eq!(items[1].title, "Review PR");
    }

    #[test]
    fn test_items_for_tag() {
        let doc = sample_document();
        let items = doc.items_for_tag("urgent");
        assert_eq!(items.len(), 3);
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.contains(&"Send email"));
        assert!(titles.contains(&"Exercise"));
        assert!(titles.contains(&"Unassigned task"));
    }

    #[test]
    fn test_unassigned_items() {
        let doc = sample_document();
        let items = doc.unassigned_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Unassigned task");
    }

    #[test]
    fn test_ordered_lists() {
        let doc = sample_document();
        let lists = doc.ordered_lists();
        assert_eq!(lists.len(), 2);
        assert_eq!(lists[0].name, "Work");
        assert_eq!(lists[1].name, "Personal");
    }

    #[test]
    fn test_all_tags() {
        let doc = sample_document();
        let tags = doc.all_tags();
        assert_eq!(tags, vec!["code", "health", "urgent"]);
    }

    #[test]
    fn test_next_position_for_list() {
        let doc = sample_document();
        let pos = doc.next_position_for_list("l1");
        assert!(pos > 1.0);
    }

    #[test]
    fn test_next_position_for_empty_list() {
        let doc = sample_document();
        let pos = doc.next_position_for_list("nonexistent");
        assert_eq!(pos, 0.0);
    }

    #[test]
    fn test_next_list_position() {
        let doc = sample_document();
        let pos = doc.next_list_position();
        assert!(pos > 1.0);
    }

    #[test]
    fn test_load_context_document_empty_dir_creates_inbox() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = tmp.path().join("ctx");
        fs::create_dir_all(&ctx).unwrap();

        let (_, doc) = load_context_document(&ctx).unwrap();
        assert_eq!(doc.lists.len(), 1);
        let list = doc.lists.values().next().unwrap();
        assert_eq!(list.name, "Inbox");
        assert!(ctx.join("todui.automerge").exists());
    }

    #[test]
    fn test_load_context_document_from_automerge_file() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = tmp.path().join("ctx");
        fs::create_dir_all(&ctx).unwrap();

        let mut original = CrdtDocument::default();
        original.lists.insert(
            "l1".to_string(),
            CrdtList {
                id: "l1".to_string(),
                name: "Work".to_string(),
                list_type: "normal".to_string(),
                last_reset: None,
                position: 0.0,
            },
        );
        save_document_fresh(&ctx.join("todui.automerge"), &original).unwrap();

        let (_, loaded) = load_context_document(&ctx).unwrap();
        assert_eq!(loaded.lists.len(), 1);
        assert_eq!(loaded.lists["l1"].name, "Work");
    }

    #[test]
    fn test_save_and_load_context_document_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = tmp.path().join("ctx");
        fs::create_dir_all(&ctx).unwrap();

        let mut doc = CrdtDocument::default();
        doc.lists.insert(
            "l1".to_string(),
            CrdtList {
                id: "l1".to_string(),
                name: "Projects".to_string(),
                list_type: "daily".to_string(),
                last_reset: Some("2026-03-07".to_string()),
                position: 0.0,
            },
        );
        doc.items.insert(
            "i1".to_string(),
            CrdtItem {
                id: "i1".to_string(),
                title: "Ship it".to_string(),
                done: false,
                tags: vec!["dev".to_string()],
                time_secs: 3600,
                list_id: Some("l1".to_string()),
                position: 0.0,
            },
        );

        let mut auto_doc = AutoCommit::new();
        save_context_document(&ctx, &mut auto_doc, &doc).unwrap();
        let (_, loaded) = load_context_document(&ctx).unwrap();
        assert_eq!(loaded.lists.len(), 1);
        assert_eq!(loaded.lists["l1"].name, "Projects");
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items["i1"].title, "Ship it");
        assert_eq!(loaded.items["i1"].time_secs, 3600);
    }

    #[test]
    fn test_load_context_document_migrates_md_files() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = tmp.path().join("ctx");
        fs::create_dir_all(&ctx).unwrap();

        let mut work = TodoList::new("Work");
        work.items = vec![TodoItem {
            title: "Review PR".to_string(),
            done: false,
            tags: vec!["code".to_string()],
            time_secs: 2700,
        }];
        let personal = TodoList::new("Personal");

        fs::write(ctx.join("work.md"), crate::storage::serialize_list(&work)).unwrap();
        fs::write(
            ctx.join("personal.md"),
            crate::storage::serialize_list(&personal),
        )
        .unwrap();

        let (_, doc) = load_context_document(&ctx).unwrap();
        assert_eq!(doc.lists.len(), 2);
        assert_eq!(doc.items.len(), 1);
        let review_item = doc.items.values().find(|i| i.title == "Review PR").unwrap();
        assert_eq!(review_item.tags, vec!["code".to_string()]);
        assert_eq!(review_item.time_secs, 2700);

        assert!(ctx.join("todui.automerge").exists());
        assert!(ctx.join(".md-backup").exists());
        assert!(ctx.join(".md-backup").join("work.md").exists());
        assert!(ctx.join(".md-backup").join("personal.md").exists());
        assert!(!ctx.join("work.md").exists());
        assert!(!ctx.join("personal.md").exists());
    }
}
