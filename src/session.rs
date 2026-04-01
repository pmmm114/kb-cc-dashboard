use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Intake,
    Planning,
    PlanReview,
    Approved,
    Implementing,
    Verifying,
    ConfigPlanning,
    ConfigPlanReview,
    ConfigEditing,
    ConfigVerifying,
}

impl Phase {
    pub fn color(&self) -> Color {
        match self {
            Phase::Idle => Color::DarkGray,
            Phase::Intake | Phase::Planning => Color::Yellow,
            Phase::PlanReview => Color::Cyan,
            Phase::Approved => Color::Green,
            Phase::Implementing => Color::Blue,
            Phase::Verifying => Color::Magenta,
            Phase::ConfigPlanning | Phase::ConfigPlanReview => Color::Yellow,
            Phase::ConfigEditing => Color::Blue,
            Phase::ConfigVerifying => Color::Magenta,
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Phase::Idle => "idle",
            Phase::Intake => "intake",
            Phase::Planning => "planning",
            Phase::PlanReview => "plan_review",
            Phase::Approved => "approved",
            Phase::Implementing => "implementing",
            Phase::Verifying => "verifying",
            Phase::ConfigPlanning => "config_planning",
            Phase::ConfigPlanReview => "config_plan_review",
            Phase::ConfigEditing => "config_editing",
            Phase::ConfigVerifying => "config_verifying",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionState {
    pub phase: Phase,
    #[serde(default)]
    pub workflow_id: u64,
    pub flow_type: Option<String>,
    pub last_agent: Option<String>,
    #[serde(default)]
    pub context_summary: bool,
    #[serde(default)]
    pub plan_iteration: u64,
    pub last_mutation_tool: Option<String>,
    #[serde(default)]
    pub has_verification_since_mutation: bool,
    #[serde(default)]
    pub updated_at: u64,
    pub pre_compact_phase: Option<String>,
    #[serde(default)]
    pub intake_block_count: u64,
    #[serde(default)]
    pub planner_block_count: u64,
    #[serde(default)]
    pub plan_communicated: bool,
    #[serde(skip)]
    pub session_id: String,
    #[serde(skip)]
    pub file_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_deserialize_snake_case() {
        let cases = vec![
            (r#""idle""#, Phase::Idle),
            (r#""intake""#, Phase::Intake),
            (r#""planning""#, Phase::Planning),
            (r#""plan_review""#, Phase::PlanReview),
            (r#""approved""#, Phase::Approved),
            (r#""implementing""#, Phase::Implementing),
            (r#""verifying""#, Phase::Verifying),
            (r#""config_planning""#, Phase::ConfigPlanning),
            (r#""config_plan_review""#, Phase::ConfigPlanReview),
            (r#""config_editing""#, Phase::ConfigEditing),
            (r#""config_verifying""#, Phase::ConfigVerifying),
        ];

        for (json, expected) in cases {
            let parsed: Phase = serde_json::from_str(json).unwrap();
            assert_eq!(parsed, expected, "Failed for JSON: {}", json);
        }
    }

    #[test]
    fn phase_display_matches_serde() {
        let phases = vec![
            Phase::Idle,
            Phase::Intake,
            Phase::Planning,
            Phase::PlanReview,
            Phase::Approved,
            Phase::Implementing,
            Phase::Verifying,
            Phase::ConfigPlanning,
            Phase::ConfigPlanReview,
            Phase::ConfigEditing,
            Phase::ConfigVerifying,
        ];

        for phase in phases {
            let displayed = phase.to_string();
            let json = format!("\"{}\"", displayed);
            let parsed: Phase = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, phase, "Display/serde roundtrip failed for {:?}", phase);
        }
    }

    #[test]
    fn phase_colors_assigned() {
        assert_eq!(Phase::Idle.color(), Color::DarkGray);
        assert_eq!(Phase::Approved.color(), Color::Green);
        assert_eq!(Phase::Implementing.color(), Color::Blue);
        assert_eq!(Phase::PlanReview.color(), Color::Cyan);
    }

    #[test]
    fn session_state_deserialize_full_json() {
        let json = r#"{
            "phase": "idle",
            "workflow_id": 0,
            "flow_type": null,
            "last_agent": null,
            "context_summary": false,
            "plan_iteration": 0,
            "last_mutation_tool": null,
            "has_verification_since_mutation": false,
            "updated_at": 1774637390,
            "pre_compact_phase": null,
            "intake_block_count": 1,
            "planner_block_count": 0,
            "plan_communicated": false
        }"#;

        let state: SessionState = serde_json::from_str(json).unwrap();
        assert_eq!(state.phase, Phase::Idle);
        assert_eq!(state.workflow_id, 0);
        assert!(state.flow_type.is_none());
        assert!(state.last_agent.is_none());
        assert!(!state.context_summary);
        assert_eq!(state.plan_iteration, 0);
        assert!(state.last_mutation_tool.is_none());
        assert!(!state.has_verification_since_mutation);
        assert_eq!(state.updated_at, 1774637390);
        assert!(state.pre_compact_phase.is_none());
        assert_eq!(state.intake_block_count, 1);
        assert_eq!(state.planner_block_count, 0);
        assert!(!state.plan_communicated);
        // skip fields default to empty
        assert!(state.session_id.is_empty());
        assert!(state.file_path.is_empty());
    }

    #[test]
    fn session_state_deserialize_minimal_json() {
        // Only required field is phase; all others have defaults
        let json = r#"{"phase": "implementing"}"#;
        let state: SessionState = serde_json::from_str(json).unwrap();
        assert_eq!(state.phase, Phase::Implementing);
        assert_eq!(state.workflow_id, 0);
        assert_eq!(state.updated_at, 0);
    }

    #[test]
    fn session_state_deserialize_active_session() {
        let json = r#"{
            "phase": "plan_review",
            "workflow_id": 42,
            "flow_type": "code",
            "last_agent": "planner",
            "context_summary": true,
            "plan_iteration": 2,
            "last_mutation_tool": "Edit",
            "has_verification_since_mutation": true,
            "updated_at": 1774600000,
            "pre_compact_phase": "planning",
            "intake_block_count": 3,
            "planner_block_count": 5,
            "plan_communicated": true
        }"#;

        let state: SessionState = serde_json::from_str(json).unwrap();
        assert_eq!(state.phase, Phase::PlanReview);
        assert_eq!(state.workflow_id, 42);
        assert_eq!(state.flow_type, Some("code".to_string()));
        assert_eq!(state.last_agent, Some("planner".to_string()));
        assert!(state.context_summary);
        assert_eq!(state.plan_iteration, 2);
        assert_eq!(state.last_mutation_tool, Some("Edit".to_string()));
        assert!(state.has_verification_since_mutation);
        assert_eq!(state.pre_compact_phase, Some("planning".to_string()));
        assert!(state.plan_communicated);
    }
}
