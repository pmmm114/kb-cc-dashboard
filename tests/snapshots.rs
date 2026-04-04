use claude_dashboard::app::{App, SessionFocus, Tab};
use claude_dashboard::test_helpers::test_utils::{buffer_to_string, mock_populated_app};
use claude_dashboard::ui;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_app(app: &App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| ui::draw(f, app))
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    buffer_to_string(&buf)
}

#[test]
fn snapshot_sessions_tab_list_focus() {
    let app = mock_populated_app();
    assert_eq!(app.active_tab, Tab::Sessions);
    assert_eq!(app.session_focus, SessionFocus::List);

    let output = render_app(&app);

    // Tab bar should show all three tabs
    assert!(output.contains("Sessions"), "Should show Sessions tab");
    assert!(output.contains("Config"), "Should show Config tab");
    assert!(output.contains("Events"), "Should show Events tab");

    // Should display session IDs (hex strings)
    assert!(output.contains("a1b2c3d4"), "Should show active session ID");
    assert!(output.contains("e5f6a7b8"), "Should show ended session ID");
}

#[test]
fn snapshot_sessions_tab_segment_focus() {
    let mut app = mock_populated_app();
    app.session_focus = SessionFocus::Segment;

    let output = render_app(&app);

    // Should still be on sessions tab
    assert!(output.contains("Sessions"), "Should show Sessions tab");
    // Should show segment information (prompt text is visible in segment view)
    // The first session has 3 segments: init + 2 prompts
    assert!(
        output.contains("initialization") || output.contains("Implement"),
        "Should show segment prompt text or initialization label"
    );
}

#[test]
fn snapshot_events_tab() {
    let mut app = mock_populated_app();
    app.active_tab = Tab::Events;

    let output = render_app(&app);

    // Should show Events tab as active
    assert!(output.contains("Events"), "Should show Events tab");

    // Should contain event kind names from the mixed events
    assert!(
        output.contains("PostToolUse") || output.contains("Read") || output.contains("Edit"),
        "Should show tool event information"
    );
}

#[test]
fn snapshot_config_tab() {
    let mut app = mock_populated_app();
    app.active_tab = Tab::Config;

    let output = render_app(&app);

    // Should show Config tab content
    assert!(output.contains("Config"), "Should show Config tab");
    // Config tab shows categories: Agents, Skills, Rules, Hooks, Plugins
    assert!(output.contains("Agents"), "Should show Agents category");
}

#[test]
fn mock_has_required_data() {
    let app = mock_populated_app();

    // At least 2 sessions
    assert!(
        app.session_records.len() >= 2,
        "Should have at least 2 sessions, got {}",
        app.session_records.len()
    );

    // Check active session has 3 segments and 2 agents
    let active = app.session_records.get("a1b2c3d4").expect("Active session should exist");
    assert!(!active.ended, "Active session should not be ended");
    assert!(
        active.prompt_segments.len() >= 3,
        "Active session should have at least 3 segments, got {}",
        active.prompt_segments.len()
    );
    assert!(
        active.agent_records.len() >= 2,
        "Active session should have at least 2 agents, got {}",
        active.agent_records.len()
    );

    // Check ended session
    let ended = app.session_records.get("e5f6a7b8").expect("Ended session should exist");
    assert!(ended.ended, "Ended session should be ended");
    assert!(
        ended.prompt_segments.len() >= 1,
        "Ended session should have at least 1 segment"
    );

    // Check mixed events exist
    assert!(
        app.events.len() >= 5,
        "Should have at least 5 events, got {}",
        app.events.len()
    );
}
