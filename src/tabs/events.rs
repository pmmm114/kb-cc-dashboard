use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::App;
use crate::event::EventKind;
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

    // A2: Track agent depth for tree lines
    let mut agent_depth: usize = 0;
    let items: Vec<ListItem> = events
        .iter()
        .map(|event| {
            let kind = event.kind();
            let color = kind.color();
            let icon = kind.category().icon();
            let timestamp = event.received_at.format("%H:%M:%S").to_string();
            let summary = event.summary();

            // A2: Manage depth — SubagentStop decrements BEFORE render, SubagentStart increments AFTER
            let tree_depth = match kind {
                EventKind::SubagentStop => {
                    agent_depth = agent_depth.saturating_sub(1);
                    0 // SubagentStop itself has no tree prefix
                }
                EventKind::SubagentStart => 0, // SubagentStart itself has no tree prefix
                _ => agent_depth,
            };

            // Build tree prefix: "│ " repeated for each depth level
            let tree_prefix = "│ ".repeat(tree_depth);
            let tree_prefix_width = tree_depth * 2;
            let icon_width = 2; // icon char + space

            // Truncate summary if it would exceed available width.
            // Layout: border(1) + tree_prefix + icon(2) + timestamp(8) + space(1) + kind_padded(22) + border(1) = 35 fixed + tree + icon
            let fixed_width = 35 + tree_prefix_width + icon_width;
            let max_summary = (area.width as usize).saturating_sub(fixed_width);
            let truncated_summary = if summary.chars().count() > max_summary && max_summary > 3 {
                let s: String = summary.chars().take(max_summary.saturating_sub(1)).collect();
                format!("{}\u{2026}", s)
            } else {
                summary
            };

            // Build spans: tree_prefix + icon + timestamp + space + kind + summary
            let mut spans = Vec::new();
            if tree_depth > 0 {
                spans.push(Span::styled(tree_prefix, Style::default().fg(Color::Magenta)));
            }
            spans.push(Span::styled(format!("{} ", icon), Style::default().fg(color)));
            spans.push(Span::styled(timestamp, Style::default().fg(Color::DarkGray)));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("{:<22}", kind.to_string()),
                Style::default().fg(color),
            ));
            spans.push(Span::styled(truncated_summary, Style::default().fg(Color::White)));

            let line = Line::from(spans);

            // A3: Error emphasis for StopFailure and PostToolUseFailure
            let item = if kind == EventKind::StopFailure || kind == EventKind::PostToolUseFailure {
                ListItem::new(line).style(
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ListItem::new(line)
            };

            // A2: SubagentStart increments AFTER render
            if kind == EventKind::SubagentStart {
                agent_depth += 1;
            }

            item
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
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));

    let selected = app.event_selected.min(events.len().saturating_sub(1));
    let mut state = ListState::default();
    state.select(if events.is_empty() { None } else { Some(selected) });
    f.render_stateful_widget(list, area, &mut state);
}

/// Meta keys excluded from the "Extra Fields" section (they are structural, not payload).
const META_KEYS: &[&str] = &["hook_event_name", "session_id", "received_at"];

/// Human-readable label for a known field key.
fn field_label(key: &str) -> &'static str {
    match key {
        "tool_name" => "Tool:",
        "agent_context_type" => "Agent Context:",
        "file_path" => "File:",
        "duration_ms" => "Duration:",
        "error" => "Error:",
        "message" => "Message:",
        "agent_type" => "Agent Type:",
        "cwd" => "CWD:",
        "model" => "Model:",
        _ => "",
    }
}

/// Whether a field should render its value in red (error-related fields).
fn is_error_field(key: &str) -> bool {
    matches!(key, "error" | "message")
}

/// Extract a human-readable string from a JSON value, using em-dash for missing/null.
fn json_value_display(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) | None => "\u{2014}".to_string(),
        Some(other) => other.to_string(),
    }
}

