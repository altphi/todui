mod app;
mod config;
mod crdt;
mod input;
mod model;
mod storage;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

fn main() -> Result<()> {
    color_eyre::install()?;

    let (data_dir, ascii_mode) = parse_args();

    if let Err(e) = storage::migrate_to_contexts(&data_dir) {
        eprintln!("Error migrating data: {}", e);
        std::process::exit(1);
    }

    let context = storage::load_last_context(&data_dir).unwrap_or_else(|| "default".into());

    if let Err(e) = storage::acquire_lock(&data_dir) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("todui");
    let key_config = config::KeyConfig::load(&config_dir);

    let mut app = app::App::new(data_dir.clone(), context, ascii_mode, key_config)?;
    app.reset_daily_lists();
    let mut current_date = chrono::Local::now().format("%Y-%m-%d").to_string();

    let terminal = ratatui::init();
    let result = run(&mut app, terminal, &mut current_date);
    ratatui::restore();
    storage::release_lock(&data_dir);
    result
}

fn parse_args() -> (PathBuf, bool) {
    let args: Vec<String> = std::env::args().collect();
    let mut data_dir: Option<PathBuf> = None;
    let mut ascii_mode = false;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--dir"
            && let Some(dir) = args.get(i + 1)
        {
            data_dir = Some(PathBuf::from(shellexpand::tilde(dir).as_ref()));
            i += 1;
        } else if args[i] == "--ascii" {
            ascii_mode = true;
        }
        i += 1;
    }
    let data_dir = data_dir.unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("todos")
    });
    (data_dir, ascii_mode)
}

fn run(app: &mut app::App, mut terminal: DefaultTerminal, current_date: &mut String) -> Result<()> {
    let mut show_help = false;

    while app.running {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if today != *current_date {
            *current_date = today;
            app.reset_daily_lists();
        }

        terminal.draw(|frame| {
            ui::render(app, frame);
            if show_help {
                ui::render_help(frame, &app.key_config);
            }
        })?;

        let timeout = if app.input_mode == model::InputMode::Focused {
            Duration::from_millis(250)
        } else if app.is_dirty() {
            Duration::from_millis(500)
        } else {
            Duration::from_secs(60)
        };
        let has_event = event::poll(timeout)?;

        if !has_event {
            app.flush();
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if key.code == crossterm::event::KeyCode::Char('?')
                && app.input_mode == model::InputMode::Normal
            {
                show_help = !show_help;
                continue;
            }

            if show_help {
                show_help = false;
                continue;
            }

            input::handle_key(app, key);
        }
    }

    Ok(())
}
