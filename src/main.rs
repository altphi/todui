mod app;
mod config;
mod crdt;
mod input;
mod model;
mod storage;
mod sync_auth;
mod sync_transport;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

fn main() -> Result<()> {
    color_eyre::install()?;

    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--login".into()) {
        return handle_login();
    }
    if args.contains(&"--logout".into()) {
        sync_auth::clear_config().ok();
        println!("Logged out.");
        return Ok(());
    }

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

        let sync_changed = app.receive_sync_messages();

        terminal.draw(|frame| {
            ui::render(app, frame);
            if show_help {
                ui::render_help(frame, &app.key_config);
            }
        })?;

        let timeout = if app.input_mode == model::InputMode::Focused {
            Duration::from_millis(250)
        } else if app.is_dirty() || sync_changed || app.has_sync() {
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

fn handle_login() -> color_eyre::Result<()> {
    use std::io::Write;

    print!("Server URL: ");
    std::io::stdout().flush()?;
    let mut server_url = String::new();
    std::io::stdin().read_line(&mut server_url)?;
    let server_url = server_url.trim().to_string();
    if server_url.is_empty() {
        eprintln!("Server URL is required.");
        std::process::exit(1);
    }

    print!("Email: ");
    std::io::stdout().flush()?;
    let mut email = String::new();
    std::io::stdin().read_line(&mut email)?;
    let email = email.trim().to_string();
    if email.is_empty() {
        eprintln!("Email is required.");
        std::process::exit(1);
    }

    println!("Sending magic link to {}...", email);

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{}/auth/login", server_url))
        .json(&serde_json::json!({ "email": email }))
        .send()?;

    if !resp.status().is_success() {
        eprintln!("Error: {}", resp.text()?);
        std::process::exit(1);
    }

    let body: serde_json::Value = resp.json()?;
    let poll_id = body["poll_id"]
        .as_str()
        .ok_or_else(|| color_eyre::eyre::eyre!("Invalid response: missing poll_id"))?;

    println!("Check your email and click the login link. Waiting...");

    loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let resp = client
            .get(format!("{}/auth/poll?poll_id={}", server_url, poll_id))
            .send()?;
        let body: serde_json::Value = resp.json()?;

        match body["status"].as_str() {
            Some("verified") => {
                let token = body["token"]
                    .as_str()
                    .ok_or_else(|| color_eyre::eyre::eyre!("Invalid response: missing token"))?
                    .to_string();
                let config = sync_auth::SyncConfig {
                    server_url,
                    token,
                    email,
                };
                sync_auth::save_config(&config)?;
                println!("Logged in successfully!");
                return Ok(());
            }
            Some("expired") => {
                eprintln!("Login link expired. Please try again.");
                std::process::exit(1);
            }
            _ => {
                // Still pending, continue polling
            }
        }
    }
}
