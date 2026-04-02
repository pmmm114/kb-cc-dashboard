use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, ToolRecord};
use super::helpers::{detail_line, format_relative_time, render_scroll_indicators};

/// Extracts the last path segment if the path looks like a worktree.
fn extract_worktree_suffix(path: &str) -> Option<&str> {
    if path.contains("/worktrees/") || path.contains("/tmp/claude-config-") {
        let trimmed = path.trim_end_matches('/');
        trimmed.rsplit('/').next().filter(|s| !s.is_empty())
    } else {
        None
    }
}

/// Formats tool records with counts and optional failure summary into an indented line.
/// Accepts any iterator over `&ToolRecord` to support both owned slices and sorted reference vecs.
fn format_tools_line<'a>(tools: impl Iterator<Item = &'a ToolRecord>) -> Line<'static> {
    let mut total_failures: usize = 0;
    let mut parts: Vec<String> = Vec::new();
    for t in tools {
        parts.push(format!("{} ({})", t.name, t.count));
        total_failures += t.failure_count;
    }
    if total_failures > 0 {
        parts.push(format!("[{} failed]", total_failures));
    }
    Line::from(Span::styled(
        format!("    {}", parts.join("  ")),
        Style::default().fg(Color::White),
    ))
}

/// Formats a sorted rules set into an indented line.
fn format_rules_line(rules: &std::collections::HashSet<String>) -> Line<'static> {
    let mut names: Vec<&str> = rules.iter().map(|s| s.as_str()).collect();
    names.sort();
    Line::from(Span::styled(
        format!("    Rules: {}", names.join(", ")),
        Style::default().fg(Color::DarkGray),
    ))
}

pub fn draw_sessions(f: &mut Frame, app: &App, list_area: Rect, detail_area: Rect) {
    draw_session_list(f, app, list_area);
    draw_session_detail(f, app, detail_area);
}

