use chrono::Utc;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, SessionFocus, SessionRecord, ToolRecord};
use super::helpers::{format_duration, format_relative_time_dt, render_scroll_indicators};

/// Extracts the last path segment if the path looks like a worktree.
fn extract_worktree_suffix(path: &str) -> Option<&str> {
    if path.contains("/worktrees/") || path.contains("/tmp/claude-config-") {
        let trimmed = path.trim_end_matches('/');
        trimmed.rsplit('/').next().filter(|s| !s.is_empty())
    } else {
        None
    }
}

/// Formats tool records with counts and optional failure summary.
/// Produces a string like "Read x12, Edit x3 [1 failed]".
fn format_tools_line<'a>(tools: impl Iterator<Item = &'a ToolRecord>) -> String {
    let mut total_failures: usize = 0;
    let mut parts: Vec<String> = Vec::new();
    for t in tools {
        parts.push(format!("{} x{}", t.name, t.count));
        total_failures += t.failure_count;
    }
    if total_failures > 0 {
        parts.push(format!("[{} failed]", total_failures));
    }
    parts.join(", ")
}

fn pane_block(title: &str, focused: bool) -> Block<'_> {
    let style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::default()
        .borders(Borders::ALL)
        .title(title.to_string())
        .border_style(style)
}

pub fn draw_sessions(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(30),
            Constraint::Percentage(45),
        ])
        .split(area);

    draw_session_list(f, app, chunks[0]);
    draw_segment_list(f, app, chunks[1]);
    draw_agent_tree(f, app, chunks[2]);
}

