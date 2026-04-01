use chrono::{DateTime, Utc};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEvent {
    pub hook_event_name: String,
    pub session_id: String,
    #[serde(default = "Utc::now")]
    pub received_at: DateTime<Utc>,
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

impl HookEvent {
    pub fn kind(&self) -> EventKind {
        EventKind::from_str(&self.hook_event_name)
    }

    pub fn summary(&self) -> String {
        match self.kind() {
            EventKind::PreToolUse | EventKind::PostToolUse | EventKind::PostToolUseFailure => {
                let tool = self
                    .payload
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown tool");
                match self.payload.get("agent_context_type").and_then(|v| v.as_str()) {
                    Some(agent) => format!("{} ({})", tool, agent),
                    None => tool.to_string(),
                }
            }
            EventKind::SubagentStart | EventKind::SubagentStop => self
                .payload
                .get("agent_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown agent")
                .to_string(),
            EventKind::InstructionsLoaded => self
                .payload
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown file")
                .to_string(),
            EventKind::PermissionRequest => self
                .payload
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("permission")
                .to_string(),
            EventKind::ConfigChange => self
                .payload
                .get("config_key")
                .and_then(|v| v.as_str())
                .unwrap_or("config")
                .to_string(),
            EventKind::TaskCreated | EventKind::TaskCompleted => self
                .payload
                .get("task_id")
                .and_then(|v| v.as_str())
                .unwrap_or("task")
                .to_string(),
            EventKind::SessionStart => "session started".to_string(),
            EventKind::SessionEnd => "session ended".to_string(),
            EventKind::Stop => "stopped".to_string(),
            EventKind::StopFailure => "stop failed".to_string(),
            EventKind::UserPromptSubmit => {
                let prompt = self
                    .payload
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if prompt.chars().count() > 40 {
                    let truncated: String = prompt.chars().take(37).collect();
                    format!("{}...", truncated)
                } else if prompt.is_empty() {
                    "prompt".to_string()
                } else {
                    prompt.to_string()
                }
            }
            EventKind::PreCompact => "compacting".to_string(),
            EventKind::PostCompact => "compacted".to_string(),
            EventKind::Unknown => "unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    InstructionsLoaded,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    SubagentStart,
    SubagentStop,
    UserPromptSubmit,
    PermissionRequest,
    Stop,
    SessionStart,
    SessionEnd,
    ConfigChange,
    TaskCreated,
    TaskCompleted,
    PreCompact,
    PostCompact,
    StopFailure,
    Unknown,
}

impl EventKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "InstructionsLoaded" => EventKind::InstructionsLoaded,
            "PreToolUse" => EventKind::PreToolUse,
            "PostToolUse" => EventKind::PostToolUse,
            "PostToolUseFailure" => EventKind::PostToolUseFailure,
            "SubagentStart" => EventKind::SubagentStart,
            "SubagentStop" => EventKind::SubagentStop,
            "UserPromptSubmit" => EventKind::UserPromptSubmit,
            "PermissionRequest" => EventKind::PermissionRequest,
            "Stop" => EventKind::Stop,
            "SessionStart" => EventKind::SessionStart,
            "SessionEnd" => EventKind::SessionEnd,
            "ConfigChange" => EventKind::ConfigChange,
            "TaskCreated" => EventKind::TaskCreated,
            "TaskCompleted" => EventKind::TaskCompleted,
            "PreCompact" => EventKind::PreCompact,
            "PostCompact" => EventKind::PostCompact,
            "StopFailure" => EventKind::StopFailure,
            _ => EventKind::Unknown,
        }
    }

    pub fn color(&self) -> Color {
        match self {
            // Tool events
            EventKind::PreToolUse | EventKind::PostToolUse | EventKind::PostToolUseFailure => {
                Color::Blue
            }
            // Agent events
            EventKind::SubagentStart | EventKind::SubagentStop => Color::Magenta,
            // Lifecycle events
            EventKind::SessionStart
            | EventKind::SessionEnd
            | EventKind::InstructionsLoaded
            | EventKind::ConfigChange => Color::Green,
            // User events
            EventKind::UserPromptSubmit | EventKind::PermissionRequest => Color::Cyan,
            // Task events
            EventKind::TaskCreated | EventKind::TaskCompleted => Color::Yellow,
            // Compaction events
            EventKind::PreCompact | EventKind::PostCompact => Color::DarkGray,
            // Error events
            EventKind::Stop => Color::White,
            EventKind::StopFailure => Color::Red,
            EventKind::Unknown => Color::DarkGray,
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EventKind::InstructionsLoaded => "InstructionsLoaded",
            EventKind::PreToolUse => "PreToolUse",
            EventKind::PostToolUse => "PostToolUse",
            EventKind::PostToolUseFailure => "PostToolUseFailure",
            EventKind::SubagentStart => "SubagentStart",
            EventKind::SubagentStop => "SubagentStop",
            EventKind::UserPromptSubmit => "UserPromptSubmit",
            EventKind::PermissionRequest => "PermissionRequest",
            EventKind::Stop => "Stop",
            EventKind::SessionStart => "SessionStart",
            EventKind::SessionEnd => "SessionEnd",
            EventKind::ConfigChange => "ConfigChange",
            EventKind::TaskCreated => "TaskCreated",
            EventKind::TaskCompleted => "TaskCompleted",
            EventKind::PreCompact => "PreCompact",
            EventKind::PostCompact => "PostCompact",
            EventKind::StopFailure => "StopFailure",
            EventKind::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_from_str_all_variants() {
        let cases = vec![
            ("InstructionsLoaded", EventKind::InstructionsLoaded),
            ("PreToolUse", EventKind::PreToolUse),
            ("PostToolUse", EventKind::PostToolUse),
            ("PostToolUseFailure", EventKind::PostToolUseFailure),
            ("SubagentStart", EventKind::SubagentStart),
            ("SubagentStop", EventKind::SubagentStop),
            ("UserPromptSubmit", EventKind::UserPromptSubmit),
            ("PermissionRequest", EventKind::PermissionRequest),
            ("Stop", EventKind::Stop),
            ("SessionStart", EventKind::SessionStart),
            ("SessionEnd", EventKind::SessionEnd),
            ("ConfigChange", EventKind::ConfigChange),
            ("TaskCreated", EventKind::TaskCreated),
            ("TaskCompleted", EventKind::TaskCompleted),
            ("PreCompact", EventKind::PreCompact),
            ("PostCompact", EventKind::PostCompact),
            ("StopFailure", EventKind::StopFailure),
        ];

        for (input, expected) in cases {
            assert_eq!(
                EventKind::from_str(input),
                expected,
                "Failed for input: {}",
                input
            );
        }
    }

    #[test]
    fn event_kind_unknown_for_unrecognized() {
        assert_eq!(EventKind::from_str("SomethingElse"), EventKind::Unknown);
        assert_eq!(EventKind::from_str(""), EventKind::Unknown);
    }

    #[test]
    fn event_kind_display_roundtrip() {
        let kinds = vec![
            EventKind::InstructionsLoaded,
            EventKind::PreToolUse,
            EventKind::PostToolUse,
            EventKind::PostToolUseFailure,
            EventKind::SubagentStart,
            EventKind::SubagentStop,
            EventKind::UserPromptSubmit,
            EventKind::PermissionRequest,
            EventKind::Stop,
            EventKind::SessionStart,
            EventKind::SessionEnd,
            EventKind::ConfigChange,
            EventKind::TaskCreated,
            EventKind::TaskCompleted,
            EventKind::PreCompact,
            EventKind::PostCompact,
            EventKind::StopFailure,
            EventKind::Unknown,
        ];

        for kind in kinds {
            let displayed = kind.to_string();
            let parsed = EventKind::from_str(&displayed);
            assert_eq!(parsed, kind, "Roundtrip failed for {:?}", kind);
        }
    }

    #[test]
    fn hook_event_deserialize_tool_event() {
        let json = r#"{
            "hook_event_name": "PreToolUse",
            "session_id": "abc-123",
            "tool_name": "Read",
            "file_path": "/tmp/test.rs"
        }"#;

        let event: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.hook_event_name, "PreToolUse");
        assert_eq!(event.session_id, "abc-123");
        assert_eq!(event.kind(), EventKind::PreToolUse);
        assert_eq!(event.summary(), "Read");
    }

    #[test]
    fn hook_event_deserialize_agent_event() {
        let json = r#"{
            "hook_event_name": "SubagentStart",
            "session_id": "def-456",
            "agent_type": "planner"
        }"#;

        let event: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.kind(), EventKind::SubagentStart);
        assert_eq!(event.summary(), "planner");
    }

    #[test]
    fn hook_event_summary_for_lifecycle() {
        let json = r#"{
            "hook_event_name": "SessionStart",
            "session_id": "ghi-789"
        }"#;

        let event: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.summary(), "session started");
    }

    #[test]
    fn hook_event_summary_truncates_long_prompt() {
        let json = r#"{
            "hook_event_name": "UserPromptSubmit",
            "session_id": "jkl-012",
            "prompt": "This is a very long prompt that should be truncated at forty characters"
        }"#;

        let event: HookEvent = serde_json::from_str(json).unwrap();
        let summary = event.summary();
        assert!(summary.len() <= 40, "Summary too long: {}", summary);
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn summary_includes_agent_context_when_present() {
        let json = r#"{
            "hook_event_name": "PreToolUse",
            "session_id": "abc-123",
            "tool_name": "Read",
            "agent_context_type": "planner"
        }"#;
        let event: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.summary(), "Read (planner)");
    }

    #[test]
    fn summary_unchanged_when_agent_context_absent() {
        let json = r#"{
            "hook_event_name": "PreToolUse",
            "session_id": "abc-123",
            "tool_name": "Read"
        }"#;
        let event: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.summary(), "Read");
    }

    #[test]
    fn event_kind_colors_are_assigned() {
        // Tool events are blue
        assert_eq!(EventKind::PreToolUse.color(), Color::Blue);
        assert_eq!(EventKind::PostToolUse.color(), Color::Blue);
        // Agent events are magenta
        assert_eq!(EventKind::SubagentStart.color(), Color::Magenta);
        // Error events are red
        assert_eq!(EventKind::StopFailure.color(), Color::Red);
        // Lifecycle events are green
        assert_eq!(EventKind::SessionStart.color(), Color::Green);
    }
}
