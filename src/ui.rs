use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Wrap},
};

use crate::app::App;
use crate::model::{InputMode, ListType, Pane, SearchResult, SidebarEntry};

pub fn render(app: &App, frame: &mut Frame) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_title_bar(frame, outer[0]);

    let longest_list_name = app
        .lists
        .iter()
        .map(|l| {
            let extra = if l.list_type == ListType::Daily { 4 } else { 0 };
            l.name.len() as u16 + extra
        })
        .max()
        .unwrap_or(4);
    let longest_tag_name = app
        .sidebar_entries
        .iter()
        .filter_map(|e| match e {
            SidebarEntry::Tag(name) => Some(name.len() as u16 + 1),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let sidebar_width = (longest_list_name.max(longest_tag_name) + 8).max(14);

    let main_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_width), Constraint::Min(0)])
        .split(outer[1]);

    render_sidebar(app, frame, main_area[0]);
    render_todo_pane(app, frame, main_area[1]);
    render_status_bar(app, frame, outer[2]);

    match app.input_mode {
        InputMode::ConfirmDelete => render_confirm_dialog(app, frame),
        InputMode::Searching => render_search_modal(app, frame),
        InputMode::Focused => render_focus_overlay(app, frame),
        InputMode::FilteringTags => render_filter_modal(app, frame),
        InputMode::MovingToList => render_move_to_list_modal(app, frame),
        InputMode::AddingItem
        | InputMode::EditingItem
        | InputMode::EditingTags
        | InputMode::AddingList
        | InputMode::RenamingList
        | InputMode::EditingTime => {
            render_input_modal(app, frame);
            if app.autocomplete_active {
                render_autocomplete_dropdown(app, frame);
            }
        }
        InputMode::Normal => {}
    }
}

fn render_title_bar(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![Span::styled(
        "  todui",
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )]))
    .style(Style::default().bg(Color::DarkGray));
    frame.render_widget(title, area);
}

fn render_sidebar(app: &App, frame: &mut Frame, area: Rect) {
    let is_active = app.active_pane == Pane::Sidebar;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title(" Lists ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .padding(Padding::new(1, 1, 0, 0));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let has_tags = app
        .sidebar_entries
        .iter()
        .any(|e| matches!(e, SidebarEntry::Tag(_)));
    let num_lists = app.lists.len();
    let divider_offset: usize = if has_tags { 1 } else { 0 };
    let total_rows = app.sidebar_entries.len() + divider_offset;

    let selected_visual_row = if has_tags && app.selected_sidebar_index >= num_lists {
        app.selected_sidebar_index + 1
    } else {
        app.selected_sidebar_index
    };

    let visible_height = inner.height as usize;
    let offset = if selected_visual_row >= visible_height {
        selected_visual_row - visible_height + 1
    } else {
        0
    };

    let mut items: Vec<ListItem> = Vec::new();
    for visual_row in offset..(offset + visible_height).min(total_rows) {
        if has_tags && visual_row == num_lists {
            let sep = "\u{2500}".repeat(inner.width as usize);
            items.push(ListItem::new(Line::from(Span::styled(
                sep,
                Style::default().fg(Color::DarkGray),
            ))));
            continue;
        }

        let entry_index = if has_tags && visual_row > num_lists {
            visual_row - 1
        } else {
            visual_row
        };

        if entry_index >= app.sidebar_entries.len() {
            break;
        }

        let is_selected = entry_index == app.selected_sidebar_index;
        match &app.sidebar_entries[entry_index] {
            SidebarEntry::List(i) => {
                let list = &app.lists[*i];
                let indicator = if list.list_type == ListType::Daily {
                    if app.ascii_mode { " (d)" } else { " \u{21bb}" }
                } else {
                    ""
                };
                let content = format!("  {}{}", list.name, indicator);
                let style = if is_selected {
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                items.push(ListItem::new(Line::from(Span::styled(content, style))));
            }
            SidebarEntry::Tag(name) => {
                let content = format!("  @{}", name);
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Blue)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Blue)
                };
                items.push(ListItem::new(Line::from(Span::styled(content, style))));
            }
        }
    }

    let list_widget = List::new(items);
    frame.render_widget(list_widget, inner);
}

