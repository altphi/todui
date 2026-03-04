use std::fs;
use std::io;
use std::path::Path;

use crate::model::{ListType, TodoItem, TodoList};

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
    let (title_and_time, tags) = extract_tags(rest);
    let (title, time_secs) = extract_time(&title_and_time);
    Some(TodoItem {
        title,
        done,
        tags,
        time_secs,
    })
}

/// Collects trailing @word tokens from right to left; stops at the first non-tag word.
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

fn extract_time(text: &str) -> (String, u64) {
    let trimmed = text.trim_end();
    if let Some(bracket_start) = trimmed.rfind('[')
        && trimmed.ends_with(']')
    {
        let time_str = &trimmed[bracket_start + 1..trimmed.len() - 1];
        if let Some(secs) = parse_time_str(time_str) {
            let title = trimmed[..bracket_start].trim_end().to_string();
            return (title, secs);
        }
    }
    (trimmed.to_string(), 0)
}

pub fn parse_list(content: &str) -> TodoList {
    let mut name = String::from("Untitled");
    let mut items = Vec::new();
    let mut list_type = ListType::Normal;
    let mut last_reset = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            let heading = heading.trim();
            if !heading.is_empty() {
                name = heading.to_string();
            }
        } else if trimmed == "<!-- type: daily -->" {
            list_type = ListType::Daily;
        } else if let Some(date) = trimmed
            .strip_prefix("<!-- last-reset: ")
            .and_then(|s| s.strip_suffix(" -->"))
        {
            last_reset = Some(date.to_string());
        } else if let Some(item) = parse_todo_line(line) {
            items.push(item);
        }
    }

    TodoList {
        name,
        items,
        list_type,
        last_reset,
    }
}

fn serialize_item(item: &TodoItem) -> String {
    let checkbox = if item.done { "[x]" } else { "[ ]" };
    let time = format_time(item.time_secs);
    let time_part = if time.is_empty() {
        String::new()
    } else {
        format!(" [{}]", time)
    };
    if item.tags.is_empty() {
        format!("- {} {}{}\n", checkbox, item.title, time_part)
    } else {
        let tags: Vec<String> = item.tags.iter().map(|t| format!("@{}", t)).collect();
        format!(
            "- {} {}{} {}\n",
            checkbox,
            item.title,
            time_part,
            tags.join(" ")
        )
    }
}

pub fn serialize_list(list: &TodoList) -> String {
    let mut output = format!("# {}\n", list.name);
    if list.list_type == ListType::Daily {
        output.push_str("<!-- type: daily -->\n");
    }
    if let Some(ref date) = list.last_reset {
        output.push_str(&format!("<!-- last-reset: {} -->\n", date));
    }
    output.push('\n');
    for item in &list.items {
        output.push_str(&serialize_item(item));
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

    let order = load_order(dir);
    if order.is_empty() {
        md_files.sort_by_key(|entry| entry.file_name());
    } else {
        md_files.sort_by_key(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            order.iter().position(|o| *o == name).unwrap_or(usize::MAX)
        });
    }

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

pub fn save_order(dir: &Path, lists: &[TodoList]) -> io::Result<()> {
    let filenames: Vec<String> = lists.iter().map(|l| name_to_filename(&l.name)).collect();
    let content = filenames.join("\n");
    fs::write(dir.join(".order"), content)
}

fn load_order(dir: &Path) -> Vec<String> {
    let path = dir.join(".order");
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

#[cfg(unix)]
fn is_pid_alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(windows)]
fn is_pid_alive(pid: &str) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .is_ok_and(|o| !String::from_utf8_lossy(&o.stdout).contains("INFO:"))
}

pub fn acquire_lock(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let lock_path = dir.join(".lock");
    if lock_path.exists()
        && let Ok(pid_str) = fs::read_to_string(&lock_path)
    {
        let pid = pid_str.trim();
        if !pid.is_empty() && is_pid_alive(pid) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Another instance is already running (PID {})", pid),
            ));
        }
    }
    fs::write(&lock_path, std::process::id().to_string())
}

