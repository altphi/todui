use std::collections::HashMap;
use std::path::Path;

use crossterm::event::{KeyCode, KeyModifiers};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Quit,
    TogglePane,
    MoveUp,
    MoveDown,
    Undo,
    Redo,
    PageDown,
    PageUp,
    JumpToFirst,
    JumpToLast,
    SwitchContext,
    ToggleDone,
    EditItem,
    EditTags,
    ToggleSelect,
    Delete,
    MoveToList,
    MoveItemUp,
    MoveItemDown,
    MoveItemToTop,
    MoveItemToBottom,
    ToggleShowDone,
    StartFocus,
    StartFilter,
    EditTime,
    StartArchive,
    AddItem,
    ClearSelection,
    ToggleTag(String),
    ToggleListType,
    AddList,
    RenameList,
    DeleteList,
    MoveListUp,
    MoveListDown,
    MoveListToTop,
    MoveListToBottom,
    Confirm,
    Cancel,
    Toggle,
    Stop,
    TogglePause,
    PrevResult,
    NextResult,
    Autocomplete,
    Yes,
    No,
}

pub fn parse_key(input: &str) -> Option<(KeyModifiers, KeyCode)> {
    let parts: Vec<&str> = input.split('+').collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers = KeyModifiers::NONE;
    let key_part = parts.last()?;

    for &part in &parts[..parts.len() - 1] {
        match part.to_lowercase().as_str() {
            "alt" => modifiers |= KeyModifiers::ALT,
            "ctrl" => modifiers |= KeyModifiers::CONTROL,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            "super" => modifiers |= KeyModifiers::SUPER,
            _ => return None,
        }
    }

    let code = match *key_part {
        "enter" | "Enter" => KeyCode::Enter,
        "space" | "Space" => KeyCode::Char(' '),
        "tab" | "Tab" => KeyCode::Tab,
        "esc" | "Esc" => KeyCode::Esc,
        "up" | "Up" => KeyCode::Up,
        "down" | "Down" => KeyCode::Down,
        "left" | "Left" => KeyCode::Left,
        "right" | "Right" => KeyCode::Right,
        "home" | "Home" => KeyCode::Home,
        "end" | "End" => KeyCode::End,
        "delete" | "Delete" => KeyCode::Delete,
        "backspace" | "Backspace" => KeyCode::Backspace,
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            if c.is_ascii_alphanumeric() {
                KeyCode::Char(c)
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some((modifiers, code))
}

type KeyMap = HashMap<(KeyModifiers, KeyCode), Action>;

#[derive(Debug, PartialEq)]
pub struct KeyConfig {
    pub normal: KeyMap,
    pub normal_main: KeyMap,
    pub normal_sidebar: KeyMap,
    pub search: KeyMap,
    pub focus: KeyMap,
    pub filter: KeyMap,
    pub input: KeyMap,
    pub confirm: KeyMap,
    pub move_to_list: KeyMap,
    pub context_switch: KeyMap,
}

impl Default for KeyConfig {
    fn default() -> Self {
        let mut normal = KeyMap::new();
        normal.insert((KeyModifiers::ALT, KeyCode::Char('Q')), Action::Quit);
        normal.insert((KeyModifiers::ALT, KeyCode::Char('q')), Action::Quit);
        normal.insert((KeyModifiers::CONTROL, KeyCode::Char('c')), Action::Quit);
        normal.insert((KeyModifiers::NONE, KeyCode::Tab), Action::TogglePane);
        normal.insert((KeyModifiers::NONE, KeyCode::Up), Action::MoveUp);
        normal.insert((KeyModifiers::NONE, KeyCode::Down), Action::MoveDown);
        normal.insert((KeyModifiers::ALT, KeyCode::Char('U')), Action::Undo);
        normal.insert((KeyModifiers::ALT, KeyCode::Char('u')), Action::Undo);
        normal.insert((KeyModifiers::CONTROL, KeyCode::Char('z')), Action::Undo);
        normal.insert((KeyModifiers::CONTROL, KeyCode::Char('y')), Action::Redo);
        normal.insert(
            (KeyModifiers::CONTROL, KeyCode::Char('d')),
            Action::PageDown,
        );
        normal.insert((KeyModifiers::CONTROL, KeyCode::Char('u')), Action::PageUp);
        normal.insert((KeyModifiers::NONE, KeyCode::Home), Action::JumpToFirst);
        normal.insert((KeyModifiers::NONE, KeyCode::End), Action::JumpToLast);
        normal.insert(
            (KeyModifiers::ALT, KeyCode::Char('c')),
            Action::SwitchContext,
        );

        let mut normal_main = KeyMap::new();
        normal_main.insert((KeyModifiers::ALT, KeyCode::Enter), Action::ToggleDone);
        normal_main.insert((KeyModifiers::NONE, KeyCode::Enter), Action::EditItem);
        normal_main.insert((KeyModifiers::ALT, KeyCode::Char('t')), Action::EditTags);
        normal_main.insert(
            (KeyModifiers::ALT, KeyCode::Char('X')),
            Action::ToggleSelect,
        );
        normal_main.insert(
            (KeyModifiers::ALT, KeyCode::Char('x')),
            Action::ToggleSelect,
        );
        normal_main.insert((KeyModifiers::NONE, KeyCode::Delete), Action::Delete);
        normal_main.insert((KeyModifiers::NONE, KeyCode::Backspace), Action::Delete);
        normal_main.insert((KeyModifiers::ALT, KeyCode::Char('M')), Action::MoveToList);
        normal_main.insert((KeyModifiers::ALT, KeyCode::Char('m')), Action::MoveToList);
        normal_main.insert((KeyModifiers::SHIFT, KeyCode::Up), Action::MoveItemUp);
        normal_main.insert((KeyModifiers::ALT, KeyCode::Up), Action::MoveItemUp);
        normal_main.insert((KeyModifiers::SHIFT, KeyCode::Down), Action::MoveItemDown);
        normal_main.insert((KeyModifiers::ALT, KeyCode::Down), Action::MoveItemDown);
        normal_main.insert(
            (KeyModifiers::ALT | KeyModifiers::SUPER, KeyCode::Up),
            Action::MoveItemToTop,
        );
        normal_main.insert(
            (KeyModifiers::ALT | KeyModifiers::SUPER, KeyCode::Down),
            Action::MoveItemToBottom,
        );
        normal_main.insert(
            (KeyModifiers::ALT, KeyCode::Char('D')),
            Action::ToggleShowDone,
        );
        normal_main.insert(
            (KeyModifiers::ALT | KeyModifiers::SHIFT, KeyCode::Char('D')),
            Action::ToggleShowDone,
        );
        normal_main.insert((KeyModifiers::ALT, KeyCode::Char('f')), Action::StartFocus);
        normal_main.insert((KeyModifiers::ALT, KeyCode::Char('F')), Action::StartFilter);
        normal_main.insert(
            (KeyModifiers::ALT | KeyModifiers::SHIFT, KeyCode::Char('F')),
            Action::StartFilter,
        );
        normal_main.insert((KeyModifiers::ALT, KeyCode::Char('T')), Action::EditTime);
        normal_main.insert(
            (KeyModifiers::ALT | KeyModifiers::SHIFT, KeyCode::Char('T')),
            Action::EditTime,
        );
        normal_main.insert(
            (KeyModifiers::ALT, KeyCode::Char('a')),
            Action::StartArchive,
        );
        normal_main.insert((KeyModifiers::NONE, KeyCode::Char(' ')), Action::AddItem);
        normal_main.insert((KeyModifiers::NONE, KeyCode::Esc), Action::ClearSelection);

        let mut normal_sidebar = KeyMap::new();
        normal_sidebar.insert(
            (KeyModifiers::ALT, KeyCode::Char('D')),
            Action::ToggleListType,
        );
        normal_sidebar.insert(
            (KeyModifiers::ALT, KeyCode::Char('d')),
            Action::ToggleListType,
        );
        normal_sidebar.insert((KeyModifiers::ALT, KeyCode::Char('N')), Action::AddList);
        normal_sidebar.insert((KeyModifiers::ALT, KeyCode::Char('n')), Action::AddList);
        normal_sidebar.insert((KeyModifiers::NONE, KeyCode::Enter), Action::RenameList);
        normal_sidebar.insert((KeyModifiers::NONE, KeyCode::Delete), Action::DeleteList);
        normal_sidebar.insert((KeyModifiers::NONE, KeyCode::Backspace), Action::DeleteList);
        normal_sidebar.insert((KeyModifiers::SHIFT, KeyCode::Up), Action::MoveListUp);
        normal_sidebar.insert((KeyModifiers::ALT, KeyCode::Up), Action::MoveListUp);
        normal_sidebar.insert((KeyModifiers::SHIFT, KeyCode::Down), Action::MoveListDown);
        normal_sidebar.insert((KeyModifiers::ALT, KeyCode::Down), Action::MoveListDown);
        normal_sidebar.insert(
            (KeyModifiers::ALT | KeyModifiers::SUPER, KeyCode::Up),
            Action::MoveListToTop,
        );
        normal_sidebar.insert(
            (KeyModifiers::ALT | KeyModifiers::SUPER, KeyCode::Down),
            Action::MoveListToBottom,
        );

        let mut search = KeyMap::new();
        search.insert((KeyModifiers::NONE, KeyCode::Enter), Action::Confirm);
        search.insert((KeyModifiers::NONE, KeyCode::Esc), Action::Cancel);
        search.insert((KeyModifiers::NONE, KeyCode::Up), Action::PrevResult);
        search.insert((KeyModifiers::NONE, KeyCode::Down), Action::NextResult);

        let mut focus = KeyMap::new();
        focus.insert((KeyModifiers::NONE, KeyCode::Esc), Action::Stop);
        focus.insert(
            (KeyModifiers::NONE, KeyCode::Char(' ')),
            Action::TogglePause,
        );

        let mut filter = KeyMap::new();
        filter.insert((KeyModifiers::NONE, KeyCode::Enter), Action::Confirm);
        filter.insert((KeyModifiers::NONE, KeyCode::Esc), Action::Cancel);
        filter.insert((KeyModifiers::NONE, KeyCode::Char(' ')), Action::Toggle);
        filter.insert((KeyModifiers::NONE, KeyCode::Up), Action::MoveUp);
        filter.insert((KeyModifiers::NONE, KeyCode::Down), Action::MoveDown);

        let mut input = KeyMap::new();
        input.insert((KeyModifiers::NONE, KeyCode::Enter), Action::Confirm);
        input.insert((KeyModifiers::NONE, KeyCode::Esc), Action::Cancel);
        input.insert((KeyModifiers::NONE, KeyCode::Tab), Action::Autocomplete);

        let mut confirm = KeyMap::new();
        confirm.insert((KeyModifiers::NONE, KeyCode::Char('y')), Action::Yes);
        confirm.insert((KeyModifiers::NONE, KeyCode::Char('n')), Action::No);
        confirm.insert((KeyModifiers::NONE, KeyCode::Esc), Action::No);

        let mut move_to_list = KeyMap::new();
        move_to_list.insert((KeyModifiers::NONE, KeyCode::Enter), Action::Confirm);
        move_to_list.insert((KeyModifiers::NONE, KeyCode::Esc), Action::Cancel);
        move_to_list.insert((KeyModifiers::NONE, KeyCode::Up), Action::MoveUp);
        move_to_list.insert((KeyModifiers::NONE, KeyCode::Down), Action::MoveDown);

        let mut context_switch = KeyMap::new();
        context_switch.insert((KeyModifiers::NONE, KeyCode::Enter), Action::Confirm);
        context_switch.insert((KeyModifiers::NONE, KeyCode::Esc), Action::Cancel);
        context_switch.insert((KeyModifiers::NONE, KeyCode::Up), Action::MoveUp);
        context_switch.insert((KeyModifiers::NONE, KeyCode::Down), Action::MoveDown);

        Self {
            normal,
            normal_main,
            normal_sidebar,
            search,
            focus,
            filter,
            input,
            confirm,
            move_to_list,
            context_switch,
        }
    }
}

fn parse_action(value: &toml::Value) -> Option<Action> {
    match value {
        toml::Value::String(s) => match s.as_str() {
            "quit" => Some(Action::Quit),
            "toggle_pane" => Some(Action::TogglePane),
            "move_up" => Some(Action::MoveUp),
            "move_down" => Some(Action::MoveDown),
            "undo" => Some(Action::Undo),
            "redo" => Some(Action::Redo),
            "page_down" => Some(Action::PageDown),
            "page_up" => Some(Action::PageUp),
            "jump_to_first" => Some(Action::JumpToFirst),
            "jump_to_last" => Some(Action::JumpToLast),
            "switch_context" => Some(Action::SwitchContext),
            "toggle_done" => Some(Action::ToggleDone),
            "edit_item" => Some(Action::EditItem),
            "edit_tags" => Some(Action::EditTags),
            "toggle_select" => Some(Action::ToggleSelect),
            "delete" => Some(Action::Delete),
            "move_to_list" => Some(Action::MoveToList),
            "move_item_up" => Some(Action::MoveItemUp),
            "move_item_down" => Some(Action::MoveItemDown),
            "move_item_to_top" => Some(Action::MoveItemToTop),
            "move_item_to_bottom" => Some(Action::MoveItemToBottom),
            "toggle_show_done" => Some(Action::ToggleShowDone),
            "start_focus" => Some(Action::StartFocus),
            "start_filter" => Some(Action::StartFilter),
            "edit_time" => Some(Action::EditTime),
            "start_archive" => Some(Action::StartArchive),
            "add_item" => Some(Action::AddItem),
            "clear_selection" => Some(Action::ClearSelection),
            "toggle_list_type" => Some(Action::ToggleListType),
            "add_list" => Some(Action::AddList),
            "rename_list" => Some(Action::RenameList),
            "delete_list" => Some(Action::DeleteList),
            "move_list_up" => Some(Action::MoveListUp),
            "move_list_down" => Some(Action::MoveListDown),
            "move_list_to_top" => Some(Action::MoveListToTop),
            "move_list_to_bottom" => Some(Action::MoveListToBottom),
            "confirm" => Some(Action::Confirm),
            "cancel" => Some(Action::Cancel),
            "toggle" => Some(Action::Toggle),
            "stop" => Some(Action::Stop),
            "toggle_pause" => Some(Action::TogglePause),
            "prev_result" => Some(Action::PrevResult),
            "next_result" => Some(Action::NextResult),
            "autocomplete" => Some(Action::Autocomplete),
            "yes" => Some(Action::Yes),
            "no" => Some(Action::No),
            _ => None,
        },
        toml::Value::Table(table) => {
            let action_str = table.get("action")?.as_str()?;
            match action_str {
                "toggle_tag" => {
                    let tag = table.get("tag")?.as_str()?;
                    Some(Action::ToggleTag(tag.to_string()))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn merge_section(target: &mut KeyMap, section: &toml::value::Table) {
    for (key_str, value) in section {
        if let Some((modifiers, code)) = parse_key(key_str)
            && let Some(action) = parse_action(value)
        {
            target.insert((modifiers, code), action);
        }
    }
}

impl KeyConfig {
    pub fn from_toml(toml_str: &str) -> Self {
        let mut config = Self::default();

        let table: toml::value::Table = match toml_str.parse() {
            Ok(t) => t,
            Err(_) => return config,
        };

        let keys = match table.get("keys") {
            Some(toml::Value::Table(k)) => k,
            _ => return config,
        };

        fn merge_if_present(keymap: &mut KeyMap, keys: &toml::value::Table, name: &str) {
            if let Some(toml::Value::Table(section)) = keys.get(name) {
                merge_section(keymap, section);
            }
        }

        merge_if_present(&mut config.normal, keys, "normal");
        merge_if_present(&mut config.normal_main, keys, "normal_main");
        merge_if_present(&mut config.normal_sidebar, keys, "normal_sidebar");
        merge_if_present(&mut config.search, keys, "search");
        merge_if_present(&mut config.focus, keys, "focus");
        merge_if_present(&mut config.filter, keys, "filter");
        merge_if_present(&mut config.input, keys, "input");
        merge_if_present(&mut config.confirm, keys, "confirm");
        merge_if_present(&mut config.move_to_list, keys, "move_to_list");
        merge_if_present(&mut config.context_switch, keys, "context_switch");

        config
    }

    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join("config.toml");
        match std::fs::read_to_string(&path) {
            Ok(contents) => Self::from_toml(&contents),
            Err(_) => Self::default(),
        }
    }

    pub fn action_keys(&self, keymaps: &[&KeyMap], action: &Action) -> Option<String> {
        let mut display_keys: Vec<String> = Vec::new();
        for keymap in keymaps {
            for ((modifiers, code), act) in keymap.iter() {
                if act == action {
                    let display = format_key_display(*modifiers, *code);
                    if !display_keys.contains(&display) {
                        display_keys.push(display);
                    }
                }
            }
        }
        display_keys.sort_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)));
        display_keys.dedup();
        display_keys.truncate(2);
        if display_keys.is_empty() {
            None
        } else {
            Some(display_keys.join(" / "))
        }
    }

    pub fn toggle_tag_entries(&self) -> Vec<(String, String)> {
        let mut entries: Vec<(String, String)> = self
            .normal_main
            .iter()
            .filter_map(|((modifiers, code), action)| {
                if let Action::ToggleTag(tag) = action {
                    Some((format_key_display(*modifiers, *code), tag.clone()))
                } else {
                    None
                }
            })
            .collect();
        entries.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then(a.0.cmp(&b.0)));
        entries
    }
}

fn format_key_display(modifiers: KeyModifiers, code: KeyCode) -> String {
    let mut parts: Vec<String> = Vec::new();

    if modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl".into());
    }
    if modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt".into());
    }
    if modifiers.contains(KeyModifiers::SUPER) {
        parts.push("Super".into());
    }

    let (add_shift, key_str) = match code {
        KeyCode::Char(' ') => (false, "Space".into()),
        KeyCode::Char(c) if c.is_uppercase() => {
            (!modifiers.contains(KeyModifiers::SHIFT), c.to_string())
        }
        KeyCode::Char(c) => (false, c.to_uppercase().to_string()),
        KeyCode::Enter => (false, "Enter".into()),
        KeyCode::Tab => (false, "Tab".into()),
        KeyCode::Esc => (false, "Esc".into()),
        KeyCode::Up => (modifiers.contains(KeyModifiers::SHIFT), "\u{2191}".into()),
        KeyCode::Down => (modifiers.contains(KeyModifiers::SHIFT), "\u{2193}".into()),
        KeyCode::Left => (modifiers.contains(KeyModifiers::SHIFT), "\u{2190}".into()),
        KeyCode::Right => (modifiers.contains(KeyModifiers::SHIFT), "\u{2192}".into()),
        KeyCode::Home => (false, "Home".into()),
        KeyCode::End => (false, "End".into()),
        KeyCode::Delete => (false, "Del".into()),
        KeyCode::Backspace => (false, "Backspace".into()),
        _ => (false, format!("{:?}", code)),
    };

    if add_shift {
        parts.push("Shift".into());
    }
    parts.push(key_str);

    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_key_simple_named_keys() {
        assert_eq!(
            parse_key("enter"),
            Some((KeyModifiers::NONE, KeyCode::Enter))
        );
        assert_eq!(
            parse_key("space"),
            Some((KeyModifiers::NONE, KeyCode::Char(' ')))
        );
        assert_eq!(parse_key("tab"), Some((KeyModifiers::NONE, KeyCode::Tab)));
        assert_eq!(parse_key("esc"), Some((KeyModifiers::NONE, KeyCode::Esc)));
        assert_eq!(parse_key("up"), Some((KeyModifiers::NONE, KeyCode::Up)));
        assert_eq!(parse_key("down"), Some((KeyModifiers::NONE, KeyCode::Down)));
        assert_eq!(parse_key("left"), Some((KeyModifiers::NONE, KeyCode::Left)));
        assert_eq!(
            parse_key("right"),
            Some((KeyModifiers::NONE, KeyCode::Right))
        );
        assert_eq!(parse_key("home"), Some((KeyModifiers::NONE, KeyCode::Home)));
        assert_eq!(parse_key("end"), Some((KeyModifiers::NONE, KeyCode::End)));
        assert_eq!(
            parse_key("delete"),
            Some((KeyModifiers::NONE, KeyCode::Delete))
        );
        assert_eq!(
            parse_key("backspace"),
            Some((KeyModifiers::NONE, KeyCode::Backspace))
        );
    }

    #[test]
    fn parse_key_char_keys() {
        assert_eq!(
            parse_key("a"),
            Some((KeyModifiers::NONE, KeyCode::Char('a')))
        );
        assert_eq!(
            parse_key("z"),
            Some((KeyModifiers::NONE, KeyCode::Char('z')))
        );
        assert_eq!(
            parse_key("0"),
            Some((KeyModifiers::NONE, KeyCode::Char('0')))
        );
        assert_eq!(
            parse_key("9"),
            Some((KeyModifiers::NONE, KeyCode::Char('9')))
        );
    }

    #[test]
    fn parse_key_uppercase_chars() {
        assert_eq!(
            parse_key("A"),
            Some((KeyModifiers::NONE, KeyCode::Char('A')))
        );
        assert_eq!(
            parse_key("Z"),
            Some((KeyModifiers::NONE, KeyCode::Char('Z')))
        );
        assert_eq!(
            parse_key("D"),
            Some((KeyModifiers::NONE, KeyCode::Char('D')))
        );
    }

    #[test]
    fn parse_key_single_modifier() {
        assert_eq!(
            parse_key("alt+q"),
            Some((KeyModifiers::ALT, KeyCode::Char('q')))
        );
        assert_eq!(
            parse_key("ctrl+c"),
            Some((KeyModifiers::CONTROL, KeyCode::Char('c')))
        );
        assert_eq!(
            parse_key("shift+up"),
            Some((KeyModifiers::SHIFT, KeyCode::Up))
        );
        assert_eq!(
            parse_key("super+a"),
            Some((KeyModifiers::SUPER, KeyCode::Char('a')))
        );
    }

    #[test]
    fn parse_key_combined_modifiers() {
        assert_eq!(
            parse_key("alt+shift+up"),
            Some((KeyModifiers::ALT | KeyModifiers::SHIFT, KeyCode::Up))
        );
        assert_eq!(
            parse_key("alt+super+down"),
            Some((KeyModifiers::ALT | KeyModifiers::SUPER, KeyCode::Down))
        );
        assert_eq!(
            parse_key("ctrl+shift+a"),
            Some((
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                KeyCode::Char('a')
            ))
        );
    }

    #[test]
    fn parse_key_case_insensitive_modifiers() {
        assert_eq!(
            parse_key("Alt+q"),
            Some((KeyModifiers::ALT, KeyCode::Char('q')))
        );
        assert_eq!(
            parse_key("ALT+q"),
            Some((KeyModifiers::ALT, KeyCode::Char('q')))
        );
        assert_eq!(
            parse_key("Ctrl+c"),
            Some((KeyModifiers::CONTROL, KeyCode::Char('c')))
        );
        assert_eq!(
            parse_key("SHIFT+up"),
            Some((KeyModifiers::SHIFT, KeyCode::Up))
        );
    }

    #[test]
    fn parse_key_case_sensitive_letter_keys() {
        assert_eq!(
            parse_key("alt+q"),
            Some((KeyModifiers::ALT, KeyCode::Char('q')))
        );
        assert_eq!(
            parse_key("alt+Q"),
            Some((KeyModifiers::ALT, KeyCode::Char('Q')))
        );
        assert_ne!(parse_key("alt+q"), parse_key("alt+Q"));
    }

    #[test]
    fn parse_key_named_keys_case_variants() {
        assert_eq!(
            parse_key("Enter"),
            Some((KeyModifiers::NONE, KeyCode::Enter))
        );
        assert_eq!(
            parse_key("Space"),
            Some((KeyModifiers::NONE, KeyCode::Char(' ')))
        );
        assert_eq!(parse_key("Tab"), Some((KeyModifiers::NONE, KeyCode::Tab)));
        assert_eq!(parse_key("Esc"), Some((KeyModifiers::NONE, KeyCode::Esc)));
        assert_eq!(parse_key("Up"), Some((KeyModifiers::NONE, KeyCode::Up)));
        assert_eq!(parse_key("Down"), Some((KeyModifiers::NONE, KeyCode::Down)));
        assert_eq!(parse_key("Home"), Some((KeyModifiers::NONE, KeyCode::Home)));
        assert_eq!(parse_key("End"), Some((KeyModifiers::NONE, KeyCode::End)));
        assert_eq!(
            parse_key("Delete"),
            Some((KeyModifiers::NONE, KeyCode::Delete))
        );
        assert_eq!(
            parse_key("Backspace"),
            Some((KeyModifiers::NONE, KeyCode::Backspace))
        );
    }

    #[test]
    fn parse_key_invalid_input() {
        assert_eq!(parse_key(""), None);
        assert_eq!(parse_key("foo"), None);
        assert_eq!(parse_key("alt+"), None);
        assert_eq!(parse_key("notamod+a"), None);
        assert_eq!(parse_key("alt+foo"), None);
        assert_eq!(parse_key("alt+ab"), None);
    }

    #[test]
    fn default_normal_quit_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config.normal.get(&(KeyModifiers::ALT, KeyCode::Char('q'))),
            Some(&Action::Quit)
        );
        assert_eq!(
            config.normal.get(&(KeyModifiers::ALT, KeyCode::Char('Q'))),
            Some(&Action::Quit)
        );
        assert_eq!(
            config
                .normal
                .get(&(KeyModifiers::CONTROL, KeyCode::Char('c'))),
            Some(&Action::Quit)
        );
    }

    #[test]
    fn default_normal_navigation_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config.normal.get(&(KeyModifiers::NONE, KeyCode::Tab)),
            Some(&Action::TogglePane)
        );
        assert_eq!(
            config.normal.get(&(KeyModifiers::NONE, KeyCode::Up)),
            Some(&Action::MoveUp)
        );
        assert_eq!(
            config.normal.get(&(KeyModifiers::NONE, KeyCode::Down)),
            Some(&Action::MoveDown)
        );
        assert_eq!(
            config.normal.get(&(KeyModifiers::NONE, KeyCode::Home)),
            Some(&Action::JumpToFirst)
        );
        assert_eq!(
            config.normal.get(&(KeyModifiers::NONE, KeyCode::End)),
            Some(&Action::JumpToLast)
        );
    }

    #[test]
    fn default_normal_undo_redo_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config.normal.get(&(KeyModifiers::ALT, KeyCode::Char('u'))),
            Some(&Action::Undo)
        );
        assert_eq!(
            config.normal.get(&(KeyModifiers::ALT, KeyCode::Char('U'))),
            Some(&Action::Undo)
        );
        assert_eq!(
            config
                .normal
                .get(&(KeyModifiers::CONTROL, KeyCode::Char('z'))),
            Some(&Action::Undo)
        );
        assert_eq!(
            config
                .normal
                .get(&(KeyModifiers::CONTROL, KeyCode::Char('y'))),
            Some(&Action::Redo)
        );
    }

    #[test]
    fn default_normal_page_and_context_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config
                .normal
                .get(&(KeyModifiers::CONTROL, KeyCode::Char('d'))),
            Some(&Action::PageDown)
        );
        assert_eq!(
            config
                .normal
                .get(&(KeyModifiers::CONTROL, KeyCode::Char('u'))),
            Some(&Action::PageUp)
        );
        assert_eq!(
            config.normal.get(&(KeyModifiers::ALT, KeyCode::Char('c'))),
            Some(&Action::SwitchContext)
        );
    }

    #[test]
    fn default_normal_main_basic_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config.normal_main.get(&(KeyModifiers::ALT, KeyCode::Enter)),
            Some(&Action::ToggleDone)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::NONE, KeyCode::Enter)),
            Some(&Action::EditItem)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('t'))),
            Some(&Action::EditTags)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::NONE, KeyCode::Char(' '))),
            Some(&Action::AddItem)
        );
    }

    #[test]
    fn default_normal_main_select_delete_move() {
        let config = KeyConfig::default();
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('x'))),
            Some(&Action::ToggleSelect)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('X'))),
            Some(&Action::ToggleSelect)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::NONE, KeyCode::Delete)),
            Some(&Action::Delete)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::NONE, KeyCode::Backspace)),
            Some(&Action::Delete)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('m'))),
            Some(&Action::MoveToList)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('M'))),
            Some(&Action::MoveToList)
        );
    }

    #[test]
    fn default_normal_main_reorder_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config.normal_main.get(&(KeyModifiers::SHIFT, KeyCode::Up)),
            Some(&Action::MoveItemUp)
        );
        assert_eq!(
            config.normal_main.get(&(KeyModifiers::ALT, KeyCode::Up)),
            Some(&Action::MoveItemUp)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::SHIFT, KeyCode::Down)),
            Some(&Action::MoveItemDown)
        );
        assert_eq!(
            config.normal_main.get(&(KeyModifiers::ALT, KeyCode::Down)),
            Some(&Action::MoveItemDown)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT | KeyModifiers::SUPER, KeyCode::Up)),
            Some(&Action::MoveItemToTop)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT | KeyModifiers::SUPER, KeyCode::Down)),
            Some(&Action::MoveItemToBottom)
        );
    }

    #[test]
    fn default_normal_main_toggle_show_done_both_modifiers() {
        let config = KeyConfig::default();
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('D'))),
            Some(&Action::ToggleShowDone)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT | KeyModifiers::SHIFT, KeyCode::Char('D'))),
            Some(&Action::ToggleShowDone)
        );
    }

    #[test]
    fn default_normal_main_focus_filter_time_archive() {
        let config = KeyConfig::default();
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('f'))),
            Some(&Action::StartFocus)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('F'))),
            Some(&Action::StartFilter)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT | KeyModifiers::SHIFT, KeyCode::Char('F'))),
            Some(&Action::StartFilter)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('T'))),
            Some(&Action::EditTime)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT | KeyModifiers::SHIFT, KeyCode::Char('T'))),
            Some(&Action::EditTime)
        );
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('a'))),
            Some(&Action::StartArchive)
        );
    }

    #[test]
    fn default_normal_sidebar_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::ALT, KeyCode::Char('d'))),
            Some(&Action::ToggleListType)
        );
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::ALT, KeyCode::Char('D'))),
            Some(&Action::ToggleListType)
        );
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::ALT, KeyCode::Char('n'))),
            Some(&Action::AddList)
        );
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::ALT, KeyCode::Char('N'))),
            Some(&Action::AddList)
        );
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::NONE, KeyCode::Enter)),
            Some(&Action::RenameList)
        );
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::NONE, KeyCode::Delete)),
            Some(&Action::DeleteList)
        );
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::NONE, KeyCode::Backspace)),
            Some(&Action::DeleteList)
        );
    }

    #[test]
    fn default_normal_sidebar_reorder_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::SHIFT, KeyCode::Up)),
            Some(&Action::MoveListUp)
        );
        assert_eq!(
            config.normal_sidebar.get(&(KeyModifiers::ALT, KeyCode::Up)),
            Some(&Action::MoveListUp)
        );
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::SHIFT, KeyCode::Down)),
            Some(&Action::MoveListDown)
        );
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::ALT, KeyCode::Down)),
            Some(&Action::MoveListDown)
        );
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::ALT | KeyModifiers::SUPER, KeyCode::Up)),
            Some(&Action::MoveListToTop)
        );
        assert_eq!(
            config
                .normal_sidebar
                .get(&(KeyModifiers::ALT | KeyModifiers::SUPER, KeyCode::Down)),
            Some(&Action::MoveListToBottom)
        );
    }

    #[test]
    fn default_search_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config.search.get(&(KeyModifiers::NONE, KeyCode::Enter)),
            Some(&Action::Confirm)
        );
        assert_eq!(
            config.search.get(&(KeyModifiers::NONE, KeyCode::Esc)),
            Some(&Action::Cancel)
        );
        assert_eq!(
            config.search.get(&(KeyModifiers::NONE, KeyCode::Up)),
            Some(&Action::PrevResult)
        );
        assert_eq!(
            config.search.get(&(KeyModifiers::NONE, KeyCode::Down)),
            Some(&Action::NextResult)
        );
    }

    #[test]
    fn default_focus_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config.focus.get(&(KeyModifiers::NONE, KeyCode::Esc)),
            Some(&Action::Stop)
        );
        assert_eq!(
            config.focus.get(&(KeyModifiers::NONE, KeyCode::Char(' '))),
            Some(&Action::TogglePause)
        );
    }

    #[test]
    fn default_filter_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config.filter.get(&(KeyModifiers::NONE, KeyCode::Enter)),
            Some(&Action::Confirm)
        );
        assert_eq!(
            config.filter.get(&(KeyModifiers::NONE, KeyCode::Esc)),
            Some(&Action::Cancel)
        );
        assert_eq!(
            config.filter.get(&(KeyModifiers::NONE, KeyCode::Char(' '))),
            Some(&Action::Toggle)
        );
        assert_eq!(
            config.filter.get(&(KeyModifiers::NONE, KeyCode::Up)),
            Some(&Action::MoveUp)
        );
        assert_eq!(
            config.filter.get(&(KeyModifiers::NONE, KeyCode::Down)),
            Some(&Action::MoveDown)
        );
    }

    #[test]
    fn default_input_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config.input.get(&(KeyModifiers::NONE, KeyCode::Enter)),
            Some(&Action::Confirm)
        );
        assert_eq!(
            config.input.get(&(KeyModifiers::NONE, KeyCode::Esc)),
            Some(&Action::Cancel)
        );
        assert_eq!(
            config.input.get(&(KeyModifiers::NONE, KeyCode::Tab)),
            Some(&Action::Autocomplete)
        );
    }

    #[test]
    fn default_confirm_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config
                .confirm
                .get(&(KeyModifiers::NONE, KeyCode::Char('y'))),
            Some(&Action::Yes)
        );
        assert_eq!(
            config
                .confirm
                .get(&(KeyModifiers::NONE, KeyCode::Char('n'))),
            Some(&Action::No)
        );
        assert_eq!(
            config.confirm.get(&(KeyModifiers::NONE, KeyCode::Esc)),
            Some(&Action::No)
        );
    }

    #[test]
    fn default_move_to_list_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config
                .move_to_list
                .get(&(KeyModifiers::NONE, KeyCode::Enter)),
            Some(&Action::Confirm)
        );
        assert_eq!(
            config.move_to_list.get(&(KeyModifiers::NONE, KeyCode::Esc)),
            Some(&Action::Cancel)
        );
        assert_eq!(
            config.move_to_list.get(&(KeyModifiers::NONE, KeyCode::Up)),
            Some(&Action::MoveUp)
        );
        assert_eq!(
            config
                .move_to_list
                .get(&(KeyModifiers::NONE, KeyCode::Down)),
            Some(&Action::MoveDown)
        );
    }

    #[test]
    fn default_context_switch_keys() {
        let config = KeyConfig::default();
        assert_eq!(
            config
                .context_switch
                .get(&(KeyModifiers::NONE, KeyCode::Enter)),
            Some(&Action::Confirm)
        );
        assert_eq!(
            config
                .context_switch
                .get(&(KeyModifiers::NONE, KeyCode::Esc)),
            Some(&Action::Cancel)
        );
        assert_eq!(
            config
                .context_switch
                .get(&(KeyModifiers::NONE, KeyCode::Up)),
            Some(&Action::MoveUp)
        );
        assert_eq!(
            config
                .context_switch
                .get(&(KeyModifiers::NONE, KeyCode::Down)),
            Some(&Action::MoveDown)
        );
    }

    #[test]
    fn parse_action_simple_strings() {
        let val = toml::Value::String("quit".into());
        assert_eq!(parse_action(&val), Some(Action::Quit));

        let val = toml::Value::String("move_up".into());
        assert_eq!(parse_action(&val), Some(Action::MoveUp));

        let val = toml::Value::String("toggle_done".into());
        assert_eq!(parse_action(&val), Some(Action::ToggleDone));
    }

    #[test]
    fn parse_action_invalid_string() {
        let val = toml::Value::String("nonexistent".into());
        assert_eq!(parse_action(&val), None);
    }

    #[test]
    fn parse_action_toggle_tag_table() {
        let mut table = toml::value::Table::new();
        table.insert("action".into(), toml::Value::String("toggle_tag".into()));
        table.insert("tag".into(), toml::Value::String("starred".into()));
        let val = toml::Value::Table(table);
        assert_eq!(
            parse_action(&val),
            Some(Action::ToggleTag("starred".into()))
        );
    }

    #[test]
    fn parse_action_table_missing_tag() {
        let mut table = toml::value::Table::new();
        table.insert("action".into(), toml::Value::String("toggle_tag".into()));
        let val = toml::Value::Table(table);
        assert_eq!(parse_action(&val), None);
    }

    #[test]
    fn parse_action_unknown_table_action() {
        let mut table = toml::value::Table::new();
        table.insert("action".into(), toml::Value::String("unknown".into()));
        let val = toml::Value::Table(table);
        assert_eq!(parse_action(&val), None);
    }

    #[test]
    fn from_toml_empty_returns_defaults() {
        let config = KeyConfig::from_toml("");
        let defaults = KeyConfig::default();
        assert_eq!(config, defaults);
    }

    #[test]
    fn from_toml_override_single_key() {
        let toml_str = r#"
[keys.normal]
"alt+s" = "quit"
"#;
        let config = KeyConfig::from_toml(toml_str);
        assert_eq!(
            config.normal.get(&(KeyModifiers::ALT, KeyCode::Char('s'))),
            Some(&Action::Quit)
        );
        assert_eq!(
            config.normal.get(&(KeyModifiers::ALT, KeyCode::Char('q'))),
            Some(&Action::Quit)
        );
    }

    #[test]
    fn from_toml_toggle_tag_inline_table() {
        let toml_str = r#"
[keys.normal_main]
"alt+s" = { action = "toggle_tag", tag = "starred" }
"#;
        let config = KeyConfig::from_toml(toml_str);
        assert_eq!(
            config
                .normal_main
                .get(&(KeyModifiers::ALT, KeyCode::Char('s'))),
            Some(&Action::ToggleTag("starred".into()))
        );
    }

    #[test]
    fn from_toml_multiple_sections() {
        let toml_str = r#"
[keys.normal]
"alt+s" = "quit"

[keys.focus]
"alt+s" = "stop"
"#;
        let config = KeyConfig::from_toml(toml_str);
        assert_eq!(
            config.normal.get(&(KeyModifiers::ALT, KeyCode::Char('s'))),
            Some(&Action::Quit)
        );
        assert_eq!(
            config.focus.get(&(KeyModifiers::ALT, KeyCode::Char('s'))),
            Some(&Action::Stop)
        );
    }

    #[test]
    fn from_toml_preserves_unmodified_defaults() {
        let toml_str = r#"
[keys.normal]
"alt+s" = "quit"
"#;
        let config = KeyConfig::from_toml(toml_str);
        let defaults = KeyConfig::default();
        assert_eq!(config.normal_main, defaults.normal_main);
        assert_eq!(config.normal_sidebar, defaults.normal_sidebar);
        assert_eq!(config.search, defaults.search);
        assert_eq!(config.focus, defaults.focus);
        assert_eq!(config.filter, defaults.filter);
        assert_eq!(config.input, defaults.input);
        assert_eq!(config.confirm, defaults.confirm);
        assert_eq!(config.move_to_list, defaults.move_to_list);
        assert_eq!(config.context_switch, defaults.context_switch);
    }

    #[test]
    fn from_toml_invalid_toml_returns_defaults() {
        let config = KeyConfig::from_toml("not valid { toml");
        let defaults = KeyConfig::default();
        assert_eq!(config, defaults);
    }

    #[test]
    fn load_nonexistent_file_returns_defaults() {
        let config = KeyConfig::load(Path::new("/tmp/nonexistent_todui_test_dir_12345"));
        let defaults = KeyConfig::default();
        assert_eq!(config, defaults);
    }

    #[test]
    fn load_reads_file() {
        let dir = tempfile::tempdir().unwrap();
        let toml_str = r#"
[keys.normal]
"alt+s" = "quit"
"#;
        std::fs::write(dir.path().join("config.toml"), toml_str).unwrap();
        let config = KeyConfig::load(dir.path());
        assert_eq!(
            config.normal.get(&(KeyModifiers::ALT, KeyCode::Char('s'))),
            Some(&Action::Quit)
        );
    }

    #[test]
    fn format_key_display_simple_keys() {
        assert_eq!(format_key_display(KeyModifiers::NONE, KeyCode::Tab), "Tab");
        assert_eq!(
            format_key_display(KeyModifiers::NONE, KeyCode::Enter),
            "Enter"
        );
        assert_eq!(format_key_display(KeyModifiers::NONE, KeyCode::Esc), "Esc");
        assert_eq!(
            format_key_display(KeyModifiers::NONE, KeyCode::Char(' ')),
            "Space"
        );
        assert_eq!(
            format_key_display(KeyModifiers::NONE, KeyCode::Home),
            "Home"
        );
        assert_eq!(format_key_display(KeyModifiers::NONE, KeyCode::End), "End");
        assert_eq!(
            format_key_display(KeyModifiers::NONE, KeyCode::Delete),
            "Del"
        );
        assert_eq!(
            format_key_display(KeyModifiers::NONE, KeyCode::Backspace),
            "Backspace"
        );
    }

    #[test]
    fn format_key_display_arrows() {
        assert_eq!(
            format_key_display(KeyModifiers::NONE, KeyCode::Up),
            "\u{2191}"
        );
        assert_eq!(
            format_key_display(KeyModifiers::NONE, KeyCode::Down),
            "\u{2193}"
        );
        assert_eq!(
            format_key_display(KeyModifiers::SHIFT, KeyCode::Up),
            "Shift+\u{2191}"
        );
    }

    #[test]
    fn format_key_display_modifiers() {
        assert_eq!(
            format_key_display(KeyModifiers::ALT, KeyCode::Char('q')),
            "Alt+Q"
        );
        assert_eq!(
            format_key_display(KeyModifiers::CONTROL, KeyCode::Char('c')),
            "Ctrl+C"
        );
        assert_eq!(
            format_key_display(KeyModifiers::ALT, KeyCode::Char('Q')),
            "Alt+Shift+Q"
        );
        assert_eq!(
            format_key_display(KeyModifiers::ALT, KeyCode::Enter),
            "Alt+Enter"
        );
        assert_eq!(
            format_key_display(KeyModifiers::ALT | KeyModifiers::SUPER, KeyCode::Up),
            "Alt+Super+\u{2191}"
        );
    }

    #[test]
    fn action_keys_finds_bindings() {
        let config = KeyConfig::default();
        let keys = config.action_keys(&[&config.normal], &Action::TogglePane);
        assert_eq!(keys, Some("Tab".into()));
    }

    #[test]
    fn action_keys_returns_none_when_unbound() {
        let config = KeyConfig::default();
        let keys = config.action_keys(&[&config.normal], &Action::AddItem);
        assert_eq!(keys, None);
    }

    #[test]
    fn action_keys_multiple_bindings() {
        let config = KeyConfig::default();
        let keys = config
            .action_keys(&[&config.normal], &Action::Quit)
            .unwrap();
        assert!(keys.contains("Alt+Q"));
        assert!(keys.contains(" / "));
    }

    #[test]
    fn action_keys_custom_config() {
        let toml_str = r#"
[keys.normal_main]
"ctrl+d" = "delete"
"#;
        let config = KeyConfig::from_toml(toml_str);
        let keys = config
            .action_keys(&[&config.normal_main], &Action::Delete)
            .unwrap();
        assert!(keys.contains("Ctrl+D"));
    }

    #[test]
    fn toggle_tag_entries_from_config() {
        let toml_str = r#"
[keys.normal_main]
"alt+s" = { action = "toggle_tag", tag = "starred" }
"#;
        let config = KeyConfig::from_toml(toml_str);
        let entries = config.toggle_tag_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "Alt+S");
        assert_eq!(entries[0].1, "starred");
    }
}
