mod app;
mod input;
mod model;
mod storage;
mod ui;

use std::path::PathBuf;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

fn main() -> Result<()> {
    color_eyre::install()?;

    let (data_dir, ascii_mode) = parse_args();
    let mut app = app::App::new(data_dir, ascii_mode)?;

    let terminal = ratatui::init();
    let result = run(&mut app, terminal);
    ratatui::restore();
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

fn run(app: &mut app::App, mut terminal: DefaultTerminal) -> Result<()> {
    let mut show_help = false;

    while app.running {
        terminal.draw(|frame| {
            ui::render(app, frame);
            if show_help {
                ui::render_help(frame);
            }
        })?;

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
