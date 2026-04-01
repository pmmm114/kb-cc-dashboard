use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, ConfigCategory, ConfigFocus};
use super::helpers::{detail_line, render_scroll_indicators};

pub fn draw_config(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(50),
        ])
        .split(area);

    let items = get_category_items(app);
    draw_category_list(f, app, chunks[0]);
    draw_item_list(f, app, &items, chunks[1]);
    draw_item_detail(f, app, &items, chunks[2]);
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

fn draw_category_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = ConfigCategory::ALL
        .iter()
        .map(|cat| {
            let count = app.config_item_count(*cat);
            let line = Line::from(vec![
                Span::styled(cat.label().to_string(), Style::default().fg(Color::White)),
                Span::styled(format!(" ({})", count), Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let idx = ConfigCategory::ALL
        .iter()
        .position(|c| *c == app.config_category)
        .unwrap_or(0);

    let focused = app.config_focus == ConfigFocus::Category;
    let list = List::new(items)
        .block(pane_block("Categories", focused))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    state.select(Some(idx));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_item_list(f: &mut Frame, app: &App, items: &[(String, Vec<Line<'static>>)], area: Rect) {
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|(name, _)| ListItem::new(Line::from(name.clone())))
        .collect();

    let title = format!("{} ({})", app.config_category.label(), items.len());
    let focused = app.config_focus == ConfigFocus::Item;
    let list = List::new(list_items)
        .block(pane_block(&title, focused))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    state.select(Some(
        app.config_item_selected.min(items.len().saturating_sub(1)),
    ));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_item_detail(f: &mut Frame, app: &App, items: &[(String, Vec<Line<'static>>)], area: Rect) {
    let focused = app.config_focus == ConfigFocus::Detail;
    let block = pane_block("Detail", focused);

    if items.is_empty() || app.config_item_selected >= items.len() {
        f.render_widget(block, area);
        return;
    }

    let (_, detail) = &items[app.config_item_selected.min(items.len() - 1)];
    let total_lines = detail.len();
    let paragraph = Paragraph::new(detail.clone())
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.config_detail_scroll as u16, 0));
    f.render_widget(paragraph, area);

    render_scroll_indicators(f, area, total_lines, app.config_detail_scroll);
}

fn get_category_items(app: &App) -> Vec<(String, Vec<Line<'static>>)> {
    match app.config_category {
        ConfigCategory::Agents => app
            .config
            .agents
            .iter()
            .map(|a| {
                (
                    a.name.clone(),
                    vec![
                        detail_line("Name", &a.name),
                        detail_line("Model", &a.model),
                        detail_line("Description", &a.description),
                        detail_line("Disallowed", &a.disallowed_tools.join(", ")),
                        detail_line("File", &a.file_path),
                    ],
                )
            })
            .collect(),
        ConfigCategory::Skills => app
            .config
            .skills
            .iter()
            .map(|s| {
                (
                    s.name.clone(),
                    vec![
                        detail_line("Name", &s.name),
                        detail_line("Description", &s.description),
                        detail_line("File", &s.file_path),
                    ],
                )
            })
            .collect(),
        ConfigCategory::Rules => app
            .config
            .rules
            .iter()
            .map(|r| {
                (
                    r.file_name.clone(),
                    vec![
                        detail_line("File", &r.file_path),
                        detail_line("Rules", &r.rule_count.to_string()),
                        detail_line("Hard Gates", &r.hard_gate_count.to_string()),
                        detail_line("Rule Names", &r.rule_names.join(", ")),
                    ],
                )
            })
            .collect(),
        ConfigCategory::Hooks => {
            let mut items: Vec<(String, Vec<Line<'static>>)> = app
                .config
                .hooks
                .iter()
                .map(|h| {
                    let name =
                        format!("{} [{}]", h.event, h.matcher.as_deref().unwrap_or("*"));
                    let mut lines = vec![
                        detail_line("Event", &h.event),
                        detail_line("Matcher", h.matcher.as_deref().unwrap_or("(all)")),
                        detail_line("Type", &h.hook_type),
                    ];
                    if let Some(cmd) = &h.command {
                        lines.push(detail_line("Command", cmd));
                    }
                    if let Some(prompt) = &h.prompt {
                        lines.push(detail_line("Prompt", prompt));
                    }
                    if let Some(timeout) = h.timeout {
                        lines.push(detail_line("Timeout", &format!("{}s", timeout)));
                    }
                    lines.push(detail_line("Async", &h.is_async.to_string()));
                    (name, lines)
                })
                .collect();
            for script in &app.config.hook_scripts {
                items.push((
                    format!("[script] {}", script),
                    vec![
                        detail_line("Type", "Script file"),
                        detail_line("File", script),
                    ],
                ));
            }
            items
        }
        ConfigCategory::Plugins => app
            .config
            .plugins
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    vec![
                        detail_line("Name", &p.name),
                        detail_line("Enabled", &p.enabled.to_string()),
                    ],
                )
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use crate::test_helpers::test_utils::buffer_to_string;
    use ratatui::{backend::TestBackend, Terminal};

    fn make_test_inventory() -> ConfigInventory {
        ConfigInventory {
            agents: vec![
                AgentConfig {
                    name: "planner".into(),
                    description: "Plans things".into(),
                    model: "opus".into(),
                    disallowed_tools: vec!["Edit".into(), "Write".into()],
                    file_path: "agents/planner.md".into(),
                },
                AgentConfig {
                    name: "tdd-implementer".into(),
                    description: "Implements via TDD".into(),
                    model: "opus".into(),
                    disallowed_tools: vec![],
                    file_path: "agents/tdd-implementer.md".into(),
                },
            ],
            skills: vec![SkillConfig {
                name: "benchmark".into(),
                description: "Runs benchmarks".into(),
                file_path: "skills/benchmark/SKILL.md".into(),
            }],
            rules: vec![RuleConfig {
                file_path: "rules/workflow.md".into(),
                file_name: "workflow.md".into(),
                rule_count: 3,
                hard_gate_count: 1,
                rule_names: vec!["plan-before-act".into(), "focused-execution".into()],
            }],
            hooks: vec![HookRegistration {
                event: "PreToolUse".into(),
                matcher: Some("Edit|Write".into()),
                hook_type: "command".into(),
                command: Some("bash pre-edit-guard.sh".into()),
                prompt: None,
                timeout: Some(5000),
                is_async: false,
            }],
            hook_scripts: vec!["hook-lib.sh".into()],
            plugins: vec![PluginConfig {
                name: "typescript-lsp".into(),
                enabled: true,
            }],
        }
    }

    fn render_config(app: &App, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                draw_config(f, app, area);
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn category_list_shows_all_five_categories() {
        let mut app = App::new(make_test_inventory());
        app.active_tab = crate::app::Tab::Config;
        let buf = render_config(&app, 80, 20);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Agents"), "Expected Agents category");
        assert!(content.contains("Skills"), "Expected Skills category");
        assert!(content.contains("Rules"), "Expected Rules category");
        assert!(content.contains("Hooks"), "Expected Hooks category");
        assert!(content.contains("Plugins"), "Expected Plugins category");
    }

    #[test]
    fn category_list_shows_counts() {
        let mut app = App::new(make_test_inventory());
        app.active_tab = crate::app::Tab::Config;
        let buf = render_config(&app, 80, 20);
        let content = buffer_to_string(&buf);
        assert!(content.contains("(2)"), "Expected agents count (2)");
        assert!(content.contains("(1)"), "Expected skills/rules/hooks/plugins count (1)");
    }

    #[test]
    fn category_list_shows_categories_title() {
        let mut app = App::new(make_test_inventory());
        app.active_tab = crate::app::Tab::Config;
        let buf = render_config(&app, 80, 20);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Categories"), "Expected Categories block title");
    }

    #[test]
    fn item_list_shows_items_for_agents() {
        let mut app = App::new(make_test_inventory());
        app.active_tab = crate::app::Tab::Config;
        app.config_focus = crate::app::ConfigFocus::Item;
        app.config_category = ConfigCategory::Agents;
        let buf = render_config(&app, 80, 20);
        let content = buffer_to_string(&buf);
        assert!(content.contains("planner"), "Expected planner agent");
        assert!(
            content.contains("tdd-implementer"),
            "Expected tdd-implementer agent"
        );
    }

    #[test]
    fn item_list_title_includes_category_and_count() {
        let mut app = App::new(make_test_inventory());
        app.active_tab = crate::app::Tab::Config;
        app.config_focus = crate::app::ConfigFocus::Item;
        app.config_category = ConfigCategory::Agents;
        let buf = render_config(&app, 80, 20);
        let content = buffer_to_string(&buf);
        assert!(
            content.contains("Agents (2)"),
            "Expected 'Agents (2)' in title"
        );
    }

    #[test]
    fn detail_panel_shows_agent_fields() {
        let mut app = App::new(make_test_inventory());
        app.active_tab = crate::app::Tab::Config;
        app.config_focus = crate::app::ConfigFocus::Item;
        app.config_category = ConfigCategory::Agents;
        app.config_item_selected = 0;
        let buf = render_config(&app, 100, 20);
        let content = buffer_to_string(&buf);
        assert!(content.contains("opus"), "Expected model 'opus' in detail");
        assert!(
            content.contains("Plans things"),
            "Expected description in detail"
        );
    }

    #[test]
    fn detail_panel_shows_hook_fields() {
        let mut app = App::new(make_test_inventory());
        app.active_tab = crate::app::Tab::Config;
        app.config_focus = crate::app::ConfigFocus::Item;
        app.config_category = ConfigCategory::Hooks;
        app.config_item_selected = 0;
        let buf = render_config(&app, 100, 20);
        let content = buffer_to_string(&buf);
        assert!(
            content.contains("PreToolUse"),
            "Expected event name in detail"
        );
        assert!(
            content.contains("command"),
            "Expected hook type in detail"
        );
    }

    #[test]
    fn detail_panel_shows_rule_fields() {
        let mut app = App::new(make_test_inventory());
        app.active_tab = crate::app::Tab::Config;
        app.config_focus = crate::app::ConfigFocus::Item;
        app.config_category = ConfigCategory::Rules;
        app.config_item_selected = 0;
        let buf = render_config(&app, 100, 20);
        let content = buffer_to_string(&buf);
        assert!(
            content.contains("workflow.md"),
            "Expected rule file path in detail"
        );
        assert!(
            content.contains("plan-before-act"),
            "Expected rule names in detail"
        );
    }

    #[test]
    fn empty_detail_does_not_panic() {
        let app = App::new(ConfigInventory::default());
        let _buf = render_config(&app, 80, 20);
    }

    #[test]
    fn draw_does_not_panic_with_out_of_bounds_selection() {
        let mut app = App::new(make_test_inventory());
        app.config_focus = crate::app::ConfigFocus::Item;
        app.config_item_selected = 999;
        let _buf = render_config(&app, 80, 20);
    }

    #[test]
    fn get_category_items_returns_correct_count_for_each_category() {
        let app = App::new(make_test_inventory());
        for cat in ConfigCategory::ALL {
            let expected = app.config_item_count(cat);
            // Temporarily set category to check
            let mut test_app = App::new(make_test_inventory());
            test_app.config_category = cat;
            let items = get_category_items(&test_app);
            assert_eq!(
                items.len(),
                expected,
                "Mismatch for {:?}: got {} expected {}",
                cat,
                items.len(),
                expected
            );
        }
    }

    #[test]
    fn plugin_detail_shows_enabled_status() {
        let mut app = App::new(make_test_inventory());
        app.config_focus = crate::app::ConfigFocus::Item;
        app.config_category = ConfigCategory::Plugins;
        app.config_item_selected = 0;
        let buf = render_config(&app, 100, 20);
        let content = buffer_to_string(&buf);
        assert!(
            content.contains("typescript-lsp"),
            "Expected plugin name"
        );
        assert!(content.contains("true"), "Expected enabled=true");
    }

    #[test]
    fn skill_detail_shows_description() {
        let mut app = App::new(make_test_inventory());
        app.config_focus = crate::app::ConfigFocus::Item;
        app.config_category = ConfigCategory::Skills;
        app.config_item_selected = 0;
        let buf = render_config(&app, 100, 20);
        let content = buffer_to_string(&buf);
        assert!(
            content.contains("Runs benchmarks"),
            "Expected skill description"
        );
    }

    #[test]
    fn all_three_panes_visible_simultaneously() {
        let mut app = App::new(make_test_inventory());
        app.active_tab = crate::app::Tab::Config;
        app.config_category = ConfigCategory::Agents;
        // With default focus on Category, all 3 panes should still render
        let buf = render_config(&app, 120, 20);
        let content = buffer_to_string(&buf);
        // Category pane title
        assert!(content.contains("Categories"), "Expected Categories pane");
        // Item pane title (shows category name + count)
        assert!(content.contains("Agents (2)"), "Expected Items pane with Agents (2)");
        // Detail pane title
        assert!(content.contains("Detail"), "Expected Detail pane");
        // Items are visible even without Item focus (3-pane always shows all)
        assert!(content.contains("planner"), "Expected planner in items pane");
    }

    #[test]
    fn three_pane_layout_renders_all_categories() {
        // Even when focus is on Detail, category list is visible
        let mut app = App::new(make_test_inventory());
        app.active_tab = crate::app::Tab::Config;
        app.config_focus = crate::app::ConfigFocus::Detail;
        let buf = render_config(&app, 120, 20);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Agents"), "Categories pane should show Agents");
        assert!(content.contains("Skills"), "Categories pane should show Skills");
    }
}
