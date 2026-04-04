/// Test utilities for unit tests and integration tests.
///
/// Always compiled so that integration tests in `tests/` can import them.
/// The dead_code warnings are suppressed since these are only used in test contexts.
#[allow(dead_code)]
pub mod test_utils {
    use ratatui::buffer::Buffer;

    pub fn buffer_to_string(buf: &Buffer) -> String {
        let mut output = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                output.push_str(cell.symbol());
            }
            output.push('\n');
        }
        output
    }

    pub fn make_test_event(json: &str) -> crate::event::HookEvent {
        serde_json::from_str(json).unwrap()
    }

    /// Build a fully populated App for snapshot and integration testing.
    ///
    /// Contains:
    /// - 2 sessions: "a1b2c3d4" (active, 3 segments, 2 agents) and "e5f6a7b8" (ended, 1 segment)
    /// - Mixed event types across sessions
    /// - Agent records with tools_used, rules, skills, agent_definitions
    /// - Realistic data: hex session IDs, tool names like Read/Edit/Bash, agent types like Explore/tdd-implementer
    pub fn mock_populated_app() -> crate::app::App {
        use crate::app::*;
        use crate::config::*;
        use crate::event::HookEvent;
        use chrono::{TimeZone, Utc};
        use std::collections::{HashMap, HashSet, VecDeque};

        let base_time = Utc.with_ymd_and_hms(2026, 4, 4, 10, 0, 0).unwrap();

        // --- Active session: a1b2c3d4 ---

        let agent_explore = AgentRecord {
            id: 0,
            agent_type: "Explore".to_string(),
            cwd: Some("/Users/dev/project".to_string()),
            started_at: base_time + chrono::Duration::seconds(5),
            ended_at: Some(base_time + chrono::Duration::seconds(30)),
            context: AgentContext {
                agent_definitions: vec!["planner.md".to_string()],
                skills: vec!["SKILL.md".to_string()],
                rules: vec!["workflow.md".to_string(), "code-quality.md".to_string()],
                memory: vec!["CLAUDE.md".to_string()],
                other: Vec::new(),
            },
            tools: vec![
                ToolRecord { name: "Read".to_string(), count: 5, failure_count: 0 },
                ToolRecord { name: "Glob".to_string(), count: 3, failure_count: 0 },
                ToolRecord { name: "Grep".to_string(), count: 2, failure_count: 1 },
            ],
        };

        let agent_tdd = AgentRecord {
            id: 1,
            agent_type: "tdd-implementer".to_string(),
            cwd: Some("/Users/dev/project".to_string()),
            started_at: base_time + chrono::Duration::seconds(35),
            ended_at: None, // still active
            context: AgentContext {
                agent_definitions: vec!["tdd-implementer.md".to_string()],
                skills: Vec::new(),
                rules: vec!["code-quality.md".to_string()],
                memory: Vec::new(),
                other: Vec::new(),
            },
            tools: vec![
                ToolRecord { name: "Read".to_string(), count: 8, failure_count: 0 },
                ToolRecord { name: "Edit".to_string(), count: 4, failure_count: 1 },
                ToolRecord { name: "Bash".to_string(), count: 6, failure_count: 0 },
                ToolRecord { name: "Write".to_string(), count: 1, failure_count: 0 },
            ],
        };

        let segment_zero = PromptSegment {
            prompt_text: "(session initialization)".to_string(),
            started_at: base_time,
            ended_at: Some(base_time + chrono::Duration::seconds(2)),
            agents: Vec::new(),
            orchestrator_tools: Vec::new(),
            orchestrator_context: AgentContext::default(),
            tasks: Vec::new(),
        };

        let segment_one = PromptSegment {
            prompt_text: "Implement the user authentication module".to_string(),
            started_at: base_time + chrono::Duration::seconds(2),
            ended_at: Some(base_time + chrono::Duration::seconds(32)),
            agents: vec![0], // Explore agent
            orchestrator_tools: vec![
                ToolRecord { name: "Read".to_string(), count: 2, failure_count: 0 },
            ],
            orchestrator_context: AgentContext {
                rules: vec!["workflow.md".to_string()],
                ..AgentContext::default()
            },
            tasks: vec![
                TaskInfo {
                    task_id: "T1".to_string(),
                    teammate_name: Some("auth-worker".to_string()),
                    completed: true,
                },
            ],
        };

        let segment_two = PromptSegment {
            prompt_text: "Add unit tests for the login handler".to_string(),
            started_at: base_time + chrono::Duration::seconds(32),
            ended_at: None, // still in progress
            agents: vec![1], // tdd-implementer agent
            orchestrator_tools: Vec::new(),
            orchestrator_context: AgentContext::default(),
            tasks: vec![
                TaskInfo {
                    task_id: "T2".to_string(),
                    teammate_name: None,
                    completed: false,
                },
            ],
        };

        let active_session = SessionRecord {
            session_id: "a1b2c3d4".to_string(),
            first_seen_at: base_time,
            last_event_at: base_time + chrono::Duration::seconds(40),
            ended: false,
            agent_records: vec![agent_explore, agent_tdd],
            prompt_segments: vec![segment_zero, segment_one, segment_two],
            next_agent_id: 2,
        };

        // --- Ended session: e5f6a7b8 ---

        let ended_session = SessionRecord {
            session_id: "e5f6a7b8".to_string(),
            first_seen_at: base_time - chrono::Duration::hours(1),
            last_event_at: base_time - chrono::Duration::minutes(30),
            ended: true,
            agent_records: Vec::new(),
            prompt_segments: vec![PromptSegment {
                prompt_text: "(session initialization)".to_string(),
                started_at: base_time - chrono::Duration::hours(1),
                ended_at: Some(base_time - chrono::Duration::minutes(30)),
                agents: Vec::new(),
                orchestrator_tools: vec![
                    ToolRecord { name: "Bash".to_string(), count: 3, failure_count: 0 },
                ],
                orchestrator_context: AgentContext::default(),
                tasks: Vec::new(),
            }],
            next_agent_id: 0,
        };

        // --- Build session_records map ---

        let mut session_records = HashMap::new();
        session_records.insert("a1b2c3d4".to_string(), active_session);
        session_records.insert("e5f6a7b8".to_string(), ended_session);

        let mut active_session_ids = HashSet::new();
        active_session_ids.insert("a1b2c3d4".to_string());

        // --- Mixed events ---

        let events: VecDeque<HookEvent> = vec![
            make_test_event(r#"{"hook_event_name":"SessionStart","session_id":"a1b2c3d4"}"#),
            make_test_event(r#"{"hook_event_name":"UserPromptSubmit","session_id":"a1b2c3d4","prompt":"Implement the user authentication module"}"#),
            make_test_event(r#"{"hook_event_name":"SubagentStart","session_id":"a1b2c3d4","agent_type":"Explore","cwd":"/Users/dev/project"}"#),
            make_test_event(r#"{"hook_event_name":"PreToolUse","session_id":"a1b2c3d4","tool_name":"Read","agent_context_type":"Explore"}"#),
            make_test_event(r#"{"hook_event_name":"PostToolUse","session_id":"a1b2c3d4","tool_name":"Read","agent_context_type":"Explore"}"#),
            make_test_event(r#"{"hook_event_name":"PostToolUseFailure","session_id":"a1b2c3d4","tool_name":"Grep","agent_context_type":"Explore","error":"pattern not found"}"#),
            make_test_event(r#"{"hook_event_name":"SubagentStop","session_id":"a1b2c3d4","agent_type":"Explore","cwd":"/Users/dev/project"}"#),
            make_test_event(r#"{"hook_event_name":"TaskCreated","session_id":"a1b2c3d4","task_id":"T1","teammate_name":"auth-worker"}"#),
            make_test_event(r#"{"hook_event_name":"SubagentStart","session_id":"a1b2c3d4","agent_type":"tdd-implementer","cwd":"/Users/dev/project"}"#),
            make_test_event(r#"{"hook_event_name":"PostToolUse","session_id":"a1b2c3d4","tool_name":"Edit","agent_context_type":"tdd-implementer"}"#),
            make_test_event(r#"{"hook_event_name":"PostToolUse","session_id":"a1b2c3d4","tool_name":"Bash","agent_context_type":"tdd-implementer"}"#),
            make_test_event(r#"{"hook_event_name":"SessionStart","session_id":"e5f6a7b8"}"#),
            make_test_event(r#"{"hook_event_name":"PostToolUse","session_id":"e5f6a7b8","tool_name":"Bash"}"#),
            make_test_event(r#"{"hook_event_name":"SessionEnd","session_id":"e5f6a7b8"}"#),
        ]
        .into();

        // --- Config inventory ---

        let config = ConfigInventory {
            agents: vec![
                AgentConfig {
                    name: "planner".to_string(),
                    description: "Investigates codebase and produces plans".to_string(),
                    model: "opus".to_string(),
                    disallowed_tools: vec!["Edit".to_string(), "Write".to_string()],
                    file_path: "agents/planner.md".to_string(),
                },
                AgentConfig {
                    name: "tdd-implementer".to_string(),
                    description: "Executes plans via TDD cycle".to_string(),
                    model: "opus".to_string(),
                    disallowed_tools: Vec::new(),
                    file_path: "agents/tdd-implementer.md".to_string(),
                },
            ],
            skills: vec![SkillConfig {
                name: "benchmark".to_string(),
                description: "Runs benchmarks".to_string(),
                file_path: "skills/benchmark/SKILL.md".to_string(),
            }],
            rules: vec![RuleConfig {
                file_path: "rules/workflow.md".to_string(),
                file_name: "workflow.md".to_string(),
                rule_count: 5,
                hard_gate_count: 2,
                rule_names: vec!["plan-before-act".to_string(), "verify-before-done".to_string()],
            }],
            hooks: vec![HookRegistration {
                event: "PreToolUse".to_string(),
                matcher: Some("Edit|Write".to_string()),
                hook_type: "command".to_string(),
                command: Some("bash pre-edit-guard.sh".to_string()),
                prompt: None,
                timeout: Some(5000),
                is_async: false,
            }],
            hook_scripts: Vec::new(),
            plugins: vec![PluginConfig {
                name: "dashboard".to_string(),
                enabled: true,
            }],
        };

        // --- Assemble App ---

        App {
            active_tab: Tab::Sessions,
            config,
            events,
            session_selected: 0,
            config_category: ConfigCategory::Agents,
            config_item_selected: 0,
            config_focus: ConfigFocus::Category,
            event_selected: 0,
            event_auto_scroll: false,
            should_quit: false,
            session_focus: SessionFocus::List,
            event_focus: ListDetailFocus::List,
            config_detail_scroll: 0,
            session_detail_scroll: 0,
            event_detail_scroll: 0,
            active_session_ids,
            events_session_filter: None,
            session_records,
            session_segment_selected: 0,
        }
    }
}
