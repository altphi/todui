use std::fs;
use std::io;
use std::path::Path;

use crate::model::{TodoItem, TodoList};

pub fn parse_todo_line(line: &str) -> Option<TodoItem> {
    let trimmed = line.trim_start();
    let (done, rest) = if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
        (false, rest)
    } else if let Some(rest) = trimmed.strip_prefix("- [x] ") {
        (true, rest)
    } else if let Some(rest) = trimmed.strip_prefix("- [X] ") {
        (true, rest)
    } else {
        return None;
    };
    let (title, tags) = extract_tags(rest);
    Some(TodoItem { title, done, tags })
}

/// Walks backwards collecting @word tokens. A non-@word token breaks the walk.
fn extract_tags(text: &str) -> (String, Vec<String>) {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut tag_count = 0;
    for word in words.iter().rev() {
        if word.starts_with('@')
            && word.len() > 1
            && word[1..]
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            tag_count += 1;
        } else {
            break;
        }
    }
    let split_point = words.len() - tag_count;
    let title = words[..split_point].join(" ");
    let tags: Vec<String> = words[split_point..]
        .iter()
        .map(|w| w[1..].to_string())
        .collect();
    (title, tags)
}

pub fn extract_tags_pub(text: &str) -> (String, Vec<String>) {
    extract_tags(text)
}

pub fn parse_list(content: &str) -> TodoList {
    let mut name = String::from("Untitled");
    let mut items = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            let heading = heading.trim();
            if !heading.is_empty() {
                name = heading.to_string();
            }
        } else if let Some(item) = parse_todo_line(line) {
            items.push(item);
        }
    }

    TodoList { name, items }
}

pub fn serialize_list(list: &TodoList) -> String {
    let mut output = format!("# {}\n\n", list.name);
    for item in &list.items {
        let checkbox = if item.done { "[x]" } else { "[ ]" };
        if item.tags.is_empty() {
            output.push_str(&format!("- {} {}\n", checkbox, item.title));
        } else {
            let tags: Vec<String> = item.tags.iter().map(|t| format!("@{}", t)).collect();
            output.push_str(&format!(
                "- {} {} {}\n",
                checkbox,
                item.title,
                tags.join(" ")
            ));
        }
    }
    output
}

pub fn name_to_filename(name: &str) -> String {
    let slug = name.to_lowercase().replace(' ', "-");
    format!("{}.md", slug)
}

pub fn load_lists(dir: &Path) -> io::Result<Vec<TodoList>> {
    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }

    let mut md_files: Vec<_> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();

    if md_files.is_empty() {
        let inbox = TodoList::new("Inbox");
        save_list(dir, &inbox)?;
        return Ok(vec![inbox]);
    }

    md_files.sort_by_key(|entry| entry.file_name());

    let mut lists = Vec::new();
    for entry in md_files {
        let content = fs::read_to_string(entry.path())?;
        lists.push(parse_list(&content));
    }

    Ok(lists)
}

pub fn save_list(dir: &Path, list: &TodoList) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let filename = name_to_filename(&list.name);
    let path = dir.join(filename);
    fs::write(path, serialize_list(list))
}

pub fn save_all(dir: &Path, lists: &[TodoList]) -> io::Result<()> {
    for list in lists {
        save_list(dir, list)?;
    }
    Ok(())
}