fn render_todo_pane(app: &App, frame: &mut Frame, area: Rect) {
    let is_active = app.active_pane == Pane::Main;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = if let Some(tag) = app.selected_tag_name() {
        format!(" @{} ", tag)
    } else {
        app.current_list()
            .map(|l| format!(" {} ", l.name))
            .unwrap_or_else(|| " Todos ".to_string())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .padding(Padding::new(1, 1, 0, 0));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.is_tag_view() {
        let tag_items = app.tag_visible_items();
        if tag_items.is_empty() {
            let hint = Paragraph::new("No items with this tag.")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(hint, inner);
        } else {
            let visible_height = inner.height as usize;
            let offset = if app.selected_item_index >= visible_height {
                app.selected_item_index - visible_height + 1
            } else {
                0
            };

            let items: Vec<ListItem> = tag_items
                .iter()
                .enumerate()
                .skip(offset)
                .take(visible_height)
                .map(|(vi, &(li, ii))| {
                    let item = &app.lists[li].items[ii];
                    let list_name = &app.lists[li].name;
                    let is_cursor = is_active && vi == app.selected_item_index;

                    let checkbox = if app.ascii_mode {
                        if item.done { "[x]" } else { "[ ]" }
                    } else if item.done {
                        "\u{2714}"
                    } else {
                        "\u{2751}"
                    };

                    let base_style = if item.done {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default()
                    };

                    let bg_style = if is_cursor {
                        if item.done {
                            base_style.fg(Color::Gray).bg(Color::DarkGray)
                        } else {
                            base_style.bg(Color::DarkGray)
                        }
                    } else {
                        base_style
                    };

                    let prefix = format!("  {} ", checkbox);
                    let prefix_width = prefix.chars().count();

                    let time_text = if item.time_secs > 0 {
                        format!("  {}", crate::storage::format_time(item.time_secs))
                    } else {
                        String::new()
                    };

                    let tags_text = if !item.tags.is_empty() {
                        let tags_str = item
                            .tags
                            .iter()
                            .map(|t| format!("@{}", t))
                            .collect::<Vec<_>>()
                            .join(" ");
                        format!("  {}", tags_str)
                    } else {
                        String::new()
                    };

                    let list_suffix = format!("  \u{2014} {}", list_name);
                    let suffix_width = time_text.chars().count()
                        + tags_text.chars().count()
                        + list_suffix.chars().count();
                    let available =
                        (inner.width as usize).saturating_sub(prefix_width + suffix_width);
                    let title_chars = item.title.chars().count();

                    let title_display = if title_chars > available && available > 1 {
                        let truncated: String = item.title.chars().take(available - 1).collect();
                        format!("{}\u{2026}", truncated)
                    } else {
                        item.title.clone()
                    };

                    let mut spans = vec![];
                    spans.push(Span::styled(
                        format!("{}{}", prefix, title_display),
                        bg_style,
                    ));

                    if item.time_secs > 0 {
                        let time_style = if is_cursor {
                            Style::default().fg(Color::Gray).bg(Color::DarkGray)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };
                        spans.push(Span::styled(time_text, time_style));
                    }

                    if !item.tags.is_empty() {
                        let tag_style = if item.done {
                            Style::default().fg(Color::DarkGray)
                        } else {
                            Style::default().fg(Color::Blue)
                        };
                        let tag_style = if is_cursor {
                            tag_style.bg(Color::DarkGray)
                        } else {
                            tag_style
                        };
                        spans.push(Span::styled(tags_text, tag_style));
                    }

                    let list_style = if is_cursor {
                        Style::default().fg(Color::Gray).bg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    spans.push(Span::styled(list_suffix, list_style));

                    ListItem::new(Line::from(spans))
                })
                .collect();

            let list_widget = List::new(items);
            frame.render_widget(list_widget, inner);
        }
        return;
    }

    let visible = app.visible_items();

    if visible.is_empty()
        && !matches!(
            app.input_mode,
            InputMode::AddingItem | InputMode::EditingItem | InputMode::EditingTags
        )
    {
        let hint = if app.current_list().is_some() {
            if !app.show_done {
                "All items completed. Press Shift+D to show."
            } else {
                "No items. Press n to add one."
            }
        } else {
            "No items. Press n to add one."
        };
        let hint_paragraph = Paragraph::new(hint).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint_paragraph, inner);
    } else {
        let visible_height = inner.height as usize;
        let offset = if app.selected_item_index >= visible_height {
            app.selected_item_index - visible_height + 1
        } else {
            0
        };

        let items: Vec<ListItem> = visible
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible_height)
            .map(|(vi, (real_idx, item))| {
                let is_multi_selected = app.selected_items.contains(real_idx);

                let checkbox = if is_multi_selected {
                    if app.ascii_mode { "[*]" } else { "◆" }
                } else if app.ascii_mode {
                    if item.done { "[x]" } else { "[ ]" }
                } else if item.done {
                    "✔"
                } else {
                    "❑"
                };

                let mut spans = vec![];

                let base_style = if is_multi_selected {
                    Style::default().fg(Color::Yellow)
                } else if item.done {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                };

                let is_cursor = is_active && vi == app.selected_item_index;
                let bg_style = if is_cursor {
                    if is_multi_selected {
                        base_style.bg(Color::DarkGray)
                    } else if item.done {
                        base_style.fg(Color::Gray).bg(Color::DarkGray)
                    } else {
                        base_style.bg(Color::DarkGray)
                    }
                } else {
                    base_style
                };

                let prefix = format!("  {} ", checkbox);
                let prefix_width = prefix.chars().count();

                let time_text = if item.time_secs > 0 {
                    format!("  {}", crate::storage::format_time(item.time_secs))
                } else {
                    String::new()
                };

                let tags_text = if !item.tags.is_empty() {
                    let tags_str = item
                        .tags
                        .iter()
                        .map(|t| format!("@{}", t))
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("  {}", tags_str)
                } else {
                    String::new()
                };

                let suffix_width = time_text.chars().count() + tags_text.chars().count();
                let available = (inner.width as usize).saturating_sub(prefix_width + suffix_width);
                let title_chars = item.title.chars().count();

                let title_display = if title_chars > available && available > 1 {
                    let truncated: String = item.title.chars().take(available - 1).collect();
                    format!("{}\u{2026}", truncated)
                } else {
                    item.title.clone()
                };

                spans.push(Span::styled(
                    format!("{}{}", prefix, title_display),
                    bg_style,
                ));

                if item.time_secs > 0 {
                    let time_style = Style::default().fg(Color::DarkGray);
                    let time_style = if is_cursor {
                        time_style.bg(Color::DarkGray).fg(Color::Gray)
                    } else {
                        time_style
                    };
                    spans.push(Span::styled(time_text, time_style));
                }

                if !item.tags.is_empty() {
                    let tag_style = if is_multi_selected {
                        Style::default().fg(Color::Yellow)
                    } else if item.done {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::Blue)
                    };
                    let tag_style = if is_cursor {
                        tag_style.bg(Color::DarkGray)
                    } else {
                        tag_style
                    };
                    spans.push(Span::styled(tags_text, tag_style));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list_widget = List::new(items);
        frame.render_widget(list_widget, inner);
    }
}

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let text: String = match app.input_mode {
        InputMode::ConfirmDelete => "  y: confirm delete  n/Esc: cancel".to_string(),
        InputMode::Searching => {
            "  \u{2191}/\u{2193}: navigate  Enter: select  Esc: cancel".to_string()
        }
        InputMode::Focused => "  Space: pause/resume  Esc: stop".to_string(),
        InputMode::FilteringTags => {
            "  \u{2191}/\u{2193}: navigate  Space: toggle  Enter: apply  Esc: cancel".to_string()
        }
        InputMode::MovingToList => {
            "  \u{2191}/\u{2193}: navigate  Enter: move  Esc: cancel".to_string()
        }
        InputMode::AddingItem
        | InputMode::AddingList
        | InputMode::RenamingList
        | InputMode::EditingItem
        | InputMode::EditingTags
        | InputMode::EditingTime => "  Enter: confirm  Esc: cancel".to_string(),
        InputMode::Normal => {
            let filter_indicator = if !app.filter_tags.is_empty() {
                let tags: Vec<String> = app.filter_tags.iter().map(|t| format!("@{}", t)).collect();
                format!("  Filter: {}", tags.join(" "))
            } else {
                String::new()
            };
            if !app.selected_items.is_empty() {
                format!(
                    "  {} selected  Del: delete  \u{2325}K: toggle done  \u{2325}M: move  Esc: clear{}",
                    app.selected_items.len(),
                    filter_indicator
                )
            } else {
                match app.active_pane {
                    Pane::Sidebar => {
                        if app.is_tag_view() {
                            format!(
                                "  \u{2191}/\u{2193}: navigate  Tab: todos  type: search{}",
                                filter_indicator
                            )
                        } else {
                            format!(
                                "  \u{2191}/\u{2193}: navigate  \u{21e7}\u{2191}/\u{2193}: reorder  \u{2325}N: new  Enter: rename  \u{2325}D: daily  Del: delete  Tab: todos  type: search{}",
                                filter_indicator
                            )
                        }
                    }
                    Pane::Main => {
                        if app.is_tag_view() {
                            format!(
                                "  \u{2191}/\u{2193}: navigate  \u{2325}K: toggle  Enter: edit  Del: delete  type: search{}",
                                filter_indicator
                            )
                        } else {
                            format!(
                                "  \u{2191}/\u{2193}: navigate  \u{2325}K: toggle  Space: new  Enter: edit  \u{2325}X: select  Del: delete  \u{2325}M: move  type: search{}",
                                filter_indicator
                            )
                        }
                    }
                }
            }
        }
    };

    let bar = Paragraph::new(text).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(bar, area);
}

fn input_modal_height(app: &App, modal_width: u16) -> u16 {
    let inner_width = (modal_width.saturating_sub(2)).max(1) as usize;
    let total_chars = app.input_buffer.chars().count().max(1);
    let visual_lines = total_chars.div_ceil(inner_width);
    let max_height: u16 = 10;
    (visual_lines as u16 + 2).min(max_height)
}

fn render_input_modal(app: &App, frame: &mut Frame) {
    let title = match app.input_mode {
        InputMode::AddingItem => " New Todo ",
        InputMode::EditingItem => " Edit ",
        InputMode::EditingTags => " Tags ",
        InputMode::AddingList => " New List ",
        InputMode::RenamingList => " Rename ",
        InputMode::EditingTime => " Set Time ",
        _ => "",
    };

    let modal_width: u16 = 50;
    let modal_height = input_modal_height(app, modal_width);
    let area = centered_rect(modal_width, modal_height, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    let inner_width = inner.width.max(1) as usize;

    let wrapped_text: String = app
        .input_buffer
        .chars()
        .enumerate()
        .flat_map(|(i, c)| {
            if i > 0 && i % inner_width == 0 {
                Some('\n')
            } else {
                None
            }
            .into_iter()
            .chain(std::iter::once(c))
        })
        .collect();

    let chars_before_cursor = app.input_chars_before_cursor();
    let cursor_line = chars_before_cursor / inner_width;
    let cursor_col = chars_before_cursor % inner_width;

    let max_visible_lines = inner.height as usize;
    let scroll_offset = if cursor_line >= max_visible_lines {
        cursor_line - max_visible_lines + 1
    } else {
        0
    };

    let input = Paragraph::new(wrapped_text)
        .style(Style::default().fg(Color::Yellow))
        .block(block)
        .scroll((scroll_offset as u16, 0));

    frame.render_widget(input, area);

    let cursor_x = inner.x + cursor_col as u16;
    let cursor_y = inner.y + (cursor_line - scroll_offset) as u16;
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn render_confirm_dialog(app: &App, frame: &mut Frame) {
    let area = centered_rect(40, 5, frame.area());

    frame.render_widget(Clear, area);

    let list_name = app
        .current_list()
        .map(|l| l.name.as_str())
        .unwrap_or("item");

    let text = format!("Delete \"{}\"?\n\ny: yes  n: no", list_name);

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, area);
}

fn render_search_modal(app: &App, frame: &mut Frame) {
    let term = frame.area();
    let width: u16 = 60.min(term.width.saturating_sub(4));
    let total_height = (term.height * 60 / 100).max(8);

    let area = centered_rect(width, total_height, term);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Search ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let input = Paragraph::new(app.input_buffer.as_str()).style(Style::default().fg(Color::Yellow));
    frame.render_widget(input, chunks[0]);

    let cursor_x = chunks[0].x + app.input_cursor as u16;
    let cursor_y = chunks[0].y;
    frame.set_cursor_position((cursor_x, cursor_y));

    let sep_width = chunks[1].width as usize;
    let separator =
        Paragraph::new("\u{2500}".repeat(sep_width)).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(separator, chunks[1]);

    let results_area = chunks[2];

    if app.input_buffer.is_empty() {
        let hint = Paragraph::new("Type to search...").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, results_area);
    } else if app.search_results.is_empty() {
        let no_match = Paragraph::new("No matches").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_match, results_area);
    } else {
        let visible_height = results_area.height as usize;
        let offset = if app.search_selected >= visible_height {
            app.search_selected - visible_height + 1
        } else {
            0
        };

        let items: Vec<ListItem> = app
            .search_results
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible_height)
            .map(|(i, result)| {
                let is_selected = i == app.search_selected;
                match result {
                    SearchResult::List(li) => {
                        let name = &app.lists[*li].name;
                        let style = if is_selected {
                            Style::default().fg(Color::Cyan).bg(Color::DarkGray)
                        } else {
                            Style::default().fg(Color::Cyan)
                        };
                        ListItem::new(Line::from(Span::styled(format!("  # {}", name), style)))
                    }
                    SearchResult::Item(li, ii) => {
                        let item = &app.lists[*li].items[*ii];
                        let list_name = &app.lists[*li].name;
                        let checkbox = if app.ascii_mode {
                            if item.done { "[x]" } else { "[ ]" }
                        } else if item.done {
                            "✔"
                        } else {
                            "❑"
                        };
                        let time_part = if item.time_secs > 0 {
                            format!("  {}", crate::storage::format_time(item.time_secs))
                        } else {
                            String::new()
                        };
                        let text = format!(
                            "  {} {}{}  \u{2014} {}",
                            checkbox, item.title, time_part, list_name
                        );
                        let style = if item.done {
                            if is_selected {
                                Style::default().fg(Color::Gray).bg(Color::DarkGray)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            }
                        } else if is_selected {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        };
                        ListItem::new(Line::from(Span::styled(text, style)))
                    }
                    SearchResult::Tag(name) => {
                        let style = if is_selected {
                            Style::default().fg(Color::Blue).bg(Color::DarkGray)
                        } else {
                            Style::default().fg(Color::Blue)
                        };
                        ListItem::new(Line::from(Span::styled(format!("  @ {}", name), style)))
                    }
                }
            })
            .collect();

        let list_widget = List::new(items);
        frame.render_widget(list_widget, results_area);
    }
}

