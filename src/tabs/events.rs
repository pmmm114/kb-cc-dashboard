use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::App;
use super::helpers::render_scroll_indicators;

pub fn draw_events(f: &mut Frame, app: &App, list_area: Rect, detail_area: Rect) {
    draw_event_list(f, app, list_area);
    draw_event_detail(f, app, detail_area);
}

fn draw_event_list(f: &mut Frame, app: &App, area: Rect) {
    let events = app.filtered_events();

    if events.is_empty() {
        let block = Block::default().borders(Borders::ALL).title("Events (0)");
        let msg = Paragraph::new("Waiting for hook events...")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = events
        .iter()
        .map(|event| {
            let kind = event.kind();
            let color = kind.color();
            let timestamp = event.received_at.format("%H:%M:%S").to_string();
            let summary = event.summary();
            // Truncate summary if it would exceed available width.
            // Layout: border(1) + timestamp(8) + space(1) + kind_padded(22) + border(1) = 33 fixed + 2 margin
            let max_summary = (area.width as usize).saturating_sub(35);
            let truncated_summary = if summary.chars().count() > max_summary && max_summary > 3 {
                let s: String = summary.chars().take(max_summary.saturating_sub(1)).collect();
                format!("{}\u{2026}", s)
            } else {
                summary
            };

            let line = Line::from(vec![
                Span::styled(timestamp, Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
                Span::styled(
                    format!("{:<22}", kind.to_string()),
                    Style::default().fg(color),
                ),
                Span::styled(truncated_summary, Style::default().fg(Color::White)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let filter_label = match &app.events_session_filter {
        Some(sid) => {
            let short = &sid[..8.min(sid.len())];
            format!(" [session: {}]", short)
        }
        None => " [all]".to_string(),
    };
    let auto_indicator = if app.event_auto_scroll {
        " [auto]"
    } else {
        ""
    };
    let title = format!("Events ({}){}{}", events.len(), filter_label, auto_indicator);
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let selected = app.event_selected.min(events.len().saturating_sub(1));
    let mut state = ListState::default();
    state.select(if events.is_empty() { None } else { Some(selected) });
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_event_detail(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Event Detail");

    let events = app.filtered_events();
    if events.is_empty() {
        f.render_widget(block, area);
        return;
    }

    let selected = app.event_selected.min(events.len().saturating_sub(1));
    let event = events[selected];
    let json_str =
        serde_json::to_string_pretty(&event.payload).unwrap_or_else(|_| "{}".to_string());

    let lines: Vec<Line> = json_str
        .lines()
        .map(|line| {
            if line.contains(':') {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                Line::from(vec![
                    Span::styled(parts[0].to_string(), Style::default().fg(Color::Cyan)),
                    Span::raw(":"),
                    Span::styled(
                        parts.get(1).unwrap_or(&"").to_string(),
                        Style::default().fg(Color::White),
                    ),
                ])
            } else {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::White),
                ))
            }
        })
        .collect();

    let total_lines = lines.len();
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.event_detail_scroll as u16, 0));
    f.render_widget(paragraph, area);

    render_scroll_indicators(f, area, total_lines, app.event_detail_scroll);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::ConfigInventory;
    use crate::test_helpers::test_utils::{buffer_to_string, make_test_event};
    use ratatui::{backend::TestBackend, Terminal};

    fn event_from_name(name: &str) -> crate::event::HookEvent {
        make_test_event(&format!(
            r#"{{"hook_event_name":"{}","session_id":"test-1","tool_name":"Read"}}"#,
            name
        ))
    }

    fn render_events(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                let (list_area, detail_area) = crate::ui::split_list_detail(area);
                draw_events(f, app, list_area, detail_area);
            })
            .unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    #[test]
    fn empty_events_shows_waiting_message() {
        let app = App::new(ConfigInventory::default());
        let output = render_events(&app, 80, 20);
        assert!(
            output.contains("Waiting for hook events..."),
            "Expected waiting message, got:\n{}",
            output
        );
    }

    #[test]
    fn empty_events_shows_zero_count() {
        let app = App::new(ConfigInventory::default());
        let output = render_events(&app, 80, 20);
        assert!(
            output.contains("Events (0)"),
            "Expected 'Events (0)' in title, got:\n{}",
            output
        );
    }

    #[test]
    fn events_list_shows_count() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        app.push_event(event_from_name("PostToolUse"));
        let output = render_events(&app, 80, 20);
        assert!(
            output.contains("Events (2)"),
            "Expected 'Events (2)' in title, got:\n{}",
            output
        );
    }

    #[test]
    fn events_list_shows_auto_scroll_indicator() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        assert!(app.event_auto_scroll);
        let output = render_events(&app, 80, 20);
        assert!(
            output.contains("[auto]"),
            "Expected '[auto]' indicator, got:\n{}",
            output
        );
    }

    #[test]
    fn events_list_hides_auto_when_disabled() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        app.event_auto_scroll = false;
        let output = render_events(&app, 80, 20);
        assert!(
            !output.contains("[auto]"),
            "Expected no '[auto]' indicator, got:\n{}",
            output
        );
    }

    #[test]
    fn events_list_shows_event_kind() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        let output = render_events(&app, 100, 20);
        assert!(
            output.contains("PreToolUse"),
            "Expected event kind 'PreToolUse', got:\n{}",
            output
        );
    }

    #[test]
    fn events_list_shows_summary() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        let output = render_events(&app, 100, 20);
        assert!(
            output.contains("Read"),
            "Expected summary 'Read', got:\n{}",
            output
        );
    }

    #[test]
    fn events_list_truncates_long_summary_in_narrow_terminal() {
        let mut app = App::new(ConfigInventory::default());
        // Create event with agent context for a longer summary
        let event = make_test_event(
            r#"{"hook_event_name":"PreToolUse","session_id":"test-1","tool_name":"Read","agent_context_type":"tdd-implementer"}"#,
        );
        app.push_event(event);
        // Very narrow terminal: 40 columns for list area (half of 80)
        let output = render_events(&app, 50, 20);
        // Should not panic, and the output should be rendered
        assert!(output.contains("PreToolUse"), "Expected event kind even in narrow terminal, got:\n{}", output);
    }

    #[test]
    fn detail_panel_shows_json() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        let output = render_events(&app, 100, 20);
        assert!(
            output.contains("Event Detail"),
            "Expected 'Event Detail' title, got:\n{}",
            output
        );
        // The detail should contain the JSON key from the payload
        assert!(
            output.contains("tool_name"),
            "Expected JSON key 'tool_name' in detail, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_panel_empty_when_no_events() {
        let app = App::new(ConfigInventory::default());
        let output = render_events(&app, 80, 20);
        assert!(
            output.contains("Event Detail"),
            "Expected empty detail panel with title"
        );
    }

    #[test]
    fn draw_events_does_not_panic_small_terminal() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        let _output = render_events(&app, 30, 5);
    }

    fn event_with_session(name: &str, session_id: &str) -> crate::event::HookEvent {
        make_test_event(&format!(
            r#"{{"hook_event_name":"{}","session_id":"{}","tool_name":"Read"}}"#,
            name, session_id
        ))
    }

    #[test]
    fn events_title_shows_all_when_no_filter() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        let output = render_events(&app, 100, 20);
        assert!(
            output.contains("[all]"),
            "Expected '[all]' in title when no filter set, got:\n{}",
            output
        );
    }

    #[test]
    fn events_title_shows_session_filter() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_with_session("PreToolUse", "abcdef1234567890"));
        app.push_event(event_with_session("PreToolUse", "other-session"));
        app.events_session_filter = Some("abcdef1234567890".to_string());
        let output = render_events(&app, 100, 20);
        assert!(
            output.contains("[session: abcdef12]"),
            "Expected '[session: abcdef12]' in title, got:\n{}",
            output
        );
    }

    #[test]
    fn events_list_filters_by_session() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_with_session("PreToolUse", "sess-a"));
        app.push_event(event_with_session("PostToolUse", "sess-b"));
        app.push_event(event_with_session("PreToolUse", "sess-a"));
        app.events_session_filter = Some("sess-a".to_string());
        let output = render_events(&app, 100, 20);
        // Should show count 2 (only sess-a events), not 3
        assert!(
            output.contains("Events (2)"),
            "Expected 'Events (2)' for filtered events, got:\n{}",
            output
        );
    }

    #[test]
    fn events_detail_uses_filtered_events() {
        let mut app = App::new(ConfigInventory::default());
        // Push event for sess-b first (index 0 in global), then sess-a
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PreToolUse","session_id":"sess-b","tool_name":"Write"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"sess-a","tool_name":"Edit"}"#,
        ));
        app.events_session_filter = Some("sess-a".to_string());
        app.event_selected = 0;
        let output = render_events(&app, 100, 20);
        // Detail should show the sess-a event (Edit), not the sess-b event (Write)
        assert!(
            output.contains("Edit"),
            "Expected 'Edit' tool_name from sess-a event in detail, got:\n{}",
            output
        );
    }
}