fn draw_session_list(f: &mut Frame, app: &App, area: Rect) {
    let sessions = app.visible_session_records();
    let focused = app.session_focus == SessionFocus::List;

    if sessions.is_empty() {
        let block = pane_block("Sessions (0)", focused);
        let msg = Paragraph::new("No sessions \u{2014} waiting for events...")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let now = Utc::now();
    let blink_on = now.timestamp_millis() % 1000 < 500;

    // Find boundary between active and inactive for separator
    let first_inactive_idx = sessions.iter().position(|s| s.ended);

    let mut items: Vec<ListItem> = Vec::new();
    for (i, session) in sessions.iter().enumerate() {
        // Insert separator before first inactive session
        if Some(i) == first_inactive_idx && i > 0 {
            let sep = Line::from(Span::styled(
                "\u{2500}\u{2500}\u{2500} inactive \u{2500}\u{2500}\u{2500}",
                Style::default().fg(Color::DarkGray),
            ));
            items.push(ListItem::new(sep));
        }

        let (indicator, indicator_color) = session_indicator(session, &now, blink_on);

        let id_len = 8.min(session.session_id.len());
        let session_id_short = &session.session_id[..id_len];
        let time_ago = format_relative_time_dt(&session.last_event_at);

        let id_style = if !session.ended {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let line = Line::from(vec![
            Span::styled(format!("{} ", indicator), Style::default().fg(indicator_color)),
            Span::styled(session_id_short.to_string(), id_style),
            Span::raw(" "),
            Span::styled(time_ago, Style::default().fg(Color::DarkGray)),
        ]);
        items.push(ListItem::new(line));
    }

    let title = format!("Sessions ({})", sessions.len());
    let list = List::new(items)
        .block(pane_block(&title, focused))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    // Account for separator offset when mapping selection to list index
    let selected = app.session_selected.min(sessions.len().saturating_sub(1));
    let list_index = if let Some(inactive_start) = first_inactive_idx {
        if selected >= inactive_start && inactive_start > 0 {
            selected + 1 // +1 for separator item
        } else {
            selected
        }
    } else {
        selected
    };

    let mut state = ListState::default();
    state.select(Some(list_index));
    f.render_stateful_widget(list, area, &mut state);
}

fn session_indicator(
    session: &SessionRecord,
    now: &chrono::DateTime<Utc>,
    blink_on: bool,
) -> (&'static str, Color) {
    if session.ended {
        ("\u{25CB}", Color::DarkGray) // empty circle
    } else {
        let elapsed = *now - session.last_event_at;
        if elapsed.num_seconds() < 5 {
            // Live -- blinking
            if blink_on {
                ("\u{25C9}", Color::Green) // fisheye
            } else {
                ("\u{25C9}", Color::DarkGray) // fisheye dim phase
            }
        } else {
            ("\u{25CF}", Color::Green) // solid circle
        }
    }
}

fn draw_segment_list(f: &mut Frame, app: &App, area: Rect) {
    let sessions = app.visible_session_records();
    let focused = app.session_focus == SessionFocus::Segment;

    if sessions.is_empty() {
        let block = pane_block("Segments", focused);
        f.render_widget(block, area);
        return;
    }

    let selected_session_idx = app.session_selected.min(sessions.len().saturating_sub(1));
    let session = sessions[selected_session_idx];

    if session.prompt_segments.is_empty() {
        let block = pane_block("Segments (0)", focused);
        let msg = Paragraph::new("Awaiting first prompt...")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    // Show segments in reverse order (newest first)
    let segments_reversed: Vec<(usize, _)> = session
        .prompt_segments
        .iter()
        .enumerate()
        .rev()
        .collect();

    let items: Vec<ListItem> = segments_reversed
        .iter()
        .map(|(orig_idx, segment)| {
            let indicator = if segment.ended_at.is_none() {
                "\u{25CF}" // solid circle active
            } else {
                "\u{2713}" // checkmark completed
            };

            let indicator_color = if segment.ended_at.is_none() {
                Color::Green
            } else {
                Color::DarkGray
            };

            // Truncate prompt text to 30 characters (UTF-8 safe)
            let prompt_display = if segment.prompt_text.chars().count() > 30 {
                let truncated: String = segment.prompt_text.chars().take(30).collect();
                format!("{}...", truncated)
            } else {
                segment.prompt_text.clone()
            };

            let time_ago = format_relative_time_dt(&segment.started_at);
            let seg_num = orig_idx + 1;

            let line = Line::from(vec![
                Span::styled(
                    format!("{:>2} ", seg_num),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{} ", indicator),
                    Style::default().fg(indicator_color),
                ),
                Span::styled(
                    format!("{:<30}", prompt_display),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(" {}", time_ago),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = format!("Segments ({})", session.prompt_segments.len());
    let list = List::new(items)
        .block(pane_block(&title, focused))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    // Map segment_selected (which is in original order) to reversed display index
    let seg_selected = app
        .session_segment_selected
        .min(session.prompt_segments.len().saturating_sub(1));
    let reversed_idx = session.prompt_segments.len().saturating_sub(1) - seg_selected;

    let mut state = ListState::default();
    state.select(Some(reversed_idx));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_agent_tree(f: &mut Frame, app: &App, area: Rect) {
    let sessions = app.visible_session_records();
    let focused = app.session_focus == SessionFocus::Detail;

    if sessions.is_empty() {
        let block = pane_block("Detail", focused);
        f.render_widget(block, area);
        return;
    }

    let selected_session_idx = app.session_selected.min(sessions.len().saturating_sub(1));
    let session = sessions[selected_session_idx];

    if session.prompt_segments.is_empty() {
        let block = pane_block("Detail", focused);
        f.render_widget(block, area);
        return;
    }

    let seg_idx = app
        .session_segment_selected
        .min(session.prompt_segments.len().saturating_sub(1));
    let segment = &session.prompt_segments[seg_idx];

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Header: segment number + full prompt text
    let seg_num = seg_idx + 1;
    lines.push(Line::from(Span::styled(
        format!("#{} \"{}\"", seg_num, segment.prompt_text),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        "\u{2550}".repeat(40),
        Style::default().fg(Color::DarkGray),
    )));

    // Session age from first_seen_at
    let session_age = format_relative_time_dt(&session.first_seen_at);
    lines.push(Line::from(Span::styled(
        format!("Session started {}", session_age),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    // Orchestrator context (instructions loaded at orchestrator level)
    let oc = &segment.orchestrator_context;
    let has_orch_ctx = !oc.agent_definitions.is_empty()
        || !oc.skills.is_empty()
        || !oc.rules.is_empty()
        || !oc.memory.is_empty()
        || !oc.other.is_empty();
    if has_orch_ctx {
        lines.push(Line::from(Span::styled(
            "Orchestrator Context",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
        if !oc.memory.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  Memory: {}", oc.memory.join(", ")),
                Style::default().fg(Color::DarkGray),
            )));
        }
        if !oc.rules.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  Rules: {}", oc.rules.join(", ")),
                Style::default().fg(Color::DarkGray),
            )));
        }
        if !oc.skills.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  Skills: {}", oc.skills.join(", ")),
                Style::default().fg(Color::DarkGray),
            )));
        }
        if !oc.agent_definitions.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  Agents: {}", oc.agent_definitions.join(", ")),
                Style::default().fg(Color::DarkGray),
            )));
        }
        if !oc.other.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  Other: {}", oc.other.join(", ")),
                Style::default().fg(Color::DarkGray),
            )));
        }
        lines.push(Line::from(""));
    }

    if segment.agents.is_empty() && segment.orchestrator_tools.is_empty() {
        lines.push(Line::from(Span::styled(
            "Direct response \u{2014} no agents spawned",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        // Render each agent
        for agent_id in &segment.agents {
            if let Some(agent) = session.agent_records.iter().find(|a| a.id == *agent_id) {
                render_agent_node(&mut lines, agent);
            }
        }

        // Orchestrator tools section
        if !segment.orchestrator_tools.is_empty() {
            let tools_str = format_tools_line(segment.orchestrator_tools.iter());
            lines.push(Line::from(Span::styled(
                format!("\u{2514}\u{2500} Orchestrator Tools: {}", tools_str),
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
        }

        // Tasks summary
        if !segment.tasks.is_empty() {
            let task_parts: Vec<String> = segment
                .tasks
                .iter()
                .map(|t| {
                    let status = if t.completed { "\u{2713}" } else { "\u{25CF}" };
                    let name = t
                        .teammate_name
                        .as_deref()
                        .unwrap_or(&t.task_id);
                    format!("{} {} ({})", t.task_id, status, name)
                })
                .collect();
            lines.push(Line::from(Span::styled(
                format!("Tasks: {}", task_parts.join(" \u{2502} ")),
                Style::default().fg(Color::Yellow),
            )));
        }
    }

    let total_lines = lines.len();
    let block = pane_block("Detail", focused);
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.session_detail_scroll as u16, 0));
    f.render_widget(paragraph, area);

    render_scroll_indicators(f, area, total_lines, app.session_detail_scroll);
}

fn render_agent_node(lines: &mut Vec<Line<'static>>, agent: &crate::app::AgentRecord) {
    // Agent header
    let mut header = format!("\u{25BE} {}", agent.agent_type);
    if let Some(ref cwd) = agent.cwd {
        if let Some(suffix) = extract_worktree_suffix(cwd) {
            header.push_str(&format!(" @{}", suffix));
        }
    }
    let status_str = if agent.is_active() {
        let elapsed = Utc::now() - agent.started_at;
        format!("\u{25C9} {}", format_duration(elapsed))
    } else {
        let duration = agent
            .ended_at
            .map(|end| end - agent.started_at)
            .unwrap_or_else(chrono::Duration::zero);
        format!("completed, {}", format_duration(duration))
    };
    header.push_str(&format!(" ({})", status_str));

    let header_color = if agent.is_active() {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    lines.push(Line::from(Span::styled(
        header,
        Style::default()
            .fg(header_color)
            .add_modifier(Modifier::BOLD),
    )));

    // Context sub-tree
    let ctx = &agent.context;
    let has_context = !ctx.agent_definitions.is_empty()
        || !ctx.skills.is_empty()
        || !ctx.rules.is_empty()
        || !ctx.memory.is_empty();

    if has_context {
        lines.push(Line::from(Span::styled(
            "\u{2502}  \u{251C}\u{2500} Context",
            Style::default().fg(Color::DarkGray),
        )));

        let mut context_entries: Vec<(String, String)> = Vec::new();
        if !ctx.agent_definitions.is_empty() {
            context_entries.push(("Agent".to_string(), ctx.agent_definitions.join(", ")));
        }
        if !ctx.skills.is_empty() {
            context_entries.push(("Skills".to_string(), ctx.skills.join(", ")));
        }
        if !ctx.rules.is_empty() {
            context_entries.push(("Rules".to_string(), ctx.rules.join(", ")));
        }
        if !ctx.memory.is_empty() {
            context_entries.push(("Memory".to_string(), ctx.memory.join(", ")));
        }

        for (i, (label, value)) in context_entries.iter().enumerate() {
            let connector = if i == context_entries.len() - 1 && agent.tools.is_empty() {
                "\u{2514}" // corner
            } else {
                "\u{251C}" // tee
            };
            lines.push(Line::from(Span::styled(
                format!(
                    "\u{2502}  \u{2502}  {}\u{2500} {}: {}",
                    connector, label, value
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Tools line
    if !agent.tools.is_empty() {
        let tools_str = format_tools_line(agent.tools.iter());
        lines.push(Line::from(Span::styled(
            format!("\u{2502}  \u{251C}\u{2500} Tools: {}", tools_str),
            Style::default().fg(Color::White),
        )));
    }

    // Blank line between agents
    lines.push(Line::from(Span::styled(
        "\u{2502}",
        Style::default().fg(Color::DarkGray),
    )));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AgentContext, AgentRecord, App, PromptSegment, SessionRecord, TaskInfo, ToolRecord};
    use crate::config::ConfigInventory;
    use crate::test_helpers::test_utils::buffer_to_string;
    use chrono::{Duration, Utc};
    use ratatui::{backend::TestBackend, Terminal};

    fn render_sessions_widget(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                draw_sessions(f, app, area);
            })
            .unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    fn make_session_record(id: &str, ended: bool) -> SessionRecord {
        let now = Utc::now();
        SessionRecord {
            session_id: id.to_string(),
            first_seen_at: now - Duration::minutes(10),
            last_event_at: now - Duration::seconds(if ended { 300 } else { 2 }),
            ended,
            agent_records: Vec::new(),
            prompt_segments: vec![PromptSegment {
                prompt_text: "(session initialization)".to_string(),
                started_at: now - Duration::minutes(10),
                ended_at: None,
                agents: Vec::new(),
                orchestrator_tools: Vec::new(),
                orchestrator_context: AgentContext::default(),
                tasks: Vec::new(),
            }],
            next_agent_id: 0,
        }
    }

    fn make_agent_record(id: u64, agent_type: &str, active: bool) -> AgentRecord {
        let now = Utc::now();
        AgentRecord {
            id,
            agent_type: agent_type.to_string(),
            cwd: None,
            started_at: now - Duration::minutes(5),
            ended_at: if active { None } else { Some(now - Duration::minutes(1)) },
            context: AgentContext::default(),
            tools: Vec::new(),
        }
    }

    // --- Empty state tests ---

    #[test]
    fn empty_state_shows_no_sessions_message() {
        let app = App::new(ConfigInventory::default());
        // Use wide terminal so the message fits in the 25%-width sessions pane
        let output = render_sessions_widget(&app, 200, 20);
        assert!(output.contains("No sessions"), "Expected 'No sessions' message, got:\n{}", output);
        assert!(output.contains("waiting for events"), "Expected 'waiting for events' message, got:\n{}", output);
    }

    #[test]
    fn empty_state_shows_sessions_zero_count() {
        let app = App::new(ConfigInventory::default());
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("Sessions (0)"), "Expected 'Sessions (0)' title, got:\n{}", output);
    }

    #[test]
    fn empty_segments_shows_awaiting_prompt() {
        let mut app = App::new(ConfigInventory::default());
        let now = Utc::now();
        app.session_records.insert("sess1234".to_string(), SessionRecord {
            session_id: "sess1234".to_string(),
            first_seen_at: now,
            last_event_at: now,
            ended: false,
            agent_records: Vec::new(),
            prompt_segments: Vec::new(),
            next_agent_id: 0,
        });
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("Awaiting first prompt"), "Expected 'Awaiting first prompt' message, got:\n{}", output);
    }

    #[test]
    fn empty_agents_shows_direct_response() {
        let mut app = App::new(ConfigInventory::default());
        let session = make_session_record("sess12345678", false);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("Direct response"), "Expected 'Direct response' message, got:\n{}", output);
        assert!(output.contains("no agents spawned"), "Expected 'no agents spawned' message, got:\n{}", output);
    }

    // --- Session list tests ---

    #[test]
    fn session_list_shows_truncated_id() {
        let mut app = App::new(ConfigInventory::default());
        let session = make_session_record("abcdef1234567890", false);
        app.session_records.insert("abcdef1234567890".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("abcdef12"), "Expected truncated session ID 'abcdef12', got:\n{}", output);
    }

    #[test]
    fn session_list_shows_active_indicator() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("abcdef1234567890", false);
        session.last_event_at = Utc::now() - Duration::seconds(30);
        app.session_records.insert("abcdef1234567890".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("\u{25CF}"), "Expected solid green indicator, got:\n{}", output);
    }

    #[test]
    fn session_list_shows_inactive_indicator() {
        let mut app = App::new(ConfigInventory::default());
        let session = make_session_record("abcdef1234567890", true);
        app.session_records.insert("abcdef1234567890".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("\u{25CB}"), "Expected inactive indicator, got:\n{}", output);
    }

    #[test]
    fn session_list_shows_inactive_separator() {
        let mut app = App::new(ConfigInventory::default());
        let active = make_session_record("active_session_1", false);
        let inactive = make_session_record("inactive_sess_1", true);
        app.session_records.insert("active_session_1".to_string(), active);
        app.session_records.insert("inactive_sess_1".to_string(), inactive);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("inactive"), "Expected 'inactive' separator, got:\n{}", output);
    }

    // --- Segment list tests ---

    #[test]
    fn segment_list_shows_prompt_text_truncated() {
        let mut app = App::new(ConfigInventory::default());
        let now = Utc::now();
        let mut session = make_session_record("sess12345678", false);
        session.prompt_segments.push(PromptSegment {
            prompt_text: "This is a very long prompt that should be truncated at thirty chars".to_string(),
            started_at: now - Duration::minutes(5),
            ended_at: Some(now - Duration::minutes(3)),
            agents: Vec::new(),
            orchestrator_tools: Vec::new(),
            orchestrator_context: AgentContext::default(),
            tasks: Vec::new(),
        });
        app.session_records.insert("sess12345678".to_string(), session);
        // Use wider terminal so the segment pane (30%) can display the 30-char truncated text
        let output = render_sessions_widget(&app, 200, 20);
        assert!(output.contains("This is a very long prompt tha"), "Expected truncated prompt text, got:\n{}", output);
    }

    #[test]
    fn segment_list_shows_session_init() {
        let mut app = App::new(ConfigInventory::default());
        let session = make_session_record("sess12345678", false);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("session initialization"), "Expected '(session initialization)' for segment zero, got:\n{}", output);
    }

    #[test]
    fn segment_list_shows_completed_indicator() {
        let mut app = App::new(ConfigInventory::default());
        let now = Utc::now();
        let mut session = make_session_record("sess12345678", false);
        session.prompt_segments[0].ended_at = Some(now - Duration::minutes(1));
        session.prompt_segments.push(PromptSegment {
            prompt_text: "active prompt".to_string(),
            started_at: now,
            ended_at: None,
            agents: Vec::new(),
            orchestrator_tools: Vec::new(),
            orchestrator_context: AgentContext::default(),
            tasks: Vec::new(),
        });
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("\u{2713}"), "Expected checkmark for completed segment, got:\n{}", output);
    }

    // --- Agent tree tests ---

    #[test]
    fn agent_tree_shows_agent_type_and_status() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("sess12345678", false);
        let agent = make_agent_record(0, "planner", false);
        session.agent_records.push(agent);
        session.prompt_segments[0].agents.push(0);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("planner"), "Expected agent type 'planner', got:\n{}", output);
        assert!(output.contains("completed"), "Expected 'completed' status, got:\n{}", output);
    }

    #[test]
    fn agent_tree_shows_active_agent() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("sess12345678", false);
        let agent = make_agent_record(0, "tdd-implementer", true);
        session.agent_records.push(agent);
        session.prompt_segments[0].agents.push(0);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        // Active agent shows fisheye indicator with elapsed time, e.g. "(◉ 5m 0s)"
        assert!(output.contains("\u{25C9}"), "Expected active indicator (fisheye), got:\n{}", output);
    }

    #[test]
    fn agent_tree_shows_context_classification() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("sess12345678", false);
        let mut agent = make_agent_record(0, "planner", true);
        agent.context = AgentContext {
            agent_definitions: vec!["planner.md".to_string()],
            skills: vec!["gh-cli".to_string(), "commit-convention".to_string()],
            rules: vec!["workflow.md".to_string(), "code-quality.md".to_string()],
            memory: vec!["CLAUDE.md".to_string()],
            other: Vec::new(),
        };
        session.agent_records.push(agent);
        session.prompt_segments[0].agents.push(0);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 30);
        assert!(output.contains("Context"), "Expected 'Context' sub-tree, got:\n{}", output);
        assert!(output.contains("Agent: planner.md"), "Expected agent definition, got:\n{}", output);
        assert!(output.contains("Skills: gh-cli, commit-convention"), "Expected skills, got:\n{}", output);
        assert!(output.contains("Rules: workflow.md, code-quality.md"), "Expected rules, got:\n{}", output);
        assert!(output.contains("Memory: CLAUDE.md"), "Expected memory, got:\n{}", output);
    }

    #[test]
    fn agent_tree_shows_tools_with_failure_count() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("sess12345678", false);
        let mut agent = make_agent_record(0, "tdd-implementer", true);
        agent.tools = vec![
            ToolRecord { name: "Read".to_string(), count: 12, failure_count: 0 },
            ToolRecord { name: "Edit".to_string(), count: 3, failure_count: 1 },
        ];
        session.agent_records.push(agent);
        session.prompt_segments[0].agents.push(0);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("Read x12"), "Expected 'Read x12', got:\n{}", output);
        assert!(output.contains("Edit x3"), "Expected 'Edit x3', got:\n{}", output);
        assert!(output.contains("[1 failed]"), "Expected '[1 failed]', got:\n{}", output);
    }

    #[test]
    fn agent_tree_shows_worktree_suffix() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("sess12345678", false);
        let mut agent = make_agent_record(0, "tdd-implementer", true);
        agent.cwd = Some("/Users/kb/.claude/worktrees/agent-abc123".to_string());
        session.agent_records.push(agent);
        session.prompt_segments[0].agents.push(0);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("@agent-abc123"), "Expected worktree suffix, got:\n{}", output);
    }

    #[test]
    fn agent_tree_shows_orchestrator_tools() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("sess12345678", false);
        session.prompt_segments[0].orchestrator_tools = vec![
            ToolRecord { name: "Agent".to_string(), count: 3, failure_count: 0 },
            ToolRecord { name: "Read".to_string(), count: 2, failure_count: 0 },
        ];
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("Orchestrator Tools"), "Expected 'Orchestrator Tools' section, got:\n{}", output);
        assert!(output.contains("Agent x3"), "Expected 'Agent x3', got:\n{}", output);
    }

    #[test]
    fn agent_tree_shows_tasks_summary() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("sess12345678", false);
        session.prompt_segments[0].tasks = vec![
            TaskInfo { task_id: "T1".to_string(), teammate_name: Some("login flow".to_string()), completed: true },
            TaskInfo { task_id: "T2".to_string(), teammate_name: Some("error handling".to_string()), completed: false },
        ];
        session.prompt_segments[0].orchestrator_tools = vec![
            ToolRecord { name: "Agent".to_string(), count: 2, failure_count: 0 },
        ];
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("Tasks:"), "Expected 'Tasks:' section, got:\n{}", output);
        assert!(output.contains("T1"), "Expected task T1, got:\n{}", output);
        assert!(output.contains("T2"), "Expected task T2, got:\n{}", output);
    }

    // --- Agent duration tests ---

    #[test]
    fn completed_agent_shows_duration() {
        let mut app = App::new(ConfigInventory::default());
        let now = Utc::now();
        let mut session = make_session_record("sess12345678", false);
        let agent = AgentRecord {
            id: 0,
            agent_type: "planner".to_string(),
            cwd: None,
            started_at: now - Duration::minutes(2) - Duration::seconds(34),
            ended_at: Some(now),
            context: AgentContext::default(),
            tools: Vec::new(),
        };
        session.agent_records.push(agent);
        session.prompt_segments[0].agents.push(0);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("completed, 2m 34s"), "Expected 'completed, 2m 34s', got:\n{}", output);
    }

    #[test]
    fn active_agent_shows_elapsed_time() {
        let mut app = App::new(ConfigInventory::default());
        let now = Utc::now();
        let mut session = make_session_record("sess12345678", false);
        let agent = AgentRecord {
            id: 0,
            agent_type: "tdd-implementer".to_string(),
            cwd: None,
            started_at: now - Duration::seconds(45),
            ended_at: None,
            context: AgentContext::default(),
            tools: Vec::new(),
        };
        session.agent_records.push(agent);
        session.prompt_segments[0].agents.push(0);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        // Active agent should show elapsed time with the active indicator
        assert!(output.contains("◉"), "Expected active indicator, got:\n{}", output);
        // Should contain seconds-range duration (the exact number may vary by ~1s)
        assert!(output.contains("s)"), "Expected elapsed time ending with 's)', got:\n{}", output);
    }

    // --- Session age test ---

    #[test]
    fn detail_pane_shows_session_started_age() {
        let mut app = App::new(ConfigInventory::default());
        let now = Utc::now();
        let mut session = make_session_record("sess12345678", false);
        session.first_seen_at = now - Duration::minutes(10);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("Session started"), "Expected 'Session started' line, got:\n{}", output);
        assert!(output.contains("10m ago"), "Expected '10m ago', got:\n{}", output);
    }

    // --- Safety tests ---

    #[test]
    fn does_not_panic_with_small_terminal() {
        let mut app = App::new(ConfigInventory::default());
        let session = make_session_record("sess12345678", false);
        app.session_records.insert("sess12345678".to_string(), session);
        let _output = render_sessions_widget(&app, 30, 5);
    }

    #[test]
    fn does_not_panic_with_out_of_bounds_session_selection() {
        let mut app = App::new(ConfigInventory::default());
        let session = make_session_record("sess12345678", false);
        app.session_records.insert("sess12345678".to_string(), session);
        app.session_selected = 999;
        let _output = render_sessions_widget(&app, 120, 20);
    }

    #[test]
    fn does_not_panic_with_out_of_bounds_segment_selection() {
        let mut app = App::new(ConfigInventory::default());
        let session = make_session_record("sess12345678", false);
        app.session_records.insert("sess12345678".to_string(), session);
        app.session_segment_selected = 999;
        let _output = render_sessions_widget(&app, 120, 20);
    }

    // --- Utility function tests ---

    #[test]
    fn extract_worktree_suffix_from_worktrees_path() {
        assert_eq!(extract_worktree_suffix("/Users/kb/.claude/worktrees/agent-abc123"), Some("agent-abc123"));
    }

    #[test]
    fn extract_worktree_suffix_from_config_path() {
        assert_eq!(extract_worktree_suffix("/tmp/claude-config-planner"), Some("claude-config-planner"));
    }

    #[test]
    fn extract_worktree_suffix_returns_none_for_normal_path() {
        assert_eq!(extract_worktree_suffix("/Users/kb/projects/myapp"), None);
    }

    #[test]
    fn extract_worktree_suffix_handles_trailing_slash() {
        assert_eq!(extract_worktree_suffix("/Users/kb/.claude/worktrees/agent-abc123/"), Some("agent-abc123"));
    }

    #[test]
    fn extract_worktree_suffix_returns_none_for_empty_string() {
        assert_eq!(extract_worktree_suffix(""), None);
    }

    #[test]
    fn format_tools_line_basic() {
        let tools = vec![
            ToolRecord { name: "Read".to_string(), count: 5, failure_count: 0 },
            ToolRecord { name: "Edit".to_string(), count: 3, failure_count: 0 },
        ];
        let result = format_tools_line(tools.iter());
        assert!(result.contains("Read x5"));
        assert!(result.contains("Edit x3"));
        assert!(!result.contains("failed"));
    }

    #[test]
    fn format_tools_line_with_failures() {
        let tools = vec![
            ToolRecord { name: "Edit".to_string(), count: 4, failure_count: 2 },
        ];
        let result = format_tools_line(tools.iter());
        assert!(result.contains("Edit x4"));
        assert!(result.contains("[2 failed]"));
    }

    // --- Three-pane layout test ---

    #[test]
    fn three_panes_visible_simultaneously() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("sess12345678", false);
        let agent = make_agent_record(0, "planner", true);
        session.agent_records.push(agent);
        session.prompt_segments[0].agents.push(0);
        app.session_records.insert("sess12345678".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        assert!(output.contains("Sessions ("), "Expected Sessions pane");
        assert!(output.contains("Segments ("), "Expected Segments pane");
        assert!(output.contains("Detail"), "Expected Detail pane");
    }

    #[test]
    fn focused_pane_renders_without_panic() {
        let mut app = App::new(ConfigInventory::default());
        let session = make_session_record("sess12345678", false);
        app.session_records.insert("sess12345678".to_string(), session);

        app.session_focus = SessionFocus::List;
        let _output = render_sessions_widget(&app, 120, 20);

        app.session_focus = SessionFocus::Segment;
        let _output = render_sessions_widget(&app, 120, 20);

        app.session_focus = SessionFocus::Detail;
        let _output = render_sessions_widget(&app, 120, 20);
    }

    // --- Live indicator test ---

    #[test]
    fn session_list_shows_live_indicator() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("abcdef1234567890", false);
        // last_event_at within 5 seconds -> live indicator
        session.last_event_at = Utc::now() - Duration::seconds(1);
        app.session_records.insert("abcdef1234567890".to_string(), session);
        let output = render_sessions_widget(&app, 120, 20);
        // Live indicator is U+25C9 (fisheye)
        assert!(output.contains("\u{25C9}"), "Expected live indicator (fisheye), got:\n{}", output);
    }

    // --- B1: UTF-8 safe truncation ---

    #[test]
    fn segment_panel_renders_korean_prompt_without_panic() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("sess12345678", false);
        // Korean text that would panic with byte-slicing at position 30
        session.prompt_segments.push(PromptSegment {
            prompt_text: "리팩터링을 진행해주세요 이것은 긴 한국어 프롬프트입니다".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            agents: Vec::new(),
            orchestrator_tools: Vec::new(),
            orchestrator_context: AgentContext::default(),
            tasks: Vec::new(),
        });
        app.session_records
            .insert("sess12345678".to_string(), session);
        app.session_focus = SessionFocus::Segment;
        // Must not panic
        let _output = render_sessions_widget(&app, 120, 20);
    }

    #[test]
    fn segment_panel_renders_emoji_prompt_without_panic() {
        let mut app = App::new(ConfigInventory::default());
        let mut session = make_session_record("sess12345678", false);
        session.prompt_segments.push(PromptSegment {
            prompt_text: "Fix the bug 🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛🐛".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            agents: Vec::new(),
            orchestrator_tools: Vec::new(),
            orchestrator_context: AgentContext::default(),
            tasks: Vec::new(),
        });
        app.session_records
            .insert("sess12345678".to_string(), session);
        app.session_focus = SessionFocus::Segment;
        // Must not panic
        let _output = render_sessions_widget(&app, 120, 20);
    }

    // --- Selection stability: segment bounds ---

    #[test]
    fn segment_selection_stays_within_bounds() {
        let mut app = App::new(ConfigInventory::default());
        let session = make_session_record("sess12345678", false);
        app.session_records.insert("sess12345678".to_string(), session);
        // Only 1 segment (segment zero), set selection beyond that
        app.session_segment_selected = 10;
        // Rendering should not panic
        let _output = render_sessions_widget(&app, 120, 20);
    }
}
