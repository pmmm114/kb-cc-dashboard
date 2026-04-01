use crate::config::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn extract_frontmatter(content: &str) -> Option<String> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let first_newline = trimmed.find('\n')?;
    let after_first = &trimmed[first_newline + 1..];
    let end = after_first.find("\n---")?;
    Some(after_first[..end].to_string())
}

/// Handles unquoted colons in values by falling back to line-by-line extraction.
fn parse_frontmatter(fm: &str) -> Option<HashMap<String, serde_yaml::Value>> {
    if let Ok(parsed) = serde_yaml::from_str(fm) {
        return Some(parsed);
    }
    let mut map = HashMap::new();
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut in_multiline = false;

    for line in fm.lines() {
        if !in_multiline {
            // Check if this is a key: value line (key must start at column 0, no spaces)
            if let Some(colon_pos) = line.find(':') {
                let key = &line[..colon_pos];
                if !key.contains(' ') && !key.is_empty() {
                    // Save previous key
                    if !current_key.is_empty() {
                        let val = current_value.trim().to_string();
                        map.insert(
                            current_key.clone(),
                            serde_yaml::Value::String(val),
                        );
                    }
                    current_key = key.to_string();
                    let rest = line[colon_pos + 1..].trim();
                    if rest == ">" || rest == "|" {
                        in_multiline = true;
                        current_value = String::new();
                    } else {
                        current_value = rest.to_string();
                    }
                } else {
                    // Continuation of previous value
                    current_value.push(' ');
                    current_value.push_str(line.trim());
                }
            }
        } else if line.starts_with("  ") || line.starts_with('\t') {
            // Multiline continuation
            if !current_value.is_empty() {
                current_value.push(' ');
            }
            current_value.push_str(line.trim());
        } else {
            // End of multiline — this is a new key
            in_multiline = false;
            if !current_key.is_empty() {
                let val = current_value.trim().to_string();
                map.insert(
                    current_key.clone(),
                    serde_yaml::Value::String(val),
                );
            }
            if let Some(colon_pos) = line.find(':') {
                let key = &line[..colon_pos];
                if !key.contains(' ') && !key.is_empty() {
                    current_key = key.to_string();
                    let rest = line[colon_pos + 1..].trim();
                    if rest == ">" || rest == "|" {
                        in_multiline = true;
                        current_value = String::new();
                    } else {
                        current_value = rest.to_string();
                    }
                }
            }
        }
    }
    if !current_key.is_empty() {
        let val = current_value.trim().to_string();
        map.insert(current_key, serde_yaml::Value::String(val));
    }

    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

pub fn load_agents(base: &Path) -> Vec<AgentConfig> {
    let agents_dir = base.join("agents");
    let entries = match fs::read_dir(&agents_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut agents = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let fm = match extract_frontmatter(&content) {
            Some(f) => f,
            None => continue,
        };
        let parsed = match parse_frontmatter(&fm) {
            Some(p) => p,
            None => continue,
        };

        let name = parsed
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let description = parsed
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let model = parsed
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let disallowed_tools = match parsed.get("disallowedTools") {
            Some(serde_yaml::Value::Sequence(seq)) => seq
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .collect(),
            Some(serde_yaml::Value::String(s)) => s
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
            _ => vec![],
        };

        let rel_path = format!("agents/{}", path.file_name().unwrap().to_string_lossy());
        agents.push(AgentConfig {
            name,
            description,
            model,
            disallowed_tools,
            file_path: rel_path,
        });
    }
    agents.sort_by(|a, b| a.name.cmp(&b.name));
    agents
}

pub fn load_skills(base: &Path) -> Vec<SkillConfig> {
    let skills_dir = base.join("skills");
    let entries = match fs::read_dir(&skills_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut skills = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_file = path.join("SKILL.md");
        let content = match fs::read_to_string(&skill_file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let fm = match extract_frontmatter(&content) {
            Some(f) => f,
            None => continue,
        };
        let parsed = match parse_frontmatter(&fm) {
            Some(p) => p,
            None => continue,
        };

        let dir_name = path.file_name().unwrap().to_string_lossy().to_string();
        let name = parsed
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&dir_name)
            .to_string();
        let description = parsed
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let rel_path = format!("skills/{}/SKILL.md", dir_name);
        skills.push(SkillConfig {
            name,
            description,
            file_path: rel_path,
        });
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

fn collect_md_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md_files(&path, files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            files.push(path);
        }
    }
}

pub fn load_rules(base: &Path) -> Vec<RuleConfig> {
    let rules_dir = base.join("rules");
    let mut md_files = Vec::new();
    collect_md_files(&rules_dir, &mut md_files);

    let mut rules = Vec::new();
    for path in md_files {
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut rule_count = 0;
        let mut hard_gate_count = 0;
        let mut rule_names = Vec::new();

        for line in content.lines() {
            if line.contains("<RULE") {
                rule_count += 1;
                if let Some(start) = line.find("name=\"") {
                    let after = &line[start + 6..];
                    if let Some(end) = after.find('"') {
                        rule_names.push(after[..end].to_string());
                    }
                }
            }
            if line.contains("<HARD-GATE>") || line.contains("<HARD-GATE ") {
                hard_gate_count += 1;
            }
        }

        let rel_path = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        rules.push(RuleConfig {
            file_path: rel_path,
            file_name,
            rule_count,
            hard_gate_count,
            rule_names,
        });
    }
    rules.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    rules
}

fn load_hooks_from(settings: &serde_json::Value, base: &Path) -> (Vec<HookRegistration>, Vec<String>) {
    let mut registrations = Vec::new();
    let mut scripts = Vec::new();

    if let Some(hooks_obj) = settings.get("hooks").and_then(|v| v.as_object()) {
        for (event_name, matcher_groups) in hooks_obj {
            if let Some(groups) = matcher_groups.as_array() {
                for group in groups {
                    let matcher = group
                        .get("matcher")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    if let Some(hooks_arr) = group.get("hooks").and_then(|v| v.as_array()) {
                        for hook in hooks_arr {
                            let hook_type = hook
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("command")
                                .to_string();
                            let command = hook
                                .get("command")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let prompt = hook
                                .get("prompt")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let timeout = hook
                                .get("timeout")
                                .and_then(|v| v.as_u64());
                            let is_async = hook
                                .get("async")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            registrations.push(HookRegistration {
                                event: event_name.clone(),
                                matcher: matcher.clone(),
                                hook_type,
                                command,
                                prompt,
                                timeout,
                                is_async,
                            });
                        }
                    }
                }
            }
        }
    }

    let hooks_dir = base.join("hooks");
    if let Ok(entries) = fs::read_dir(&hooks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("sh") {
                if let Some(name) = path.file_name() {
                    scripts.push(name.to_string_lossy().to_string());
                }
            }
        }
    }
    scripts.sort();

    (registrations, scripts)
}