pub fn release_lock(dir: &Path) {
    let lock_path = dir.join(".lock");
    if let Ok(contents) = fs::read_to_string(&lock_path)
        && contents.trim() == std::process::id().to_string()
    {
        let _ = fs::remove_file(lock_path);
    }
}

pub fn list_contexts(dir: &Path) -> Vec<String> {
    let mut contexts: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|_| fs::read_dir(".").unwrap())
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                && !entry.file_name().to_string_lossy().starts_with('.')
        })
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    contexts.sort();
    contexts
}

pub fn migrate_to_contexts(dir: &Path) -> io::Result<()> {
    if !dir.exists() {
        fs::create_dir_all(dir)?;
        return Ok(());
    }

    let has_subdirs = fs::read_dir(dir)?.filter_map(|e| e.ok()).any(|e| {
        e.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
            && !e.file_name().to_string_lossy().starts_with('.')
    });
    if has_subdirs {
        return Ok(());
    }

    let md_files: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .collect();
    if md_files.is_empty() {
        return Ok(());
    }

    let default_dir = dir.join("default");
    fs::create_dir_all(&default_dir)?;

    for entry in md_files {
        let dest = default_dir.join(entry.file_name());
        fs::rename(entry.path(), dest)?;
    }

    let order_path = dir.join(".order");
    if order_path.exists() {
        fs::rename(&order_path, default_dir.join(".order"))?;
    }

    Ok(())
}

pub fn save_last_context(dir: &Path, name: &str) -> io::Result<()> {
    fs::write(dir.join(".context"), name)
}

