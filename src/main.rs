use std::io;
use std::path::PathBuf;

use clap::Parser;
use crossterm::{
    event::{self as ct_event, poll, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use tokio::sync::mpsc;

use claude_dashboard::app::App;
use claude_dashboard::config_parser;
use claude_dashboard::event::HookEvent;
use claude_dashboard::listener;
use claude_dashboard::ui;

#[derive(Parser)]
#[command(name = "claude-dashboard", about = "Claude Code configuration dashboard")]
struct Cli {
    /// Path to the Unix socket for hook events
    #[arg(long, default_value = "/tmp/claude-dashboard.sock")]
    socket_path: PathBuf,

    /// Path to the Claude Code config directory
    #[arg(long)]
    claude_dir: Option<PathBuf>,
}

fn default_claude_dir() -> PathBuf {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".claude"))
        .unwrap_or_else(|_| PathBuf::from(".claude"))
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();

    install_panic_hook();

    let claude_dir = cli.claude_dir.unwrap_or_else(default_claude_dir);
    let config = config_parser::load_all(&claude_dir);

    let (event_tx, mut event_rx) = mpsc::channel::<HookEvent>(256);

    let socket_path = cli.socket_path.clone();
    let socket_path_cleanup = cli.socket_path.clone();
    tokio::spawn(async move {
        if let Err(e) = listener::start_listener(socket_path, event_tx).await {
            eprintln!("Socket listener error: {}", e);
        }
    });

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);

    loop {
        // Render
        terminal.draw(|f| ui::draw(f, &app))?;

        // Poll for keyboard events with short timeout so we can check channels
        if poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = ct_event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.on_key(key);
                    if app.should_quit {
                        break;
                    }
                }
            }
        }

        // Drain hook event channel
        while let Ok(event) = event_rx.try_recv() {
            app.push_event(event);
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    listener::cleanup_socket(&socket_path_cleanup);

    Ok(())
}

fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));
}