fn draw_session_list(f: &mut Frame, app: &App, area: Rect) {
    let sessions = app.visible_sessions();

    if sessions.is_empty() {
        let block = Block::default().borders(Borders::ALL).title("Sessions (0)");
        let msg = Paragraph::new("No sessions found \u{2014} waiting for data...")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let is_active = app.is_session_active(&s.session_id);
            let id_len = 8.min(s.session_id.len());
            let session_id_short = &s.session_id[..id_len];
            let time_ago = format_relative_time(s.updated_at);

            let dim = Style::default().fg(Color::DarkGray);
            let id_style = if is_active { Style::default().fg(Color::White) } else { dim };
            let phase_style = if is_active {
                Style::default().fg(s.phase.color())
            } else {
                dim
            };

            let line = Line::from(vec![
                Span::styled(format!("{:>2} ", i + 1), dim),
                Span::styled(session_id_short.to_string(), id_style),
                Span::raw(" "),
                Span::styled(format!("{:<14}", s.phase), phase_style),
                Span::styled(time_ago, dim),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = format!("Sessions ({})", sessions.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let selected = app.session_selected.min(sessions.len().saturating_sub(1));
    let mut state = ListState::default();
    state.select(Some(selected));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_session_detail(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Session Detail");
    let sessions = app.visible_sessions();

    if sessions.is_empty() {
        f.render_widget(block, area);
        return;
    }

    let selected = app.session_selected.min(sessions.len().saturating_sub(1));
    let session = &sessions[selected];
    let phase_str = session.phase.to_string();
    let flow_type_str = session.flow_type.as_deref().unwrap_or("\u{2014}");
    let last_agent_str = session.last_agent.as_deref().unwrap_or("\u{2014}");
    let workflow_id_str = session.workflow_id.to_string();
    let plan_iteration_str = session.plan_iteration.to_string();
    let last_mutation_str = session
        .last_mutation_tool
        .as_deref()
        .unwrap_or("\u{2014}");
    let verified_str = session.has_verification_since_mutation.to_string();
    let context_summary_str = session.context_summary.to_string();
    let plan_communicated_str = session.plan_communicated.to_string();
    let updated_str = format_relative_time(session.updated_at);

    let mut lines = vec![
        detail_line("Session ID", &session.session_id),
        detail_line("Phase", &phase_str),
        detail_line("Flow Type", flow_type_str),
        detail_line("Last Agent", last_agent_str),
        detail_line("Workflow ID", &workflow_id_str),
        detail_line("Plan Iteration", &plan_iteration_str),
        detail_line("Last Mutation", last_mutation_str),
        detail_line("Verified", &verified_str),
        detail_line("Context Summary", &context_summary_str),
        detail_line("Plan Communicated", &plan_communicated_str),
        detail_line("Updated", &updated_str),
    ];

    // Separator
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "\u{2500}\u{2500} Live Monitoring \u{2500}\u{2500}",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    let agents = app.session_active_agents.get(&session.session_id);
    let orch_tools = app.session_orchestrator_tools.get(&session.session_id);
    let orch_rules = app.session_orchestrator_rules.get(&session.session_id);
    let tasks = app.session_tasks.get(&session.session_id);

    let has_monitoring_data = agents.is_some_and(|a| !a.is_empty())
        || orch_tools.is_some_and(|t| !t.is_empty())
        || tasks.is_some_and(|t| !t.is_empty());

    if !has_monitoring_data {
        lines.push(Line::from(Span::styled(
            "No active agents \u{2014} waiting for events...",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        // Render each agent as a tree node
        if let Some(agent_list) = agents {
            for agent in agent_list {
                let mut header = format!("\u{25b8} {}", agent.agent_type);
                if let Some(ref cwd) = agent.cwd {
                    if let Some(suffix) = extract_worktree_suffix(cwd) {
                        header.push_str(&format!(" @{}", suffix));
                    }
                }
                lines.push(Line::from(Span::styled(
                    header,
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )));

                if !agent.tools.is_empty() {
                    lines.push(format_tools_line(agent.tools.iter()));
                }
                if !agent.rules.is_empty() {
                    lines.push(format_rules_line(&agent.rules));
                }

                lines.push(Line::from(""));
            }
        }

        // Orchestrator section
        if let Some(tool_vec) = orch_tools.filter(|t| !t.is_empty()) {
            lines.push(Line::from(Span::styled(
                "\u{25b8} orchestrator",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));

            let mut sorted: Vec<&ToolRecord> = tool_vec.iter().collect();
            sorted.sort_by(|a, b| b.count.cmp(&a.count));
            lines.push(format_tools_line(sorted.into_iter()));

            if let Some(rule_set) = orch_rules.filter(|r| !r.is_empty()) {
                lines.push(format_rules_line(rule_set));
            }

            lines.push(Line::from(""));
        }

        // Tasks section
        if let Some(task_list) = tasks.filter(|t| !t.is_empty()) {
            lines.push(Line::from(Span::styled(
                "\u{2500}\u{2500} Tasks \u{2500}\u{2500}",
                Style::default().fg(Color::DarkGray),
            )));
            for task in task_list {
                let status = if task.completed { "\u{2713}" } else { "..." };
                let teammate = task.teammate_name.as_deref().unwrap_or("\u{2014}");
                let color = if task.completed { Color::Green } else { Color::Yellow };
                lines.push(Line::from(Span::styled(
                    format!("  {}: {} {}", task.task_id, teammate, status),
                    Style::default().fg(color),
                )));
            }
        }
    }

    // Event count
    lines.push(Line::from(""));
    let event_count = app
        .session_events
        .get(&session.session_id)
        .map(|q| q.len())
        .unwrap_or(0);
    lines.push(detail_line("Events", &event_count.to_string()));

    let total_lines = lines.len();
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.session_detail_scroll as u16, 0));
    f.render_widget(paragraph, area);

    render_scroll_indicators(f, area, total_lines, app.session_detail_scroll);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::ConfigInventory;
    use crate::session::{Phase, SessionState};
    use crate::test_helpers::test_utils::buffer_to_string;
    use std::collections::HashMap;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_sessions(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                let list_area = Rect::new(0, 0, area.width / 2, area.height);
                let detail_area = Rect::new(area.width / 2, 0, area.width / 2, area.height);
                draw_sessions(f, app, list_area, detail_area);
            })
            .unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    fn make_session(id: &str, phase: Phase, updated_at: u64) -> SessionState {
        SessionState {
            phase,
            session_id: id.to_string(),
            workflow_id: 1,
            flow_type: Some("code".to_string()),
            last_agent: Some("planner".to_string()),
            context_summary: true,
            plan_iteration: 2,
            last_mutation_tool: Some("Edit".to_string()),
            has_verification_since_mutation: true,
            updated_at,
            pre_compact_phase: None,
            intake_block_count: 0,
            planner_block_count: 0,
            plan_communicated: true,
            file_path: String::new(),
            tasks: HashMap::new(),
        }
    }

    #[test]
    fn empty_state_shows_waiting_message() {
        let app = App::new(ConfigInventory::default());
        let output = render_sessions(&app, 100, 20);
        assert!(
            output.contains("No sessions found"),
            "Expected empty state message, got:\n{}",
            output
        );
        assert!(
            output.contains("waiting for data"),
            "Expected 'waiting for data' in message, got:\n{}",
            output
        );
    }

    #[test]
    fn empty_state_shows_zero_count_title() {
        let app = App::new(ConfigInventory::default());
        let output = render_sessions(&app, 100, 20);
        assert!(
            output.contains("Sessions (0)"),
            "Expected 'Sessions (0)' title, got:\n{}",
            output
        );
    }

    #[test]
    fn session_list_shows_total_count_in_title() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![
            make_session("abcdef1234567890", Phase::Idle, now),
            make_session("12345678abcdefgh", Phase::Planning, now - 60),
        ]);
        // Only one session is active, but title shows total count
        app.active_session_ids.insert("abcdef1234567890".to_string());
        let output = render_sessions(&app, 100, 20);
        assert!(
            output.contains("Sessions (2)"),
            "Expected 'Sessions (2)' title (total sessions), got:\n{}",
            output
        );
    }

    #[test]
    fn session_list_shows_truncated_id_for_visible_session() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Idle, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());
        let output = render_sessions(&app, 100, 20);
        assert!(
            output.contains("abcdef12"),
            "Expected truncated session ID 'abcdef12', got:\n{}",
            output
        );
    }

    #[test]
    fn session_list_shows_inactive_sessions() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![
            make_session("abcdef1234567890", Phase::Idle, now),
            make_session("stale_session_id0", Phase::Planning, now - 60),
        ]);
        // Only abcdef is active
        app.active_session_ids.insert("abcdef1234567890".to_string());
        let output = render_sessions(&app, 100, 20);
        assert!(
            output.contains("abcdef12"),
            "Expected active session, got:\n{}",
            output
        );
        assert!(
            output.contains("stale_se"),
            "Inactive session should appear in list, got:\n{}",
            output
        );
    }

    #[test]
    fn session_list_shows_phase() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Planning, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());
        let output = render_sessions(&app, 100, 20);
        assert!(
            output.contains("planning"),
            "Expected phase 'planning' in output, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_panel_shows_session_fields() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::PlanReview, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());
        let output = render_sessions(&app, 100, 40);

        assert!(output.contains("Session Detail"), "Missing detail title");
        assert!(output.contains("Session ID"), "Missing Session ID label");
        assert!(output.contains("Phase"), "Missing Phase label");
        assert!(output.contains("plan_review"), "Missing phase value");
        assert!(output.contains("Flow Type"), "Missing Flow Type label");
        assert!(output.contains("code"), "Missing flow_type value");
        assert!(output.contains("Last Agent"), "Missing Last Agent label");
        assert!(output.contains("planner"), "Missing last_agent value");
        assert!(output.contains("Workflow ID"), "Missing Workflow ID label");
        assert!(output.contains("Plan Iteration"), "Missing Plan Iteration label");
        assert!(output.contains("Verified"), "Missing Verified label");
        assert!(output.contains("Context Summary"), "Missing Context Summary label");
        assert!(output.contains("Plan Communicated"), "Missing Plan Communicated label");
    }

    #[test]
    fn detail_panel_empty_when_no_sessions() {
        let app = App::new(ConfigInventory::default());
        let output = render_sessions(&app, 100, 20);
        assert!(output.contains("Session Detail"), "Missing detail block title");
        // Should not contain field labels when empty
        assert!(
            !output.contains("Workflow ID"),
            "Should not show fields when no sessions"
        );
    }

    #[test]
    fn detail_shows_em_dash_for_none_fields() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        let mut session = make_session("abcdef1234567890", Phase::Idle, now);
        session.flow_type = None;
        session.last_agent = None;
        session.last_mutation_tool = None;
        app.update_sessions(vec![session]);
        app.active_session_ids.insert("abcdef1234567890".to_string());
        let output = render_sessions(&app, 100, 20);
        // em dash character
        assert!(
            output.contains("\u{2014}"),
            "Expected em dash for None fields, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_shows_monitoring_section() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        // Add monitoring data using new data model
        use crate::app::{ActiveAgent, ToolRecord};

        let mut rules = std::collections::HashSet::new();
        rules.insert("workflow.md".to_string());

        let agent = ActiveAgent {
            agent_type: "planner".to_string(),
            cwd: None,
            tools: vec![
                ToolRecord { name: "Read".to_string(), count: 5, failure_count: 0 },
            ],
            rules,
        };
        app.session_active_agents
            .insert("abcdef1234567890".to_string(), vec![agent]);

        let tools = vec![
            ToolRecord { name: "Read".to_string(), count: 6, failure_count: 0 },
            ToolRecord { name: "Edit".to_string(), count: 2, failure_count: 0 },
        ];
        app.session_orchestrator_tools
            .insert("abcdef1234567890".to_string(), tools);

        let mut orch_rules = std::collections::HashSet::new();
        orch_rules.insert("workflow.md".to_string());
        app.session_orchestrator_rules
            .insert("abcdef1234567890".to_string(), orch_rules);

        // Add some events to the session
        use crate::test_helpers::test_utils::make_test_event;
        let event = make_test_event(
            r#"{"hook_event_name":"PreToolUse","session_id":"abcdef1234567890","tool_name":"Read"}"#,
        );
        app.push_event(event);

        let output = render_sessions(&app, 100, 40);

        assert!(
            output.contains("Live Monitoring"),
            "Expected 'Live Monitoring' separator, got:\n{}",
            output
        );
        assert!(
            output.contains("\u{25b8} planner"),
            "Expected agent tree node 'planner', got:\n{}",
            output
        );
        assert!(
            output.contains("Read (5)"),
            "Expected tool 'Read (5)' under planner, got:\n{}",
            output
        );
        assert!(
            output.contains("workflow.md"),
            "Expected rule 'workflow.md', got:\n{}",
            output
        );
        assert!(
            output.contains("\u{25b8} orchestrator"),
            "Expected orchestrator tree node, got:\n{}",
            output
        );
        assert!(
            output.contains("Events"),
            "Expected 'Events' count label, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_monitoring_shows_empty_state_when_no_data() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Idle, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        let output = render_sessions(&app, 100, 40);
        assert!(
            output.contains("No active agents"),
            "Expected empty state message, got:\n{}",
            output
        );
    }

    #[test]
    fn draw_does_not_panic_with_short_session_id() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        // Session ID shorter than 8 chars should not panic
        app.update_sessions(vec![make_session("abc", Phase::Idle, now)]);
        app.active_session_ids.insert("abc".to_string());
        let _output = render_sessions(&app, 100, 20);
    }

    #[test]
    fn draw_does_not_panic_with_selection_out_of_bounds() {
        let mut app = App::new(ConfigInventory::default());
        app.session_selected = 99; // way beyond any list
        let _output = render_sessions(&app, 100, 20);
    }

    #[test]
    fn tree_renders_agent_with_tools_and_rules() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        use crate::app::{ActiveAgent, ToolRecord};

        let mut rules = std::collections::HashSet::new();
        rules.insert("workflow.md".to_string());
        rules.insert("investigation.md".to_string());

        let agent = ActiveAgent {
            agent_type: "planner".to_string(),
            cwd: None,
            tools: vec![
                ToolRecord { name: "Read".to_string(), count: 5, failure_count: 0 },
                ToolRecord { name: "Grep".to_string(), count: 3, failure_count: 0 },
            ],
            rules,
        };
        app.session_active_agents
            .insert("abcdef1234567890".to_string(), vec![agent]);

        let output = render_sessions(&app, 100, 40);
        assert!(output.contains("\u{25b8} planner"), "Expected agent header with triangle, got:\n{}", output);
        assert!(output.contains("Read (5)"), "Expected tool count, got:\n{}", output);
        assert!(output.contains("Grep (3)"), "Expected tool count, got:\n{}", output);
        assert!(output.contains("Rules:"), "Expected Rules line, got:\n{}", output);
        assert!(output.contains("workflow.md"), "Expected rule name, got:\n{}", output);
    }

    #[test]
    fn tree_renders_multiple_agents() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        use crate::app::{ActiveAgent, ToolRecord};

        let agents = vec![
            ActiveAgent {
                agent_type: "planner".to_string(),
                cwd: None,
                tools: vec![ToolRecord { name: "Read".to_string(), count: 5, failure_count: 0 }],
                rules: std::collections::HashSet::new(),
            },
            ActiveAgent {
                agent_type: "tdd-implementer".to_string(),
                cwd: None,
                tools: vec![ToolRecord { name: "Edit".to_string(), count: 4, failure_count: 0 }],
                rules: std::collections::HashSet::new(),
            },
        ];
        app.session_active_agents
            .insert("abcdef1234567890".to_string(), vec![agents[0].clone(), agents[1].clone()]);

        let output = render_sessions(&app, 100, 40);
        assert!(output.contains("\u{25b8} planner"), "Expected planner header, got:\n{}", output);
        assert!(output.contains("\u{25b8} tdd-implementer"), "Expected tdd-implementer header, got:\n{}", output);
    }

    #[test]
    fn tree_shows_orchestrator_for_unattributed_tools() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        use crate::app::ToolRecord;
        let tools = vec![
            ToolRecord { name: "Agent".to_string(), count: 2, failure_count: 0 },
            ToolRecord { name: "Read".to_string(), count: 1, failure_count: 0 },
        ];
        app.session_orchestrator_tools
            .insert("abcdef1234567890".to_string(), tools);

        let output = render_sessions(&app, 100, 40);
        assert!(output.contains("\u{25b8} orchestrator"), "Expected orchestrator header, got:\n{}", output);
        assert!(output.contains("Agent (2)"), "Expected tool count, got:\n{}", output);
    }

    #[test]
    fn worktree_suffix_from_config_path() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        use crate::app::ActiveAgent;

        let agent = ActiveAgent {
            agent_type: "config-editor".to_string(),
            cwd: Some("/tmp/claude-config-planner".to_string()),
            tools: Vec::new(),
            rules: std::collections::HashSet::new(),
        };
        app.session_active_agents
            .insert("abcdef1234567890".to_string(), vec![agent]);

        let output = render_sessions(&app, 100, 40);
        assert!(output.contains("@claude-config-planner"), "Expected config worktree suffix, got:\n{}", output);
    }

    #[test]
    fn no_worktree_suffix_for_normal_path() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        use crate::app::ActiveAgent;

        let agent = ActiveAgent {
            agent_type: "planner".to_string(),
            cwd: Some("/Users/kb/projects/myapp".to_string()),
            tools: Vec::new(),
            rules: std::collections::HashSet::new(),
        };
        app.session_active_agents
            .insert("abcdef1234567890".to_string(), vec![agent]);

        let output = render_sessions(&app, 100, 40);
        assert!(!output.contains("@"), "Expected no worktree suffix for normal path, got:\n{}", output);
    }

    #[test]
    fn tree_shows_worktree_suffix() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        use crate::app::ActiveAgent;

        let agent = ActiveAgent {
            agent_type: "tdd-implementer".to_string(),
            cwd: Some("/Users/kb/.claude/worktrees/agent-abc123".to_string()),
            tools: Vec::new(),
            rules: std::collections::HashSet::new(),
        };
        app.session_active_agents
            .insert("abcdef1234567890".to_string(), vec![agent]);

        let output = render_sessions(&app, 100, 40);
        assert!(output.contains("@agent-abc123"), "Expected worktree suffix, got:\n{}", output);
    }

    #[test]
    fn tree_shows_tasks_section() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        use crate::app::TaskInfo;
        let tasks = vec![
            TaskInfo { task_id: "T1".to_string(), teammate_name: Some("tdd-implementer".to_string()), completed: true },
            TaskInfo { task_id: "T2".to_string(), teammate_name: Some("tdd-implementer".to_string()), completed: false },
        ];
        app.session_tasks.insert("abcdef1234567890".to_string(), tasks);

        let output = render_sessions(&app, 100, 40);
        assert!(output.contains("Tasks"), "Expected Tasks section, got:\n{}", output);
        assert!(output.contains("T1"), "Expected task T1, got:\n{}", output);
        // Completed task should have checkmark
        assert!(output.contains("\u{2713}"), "Expected checkmark for completed task, got:\n{}", output);
        // Incomplete task should have ellipsis
        assert!(output.contains("..."), "Expected ellipsis for incomplete task, got:\n{}", output);
    }

    #[test]
    fn tree_empty_state_shows_waiting_message() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());
        // No agents, no orchestrator tools, no tasks
        let output = render_sessions(&app, 100, 40);
        assert!(output.contains("No active agents"), "Expected empty state message, got:\n{}", output);
    }

    #[test]
    fn tree_failed_tools_show_failure_indicator() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        use crate::app::{ActiveAgent, ToolRecord};

        let agent = ActiveAgent {
            agent_type: "tdd-implementer".to_string(),
            cwd: None,
            tools: vec![
                ToolRecord { name: "Edit".to_string(), count: 4, failure_count: 0 },
                ToolRecord { name: "Bash".to_string(), count: 3, failure_count: 1 },
            ],
            rules: std::collections::HashSet::new(),
        };
        app.session_active_agents
            .insert("abcdef1234567890".to_string(), vec![agent]);

        let output = render_sessions(&app, 100, 40);
        assert!(output.contains("[1 failed]"), "Expected failure indicator, got:\n{}", output);
    }

    #[test]
    fn detail_scroll_indicator_shown_when_content_clipped() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        // Add monitoring data to make content long
        use crate::app::{ActiveAgent, ToolRecord};

        let agents = vec![
            ActiveAgent {
                agent_type: "planner".to_string(),
                cwd: None,
                tools: Vec::new(),
                rules: std::collections::HashSet::new(),
            },
            ActiveAgent {
                agent_type: "tdd-implementer".to_string(),
                cwd: None,
                tools: Vec::new(),
                rules: std::collections::HashSet::new(),
            },
        ];
        app.session_active_agents.insert("abcdef1234567890".to_string(), agents);

        let tools = vec![
            ToolRecord { name: "Read".to_string(), count: 50, failure_count: 0 },
            ToolRecord { name: "Edit".to_string(), count: 20, failure_count: 0 },
            ToolRecord { name: "Grep".to_string(), count: 15, failure_count: 0 },
            ToolRecord { name: "Bash".to_string(), count: 10, failure_count: 0 },
        ];
        app.session_orchestrator_tools.insert("abcdef1234567890".to_string(), tools);

        // Use a very short terminal height to force clipping
        let output = render_sessions(&app, 100, 8);
        assert!(
            output.contains("\u{25bc}"),
            "Expected scroll-down indicator when content is clipped, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_scroll_indicator_hidden_when_content_fits() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Idle, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());

        let output = render_sessions(&app, 100, 40);
        assert!(
            !output.contains("\u{25bc}"),
            "Expected no scroll-down indicator when content fits, got:\n{}",
            output
        );
    }

    #[test]
    fn detail_scroll_up_indicator_shown_when_scrolled() {
        let mut app = App::new(ConfigInventory::default());
        let now = chrono::Utc::now().timestamp() as u64;
        app.update_sessions(vec![make_session("abcdef1234567890", Phase::Implementing, now)]);
        app.active_session_ids.insert("abcdef1234567890".to_string());
        app.session_detail_scroll = 3;

        let output = render_sessions(&app, 100, 40);
        assert!(
            output.contains("\u{25b2}"),
            "Expected scroll-up indicator when scrolled down, got:\n{}",
            output
        );
    }

    #[test]
    fn extract_worktree_suffix_from_worktrees_path() {
        assert_eq!(
            extract_worktree_suffix("/Users/kb/.claude/worktrees/agent-abc123"),
            Some("agent-abc123")
        );
    }

    #[test]
    fn extract_worktree_suffix_from_config_path() {
        assert_eq!(
            extract_worktree_suffix("/tmp/claude-config-planner"),
            Some("claude-config-planner")
        );
    }

    #[test]
    fn extract_worktree_suffix_returns_none_for_normal_path() {
        assert_eq!(
            extract_worktree_suffix("/Users/kb/projects/myapp"),
            None
        );
    }

    #[test]
    fn extract_worktree_suffix_handles_trailing_slash() {
        assert_eq!(
            extract_worktree_suffix("/Users/kb/.claude/worktrees/agent-abc123/"),
            Some("agent-abc123")
        );
    }

    #[test]
    fn extract_worktree_suffix_returns_none_for_empty_string() {
        assert_eq!(extract_worktree_suffix(""), None);
    }
}
