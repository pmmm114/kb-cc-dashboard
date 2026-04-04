//! Snapshot dump test — writes rendered TUI snapshots to disk for agent-based user testing.
//!
//! This test is `#[ignore]` by default. Run with:
//!   SNAPSHOT_OUTPUT_DIR=/path/to/dir cargo test --test snapshot_dump -- --ignored --nocapture

use claude_dashboard::app::{App, SessionFocus, Tab};
use claude_dashboard::test_helpers::test_utils::{buffer_to_string, mock_populated_app};
use claude_dashboard::ui;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::fs;

fn render_app(app: &App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, app)).unwrap();
    let buf = terminal.backend().buffer().clone();
    buffer_to_string(&buf)
}

#[test]
#[ignore]
fn dump_snapshots_to_env_dir() {
    let output_dir = std::env::var("SNAPSHOT_OUTPUT_DIR")
        .expect("Set SNAPSHOT_OUTPUT_DIR to the target directory");

    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Sessions tab — list focus
    let app = mock_populated_app();
    let path = format!("{}/sessions_list.txt", output_dir);
    fs::write(&path, render_app(&app)).unwrap();
    eprintln!("SNAPSHOT_FILE:sessions_list:{}", path);

    // Sessions tab — segment focus
    let mut app = mock_populated_app();
    app.session_focus = SessionFocus::Segment;
    let path = format!("{}/sessions_segment.txt", output_dir);
    fs::write(&path, render_app(&app)).unwrap();
    eprintln!("SNAPSHOT_FILE:sessions_segment:{}", path);

    // Events tab
    let mut app = mock_populated_app();
    app.active_tab = Tab::Events;
    let path = format!("{}/events.txt", output_dir);
    fs::write(&path, render_app(&app)).unwrap();
    eprintln!("SNAPSHOT_FILE:events:{}", path);

    // Config tab
    let mut app = mock_populated_app();
    app.active_tab = Tab::Config;
    let path = format!("{}/config.txt", output_dir);
    fs::write(&path, render_app(&app)).unwrap();
    eprintln!("SNAPSHOT_FILE:config:{}", path);

    eprintln!("All snapshots written to {}", output_dir);
}