fn load_plugins_from(settings: &serde_json::Value) -> Vec<PluginConfig> {
    let mut plugins = Vec::new();
    if let Some(obj) = settings.get("enabledPlugins").and_then(|v| v.as_object()) {
        for (name, val) in obj {
            let enabled = val.as_bool().unwrap_or(false);
            plugins.push(PluginConfig {
                name: name.clone(),
                enabled,
            });
        }
    }
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    plugins
}

pub fn load_all(base: &Path) -> ConfigInventory {
    let settings_path = base.join("settings.json");
    let settings: serde_json::Value = fs::read_to_string(&settings_path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or(serde_json::Value::Null);

    let (hooks, hook_scripts) = load_hooks_from(&settings, base);
    ConfigInventory {
        agents: load_agents(base),
        skills: load_skills(base),
        rules: load_rules(base),
        hooks,
        hook_scripts,
        plugins: load_plugins_from(&settings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn claude_dir() -> PathBuf {
        PathBuf::from(env!("HOME")).join(".claude")
    }

    fn read_settings(base: &Path) -> serde_json::Value {
        let path = base.join("settings.json");
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or(serde_json::Value::Null)
    }

    #[test]
    fn load_agents_finds_all_agents() {
        let agents = load_agents(&claude_dir());
        assert!(
            agents.len() >= 5,
            "expected at least 5 agents, got {}",
            agents.len()
        );
    }

    #[test]
    fn load_agents_parses_planner_correctly() {
        let agents = load_agents(&claude_dir());
        let planner = agents
            .iter()
            .find(|a| a.name == "planner")
            .expect("planner agent not found");
        assert_eq!(planner.model, "opus");
        assert!(!planner.description.is_empty());
        assert!(planner.disallowed_tools.contains(&"Edit".to_string()));
    }

    #[test]
    fn load_skills_finds_all_skills() {
        let skills = load_skills(&claude_dir());
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(
            skills.len() >= 8,
            "expected at least 8 skills, got {}; names: {:?}",
            skills.len(),
            names
        );
    }

    #[test]
    fn load_rules_finds_all_rules() {
        let rules = load_rules(&claude_dir());
        assert!(
            rules.len() >= 11,
            "expected at least 11 rules, got {}",
            rules.len()
        );
    }

    #[test]
    fn load_rules_counts_workflow_rules() {
        let rules = load_rules(&claude_dir());
        let workflow = rules
            .iter()
            .find(|r| r.file_name == "workflow.md")
            .expect("workflow.md not found");
        assert!(
            workflow.rule_count >= 5,
            "expected at least 5 rules in workflow.md, got {}",
            workflow.rule_count
        );
    }

    #[test]
    fn load_hooks_finds_pretooluse() {
        let base = claude_dir();
        let settings = read_settings(&base);
        let (hooks, _scripts) = load_hooks_from(&settings, &base);
        let has_pretooluse = hooks.iter().any(|h| h.event == "PreToolUse");
        assert!(has_pretooluse, "expected PreToolUse hook registration");
    }

    #[test]
    fn load_hooks_lists_scripts() {
        let base = claude_dir();
        let settings = read_settings(&base);
        let (_hooks, scripts) = load_hooks_from(&settings, &base);
        assert!(
            scripts.len() >= 9,
            "expected at least 9 hook scripts, got {}",
            scripts.len()
        );
    }

    #[test]
    fn load_plugins_finds_all() {
        let base = claude_dir();
        let settings = read_settings(&base);
        let plugins = load_plugins_from(&settings);
        assert!(
            plugins.len() >= 5,
            "expected at least 5 plugins, got {}",
            plugins.len()
        );
    }

    #[test]
    fn load_all_populates_inventory() {
        let inv = load_all(&claude_dir());
        assert!(!inv.agents.is_empty());
        assert!(!inv.skills.is_empty());
        assert!(!inv.rules.is_empty());
        assert!(!inv.hooks.is_empty());
        assert!(!inv.hook_scripts.is_empty());
        assert!(!inv.plugins.is_empty());
    }

    #[test]
    fn missing_directory_returns_empty() {
        let fake = PathBuf::from("/tmp/nonexistent-claude-dir");
        let inv = load_all(&fake);
        assert_eq!(inv.total_items(), 0);
    }

    #[test]
    fn extract_frontmatter_parses_yaml_block() {
        let content = "---\nname: test\nmodel: opus\n---\nBody here";
        let fm = extract_frontmatter(content).unwrap();
        assert!(fm.contains("name: test"));
        assert!(fm.contains("model: opus"));
    }

    #[test]
    fn extract_frontmatter_returns_none_without_delimiter() {
        assert!(extract_frontmatter("no frontmatter here").is_none());
    }

    #[test]
    fn parse_frontmatter_handles_unquoted_colons() {
        let path = claude_dir().join("skills/commit-convention/SKILL.md");
        let content = std::fs::read_to_string(&path).expect("file exists");
        let fm = extract_frontmatter(&content).expect("frontmatter exists");
        let parsed = parse_frontmatter(&fm).expect("parse should succeed");
        let name = parsed.get("name").and_then(|v| v.as_str());
        assert_eq!(name, Some("commit-convention"));
    }
}