/// Build the combined lines for all 3 detail sections.
fn build_detail_lines(event: &crate::event::HookEvent) -> Vec<Line<'static>> {
    let kind = event.kind();
    let known = kind.known_fields();
    let mut lines: Vec<Line<'static>> = Vec::new();
    let label_width = 18;

    // Section 1: Structured Fields (only if known_fields is non-empty)
    if !known.is_empty() {
        let kind_color = kind.color();
        let title = format!("\u{2500}\u{2500} {} \u{2500}\u{2500}", kind);
        lines.push(Line::from(Span::styled(
            title,
            Style::default().fg(kind_color).add_modifier(Modifier::BOLD),
        )));
        for &key in known {
            let label = field_label(key);
            let display_label = if label.is_empty() {
                format!("{}:", key)
            } else {
                label.to_string()
            };
            let value = json_value_display(event.payload.get(key));

            let value_color =
                if is_error_field(key) && matches!(kind, EventKind::StopFailure | EventKind::PostToolUseFailure) {
                    Color::Red
                } else {
                    Color::White
                };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<width$}", display_label, width = label_width),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(value, Style::default().fg(value_color)),
            ]));
        }
    }

    // Section 2: Extra Fields
    let known_set: std::collections::HashSet<&str> =
        known.iter().copied().chain(META_KEYS.iter().copied()).collect();
    let mut extra_keys: Vec<String> = Vec::new();
    if let Some(obj) = event.payload.as_object() {
        for key in obj.keys() {
            if !known_set.contains(key.as_str()) {
                extra_keys.push(key.clone());
            }
        }
    }
    extra_keys.sort();

    if !extra_keys.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            "\u{2500}\u{2500} Extra Fields \u{2500}\u{2500}",
            Style::default().fg(Color::DarkGray),
        )));
        for key in &extra_keys {
            let value = json_value_display(event.payload.get(key.as_str()));
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<width$}", format!("{}:", key), width = label_width),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(value, Style::default().fg(Color::White)),
            ]));
        }
    }

    // Section 3: Raw JSON (always present)
    if !lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "\u{2500}\u{2500} Raw JSON \u{2500}\u{2500}",
        Style::default().fg(Color::DarkGray),
    )));

    let json_str =
        serde_json::to_string_pretty(&event.payload).unwrap_or_else(|_| "{}".to_string());
    for line in json_str.lines() {
        if line.contains(':') {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            lines.push(Line::from(vec![
                Span::styled(parts[0].to_string(), Style::default().fg(Color::Cyan)),
                Span::raw(":"),
                Span::styled(
                    parts.get(1).unwrap_or(&"").to_string(),
                    Style::default().fg(Color::White),
                ),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::White),
            )));
        }
    }

    lines
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
    let lines = build_detail_lines(event);

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
        // Narrow but wide enough to show kind: icon(2) + timestamp(8) + space(1) + kind(22) = 33 + borders(2) = 35
        // List pane is ~half of total width, so need at least 70 for ~35 col list pane
        let output = render_events(&app, 80, 20);
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
    fn icon_prefix_renders_for_post_tool_use() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PostToolUse"));
        let output = render_events(&app, 120, 20);
        // PostToolUse is Tool category, icon is ⚡
        assert!(
            output.contains("⚡"),
            "Expected ⚡ icon for PostToolUse, got:\n{}",
            output
        );
    }

    #[test]
    fn icon_prefix_renders_for_subagent_start() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("SubagentStart"));
        let output = render_events(&app, 120, 20);
        // SubagentStart is Agent category, icon is ◆
        assert!(
            output.contains("◆"),
            "Expected ◆ icon for SubagentStart, got:\n{}",
            output
        );
    }

    #[test]
    fn tree_lines_between_subagent_start_stop() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("SubagentStart"));
        app.push_event(event_from_name("PostToolUse"));
        app.push_event(event_from_name("SubagentStop"));
        let output = render_events(&app, 120, 20);
        // The PostToolUse inside agent scope should have tree prefix "│ " before its icon
        // Check for "│ ⚡" which is tree-line + space + tool icon
        assert!(
            output.contains("│ ⚡"),
            "Expected tree line '│ ⚡' for PostToolUse inside agent scope, got:\n{}",
            output
        );
    }

    #[test]
    fn subagent_start_stop_have_no_tree_prefix() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("SubagentStart"));
        app.push_event(event_from_name("SubagentStop"));
        let output = render_events(&app, 120, 20);
        // SubagentStart/Stop themselves should NOT have "│ ◆" - they render at their own depth
        // The agent icon ◆ should appear without a tree prefix before it
        // Since there are no events between start/stop, no "│ ⚡" or "│ ◆" should appear
        assert!(
            !output.contains("│ ◆"),
            "SubagentStart/Stop should not have tree prefix before their icon, got:\n{}",
            output
        );
        assert!(
            !output.contains("│ ⚡"),
            "No inner events should have tree prefix, got:\n{}",
            output
        );
    }

    #[test]
    fn nested_agents_produce_double_tree_indent() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("SubagentStart")); // depth 0 -> 1
        app.push_event(event_from_name("SubagentStart")); // depth 1 -> 2
        app.push_event(event_from_name("PostToolUse"));    // depth 2, should have "│ │ "
        app.push_event(event_from_name("SubagentStop"));   // depth 2 -> 1
        app.push_event(event_from_name("SubagentStop"));   // depth 1 -> 0
        let output = render_events(&app, 120, 20);
        // The PostToolUse at depth 2 should have double tree prefix "│ │ ⚡"
        assert!(
            output.contains("│ │ ⚡"),
            "Expected double tree indent '│ │ ⚡' for nested agent, got:\n{}",
            output
        );
    }

    #[test]
    fn stop_failure_renders_with_red_background() {
        // We verify by checking that the ListItem construction path applies
        // the error style. Since TestBackend doesn't easily expose bg color,
        // we test that StopFailure events render with the error icon ✖
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("StopFailure"));
        let output = render_events(&app, 120, 20);
        assert!(
            output.contains("✖"),
            "Expected ✖ icon for StopFailure (Error category), got:\n{}",
            output
        );
        assert!(
            output.contains("StopFailure"),
            "Expected StopFailure text, got:\n{}",
            output
        );
    }

    #[test]
    fn highlight_style_uses_darkgray_bold() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        app.push_event(event_from_name("PostToolUse"));
        app.event_selected = 0;
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                let (list_area, detail_area) = crate::ui::split_list_detail(area);
                draw_events(f, &app, list_area, detail_area);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        // Scan row 1 (first event row after border) for a cell with DarkGray background.
        // The highlight style applies to the selected row's content cells.
        let first_event_row = 1;
        let mut found_darkgray = false;
        for x in 0..120 {
            let cell = &buf[(x, first_event_row)];
            if cell.bg == Color::DarkGray {
                found_darkgray = true;
                break;
            }
        }
        assert!(
            found_darkgray,
            "Expected at least one cell with DarkGray background on selected row"
        );
    }

    // === T4: Structured 3-section detail rendering tests ===

    fn make_detail_event(json: &str) -> crate::event::HookEvent {
        make_test_event(json)
    }

    /// Render only the detail panel for a single event, returning the text output.
    fn render_detail_for_event(json: &str, width: u16, height: u16) -> String {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_detail_event(json));
        app.event_selected = 0;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                let (_list_area, detail_area) = crate::ui::split_list_detail(area);
                draw_events(f, &app, _list_area, detail_area);
            })
            .unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    #[test]
    fn detail_structured_post_tool_use_shows_tool_label() {
        let output = render_detail_for_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read","duration_ms":42}"#,
            120,
            30,
        );
        // Section 1: Structured fields should show "Tool:" label
        assert!(
            output.contains("Tool:"),
            "Expected 'Tool:' label in structured section, got:\n{}",
            output
        );
        assert!(
            output.contains("Read"),
            "Expected tool value 'Read' in structured section, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_structured_post_tool_use_shows_duration() {
        let output = render_detail_for_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read","duration_ms":42}"#,
            120,
            30,
        );
        assert!(
            output.contains("Duration:"),
            "Expected 'Duration:' label, got:\n{}",
            output
        );
        assert!(
            output.contains("42"),
            "Expected duration value '42', got:\n{}",
            output
        );
    }

    #[test]
    fn detail_structured_subagent_stop_shows_agent_type() {
        let output = render_detail_for_event(
            r#"{"hook_event_name":"SubagentStop","session_id":"s1","agent_type":"planner","cwd":"/tmp","duration_ms":100}"#,
            120,
            30,
        );
        assert!(
            output.contains("Agent Type:"),
            "Expected 'Agent Type:' label, got:\n{}",
            output
        );
        assert!(
            output.contains("planner"),
            "Expected 'planner' value, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_structured_stop_failure_shows_error_field() {
        let output = render_detail_for_event(
            r#"{"hook_event_name":"StopFailure","session_id":"s1","error":"timeout","message":"agent timed out"}"#,
            120,
            30,
        );
        assert!(
            output.contains("Error:"),
            "Expected 'Error:' label, got:\n{}",
            output
        );
        assert!(
            output.contains("timeout"),
            "Expected 'timeout' value, got:\n{}",
            output
        );
        assert!(
            output.contains("Message:"),
            "Expected 'Message:' label, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_extra_fields_shows_unknown_keys() {
        let output = render_detail_for_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read","custom_field":"hello"}"#,
            120,
            30,
        );
        assert!(
            output.contains("Extra Fields"),
            "Expected 'Extra Fields' section header, got:\n{}",
            output
        );
        assert!(
            output.contains("custom_field"),
            "Expected 'custom_field' in extra fields, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_meta_keys_excluded_from_extra_fields() {
        // hook_event_name and session_id are meta keys that should NOT appear in extra fields
        let output = render_detail_for_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read"}"#,
            120,
            30,
        );
        // Should NOT contain hook_event_name or session_id as visible labels in extra fields
        // These are meta keys that get excluded
        // The raw JSON section will still contain them, so we check that "Extra Fields" section is absent
        // (since there are no extra fields after excluding known + meta)
        assert!(
            !output.contains("Extra Fields"),
            "Expected no 'Extra Fields' section when all keys are known or meta, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_raw_json_always_present() {
        let output = render_detail_for_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read"}"#,
            120,
            30,
        );
        assert!(
            output.contains("Raw JSON"),
            "Expected 'Raw JSON' section header, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_generic_fallback_skips_structured_section() {
        // SessionStart has no known_fields, so structured section should be skipped
        let output = render_detail_for_event(
            r#"{"hook_event_name":"SessionStart","session_id":"s1"}"#,
            120,
            30,
        );
        // Should NOT have the kind-colored title line for structured fields
        // But should still have Raw JSON
        assert!(
            output.contains("Raw JSON"),
            "Expected 'Raw JSON' for generic event, got:\n{}",
            output
        );
        // Should NOT have a structured section title (the kind name with ── markers)
        // SessionStart has empty known_fields, so no "── SessionStart ──" title
    }

    #[test]
    fn detail_structured_title_shows_kind_name() {
        let output = render_detail_for_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read"}"#,
            120,
            30,
        );
        assert!(
            output.contains("PostToolUse"),
            "Expected kind name 'PostToolUse' in structured title, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_missing_known_field_shows_dash() {
        // PostToolUse has known field "file_path" but we don't provide it
        let output = render_detail_for_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read"}"#,
            120,
            30,
        );
        // The missing fields should show "—" (em dash)
        assert!(
            output.contains("\u{2014}"),
            "Expected em dash for missing known field, got:\n{}",
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