pub fn load_last_context(dir: &Path) -> Option<String> {
    fs::read_to_string(dir.join(".context"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn create_context_dir(dir: &Path, name: &str) -> io::Result<()> {
    let context_dir = dir.join(name);
    fs::create_dir_all(&context_dir)?;
    let inbox = TodoList::new("Inbox");
    save_list(&context_dir, &inbox)?;
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

pub fn append_to_archive(
    context_dir: &Path,
    list_name: &str,
    items: &[TodoItem],
) -> io::Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let archive_dir = context_dir.join(".archive");
    fs::create_dir_all(&archive_dir)?;
    let filename = name_to_filename(list_name);
    let archive_path = archive_dir.join(filename);

    let mut content = String::new();
    for item in items {
        content.push_str(&serialize_item(item));
    }

    use std::fs::OpenOptions;
    use std::io::Write;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(archive_path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

pub fn format_time(secs: u64) -> String {
    if secs == 0 {
        return String::new();
    }
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    match (hours, minutes, seconds) {
        (0, 0, s) => format!("{}s", s),
        (0, m, 0) => format!("{}m", m),
        (0, m, s) => format!("{}m{}s", m, s),
        (h, 0, 0) => format!("{}h", h),
        (h, 0, s) => format!("{}h{}s", h, s),
        (h, m, 0) => format!("{}h{}m", h, m),
        (h, m, s) => format!("{}h{}m{}s", h, m, s),
    }
}

pub fn parse_time_str(s: &str) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    let mut hours: u64 = 0;
    let mut minutes: u64 = 0;
    let mut seconds: u64 = 0;
    let mut num_buf = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            num_buf.push(c);
        } else if c == 'h' {
            hours = num_buf.parse().ok()?;
            num_buf.clear();
        } else if c == 'm' {
            minutes = num_buf.parse().ok()?;
            num_buf.clear();
        } else if c == 's' {
            seconds = num_buf.parse().ok()?;
            num_buf.clear();
        } else {
            return None;
        }
    }
    if !num_buf.is_empty() {
        return None;
    }
    if hours == 0 && minutes == 0 && seconds == 0 {
        return None;
    }
    Some(hours * 3600 + minutes * 60 + seconds)
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
        let mut list = TodoList::new("Work");
        list.items = vec![
            TodoItem {
                title: "Send invoice".to_string(),
                done: false,
                tags: vec!["client".to_string()],
                time_secs: 0,
            },
            TodoItem {
                title: "Review PR".to_string(),
                done: true,
                tags: vec!["code".to_string()],
                time_secs: 0,
            },
        ];
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
    fn test_format_time_zero() {
        assert_eq!(format_time(0), "");
    }

    #[test]
    fn test_format_time_minutes_only() {
        assert_eq!(format_time(2700), "45m");
    }

    #[test]
    fn test_format_time_hours_only() {
        assert_eq!(format_time(7200), "2h");
    }

    #[test]
    fn test_format_time_hours_and_minutes() {
        assert_eq!(format_time(5400), "1h30m");
    }

    #[test]
    fn test_parse_time_str_minutes() {
        assert_eq!(parse_time_str("45m"), Some(2700));
    }

    #[test]
    fn test_parse_time_str_hours() {
        assert_eq!(parse_time_str("2h"), Some(7200));
    }

    #[test]
    fn test_parse_time_str_hours_minutes() {
        assert_eq!(parse_time_str("1h30m"), Some(5400));
    }

    #[test]
    fn test_parse_time_str_large_minutes() {
        assert_eq!(parse_time_str("90m"), Some(5400));
    }

    #[test]
    fn test_format_time_seconds_only() {
        assert_eq!(format_time(30), "30s");
    }

    #[test]
    fn test_format_time_minutes_and_seconds() {
        assert_eq!(format_time(90), "1m30s");
    }

    #[test]
    fn test_format_time_hours_minutes_seconds() {
        assert_eq!(format_time(3661), "1h1m1s");
    }

    #[test]
    fn test_parse_time_str_seconds() {
        assert_eq!(parse_time_str("30s"), Some(30));
    }

    #[test]
    fn test_parse_time_str_minutes_seconds() {
        assert_eq!(parse_time_str("1m30s"), Some(90));
    }

    #[test]
    fn test_parse_time_str_invalid() {
        assert_eq!(parse_time_str("abc"), None);
        assert_eq!(parse_time_str(""), None);
    }

    #[test]
    fn test_parse_todo_line_with_time() {
        let item = parse_todo_line("- [ ] Buy groceries [45m] @errands").unwrap();
        assert_eq!(item.title, "Buy groceries");
        assert_eq!(item.time_secs, 2700);
        assert_eq!(item.tags, vec!["errands"]);
    }

    #[test]
    fn test_parse_todo_line_with_time_no_tags() {
        let item = parse_todo_line("- [x] Review PR [1h30m]").unwrap();
        assert_eq!(item.title, "Review PR");
        assert_eq!(item.time_secs, 5400);
        assert!(item.tags.is_empty());
    }

    #[test]
    fn test_serialize_list_with_time() {
        let mut list = TodoList::new("Work");
        list.items = vec![
            TodoItem {
                title: "Task A".to_string(),
                done: false,
                tags: vec!["code".to_string()],
                time_secs: 2700,
            },
            TodoItem {
                title: "Task B".to_string(),
                done: true,
                tags: vec![],
                time_secs: 0,
            },
        ];
        let output = serialize_list(&list);
        assert_eq!(output, "# Work\n\n- [ ] Task A [45m] @code\n- [x] Task B\n");
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

        let mut list_a = TodoList::new("Alpha");
        list_a.items.push(TodoItem {
            title: "Task A".to_string(),
            done: false,
            tags: vec!["tag1".to_string()],
            time_secs: 0,
        });
        let mut list_b = TodoList::new("Beta");
        list_b.items.push(TodoItem {
            title: "Task B".to_string(),
            done: true,
            tags: vec![],
            time_secs: 0,
        });

        save_all(&dir, &[list_a.clone(), list_b.clone()]).unwrap();
        let loaded = load_lists(&dir).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "Alpha");
        assert_eq!(loaded[1].name, "Beta");
        assert_eq!(loaded[0], list_a);
        assert_eq!(loaded[1], list_b);
    }

    #[test]
    fn test_save_and_load_order() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        fs::create_dir_all(&dir).unwrap();

        let list_a = TodoList::new("Alpha");
        let list_b = TodoList::new("Beta");
        let list_c = TodoList::new("Gamma");

        save_all(&dir, &[list_a, list_b, list_c]).unwrap();

        let loaded = load_lists(&dir).unwrap();
        assert_eq!(loaded[0].name, "Alpha");
        assert_eq!(loaded[1].name, "Beta");
        assert_eq!(loaded[2].name, "Gamma");

        let reordered = vec![
            TodoList::new("Gamma"),
            TodoList::new("Alpha"),
            TodoList::new("Beta"),
        ];
        save_order(&dir, &reordered).unwrap();

        let loaded = load_lists(&dir).unwrap();
        assert_eq!(loaded[0].name, "Gamma");
        assert_eq!(loaded[1].name, "Alpha");
        assert_eq!(loaded[2].name, "Beta");
    }

    #[test]
    fn test_acquire_lock_creates_lock_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        acquire_lock(&dir).unwrap();
        let lock_path = dir.join(".lock");
        assert!(lock_path.exists());
        let contents = fs::read_to_string(&lock_path).unwrap();
        assert_eq!(contents.trim(), std::process::id().to_string());
    }

    #[test]
    fn test_acquire_lock_fails_if_pid_alive() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".lock"), std::process::id().to_string()).unwrap();
        let result = acquire_lock(&dir);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
    }

    #[test]
    fn test_acquire_lock_clears_stale_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".lock"), "999999999").unwrap();
        acquire_lock(&dir).unwrap();
        let contents = fs::read_to_string(dir.join(".lock")).unwrap();
        assert_eq!(contents.trim(), std::process::id().to_string());
    }

    #[test]
    fn test_release_lock_removes_lock_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        acquire_lock(&dir).unwrap();
        assert!(dir.join(".lock").exists());
        release_lock(&dir);
        assert!(!dir.join(".lock").exists());
    }

    #[test]
    fn test_release_lock_only_removes_own_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".lock"), "12345").unwrap();
        release_lock(&dir);
        assert!(dir.join(".lock").exists());
    }

    #[test]
    fn test_serialize_daily_list() {
        let mut list = TodoList::new("Daily Tasks");
        list.list_type = ListType::Daily;
        list.last_reset = Some("2026-02-26".to_string());
        list.items.push(TodoItem {
            title: "Exercise".to_string(),
            done: false,
            tags: vec![],
            time_secs: 0,
        });
        let output = serialize_list(&list);
        let reparsed = parse_list(&output);
        assert_eq!(reparsed.list_type, ListType::Daily);
        assert_eq!(reparsed.last_reset, Some("2026-02-26".to_string()));
        assert_eq!(reparsed.items.len(), 1);
        assert_eq!(reparsed.items[0].title, "Exercise");
    }

    #[test]
    fn test_parse_daily_list() {
        let md = "# Habits\n<!-- type: daily -->\n<!-- last-reset: 2026-02-25 -->\n\n- [ ] Meditate\n- [x] Read\n";
        let list = parse_list(md);
        assert_eq!(list.name, "Habits");
        assert_eq!(list.list_type, ListType::Daily);
        assert_eq!(list.last_reset, Some("2026-02-25".to_string()));
        assert_eq!(list.items.len(), 2);
    }

    #[test]
    fn test_parse_normal_list_unchanged() {
        let md = "# Work\n\n- [ ] Task A\n- [x] Task B\n";
        let list = parse_list(md);
        assert_eq!(list.list_type, ListType::Normal);
        assert_eq!(list.last_reset, None);
        assert_eq!(list.items.len(), 2);
    }

    #[test]
    fn test_list_contexts_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let contexts = list_contexts(tmp.path());
        assert!(contexts.is_empty());
    }

    #[test]
    fn test_list_contexts_finds_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("work")).unwrap();
        fs::create_dir(tmp.path().join("home")).unwrap();
        fs::create_dir(tmp.path().join(".hidden")).unwrap();
        fs::write(tmp.path().join("file.txt"), "not a dir").unwrap();
        let contexts = list_contexts(tmp.path());
        assert_eq!(contexts, vec!["home", "work"]);
    }

    #[test]
    fn test_migrate_to_contexts_moves_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("inbox.md"), "# Inbox\n").unwrap();
        fs::write(dir.join("work.md"), "# Work\n").unwrap();
        fs::write(dir.join(".order"), "inbox.md\nwork.md\n").unwrap();

        migrate_to_contexts(&dir).unwrap();

        assert!(!dir.join("inbox.md").exists());
        assert!(!dir.join("work.md").exists());
        assert!(!dir.join(".order").exists());
        assert!(dir.join("default").join("inbox.md").exists());
        assert!(dir.join("default").join("work.md").exists());
        assert!(dir.join("default").join(".order").exists());
    }

    #[test]
    fn test_migrate_to_contexts_noop_with_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        fs::create_dir_all(dir.join("existing")).unwrap();
        fs::write(dir.join("stray.md"), "# Stray\n").unwrap();

        migrate_to_contexts(&dir).unwrap();

        assert!(dir.join("stray.md").exists());
        assert!(!dir.join("default").exists());
    }

    #[test]
    fn test_migrate_to_contexts_noop_no_md_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("todos");
        fs::create_dir_all(&dir).unwrap();

        migrate_to_contexts(&dir).unwrap();

        assert!(!dir.join("default").exists());
    }

    #[test]
    fn test_save_load_last_context() {
        let tmp = tempfile::tempdir().unwrap();
        save_last_context(tmp.path(), "work").unwrap();
        assert_eq!(load_last_context(tmp.path()), Some("work".to_string()));
    }

    #[test]
    fn test_load_last_context_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(load_last_context(tmp.path()), None);
    }

    #[test]
    fn test_create_context_dir() {
        let tmp = tempfile::tempdir().unwrap();
        create_context_dir(tmp.path(), "work").unwrap();
        let work_dir = tmp.path().join("work");
        assert!(work_dir.exists());
        assert!(work_dir.join("inbox.md").exists());
    }

    #[test]
    fn test_append_to_archive_creates_dir_and_file() {
        let tmp = tempfile::tempdir().unwrap();
        let context_dir = tmp.path().join("ctx");
        fs::create_dir_all(&context_dir).unwrap();

        let items = vec![TodoItem {
            title: "Done task".to_string(),
            done: true,
            tags: vec!["work".to_string()],
            time_secs: 2700,
        }];

        append_to_archive(&context_dir, "work", &items).unwrap();

        let archive_path = context_dir.join(".archive").join("work.md");
        assert!(archive_path.exists());
        let content = fs::read_to_string(&archive_path).unwrap();
        assert!(content.contains("- [x] Done task [45m] @work"));
    }

    #[test]
    fn test_append_to_archive_appends_to_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let context_dir = tmp.path().join("ctx");
        let archive_dir = context_dir.join(".archive");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("inbox.md"), "- [x] Old task\n").unwrap();

        let items = vec![TodoItem {
            title: "New task".to_string(),
            done: true,
            tags: vec![],
            time_secs: 0,
        }];

        append_to_archive(&context_dir, "inbox", &items).unwrap();

        let content = fs::read_to_string(archive_dir.join("inbox.md")).unwrap();
        assert!(content.contains("- [x] Old task"));
        assert!(content.contains("- [x] New task"));
    }

    #[test]
    fn test_append_to_archive_empty_items_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let context_dir = tmp.path().join("ctx");
        fs::create_dir_all(&context_dir).unwrap();

        append_to_archive(&context_dir, "work", &[]).unwrap();

        assert!(!context_dir.join(".archive").exists());
    }
}