pub fn render_help(frame: &mut Frame) {
    let area = centered_rect(65, 35, frame.area());

    frame.render_widget(Clear, area);

    let key_style = Style::default().fg(Color::Yellow);
    let help_row = |key: &str, desc: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {:<28}", key), key_style),
            Span::raw(desc.to_string()),
        ])
    };
    let section = |title: &str| -> Line<'static> {
        Line::from(Span::styled(
            format!(" {title}"),
            Style::default().add_modifier(Modifier::BOLD),
        ))
    };

    let help_text = vec![
        section("Navigation"),
        help_row("\u{2191} / \u{2193}", "Move up / down"),
        help_row("Tab", "Switch pane"),
        help_row("Home / End", "Jump to first / last"),
        help_row("Ctrl+D / Ctrl+U", "Half-page down / up"),
        Line::from(""),
        section("Todos"),
        help_row("Space", "New todo"),
        help_row("Enter", "Edit todo title"),
        help_row("Alt+K", "Toggle done"),
        help_row("Alt+T", "Edit tags"),
        help_row("Alt+X", "Toggle select"),
        help_row("Del / Backspace", "Delete todo(s)"),
        help_row("Alt+M", "Move to list"),
        help_row("Esc", "Clear selection"),
        help_row(
            "Shift+\u{2191}/\u{2193} / Alt+\u{2191}/\u{2193}",
            "Reorder up / down",
        ),
        help_row("Alt+Super+\u{2191}/\u{2193}", "Move to top / bottom"),
        help_row("Alt+Shift+D", "Toggle show done"),
        help_row("Alt+Shift+F", "Filter by tag"),
        help_row("Alt+F", "Focus mode (timer)"),
        help_row("Alt+Shift+T", "Edit time"),
        Line::from(""),
        section("Lists"),
        help_row("Alt+N", "Add new list (sidebar)"),
        help_row("Enter", "Rename list (sidebar)"),
        help_row("Alt+D", "Toggle daily (auto-reset)"),
        help_row(
            "Shift+\u{2191}/\u{2193} / Alt+\u{2191}/\u{2193}",
            "Reorder up / down",
        ),
        help_row("Alt+Super+\u{2191}/\u{2193}", "Move to top / bottom"),
        help_row("Del / Backspace", "Delete list (sidebar)"),
        Line::from(""),
        section("General"),
        help_row("Type any character", "Search"),
        help_row("Alt+U / Ctrl+Z", "Undo"),
        help_row("Ctrl+Y", "Redo"),
        help_row("?", "Toggle help"),
        help_row("Alt+Q", "Quit"),
    ];

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn render_focus_overlay(app: &App, frame: &mut Frame) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let item = &app.lists[app.focus_list].items[app.focus_item];
    let session_secs = app.focus_elapsed_secs();
    let total_secs = item.time_secs + session_secs;
    let is_paused = app.focus_start.is_none();

    let session_h = session_secs / 3600;
    let session_m = (session_secs % 3600) / 60;
    let session_s = session_secs % 60;
    let session_str = format!("{:02}:{:02}:{:02}", session_h, session_m, session_s);

    let total_str = crate::storage::format_time(total_secs);
    let total_display = if total_str.is_empty() {
        "0m".to_string()
    } else {
        total_str
    };

    let pause_hint = if is_paused {
        "Space: resume"
    } else {
        "Space: pause"
    };
    let pause_label = if is_paused { "  PAUSED" } else { "" };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            item.title.clone(),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{}{}", session_str, pause_label),
            Style::default().fg(if is_paused {
                Color::Yellow
            } else {
                Color::Green
            }),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Total: {}", total_display),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Esc: stop  {}", pause_hint),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    let v_offset = inner.height.saturating_sub(8) / 2;
    let centered = Rect::new(
        inner.x,
        inner.y + v_offset,
        inner.width,
        8.min(inner.height),
    );
    frame.render_widget(paragraph, centered);
}

