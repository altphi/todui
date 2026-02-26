use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::app::App;
use crate::model::{InputMode, Pane, SearchResult};

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

    let main_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(80),
        ])
        .split(outer[1]);

    render_sidebar(app, frame, main_area[0]);
    render_todo_pane(app, frame, main_area[1]);
    render_status_bar(app, frame, outer[2]);

    match app.input_mode {
        InputMode::ConfirmDelete => render_confirm_dialog(app, frame),
        InputMode::Searching => render_search_modal(app, frame),
        InputMode::Focused => render_focus_overlay(app, frame),
        InputMode::FilteringTags => render_filter_modal(app, frame),
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
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = app
        .lists
        .iter()
        .enumerate()
        .map(|(i, list)| {
            let content = format!("  {}", list.name);
            let style = if i == app.selected_list_index {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(content, style)))
        })
        .collect();

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

    let title = app
        .current_list()
        .map(|l| format!(" {} ", l.name))
        .unwrap_or_else(|| " Todos ".to_string());

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = app.visible_items();

    if visible.is_empty() && !matches!(app.input_mode, InputMode::AddingItem | InputMode::EditingItem | InputMode::EditingTags) {
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
        let items: Vec<ListItem> = visible
            .iter()
            .enumerate()
            .map(|(vi, (_real_idx, item))| {
                let checkbox = if app.ascii_mode {
                    if item.done { "[x]" } else { "[ ]" }
                } else if item.done {
                    "☑"
                } else {
                    "☐"
                };

                let mut spans = vec![];

                let base_style = if item.done {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                };

                let is_selected = is_active && vi == app.selected_item_index;
                let bg_style = if is_selected {
                    if item.done {
                        base_style.fg(Color::Gray).bg(Color::DarkGray)
                    } else {
                        base_style.bg(Color::DarkGray)
                    }
                } else {
                    base_style
                };

                spans.push(Span::styled(
                    format!("  {} {}", checkbox, item.title),
                    bg_style,
                ));

                if item.time_secs > 0 {
                    let time_str = crate::storage::format_time(item.time_secs);
                    let time_style = Style::default().fg(Color::DarkGray);
                    let time_style = if is_selected {
                        time_style.bg(Color::DarkGray).fg(Color::Gray)
                    } else {
                        time_style
                    };
                    spans.push(Span::styled(format!("  {}", time_str), time_style));
                }

                if !item.tags.is_empty() {
                    let tags_str = item
                        .tags
                        .iter()
                        .map(|t| format!("@{}", t))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let tag_style = if item.done {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::Blue)
                    };
                    let tag_style = if is_selected {
                        if item.done {
                            tag_style.fg(Color::Gray).bg(Color::DarkGray)
                        } else {
                            tag_style.bg(Color::DarkGray)
                        }
                    } else {
                        tag_style
                    };
                    spans.push(Span::styled(format!("  {}", tags_str), tag_style));
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
        InputMode::ConfirmDelete => {
            "  y: confirm delete  n/Esc: cancel".to_string()
        }
        InputMode::Searching => {
            "  \u{2191}/\u{2193}: navigate  Enter: select  Esc: cancel".to_string()
        }
        InputMode::Focused => "  Space: pause/resume  Esc: stop".to_string(),
        InputMode::FilteringTags => {
            "  j/k: navigate  Space: toggle  Enter: apply  Esc: cancel".to_string()
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
            match app.active_pane {
                Pane::Sidebar => {
                    format!("  j/k: navigate  Shift+K/J: reorder  n: new  Enter: rename  x: delete  Tab: todos  ?: help{}", filter_indicator)
                }
                Pane::Main => {
                    format!("  j/k: navigate  Space: toggle  n: new  Enter: edit  x: delete  t: tags  f: filter  ?: help{}", filter_indicator)
                }
            }
        }
    };

    let bar = Paragraph::new(text).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(bar, area);
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

    let area = centered_rect(50, 3, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);

    let input = Paragraph::new(app.input_buffer.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(block);
    frame.render_widget(input, area);

    let cursor_x = inner.x + app.input_cursor as u16;
    let cursor_y = inner.y;
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

    let input = Paragraph::new(app.input_buffer.as_str())
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(input, chunks[0]);

    let cursor_x = chunks[0].x + app.input_cursor as u16;
    let cursor_y = chunks[0].y;
    frame.set_cursor_position((cursor_x, cursor_y));

    let sep_width = chunks[1].width as usize;
    let separator = Paragraph::new("\u{2500}".repeat(sep_width))
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(separator, chunks[1]);

    let results_area = chunks[2];

    if app.input_buffer.is_empty() {
        let hint = Paragraph::new("Type to search...")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, results_area);
    } else if app.search_results.is_empty() {
        let no_match = Paragraph::new("No matches")
            .style(Style::default().fg(Color::DarkGray));
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
                        ListItem::new(Line::from(Span::styled(
                            format!("  # {}", name),
                            style,
                        )))
                    }
                    SearchResult::Item(li, ii) => {
                        let item = &app.lists[*li].items[*ii];
                        let list_name = &app.lists[*li].name;
                        let checkbox = if app.ascii_mode {
                            if item.done { "[x]" } else { "[ ]" }
                        } else if item.done {
                            "☑"
                        } else {
                            "☐"
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
                }
            })
            .collect();

        let list_widget = List::new(items);
        frame.render_widget(list_widget, results_area);
    }
}

pub fn render_help(frame: &mut Frame) {
    let area = centered_rect(55, 29, frame.area());

    frame.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled(
            " Navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  j / k          Move down / up"),
        Line::from("  Tab            Switch pane"),
        Line::from("  g / G          Jump to first / last"),
        Line::from(""),
        Line::from(Span::styled(
            " Todos",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  n              Add new todo"),
        Line::from("  Enter          Edit todo title"),
        Line::from("  Space          Toggle done"),
        Line::from("  t              Edit tags"),
        Line::from("  x              Delete todo"),
        Line::from("  Shift+K / J    Move todo up / down"),
        Line::from("  Shift+D        Toggle show done"),
        Line::from("  f              Filter by tag"),
        Line::from("  Shift+F        Focus mode (timer)"),
        Line::from("  Shift+T        Edit time"),
        Line::from(""),
        Line::from(Span::styled(
            " Lists",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  n              Add new list (sidebar)"),
        Line::from("  Enter          Rename list (sidebar)"),
        Line::from("  Shift+K / J    Move list up / down"),
        Line::from("  x              Delete list (sidebar)"),
        Line::from(""),
        Line::from(Span::styled(
            " General",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("  /              Search"),
        Line::from("  u / Ctrl+R     Undo / Redo"),
        Line::from("  ?              Toggle help"),
        Line::from("  q              Quit"),
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

    let pause_hint = if is_paused { "Space: resume" } else { "Space: pause" };
    let pause_label = if is_paused { "  PAUSED" } else { "" };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            item.title.clone(),
            Style::default().add_modifier(Modifier::BOLD).fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{}{}", session_str, pause_label),
            Style::default().fg(if is_paused { Color::Yellow } else { Color::Green }),
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

    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Center);
    let v_offset = inner.height.saturating_sub(8) / 2;
    let centered = Rect::new(inner.x, inner.y + v_offset, inner.width, 8.min(inner.height));
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

fn render_autocomplete_dropdown(app: &App, frame: &mut Frame) {
    if app.autocomplete_suggestions.is_empty() {
        return;
    }

    // Position dropdown below the input modal
    let modal_area = centered_rect(50, 3, frame.area());
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