pub fn delete_list_file(dir: &Path, list_name: &str) -> io::Result<()> {
    let filename = name_to_filename(list_name);
    let path = dir.join(filename);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tags_no_tags() {
        let (title, tags) = extract_tags("Buy groceries");
        assert_eq!(title, "Buy groceries");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_extract_tags_single() {
        let (title, tags) = extract_tags("Buy groceries @errands");
        assert_eq!(title, "Buy groceries");
        assert_eq!(tags, vec!["errands"]);
    }

    #[test]
    fn test_extract_tags_multiple() {
        let (title, tags) = extract_tags("Buy groceries @errands @today");
        assert_eq!(title, "Buy groceries");
        assert_eq!(tags, vec!["errands", "today"]);
    }

    #[test]
    fn test_extract_tags_at_in_middle_not_a_tag() {
        let (title, tags) = extract_tags("Email user@example.com @work");
        assert_eq!(title, "Email user@example.com");
        assert_eq!(tags, vec!["work"]);
    }

    #[test]
    fn test_parse_todo_line_unchecked() {
        let item = parse_todo_line("- [ ] Buy groceries @errands").unwrap();
        assert_eq!(item.title, "Buy groceries");
        assert!(!item.done);
        assert_eq!(item.tags, vec!["errands"]);
    }

    #[test]
    fn test_parse_todo_line_checked() {
        let item = parse_todo_line("- [x] Review PR @code").unwrap();
        assert_eq!(item.title, "Review PR");
        assert!(item.done);
        assert_eq!(item.tags, vec!["code"]);
    }

    #[test]
    fn test_parse_todo_line_checked_uppercase() {
        let item = parse_todo_line("- [X] Review PR @code").unwrap();
        assert_eq!(item.title, "Review PR");
        assert!(item.done);
        assert_eq!(item.tags, vec!["code"]);
    }

    #[test]
    fn test_parse_todo_line_not_a_todo() {
        assert!(parse_todo_line("Just a regular line").is_none());
        assert!(parse_todo_line("- A list item").is_none());
        assert!(parse_todo_line("# A heading").is_none());
    }

    #[test]
    fn test_parse_list() {
        let md = "# Work\n\n- [ ] Send invoice @client\n- [x] Review PR @code\n";
        let list = parse_list(md);
        assert_eq!(list.name, "Work");
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].title, "Send invoice");
        assert!(!list.items[0].done);
        assert_eq!(list.items[0].tags, vec!["client"]);
        assert_eq!(list.items[1].title, "Review PR");
        assert!(list.items[1].done);
        assert_eq!(list.items[1].tags, vec!["code"]);
    }

    #[test]
    fn test_parse_list_no_heading() {
        let md = "- [ ] Something\n";
        let list = parse_list(md);
        assert_eq!(list.name, "Untitled");
        assert_eq!(list.items.len(), 1);
    }

    #[test]
    fn test_serialize_list() {
        let list = TodoList {
            name: "Work".to_string(),
            items: vec![
                TodoItem {
                    title: "Send invoice".to_string(),
                    done: false,
                    tags: vec!["client".to_string()],
                },
                TodoItem {
                    title: "Review PR".to_string(),
                    done: true,
                    tags: vec!["code".to_string()],
                },
            ],
        };
        let output = serialize_list(&list);
        assert_eq!(
            output,
            "# Work\n\n- [ ] Send invoice @client\n- [x] Review PR @code\n"
        );
    }

    #[test]
    fn test_serialize_roundtrip() {
        let md = "# Projects\n\n- [ ] Build app @dev @urgent\n- [x] Write docs @docs\n";
        let list = parse_list(md);
        let serialized = serialize_list(&list);
        let reparsed = parse_list(&serialized);
        assert_eq!(list, reparsed);
    }

    #[test]
    fn test_name_to_filename() {
        assert_eq!(name_to_filename("Work"), "work.md");
        assert_eq!(name_to_filename("My Projects"), "my-projects.md");
    }

    #[test]
    fn test_load_creates_default_inbox() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        let lists = load_lists(&dir).unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].name, "Inbox");
        assert!(dir.join("inbox.md").exists());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        fs::create_dir_all(&dir).unwrap();

        let list_a = TodoList {
            name: "Alpha".to_string(),
            items: vec![TodoItem {
                title: "Task A".to_string(),
                done: false,
                tags: vec!["tag1".to_string()],
            }],
        };
        let list_b = TodoList {
            name: "Beta".to_string(),
            items: vec![TodoItem {
                title: "Task B".to_string(),
                done: true,
                tags: vec![],
            }],
        };

        save_all(&dir, &[list_a.clone(), list_b.clone()]).unwrap();
        let loaded = load_lists(&dir).unwrap();
        assert_eq!(loaded.len(), 2);
        // Loaded order is alphabetical by filename
        assert_eq!(loaded[0].name, "Alpha");
        assert_eq!(loaded[1].name, "Beta");
        assert_eq!(loaded[0], list_a);
        assert_eq!(loaded[1], list_b);
    }
}