fn render_filter_modal(app: &App, frame: &mut Frame) {
    let tag_count = app.filter_available_tags.len() as u16;
    let height = (tag_count + 2).min(20);
    let width: u16 = 40.min(frame.area().width.saturating_sub(4));

    let area = centered_rect(width, height, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Filter by Tag ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = app
        .filter_available_tags
        .iter()
        .enumerate()
        .map(|(i, tag)| {
            let checked = app.filter_selected.get(i).copied().unwrap_or(false);
            let checkbox = if checked { "[x]" } else { "[ ]" };
            let is_selected = i == app.filter_cursor;
            let style = if is_selected {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(
                format!("  {} @{}", checkbox, tag),
                style,
            )))
        })
        .collect();

    let list_widget = List::new(items);
    frame.render_widget(list_widget, inner);
}

fn render_move_to_list_modal(app: &App, frame: &mut Frame) {
    let targets = app.move_to_list_targets();
    let target_count = targets.len().max(1) as u16;
    let height = (target_count + 4).min(20);
    let width: u16 = 40.min(frame.area().width.saturating_sub(4));

    let area = centered_rect(width, height, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Move to List ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let input =
        Paragraph::new(app.move_to_list_filter.as_str()).style(Style::default().fg(Color::Yellow));
    frame.render_widget(input, chunks[0]);

    let cursor_x = chunks[0].x + app.move_to_list_filter.len() as u16;
    let cursor_y = chunks[0].y;
    frame.set_cursor_position((cursor_x, cursor_y));

    let sep_width = chunks[1].width as usize;
    let separator =
        Paragraph::new("\u{2500}".repeat(sep_width)).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(separator, chunks[1]);

    if targets.is_empty() {
        let hint = Paragraph::new("No matching lists").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, chunks[2]);
    } else {
        let items: Vec<ListItem> = targets
            .iter()
            .enumerate()
            .map(|(i, (_, name))| {
                let is_selected = i == app.move_to_list_cursor;
                let style = if is_selected {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(format!("  {}", name), style)))
            })
            .collect();

        let list_widget = List::new(items);
        frame.render_widget(list_widget, chunks[2]);
    }
}

