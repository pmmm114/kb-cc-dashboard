#[derive(Debug, Clone, Default)]
pub struct ConfigInventory {
    pub agents: Vec<AgentConfig>,
    pub skills: Vec<SkillConfig>,
    pub rules: Vec<RuleConfig>,
    pub hooks: Vec<HookRegistration>,
    pub hook_scripts: Vec<String>,
    pub plugins: Vec<PluginConfig>,
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    pub model: String,
    pub disallowed_tools: Vec<String>,
    pub file_path: String,
}

#[derive(Debug, Clone)]
pub struct SkillConfig {
    pub name: String,
    pub description: String,
    pub file_path: String,
}

#[derive(Debug, Clone)]
pub struct RuleConfig {
    pub file_path: String,
    pub file_name: String,
    pub rule_count: usize,
    pub hard_gate_count: usize,
    pub rule_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HookRegistration {
    pub event: String,
    pub matcher: Option<String>,
    pub hook_type: String,
    pub command: Option<String>,
    pub prompt: Option<String>,
    pub timeout: Option<u64>,
    pub is_async: bool,
}

#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub name: String,
    pub enabled: bool,
}

impl ConfigInventory {
    pub fn total_items(&self) -> usize {
        self.agents.len()
            + self.skills.len()
            + self.rules.len()
            + self.hooks.len()
            + self.hook_scripts.len()
            + self.plugins.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_inventory_default_is_empty() {
        let inv = ConfigInventory::default();
        assert_eq!(inv.total_items(), 0);
        assert!(inv.agents.is_empty());
        assert!(inv.skills.is_empty());
        assert!(inv.rules.is_empty());
        assert!(inv.hooks.is_empty());
        assert!(inv.hook_scripts.is_empty());
        assert!(inv.plugins.is_empty());
    }

    #[test]
    fn config_inventory_total_items() {
        let mut inv = ConfigInventory::default();
        inv.agents.push(AgentConfig {
            name: "planner".into(),
            description: "Plans things".into(),
            model: "opus".into(),
            disallowed_tools: vec![],
            file_path: "agents/planner.md".into(),
        });
        inv.skills.push(SkillConfig {
            name: "benchmark".into(),
            description: "Runs benchmarks".into(),
            file_path: "skills/benchmark/SKILL.md".into(),
        });
        inv.rules.push(RuleConfig {
            file_path: "rules/workflow.md".into(),
            file_name: "workflow.md".into(),
            rule_count: 3,
            hard_gate_count: 1,
            rule_names: vec!["plan-before-act".into()],
        });
        assert_eq!(inv.total_items(), 3);
    }

    #[test]
    fn hook_registration_fields() {
        let hook = HookRegistration {
            event: "PreToolUse".into(),
            matcher: Some("Edit|Write".into()),
            hook_type: "command".into(),
            command: Some("bash pre-edit-guard.sh".into()),
            prompt: None,
            timeout: Some(5000),
            is_async: false,
        };
        assert_eq!(hook.event, "PreToolUse");
        assert_eq!(hook.matcher, Some("Edit|Write".to_string()));
        assert!(!hook.is_async);
    }
}