fn render_autocomplete_dropdown(app: &App, frame: &mut Frame) {
    if app.autocomplete_suggestions.is_empty() {
        return;
    }

    // Position dropdown below the input modal
    let modal_width: u16 = 50;
    let modal_height = input_modal_height(app, modal_width);
    let modal_area = centered_rect(modal_width, modal_height, frame.area());
    let suggestion_count = app.autocomplete_suggestions.len().min(5) as u16;
    let dropdown_y = modal_area.y + modal_area.height;
    let dropdown_width = modal_area.width.min(30);
    let dropdown_x = modal_area.x;

    if dropdown_y + suggestion_count > frame.area().height {
        return;
    }

    let dropdown_area = Rect::new(dropdown_x, dropdown_y, dropdown_width, suggestion_count);
    frame.render_widget(Clear, dropdown_area);

    let items: Vec<ListItem> = app
        .autocomplete_suggestions
        .iter()
        .enumerate()
        .take(5)
        .map(|(i, tag)| {
            let is_selected = i == app.autocomplete_cursor;
            let style = if is_selected {
                Style::default().fg(Color::Yellow).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White).bg(Color::Black)
            };
            ListItem::new(Line::from(Span::styled(format!(" @{}", tag), style)))
        })
        .collect();

    let list_widget = List::new(items).style(Style::default().bg(Color::Black));
    frame.render_widget(list_widget, dropdown_area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(x, y, w, h)
}
