use crate::config::ConfigInventory;
use crate::event::{EventKind, HookEvent};
use crate::session::SessionState;
use std::collections::{HashMap, HashSet, VecDeque};

const MAX_EVENTS: usize = 500;

#[derive(Clone, Debug)]
pub struct ToolRecord {
    pub name: String,
    pub count: usize,
    pub failure_count: usize,
}

#[derive(Clone, Debug)]
pub struct ActiveAgent {
    pub agent_type: String,
    pub cwd: Option<String>,
    pub tools: Vec<ToolRecord>,
    pub rules: HashSet<String>,
}

#[derive(Clone, Debug)]
pub struct TaskInfo {
    pub task_id: String,
    pub teammate_name: Option<String>,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Sessions,
    Config,
    Events,
}

impl Tab {
    pub fn next(&self) -> Self {
        match self {
            Tab::Sessions => Tab::Config,
            Tab::Config => Tab::Events,
            Tab::Events => Tab::Sessions,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Tab::Sessions => "Sessions",
            Tab::Config => "Config",
            Tab::Events => "Events",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigCategory {
    Agents,
    Skills,
    Rules,
    Hooks,
    Plugins,
}

impl ConfigCategory {
    pub const ALL: [ConfigCategory; 5] = [
        ConfigCategory::Agents,
        ConfigCategory::Skills,
        ConfigCategory::Rules,
        ConfigCategory::Hooks,
        ConfigCategory::Plugins,
    ];

    pub fn next(&self) -> Self {
        match self {
            ConfigCategory::Agents => ConfigCategory::Skills,
            ConfigCategory::Skills => ConfigCategory::Rules,
            ConfigCategory::Rules => ConfigCategory::Hooks,
            ConfigCategory::Hooks => ConfigCategory::Plugins,
            ConfigCategory::Plugins => ConfigCategory::Agents,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            ConfigCategory::Agents => ConfigCategory::Plugins,
            ConfigCategory::Skills => ConfigCategory::Agents,
            ConfigCategory::Rules => ConfigCategory::Skills,
            ConfigCategory::Hooks => ConfigCategory::Rules,
            ConfigCategory::Plugins => ConfigCategory::Hooks,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            ConfigCategory::Agents => "Agents",
            ConfigCategory::Skills => "Skills",
            ConfigCategory::Rules => "Rules",
            ConfigCategory::Hooks => "Hooks",
            ConfigCategory::Plugins => "Plugins",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFocus {
    Category,
    Item,
    Detail,
}

/// Focus state for two-pane tabs (Sessions, Events).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListDetailFocus {
    List,
    Detail,
}

pub struct App {
    pub active_tab: Tab,
    pub sessions: Vec<SessionState>,
    pub config: ConfigInventory,
    pub events: VecDeque<HookEvent>,

    pub session_selected: usize,
    pub config_category: ConfigCategory,
    pub config_item_selected: usize,
    pub config_focus: ConfigFocus,
    pub event_selected: usize,
    pub event_auto_scroll: bool,

    pub should_quit: bool,

    // Per-tab focus state
    pub session_focus: ListDetailFocus,
    pub event_focus: ListDetailFocus,

    // Per-tab detail panel scroll offsets
    pub config_detail_scroll: usize,
    pub session_detail_scroll: usize,
    pub event_detail_scroll: usize,

    // Per-session event indexing
    pub session_events: HashMap<String, VecDeque<HookEvent>>,
    pub active_session_ids: HashSet<String>,
    pub events_session_filter: Option<String>,

    // Agent-to-tool hierarchy
    pub session_active_agents: HashMap<String, Vec<ActiveAgent>>,
    pub session_orchestrator_tools: HashMap<String, Vec<ToolRecord>>,
    pub session_orchestrator_rules: HashMap<String, HashSet<String>>,
    pub session_tasks: HashMap<String, Vec<TaskInfo>>,
}

impl App {
    pub fn new(config: ConfigInventory) -> Self {
        Self {
            active_tab: Tab::Sessions,
            sessions: Vec::new(),
            config,
            events: VecDeque::new(),
            session_selected: 0,
            config_category: ConfigCategory::Agents,
            config_item_selected: 0,
            config_focus: ConfigFocus::Category,
            event_selected: 0,
            event_auto_scroll: true,
            should_quit: false,
            session_focus: ListDetailFocus::List,
            event_focus: ListDetailFocus::List,
            config_detail_scroll: 0,
            session_detail_scroll: 0,
            event_detail_scroll: 0,
            session_events: HashMap::new(),
            active_session_ids: HashSet::new(),
            events_session_filter: None,
            session_active_agents: HashMap::new(),
            session_orchestrator_tools: HashMap::new(),
            session_orchestrator_rules: HashMap::new(),
            session_tasks: HashMap::new(),
        }
    }

    pub fn push_event(&mut self, event: HookEvent) {
        let sid = event.session_id.clone();

        // Track active session
        self.active_session_ids.insert(sid.clone());

        // Index event per session
        let session_queue = self.session_events.entry(sid.clone()).or_default();
        session_queue.push_back(event.clone());
        if session_queue.len() > MAX_EVENTS {
            session_queue.pop_front();
        }

        // Aggregate based on event kind
        match event.kind() {
            EventKind::SubagentStart => {
                if let Some(agent_type) = event.payload.get("agent_type").and_then(|v| v.as_str())
                {
                    let cwd = event
                        .payload
                        .get("cwd")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let agent = ActiveAgent {
                        agent_type: agent_type.to_string(),
                        cwd,
                        tools: Vec::new(),
                        rules: HashSet::new(),
                    };
                    self.session_active_agents
                        .entry(sid.clone())
                        .or_default()
                        .push(agent);
                }
            }
            EventKind::SubagentStop => {
                if let Some(agent_type) = event.payload.get("agent_type").and_then(|v| v.as_str())
                {
                    let cwd = event
                        .payload
                        .get("cwd")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    if let Some(agents) = self.session_active_agents.get_mut(&sid) {
                        // Remove last match by agent_type + cwd
                        if let Some(pos) = agents.iter().rposition(|a| {
                            a.agent_type == agent_type && a.cwd == cwd
                        }) {
                            agents.remove(pos);
                        }
                    }
                }
            }
            EventKind::InstructionsLoaded => {
                if let Some(file_path) = event.payload.get("file_path").and_then(|v| v.as_str()) {
                    let name = std::path::Path::new(file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(file_path)
                        .to_string();
                    let agent_ctx = event
                        .payload
                        .get("agent_context_type")
                        .and_then(|v| v.as_str());
                    let routed_to_agent = agent_ctx
                        .and_then(|act| self.find_active_agent_mut(&sid, act))
                        .map(|agent| agent.rules.insert(name.clone()))
                        .is_some();
                    if !routed_to_agent {
                        self.session_orchestrator_rules
                            .entry(sid.clone())
                            .or_default()
                            .insert(name);
                    }
                }
            }
            EventKind::PreToolUse | EventKind::PostToolUse => {
                if let Some(tool_name) = event.payload.get("tool_name").and_then(|v| v.as_str()) {
                    let agent_ctx = event
                        .payload
                        .get("agent_context_type")
                        .and_then(|v| v.as_str());
                    self.increment_tool(&sid, tool_name, agent_ctx, false);
                }
            }
            EventKind::PostToolUseFailure => {
                if let Some(tool_name) = event.payload.get("tool_name").and_then(|v| v.as_str()) {
                    let agent_ctx = event
                        .payload
                        .get("agent_context_type")
                        .and_then(|v| v.as_str());
                    self.increment_tool(&sid, tool_name, agent_ctx, true);
                }
            }
            EventKind::TaskCreated => {
                let task_id = event
                    .payload
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let teammate_name = event
                    .payload
                    .get("teammate_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                self.session_tasks
                    .entry(sid.clone())
                    .or_default()
                    .push(TaskInfo {
                        task_id,
                        teammate_name,
                        completed: false,
                    });
            }
            EventKind::TaskCompleted => {
                if let Some(task_id) = event.payload.get("task_id").and_then(|v| v.as_str()) {
                    if let Some(tasks) = self.session_tasks.get_mut(&sid) {
                        if let Some(task) = tasks.iter_mut().rfind(|t| t.task_id == task_id) {
                            task.completed = true;
                        }
                    }
                }
            }
            EventKind::SessionEnd => {
                self.active_session_ids.remove(&sid);
            }
            _ => {}
        }

        // Keep existing flat queue behavior
        self.events.push_back(event);
        if self.events.len() > MAX_EVENTS {
            self.events.pop_front();
            if !self.event_auto_scroll && self.event_selected > 0 {
                self.event_selected -= 1;
            }
        }
        if self.event_auto_scroll {
            self.event_selected = self.events.len().saturating_sub(1);
        }
    }

    /// Find the last active agent matching the given agent_context_type.
    pub fn find_active_agent_mut(
        &mut self,
        sid: &str,
        agent_context_type: &str,
    ) -> Option<&mut ActiveAgent> {
        self.session_active_agents
            .get_mut(sid)
            .and_then(|agents| {
                agents
                    .iter_mut()
                    .rev()
                    .find(|a| a.agent_type == agent_context_type)
            })
    }

    /// Increment a tool record, routing to the matching active agent or the orchestrator.
    fn increment_tool(
        &mut self,
        sid: &str,
        tool_name: &str,
        agent_context_type: Option<&str>,
        is_failure: bool,
    ) {
        let tools = if let Some(act) = agent_context_type {
            if let Some(agent) = self.find_active_agent_mut(sid, act) {
                &mut agent.tools
            } else {
                self.session_orchestrator_tools
                    .entry(sid.to_string())
                    .or_default()
            }
        } else {
            self.session_orchestrator_tools
                .entry(sid.to_string())
                .or_default()
        };
        let record = Self::get_or_create_tool(tools, tool_name);
        record.count += 1;
        if is_failure {
            record.failure_count += 1;
        }
    }

    /// Get or create a ToolRecord by name in the given tool vec.
    pub fn get_or_create_tool<'a>(tools: &'a mut Vec<ToolRecord>, name: &str) -> &'a mut ToolRecord {
        let pos = tools.iter().position(|t| t.name == name);
        match pos {
            Some(i) => &mut tools[i],
            None => {
                tools.push(ToolRecord {
                    name: name.to_string(),
                    count: 0,
                    failure_count: 0,
                });
                tools.last_mut().unwrap()
            }
        }
    }

    pub fn update_sessions(&mut self, sessions: Vec<SessionState>) {
        self.sessions = sessions;
        if self.session_selected >= self.sessions.len() && !self.sessions.is_empty() {
            self.session_selected = self.sessions.len() - 1;
        }
    }

    pub fn on_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => self.on_esc(),
            KeyCode::Tab => {
                self.active_tab = self.active_tab.next();
                // Reset target tab's focus and scroll
                match self.active_tab {
                    Tab::Sessions => {
                        self.session_focus = ListDetailFocus::List;
                        self.session_detail_scroll = 0;
                    }
                    Tab::Events => {
                        self.event_focus = ListDetailFocus::List;
                        self.event_detail_scroll = 0;
                    }
                    Tab::Config => {
                        self.config_focus = ConfigFocus::Category;
                        self.config_detail_scroll = 0;
                    }
                }
            }
            KeyCode::Up => self.navigate_up(),
            KeyCode::Down => self.navigate_down(),
            KeyCode::Left => self.navigate_left(),
            KeyCode::Right => self.navigate_right(),
            KeyCode::Enter => self.on_enter(),
            KeyCode::Char('G') | KeyCode::End => {
                if self.active_tab == Tab::Events {
                    self.event_auto_scroll = true;
                    self.event_selected = self.events.len().saturating_sub(1);
                }
            }
            KeyCode::PageDown => {
                match self.active_tab {
                    Tab::Sessions if self.session_focus == ListDetailFocus::Detail => {
                        self.session_detail_scroll += 5;
                    }
                    Tab::Events if self.event_focus == ListDetailFocus::Detail => {
                        self.event_detail_scroll += 5;
                    }
                    Tab::Config if self.config_focus == ConfigFocus::Detail => {
                        self.config_detail_scroll += 5;
                    }
                    _ => {}
                }
            }
            KeyCode::PageUp => {
                match self.active_tab {
                    Tab::Sessions if self.session_focus == ListDetailFocus::Detail => {
                        self.session_detail_scroll = self.session_detail_scroll.saturating_sub(5);
                    }
                    Tab::Events if self.event_focus == ListDetailFocus::Detail => {
                        self.event_detail_scroll = self.event_detail_scroll.saturating_sub(5);
                    }
                    Tab::Config if self.config_focus == ConfigFocus::Detail => {
                        self.config_detail_scroll = self.config_detail_scroll.saturating_sub(5);
                    }
                    _ => {}
                }
            }
            KeyCode::Char('f') => {
                if self.active_tab == Tab::Events
                    && self.event_focus == ListDetailFocus::List
                {
                    self.cycle_session_filter();
                }
            }
            _ => {}
        }
    }

    fn on_esc(&mut self) {
        match self.active_tab {
            Tab::Sessions => {
                if self.session_focus == ListDetailFocus::Detail {
                    self.session_focus = ListDetailFocus::List;
                    self.session_detail_scroll = 0;
                }
                // No-op at List level
            }
            Tab::Events => {
                if self.event_focus == ListDetailFocus::Detail {
                    self.event_focus = ListDetailFocus::List;
                    self.event_detail_scroll = 0;
                }
                // No-op at List level
            }
            Tab::Config => {
                match self.config_focus {
                    ConfigFocus::Detail => {
                        self.config_focus = ConfigFocus::Item;
                        self.config_detail_scroll = 0;
                    }
                    ConfigFocus::Item => self.config_focus = ConfigFocus::Category,
                    ConfigFocus::Category => {} // No-op at root
                }
            }
        }
    }

    fn navigate_up(&mut self) {
        match self.active_tab {
            Tab::Sessions => match self.session_focus {
                ListDetailFocus::List => {
                    self.session_selected = self.session_selected.saturating_sub(1);
                    self.session_detail_scroll = 0;
                }
                ListDetailFocus::Detail => {
                    self.session_detail_scroll = self.session_detail_scroll.saturating_sub(1);
                }
            },
            Tab::Config => match self.config_focus {
                ConfigFocus::Category => {
                    self.config_category = self.config_category.prev();
                    self.config_item_selected = 0;
                    self.config_detail_scroll = 0;
                }
                ConfigFocus::Item => {
                    self.config_item_selected = self.config_item_selected.saturating_sub(1);
                    self.config_detail_scroll = 0;
                }
                ConfigFocus::Detail => {
                    self.config_detail_scroll = self.config_detail_scroll.saturating_sub(1);
                }
            },
            Tab::Events => match self.event_focus {
                ListDetailFocus::List => {
                    self.event_auto_scroll = false;
                    self.event_selected = self.event_selected.saturating_sub(1);
                    self.event_detail_scroll = 0;
                }
                ListDetailFocus::Detail => {
                    self.event_detail_scroll = self.event_detail_scroll.saturating_sub(1);
                }
            },
        }
    }

    fn navigate_down(&mut self) {
        match self.active_tab {
            Tab::Sessions => match self.session_focus {
                ListDetailFocus::List => {
                    if !self.sessions.is_empty() {
                        self.session_selected =
                            (self.session_selected + 1).min(self.sessions.len() - 1);
                        self.session_detail_scroll = 0;
                    }
                }
                ListDetailFocus::Detail => {
                    self.session_detail_scroll += 1;
                }
            },
            Tab::Config => match self.config_focus {
                ConfigFocus::Category => {
                    self.config_category = self.config_category.next();
                    self.config_item_selected = 0;
                    self.config_detail_scroll = 0;
                }
                ConfigFocus::Item => {
                    let count = self.config_item_count(self.config_category);
                    if count > 0 {
                        self.config_item_selected =
                            (self.config_item_selected + 1).min(count - 1);
                        self.config_detail_scroll = 0;
                    }
                }
                ConfigFocus::Detail => {
                    self.config_detail_scroll += 1;
                }
            },
            Tab::Events => match self.event_focus {
                ListDetailFocus::List => {
                    if !self.events.is_empty() {
                        self.event_auto_scroll = false;
                        self.event_selected =
                            (self.event_selected + 1).min(self.events.len() - 1);
                        self.event_detail_scroll = 0;
                    }
                }
                ListDetailFocus::Detail => {
                    self.event_detail_scroll += 1;
                }
            },
        }
    }

    fn navigate_left(&mut self) {
        match self.active_tab {
            Tab::Sessions => {
                if self.session_focus == ListDetailFocus::Detail {
                    self.session_focus = ListDetailFocus::List;
                    self.session_detail_scroll = 0;
                }
            }
            Tab::Events => {
                if self.event_focus == ListDetailFocus::Detail {
                    self.event_focus = ListDetailFocus::List;
                    self.event_detail_scroll = 0;
                }
            }
            Tab::Config => match self.config_focus {
                ConfigFocus::Detail => {
                    self.config_focus = ConfigFocus::Item;
                    self.config_detail_scroll = 0;
                }
                ConfigFocus::Item => self.config_focus = ConfigFocus::Category,
                ConfigFocus::Category => {}
            },
        }
    }

    fn navigate_right(&mut self) {
        match self.active_tab {
            Tab::Sessions => {
                if self.session_focus == ListDetailFocus::List {
                    self.session_focus = ListDetailFocus::Detail;
                }
            }
            Tab::Events => {
                if self.event_focus == ListDetailFocus::List {
                    self.event_focus = ListDetailFocus::Detail;
                }
            }
            Tab::Config => match self.config_focus {
                ConfigFocus::Category => self.config_focus = ConfigFocus::Item,
                ConfigFocus::Item => self.config_focus = ConfigFocus::Detail,
                ConfigFocus::Detail => {}
            },
        }
    }

    fn on_enter(&mut self) {
        match self.active_tab {
            Tab::Sessions => {
                if self.session_focus == ListDetailFocus::List {
                    self.session_focus = ListDetailFocus::Detail;
                }
            }
            Tab::Events => {
                if self.event_focus == ListDetailFocus::List {
                    self.event_focus = ListDetailFocus::Detail;
                }
            }
            Tab::Config => {
                // Enter acts as Right on Config tab
                self.navigate_right();
            }
        }
    }

    /// Returns events filtered by session if filter is set
    pub fn filtered_events(&self) -> Vec<&HookEvent> {
        match &self.events_session_filter {
            Some(sid) => self.events.iter().filter(|e| &e.session_id == sid).collect(),
            None => self.events.iter().collect(),
        }
    }

    /// Returns all sessions (active and inactive)
    pub fn visible_sessions(&self) -> &[SessionState] {
        &self.sessions
    }

    /// Returns true if the session has sent events to the dashboard
    pub fn is_session_active(&self, session_id: &str) -> bool {
        self.active_session_ids.contains(session_id)
    }

    /// Cycle events_session_filter through None -> each active session -> None
    pub fn cycle_session_filter(&mut self) {
        let mut active: Vec<String> = self.active_session_ids.iter().cloned().collect();
        active.sort();

        self.events_session_filter = match &self.events_session_filter {
            None => active.first().cloned(),
            Some(current) => {
                let pos = active.iter().position(|s| s == current);
                match pos {
                    Some(i) if i + 1 < active.len() => Some(active[i + 1].clone()),
                    _ => None,
                }
            }
        };
        // SE-1: Reset selection and scroll when filter changes
        self.event_selected = 0;
        self.event_detail_scroll = 0;
        self.event_focus = ListDetailFocus::List;
    }

    pub fn config_item_count(&self, category: ConfigCategory) -> usize {
        match category {
            ConfigCategory::Agents => self.config.agents.len(),
            ConfigCategory::Skills => self.config.skills.len(),
            ConfigCategory::Rules => self.config.rules.len(),
            ConfigCategory::Hooks => self.config.hooks.len() + self.config.hook_scripts.len(),
            ConfigCategory::Plugins => self.config.plugins.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_utils::make_test_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    fn event_from_name(name: &str) -> HookEvent {
        make_test_event(&format!(
            r#"{{"hook_event_name":"{}","session_id":"test-1"}}"#,
            name
        ))
    }

    #[test]
    fn new_app_defaults() {
        let app = App::new(ConfigInventory::default());
        assert_eq!(app.active_tab, Tab::Sessions);
        assert!(app.sessions.is_empty());
        assert!(app.events.is_empty());
        assert_eq!(app.session_selected, 0);
        assert_eq!(app.config_category, ConfigCategory::Agents);
        assert_eq!(app.config_focus, ConfigFocus::Category);
        assert!(app.event_auto_scroll);
        assert!(!app.should_quit);
        assert_eq!(app.session_focus, ListDetailFocus::List);
        assert_eq!(app.event_focus, ListDetailFocus::List);
        assert_eq!(app.config_detail_scroll, 0);
        assert_eq!(app.session_detail_scroll, 0);
        assert_eq!(app.event_detail_scroll, 0);
    }

    #[test]
    fn tab_cycles() {
        assert_eq!(Tab::Sessions.next(), Tab::Config);
        assert_eq!(Tab::Config.next(), Tab::Events);
        assert_eq!(Tab::Events.next(), Tab::Sessions);
    }

    #[test]
    fn tab_labels() {
        assert_eq!(Tab::Sessions.label(), "Sessions");
        assert_eq!(Tab::Config.label(), "Config");
        assert_eq!(Tab::Events.label(), "Events");
    }

    #[test]
    fn config_category_cycles() {
        assert_eq!(ConfigCategory::Agents.next(), ConfigCategory::Skills);
        assert_eq!(ConfigCategory::Plugins.next(), ConfigCategory::Agents);
        assert_eq!(ConfigCategory::Agents.prev(), ConfigCategory::Plugins);
        assert_eq!(ConfigCategory::Skills.prev(), ConfigCategory::Agents);
    }

    #[test]
    fn config_category_labels() {
        assert_eq!(ConfigCategory::Agents.label(), "Agents");
        assert_eq!(ConfigCategory::Rules.label(), "Rules");
    }

    #[test]
    fn push_event_appends_and_auto_scrolls() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        assert_eq!(app.events.len(), 1);
        assert_eq!(app.event_selected, 0);

        app.push_event(event_from_name("PostToolUse"));
        assert_eq!(app.events.len(), 2);
        assert_eq!(app.event_selected, 1);
    }

    #[test]
    fn push_event_respects_max_capacity() {
        let mut app = App::new(ConfigInventory::default());
        for i in 0..=MAX_EVENTS {
            app.push_event(event_from_name(&format!("Event{}", i)));
        }
        assert_eq!(app.events.len(), MAX_EVENTS);
        assert_eq!(app.event_selected, MAX_EVENTS - 1);
    }

    #[test]
    fn push_event_does_not_auto_scroll_when_disabled() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        app.event_auto_scroll = false;
        app.event_selected = 0;

        app.push_event(event_from_name("PostToolUse"));
        assert_eq!(app.event_selected, 0);
    }

    #[test]
    fn quit_on_q() {
        let mut app = App::new(ConfigInventory::default());
        app.on_key(make_key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn esc_does_not_quit() {
        let mut app = App::new(ConfigInventory::default());
        app.on_key(make_key(KeyCode::Esc));
        assert!(!app.should_quit, "Esc should never quit; only q quits");
    }

    #[test]
    fn tab_key_cycles_tabs() {
        let mut app = App::new(ConfigInventory::default());
        assert_eq!(app.active_tab, Tab::Sessions);
        app.on_key(make_key(KeyCode::Tab));
        assert_eq!(app.active_tab, Tab::Config);
        app.on_key(make_key(KeyCode::Tab));
        assert_eq!(app.active_tab, Tab::Events);
        app.on_key(make_key(KeyCode::Tab));
        assert_eq!(app.active_tab, Tab::Sessions);
    }

    #[test]
    fn navigate_sessions_up_down() {
        let mut app = App::new(ConfigInventory::default());
        let s1: SessionState = serde_json::from_str(r#"{"phase":"idle"}"#).unwrap();
        let s2: SessionState = serde_json::from_str(r#"{"phase":"planning"}"#).unwrap();
        app.update_sessions(vec![s1, s2]);

        assert_eq!(app.session_selected, 0);
        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.session_selected, 1);
        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.session_selected, 1); // clamped
        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.session_selected, 0);
        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.session_selected, 0); // clamped at 0
    }

    #[test]
    fn navigate_events_disables_auto_scroll() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.push_event(event_from_name("PreToolUse"));
        app.push_event(event_from_name("PostToolUse"));
        assert!(app.event_auto_scroll);

        app.on_key(make_key(KeyCode::Up));
        assert!(!app.event_auto_scroll);
    }

    #[test]
    fn end_key_enables_auto_scroll() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.push_event(event_from_name("PreToolUse"));
        app.push_event(event_from_name("PostToolUse"));
        app.event_auto_scroll = false;
        app.event_selected = 0;

        app.on_key(make_key(KeyCode::End));
        assert!(app.event_auto_scroll);
        assert_eq!(app.event_selected, 1);
    }

    #[test]
    fn config_focus_starts_at_category() {
        let app = App::new(ConfigInventory::default());
        assert_eq!(app.config_focus, ConfigFocus::Category);
    }

    #[test]
    fn config_right_moves_focus_category_to_item() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        assert_eq!(app.config_focus, ConfigFocus::Category);

        app.on_key(make_key(KeyCode::Right));
        assert_eq!(app.config_focus, ConfigFocus::Item);
    }

    #[test]
    fn config_right_moves_focus_item_to_detail() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Item;

        app.on_key(make_key(KeyCode::Right));
        assert_eq!(app.config_focus, ConfigFocus::Detail);
    }

    #[test]
    fn config_right_stops_at_detail() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Detail;

        app.on_key(make_key(KeyCode::Right));
        assert_eq!(app.config_focus, ConfigFocus::Detail);
    }

    #[test]
    fn config_left_moves_focus_detail_to_item() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Detail;

        app.on_key(make_key(KeyCode::Left));
        assert_eq!(app.config_focus, ConfigFocus::Item);
    }

    #[test]
    fn config_left_moves_focus_item_to_category() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Item;

        app.on_key(make_key(KeyCode::Left));
        assert_eq!(app.config_focus, ConfigFocus::Category);
    }

    #[test]
    fn config_left_stops_at_category() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        assert_eq!(app.config_focus, ConfigFocus::Category);

        app.on_key(make_key(KeyCode::Left));
        assert_eq!(app.config_focus, ConfigFocus::Category);
    }

    #[test]
    fn config_up_cycles_category_when_category_focused() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Category;
        assert_eq!(app.config_category, ConfigCategory::Agents);

        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.config_category, ConfigCategory::Plugins);
    }

    #[test]
    fn config_down_cycles_category_when_category_focused() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Category;
        assert_eq!(app.config_category, ConfigCategory::Agents);

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.config_category, ConfigCategory::Skills);
    }

    #[test]
    fn config_category_change_resets_item_selection() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Category;
        app.config_item_selected = 3;

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.config_item_selected, 0);
    }

    #[test]
    fn config_up_down_navigates_items_when_item_focused() {
        let mut app = App::new(crate::config::ConfigInventory {
            agents: vec![
                crate::config::AgentConfig {
                    name: "a".into(),
                    description: "".into(),
                    model: "".into(),
                    disallowed_tools: vec![],
                    file_path: "".into(),
                },
                crate::config::AgentConfig {
                    name: "b".into(),
                    description: "".into(),
                    model: "".into(),
                    disallowed_tools: vec![],
                    file_path: "".into(),
                },
            ],
            ..Default::default()
        });
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Item;
        assert_eq!(app.config_item_selected, 0);

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.config_item_selected, 1);

        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.config_item_selected, 0);
    }

    #[test]
    fn config_up_down_scrolls_when_detail_focused() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Detail;
        let cat_before = app.config_category;
        let item_before = app.config_item_selected;

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.config_detail_scroll, 1, "Down should scroll detail");
        assert_eq!(app.config_category, cat_before, "Category must not change");
        assert_eq!(app.config_item_selected, item_before, "Item must not change");

        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.config_detail_scroll, 0, "Up should scroll detail back");
    }

    #[test]
    fn config_enter_acts_as_right() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        assert_eq!(app.config_focus, ConfigFocus::Category);

        app.on_key(make_key(KeyCode::Enter));
        assert_eq!(app.config_focus, ConfigFocus::Item, "Enter should move Category -> Item");

        app.on_key(make_key(KeyCode::Enter));
        assert_eq!(app.config_focus, ConfigFocus::Detail, "Enter should move Item -> Detail");

        app.on_key(make_key(KeyCode::Enter));
        assert_eq!(app.config_focus, ConfigFocus::Detail, "Enter should stop at Detail");
    }

    #[test]
    fn update_sessions_replaces_list() {
        let mut app = App::new(ConfigInventory::default());
        assert!(app.sessions.is_empty());

        let s1: SessionState = serde_json::from_str(r#"{"phase":"idle"}"#).unwrap();
        app.update_sessions(vec![s1]);
        assert_eq!(app.sessions.len(), 1);

        app.update_sessions(vec![]);
        assert!(app.sessions.is_empty());
    }

    // --- Per-session event indexing tests ---

    #[test]
    fn new_app_has_empty_session_tracking_fields() {
        let app = App::new(ConfigInventory::default());
        assert!(app.session_events.is_empty());
        assert!(app.active_session_ids.is_empty());
        assert!(app.events_session_filter.is_none());
        assert!(app.session_active_agents.is_empty());
        assert!(app.session_orchestrator_tools.is_empty());
        assert!(app.session_orchestrator_rules.is_empty());
        assert!(app.session_tasks.is_empty());
    }

    #[test]
    fn push_event_tracks_active_session() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        assert!(app.active_session_ids.contains("test-1"));
    }

    #[test]
    fn push_event_indexes_by_session() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(event_from_name("PreToolUse"));
        app.push_event(event_from_name("PostToolUse"));

        let session_events = app.session_events.get("test-1").unwrap();
        assert_eq!(session_events.len(), 2);
    }

    #[test]
    fn push_event_session_queue_respects_max() {
        let mut app = App::new(ConfigInventory::default());
        for i in 0..=MAX_EVENTS {
            app.push_event(event_from_name(&format!("Event{}", i)));
        }
        let session_events = app.session_events.get("test-1").unwrap();
        assert_eq!(session_events.len(), MAX_EVENTS);
    }

    fn make_event_with_session(name: &str, session_id: &str) -> HookEvent {
        make_test_event(&format!(
            r#"{{"hook_event_name":"{}","session_id":"{}"}}"#,
            name, session_id
        ))
    }

    #[test]
    fn push_event_indexes_multiple_sessions_separately() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("PreToolUse", "sess-a"));
        app.push_event(make_event_with_session("PreToolUse", "sess-b"));
        app.push_event(make_event_with_session("PostToolUse", "sess-a"));

        assert_eq!(app.session_events.get("sess-a").unwrap().len(), 2);
        assert_eq!(app.session_events.get("sess-b").unwrap().len(), 1);
        assert!(app.active_session_ids.contains("sess-a"));
        assert!(app.active_session_ids.contains("sess-b"));
    }

    // --- Aggregation tests ---

    #[test]
    fn push_event_tracks_subagent_start() {
        let mut app = App::new(ConfigInventory::default());
        let event = make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        );
        app.push_event(event);

        let agents = app.session_active_agents.get("s1").unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].agent_type, "planner");
        assert!(agents[0].cwd.is_none());
        assert!(agents[0].tools.is_empty());
        assert!(agents[0].rules.is_empty());
    }

    #[test]
    fn push_event_tracks_subagent_start_with_cwd() {
        let mut app = App::new(ConfigInventory::default());
        let event = make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd-implementer","cwd":"/tmp/worktree-a"}"#,
        );
        app.push_event(event);

        let agents = app.session_active_agents.get("s1").unwrap();
        assert_eq!(agents[0].cwd, Some("/tmp/worktree-a".to_string()));
    }

    #[test]
    fn push_event_removes_agent_on_subagent_stop() {
        let mut app = App::new(ConfigInventory::default());
        let start = make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        );
        let stop = make_test_event(
            r#"{"hook_event_name":"SubagentStop","session_id":"s1","agent_type":"planner"}"#,
        );
        app.push_event(start);
        app.push_event(stop);

        let agents = app.session_active_agents.get("s1").unwrap();
        assert!(agents.is_empty());
    }

    #[test]
    fn push_event_stop_removes_last_match_for_same_type() {
        let mut app = App::new(ConfigInventory::default());
        // Two agents of same type but different cwd
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd","cwd":"/a"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd","cwd":"/b"}"#,
        ));
        // Stop the one with cwd /b
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStop","session_id":"s1","agent_type":"tdd","cwd":"/b"}"#,
        ));

        let agents = app.session_active_agents.get("s1").unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].cwd, Some("/a".to_string()));
    }

    #[test]
    fn push_event_tracks_orchestrator_rules_without_agent_context() {
        let mut app = App::new(ConfigInventory::default());
        let event = make_test_event(
            r#"{"hook_event_name":"InstructionsLoaded","session_id":"s1","file_path":"/home/user/.claude/rules/workflow.md"}"#,
        );
        app.push_event(event);

        let rules = app.session_orchestrator_rules.get("s1").unwrap();
        assert!(rules.contains("workflow.md"));
    }

    #[test]
    fn push_event_tracks_agent_rules_with_agent_context() {
        let mut app = App::new(ConfigInventory::default());
        // Start an agent first
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        ));
        // Load a rule with agent context
        app.push_event(make_test_event(
            r#"{"hook_event_name":"InstructionsLoaded","session_id":"s1","file_path":"/home/user/.claude/rules/investigation.md","agent_context_type":"planner"}"#,
        ));

        let agents = app.session_active_agents.get("s1").unwrap();
        assert!(agents[0].rules.contains("investigation.md"));
        // Should NOT be in orchestrator rules
        assert!(app.session_orchestrator_rules.get("s1").is_none());
    }

    #[test]
    fn push_event_tracks_orchestrator_tools_without_agent_context() {
        let mut app = App::new(ConfigInventory::default());
        let e1 = make_test_event(
            r#"{"hook_event_name":"PreToolUse","session_id":"s1","tool_name":"Read"}"#,
        );
        let e2 = make_test_event(
            r#"{"hook_event_name":"PreToolUse","session_id":"s1","tool_name":"Read"}"#,
        );
        let e3 = make_test_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Edit"}"#,
        );
        app.push_event(e1);
        app.push_event(e2);
        app.push_event(e3);

        let tools = app.session_orchestrator_tools.get("s1").unwrap();
        let read = tools.iter().find(|t| t.name == "Read").unwrap();
        assert_eq!(read.count, 2);
        let edit = tools.iter().find(|t| t.name == "Edit").unwrap();
        assert_eq!(edit.count, 1);
    }

    #[test]
    fn push_event_tracks_agent_tools_with_agent_context() {
        let mut app = App::new(ConfigInventory::default());
        // Start an agent
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        ));
        // Tool use with agent context
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PreToolUse","session_id":"s1","tool_name":"Read","agent_context_type":"planner"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read","agent_context_type":"planner"}"#,
        ));

        let agents = app.session_active_agents.get("s1").unwrap();
        let read = agents[0].tools.iter().find(|t| t.name == "Read").unwrap();
        assert_eq!(read.count, 2);
        assert_eq!(read.failure_count, 0);
        // Should NOT be in orchestrator tools
        assert!(app.session_orchestrator_tools.get("s1").is_none());
    }

    #[test]
    fn push_event_tracks_tool_failure() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUseFailure","session_id":"s1","tool_name":"Bash"}"#,
        ));

        let tools = app.session_orchestrator_tools.get("s1").unwrap();
        let bash = tools.iter().find(|t| t.name == "Bash").unwrap();
        assert_eq!(bash.count, 1);
        assert_eq!(bash.failure_count, 1);
    }

    #[test]
    fn push_event_tracks_agent_tool_failure_with_agent_context() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUseFailure","session_id":"s1","tool_name":"Bash","agent_context_type":"tdd"}"#,
        ));

        let agents = app.session_active_agents.get("s1").unwrap();
        let bash = agents[0].tools.iter().find(|t| t.name == "Bash").unwrap();
        assert_eq!(bash.count, 1);
        assert_eq!(bash.failure_count, 1);
        // Should NOT be in orchestrator tools
        assert!(app.session_orchestrator_tools.get("s1").is_none());
    }

    #[test]
    fn push_event_rules_fallback_to_orchestrator_when_agent_not_found() {
        let mut app = App::new(ConfigInventory::default());
        // Load a rule with agent_context_type but no matching active agent
        app.push_event(make_test_event(
            r#"{"hook_event_name":"InstructionsLoaded","session_id":"s1","file_path":"/rules/workflow.md","agent_context_type":"missing-agent"}"#,
        ));

        // Should fall back to orchestrator rules
        let rules = app.session_orchestrator_rules.get("s1").unwrap();
        assert!(rules.contains("workflow.md"));
        // No agents should exist
        assert!(app.session_active_agents.get("s1").is_none());
    }

    #[test]
    fn push_event_tracks_task_created() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"TaskCreated","session_id":"s1","task_id":"T1","teammate_name":"agent-a"}"#,
        ));

        let tasks = app.session_tasks.get("s1").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, "T1");
        assert_eq!(tasks[0].teammate_name, Some("agent-a".to_string()));
        assert!(!tasks[0].completed);
    }

    #[test]
    fn push_event_tracks_task_completed() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"TaskCreated","session_id":"s1","task_id":"T1"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"TaskCompleted","session_id":"s1","task_id":"T1"}"#,
        ));

        let tasks = app.session_tasks.get("s1").unwrap();
        assert!(tasks[0].completed);
    }

    #[test]
    fn push_event_orphan_stop_does_not_panic() {
        let mut app = App::new(ConfigInventory::default());
        // SubagentStop without matching Start should not crash
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStop","session_id":"s1","agent_type":"unknown"}"#,
        ));
        // No agents should exist
        let agents = app.session_active_agents.get("s1");
        assert!(agents.is_none() || agents.unwrap().is_empty());
    }

    #[test]
    fn find_active_agent_mut_returns_last_match() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd","cwd":"/a"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd","cwd":"/b"}"#,
        ));

        let agent = app.find_active_agent_mut("s1", "tdd").unwrap();
        assert_eq!(agent.cwd, Some("/b".to_string()));
    }

    #[test]
    fn get_or_create_tool_creates_new() {
        let mut tools = Vec::new();
        let tr = App::get_or_create_tool(&mut tools, "Read");
        tr.count = 5;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "Read");
        assert_eq!(tools[0].count, 5);
    }

    #[test]
    fn get_or_create_tool_returns_existing() {
        let mut tools = vec![ToolRecord {
            name: "Read".to_string(),
            count: 3,
            failure_count: 0,
        }];
        let tr = App::get_or_create_tool(&mut tools, "Read");
        tr.count += 1;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].count, 4);
    }

    #[test]
    fn push_event_session_end_removes_from_active() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("PreToolUse", "s1"));
        assert!(app.active_session_ids.contains("s1"));

        app.push_event(make_event_with_session("SessionEnd", "s1"));
        assert!(!app.active_session_ids.contains("s1"));
    }

    // --- filtered_events tests ---

    #[test]
    fn filtered_events_returns_all_when_no_filter() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("PreToolUse", "a"));
        app.push_event(make_event_with_session("PreToolUse", "b"));

        let filtered = app.filtered_events();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filtered_events_returns_only_matching_session() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("PreToolUse", "a"));
        app.push_event(make_event_with_session("PreToolUse", "b"));
        app.push_event(make_event_with_session("PostToolUse", "a"));

        app.events_session_filter = Some("a".to_string());
        let filtered = app.filtered_events();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|e| e.session_id == "a"));
    }

    // --- visible_sessions tests ---

    fn make_session_with_id(id: &str, phase: &str) -> SessionState {
        let mut s: SessionState =
            serde_json::from_str(&format!(r#"{{"phase":"{}"}}"#, phase)).unwrap();
        s.session_id = id.to_string();
        s
    }

    #[test]
    fn visible_sessions_returns_all_sessions() {
        let mut app = App::new(ConfigInventory::default());

        let s1 = make_session_with_id("sess-a", "idle");
        let s2 = make_session_with_id("sess-b", "planning");
        app.update_sessions(vec![s1, s2]);

        // Only sess-a has sent events, but visible_sessions returns all
        app.active_session_ids.insert("sess-a".to_string());

        let visible = app.visible_sessions();
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn visible_sessions_returns_sessions_even_without_events() {
        let mut app = App::new(ConfigInventory::default());
        let s1 = make_session_with_id("sess-a", "idle");
        app.update_sessions(vec![s1]);

        let visible = app.visible_sessions();
        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn is_session_active_returns_true_for_active() {
        let mut app = App::new(ConfigInventory::default());
        app.active_session_ids.insert("sess-a".to_string());
        assert!(app.is_session_active("sess-a"));
    }

    #[test]
    fn is_session_active_returns_false_for_inactive() {
        let app = App::new(ConfigInventory::default());
        assert!(!app.is_session_active("sess-a"));
    }

    // --- cycle_session_filter tests ---

    #[test]
    fn cycle_session_filter_none_to_first() {
        let mut app = App::new(ConfigInventory::default());
        app.active_session_ids.insert("alpha".to_string());
        app.active_session_ids.insert("beta".to_string());

        assert!(app.events_session_filter.is_none());
        app.cycle_session_filter();
        assert_eq!(app.events_session_filter, Some("alpha".to_string()));
    }

    #[test]
    fn cycle_session_filter_advances_to_next() {
        let mut app = App::new(ConfigInventory::default());
        app.active_session_ids.insert("alpha".to_string());
        app.active_session_ids.insert("beta".to_string());

        app.events_session_filter = Some("alpha".to_string());
        app.cycle_session_filter();
        assert_eq!(app.events_session_filter, Some("beta".to_string()));
    }

    #[test]
    fn cycle_session_filter_wraps_to_none() {
        let mut app = App::new(ConfigInventory::default());
        app.active_session_ids.insert("alpha".to_string());
        app.active_session_ids.insert("beta".to_string());

        app.events_session_filter = Some("beta".to_string());
        app.cycle_session_filter();
        assert!(app.events_session_filter.is_none());
    }

    #[test]
    fn cycle_session_filter_no_active_sessions_stays_none() {
        let mut app = App::new(ConfigInventory::default());
        app.cycle_session_filter();
        assert!(app.events_session_filter.is_none());
    }

    #[test]
    fn cycle_session_filter_stale_filter_resets_to_none() {
        let mut app = App::new(ConfigInventory::default());
        app.active_session_ids.insert("alpha".to_string());
        app.events_session_filter = Some("removed-session".to_string());

        app.cycle_session_filter();
        assert!(app.events_session_filter.is_none());
    }

    // --- 'f' key handler test ---

    #[test]
    fn f_key_cycles_filter_on_events_tab() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.active_session_ids.insert("sess-1".to_string());

        app.on_key(make_key(KeyCode::Char('f')));
        assert_eq!(app.events_session_filter, Some("sess-1".to_string()));

        app.on_key(make_key(KeyCode::Char('f')));
        assert!(app.events_session_filter.is_none());
    }

    #[test]
    fn f_key_does_nothing_on_other_tabs() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Sessions;
        app.active_session_ids.insert("sess-1".to_string());

        app.on_key(make_key(KeyCode::Char('f')));
        assert!(app.events_session_filter.is_none());
    }

    // --- Focus and per-tab scroll tests ---

    #[test]
    fn enter_transitions_session_list_to_detail() {
        let mut app = App::new(ConfigInventory::default());
        assert_eq!(app.session_focus, ListDetailFocus::List);

        app.on_key(make_key(KeyCode::Enter));
        assert_eq!(app.session_focus, ListDetailFocus::Detail);
    }

    #[test]
    fn esc_transitions_session_detail_to_list() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = ListDetailFocus::Detail;
        app.session_detail_scroll = 5;

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.session_focus, ListDetailFocus::List);
        assert_eq!(app.session_detail_scroll, 0);
    }

    #[test]
    fn esc_noop_at_session_list() {
        let mut app = App::new(ConfigInventory::default());
        assert_eq!(app.session_focus, ListDetailFocus::List);

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.session_focus, ListDetailFocus::List);
        assert!(!app.should_quit);
    }

    #[test]
    fn enter_transitions_event_list_to_detail() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        assert_eq!(app.event_focus, ListDetailFocus::List);

        app.on_key(make_key(KeyCode::Enter));
        assert_eq!(app.event_focus, ListDetailFocus::Detail);
    }

    #[test]
    fn esc_transitions_event_detail_to_list() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_focus = ListDetailFocus::Detail;
        app.event_detail_scroll = 5;

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.event_focus, ListDetailFocus::List);
        assert_eq!(app.event_detail_scroll, 0);
    }

    #[test]
    fn esc_noop_at_event_list() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        assert_eq!(app.event_focus, ListDetailFocus::List);

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.event_focus, ListDetailFocus::List);
        assert!(!app.should_quit);
    }

    #[test]
    fn esc_on_config_detail_goes_to_item() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Detail;
        app.config_detail_scroll = 3;

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.config_focus, ConfigFocus::Item);
        assert_eq!(app.config_detail_scroll, 0);
    }

    #[test]
    fn esc_on_config_item_goes_to_category() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Item;

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.config_focus, ConfigFocus::Category);
    }

    #[test]
    fn esc_on_config_category_is_noop() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        assert_eq!(app.config_focus, ConfigFocus::Category);

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.config_focus, ConfigFocus::Category);
        assert!(!app.should_quit);
    }

    #[test]
    fn tab_switch_resets_session_focus_and_scroll() {
        let mut app = App::new(ConfigInventory::default());
        // Start on Sessions, move to Detail
        app.session_focus = ListDetailFocus::Detail;
        app.session_detail_scroll = 10;

        // Tab to Config, then back through Events to Sessions
        app.on_key(make_key(KeyCode::Tab)); // -> Config
        app.on_key(make_key(KeyCode::Tab)); // -> Events
        app.on_key(make_key(KeyCode::Tab)); // -> Sessions
        assert_eq!(app.session_focus, ListDetailFocus::List);
        assert_eq!(app.session_detail_scroll, 0);
    }

    #[test]
    fn tab_switch_resets_event_focus_and_scroll() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config; // start on Config
        app.event_focus = ListDetailFocus::Detail;
        app.event_detail_scroll = 10;

        app.on_key(make_key(KeyCode::Tab)); // -> Events
        assert_eq!(app.event_focus, ListDetailFocus::List);
        assert_eq!(app.event_detail_scroll, 0);
    }

    #[test]
    fn tab_switch_resets_config_scroll() {
        let mut app = App::new(ConfigInventory::default());
        // Start on Sessions
        app.config_detail_scroll = 10;

        app.on_key(make_key(KeyCode::Tab)); // -> Config
        assert_eq!(app.config_detail_scroll, 0);
    }

    #[test]
    fn page_down_increments_session_scroll() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = ListDetailFocus::Detail;
        assert_eq!(app.session_detail_scroll, 0);

        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(app.session_detail_scroll, 5);

        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(app.session_detail_scroll, 10);
    }

    #[test]
    fn page_up_decrements_session_scroll_with_saturation() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = ListDetailFocus::Detail;
        app.session_detail_scroll = 3;

        app.on_key(make_key(KeyCode::PageUp));
        assert_eq!(app.session_detail_scroll, 0);
    }

    #[test]
    fn page_down_increments_event_scroll() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_focus = ListDetailFocus::Detail;

        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(app.event_detail_scroll, 5);
    }

    #[test]
    fn page_up_decrements_event_scroll() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_focus = ListDetailFocus::Detail;
        app.event_detail_scroll = 10;

        app.on_key(make_key(KeyCode::PageUp));
        assert_eq!(app.event_detail_scroll, 5);
    }

    #[test]
    fn page_down_increments_config_scroll() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Detail;

        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(app.config_detail_scroll, 5);
    }

    #[test]
    fn page_up_decrements_config_scroll() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Detail;
        app.config_detail_scroll = 10;

        app.on_key(make_key(KeyCode::PageUp));
        assert_eq!(app.config_detail_scroll, 5);
    }

    #[test]
    fn session_navigate_up_in_list_resets_scroll() {
        let mut app = App::new(ConfigInventory::default());
        let s1: SessionState = serde_json::from_str(r#"{"phase":"idle"}"#).unwrap();
        let s2: SessionState = serde_json::from_str(r#"{"phase":"planning"}"#).unwrap();
        app.update_sessions(vec![s1, s2]);
        app.session_selected = 1;
        app.session_detail_scroll = 10;

        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.session_detail_scroll, 0);
    }

    #[test]
    fn session_navigate_down_in_detail_scrolls() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = ListDetailFocus::Detail;
        app.session_detail_scroll = 0;

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.session_detail_scroll, 1);
    }

    #[test]
    fn session_navigate_up_in_detail_scrolls() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = ListDetailFocus::Detail;
        app.session_detail_scroll = 3;

        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.session_detail_scroll, 2);
    }

    #[test]
    fn event_navigate_down_in_detail_scrolls() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_focus = ListDetailFocus::Detail;
        app.event_detail_scroll = 0;

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.event_detail_scroll, 1);
    }

    #[test]
    fn event_navigate_up_in_list_resets_scroll() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.push_event(event_from_name("PreToolUse"));
        app.push_event(event_from_name("PostToolUse"));
        app.event_auto_scroll = false;
        app.event_selected = 1;
        app.event_detail_scroll = 10;

        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.event_detail_scroll, 0);
    }

    #[test]
    fn config_detail_focus_down_scrolls() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Detail;
        app.config_detail_scroll = 0;

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.config_detail_scroll, 1);
    }

    #[test]
    fn config_detail_focus_up_scrolls() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Detail;
        app.config_detail_scroll = 3;

        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.config_detail_scroll, 2);
    }

    #[test]
    fn config_category_change_resets_scroll() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Category;
        app.config_detail_scroll = 10;

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.config_detail_scroll, 0);
    }

    #[test]
    fn config_item_change_resets_scroll() {
        let mut app = App::new(crate::config::ConfigInventory {
            agents: vec![
                crate::config::AgentConfig {
                    name: "a".into(),
                    description: "".into(),
                    model: "".into(),
                    disallowed_tools: vec![],
                    file_path: "".into(),
                },
                crate::config::AgentConfig {
                    name: "b".into(),
                    description: "".into(),
                    model: "".into(),
                    disallowed_tools: vec![],
                    file_path: "".into(),
                },
            ],
            ..Default::default()
        });
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Item;
        app.config_detail_scroll = 10;

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.config_detail_scroll, 0);
    }

    #[test]
    fn right_transitions_session_list_to_detail() {
        let mut app = App::new(ConfigInventory::default());
        assert_eq!(app.session_focus, ListDetailFocus::List);

        app.on_key(make_key(KeyCode::Right));
        assert_eq!(app.session_focus, ListDetailFocus::Detail);
    }

    #[test]
    fn left_transitions_session_detail_to_list() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = ListDetailFocus::Detail;
        app.session_detail_scroll = 5;

        app.on_key(make_key(KeyCode::Left));
        assert_eq!(app.session_focus, ListDetailFocus::List);
        assert_eq!(app.session_detail_scroll, 0);
    }

    #[test]
    fn right_transitions_event_list_to_detail() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        assert_eq!(app.event_focus, ListDetailFocus::List);

        app.on_key(make_key(KeyCode::Right));
        assert_eq!(app.event_focus, ListDetailFocus::Detail);
    }

    #[test]
    fn left_transitions_event_detail_to_list() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_focus = ListDetailFocus::Detail;
        app.event_detail_scroll = 5;

        app.on_key(make_key(KeyCode::Left));
        assert_eq!(app.event_focus, ListDetailFocus::List);
        assert_eq!(app.event_detail_scroll, 0);
    }

    #[test]
    fn config_left_resets_scroll_from_detail() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = ConfigFocus::Detail;
        app.config_detail_scroll = 5;

        app.on_key(make_key(KeyCode::Left));
        assert_eq!(app.config_focus, ConfigFocus::Item);
        assert_eq!(app.config_detail_scroll, 0);
    }

    // --- Gap verification tests (F2-T5) ---

    #[test]
    fn tab_switch_resets_config_focus_to_category() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Sessions; // start on Sessions
        app.config_focus = ConfigFocus::Detail;

        app.on_key(make_key(KeyCode::Tab)); // -> Config
        assert_eq!(
            app.config_focus,
            ConfigFocus::Category,
            "Tab switch to Config must reset focus to Category"
        );
    }

    #[test]
    fn page_down_only_works_in_detail_focus_sessions() {
        let mut app = App::new(ConfigInventory::default());
        // Sessions tab, List focus
        assert_eq!(app.session_focus, ListDetailFocus::List);
        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(
            app.session_detail_scroll, 0,
            "PageDown should be no-op when Sessions is in List focus"
        );
    }

    #[test]
    fn page_up_only_works_in_detail_focus_sessions() {
        let mut app = App::new(ConfigInventory::default());
        app.session_detail_scroll = 5;
        assert_eq!(app.session_focus, ListDetailFocus::List);
        app.on_key(make_key(KeyCode::PageUp));
        assert_eq!(
            app.session_detail_scroll, 5,
            "PageUp should be no-op when Sessions is in List focus"
        );
    }

    #[test]
    fn page_down_only_works_in_detail_focus_events() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        assert_eq!(app.event_focus, ListDetailFocus::List);
        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(
            app.event_detail_scroll, 0,
            "PageDown should be no-op when Events is in List focus"
        );
    }

    #[test]
    fn page_up_only_works_in_detail_focus_events() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_detail_scroll = 5;
        assert_eq!(app.event_focus, ListDetailFocus::List);
        app.on_key(make_key(KeyCode::PageUp));
        assert_eq!(
            app.event_detail_scroll, 5,
            "PageUp should be no-op when Events is in List focus"
        );
    }

    #[test]
    fn page_down_only_works_in_detail_focus_config() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        assert_eq!(app.config_focus, ConfigFocus::Category);
        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(
            app.config_detail_scroll, 0,
            "PageDown should be no-op when Config is in Category focus"
        );
    }

    #[test]
    fn page_up_only_works_in_detail_focus_config() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_detail_scroll = 5;
        assert_eq!(app.config_focus, ConfigFocus::Category);
        app.on_key(make_key(KeyCode::PageUp));
        assert_eq!(
            app.config_detail_scroll, 5,
            "PageUp should be no-op when Config is in Category focus"
        );
    }

    #[test]
    fn f_key_disabled_during_events_detail_focus() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_focus = ListDetailFocus::Detail;
        app.active_session_ids.insert("sess-1".to_string());

        app.on_key(make_key(KeyCode::Char('f')));
        assert!(
            app.events_session_filter.is_none(),
            "f key must be disabled when Events tab is in Detail focus"
        );
    }

    #[test]
    fn enter_noop_at_session_detail() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = ListDetailFocus::Detail;
        app.on_key(make_key(KeyCode::Enter));
        assert_eq!(app.session_focus, ListDetailFocus::Detail);
    }

    #[test]
    fn enter_noop_at_event_detail() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_focus = ListDetailFocus::Detail;
        app.on_key(make_key(KeyCode::Enter));
        assert_eq!(app.event_focus, ListDetailFocus::Detail);
    }

    #[test]
    fn cycle_session_filter_resets_event_state() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.active_session_ids.insert("sess-1".to_string());
        app.event_selected = 5;
        app.event_detail_scroll = 10;
        app.event_focus = ListDetailFocus::Detail;

        app.cycle_session_filter();
        assert_eq!(app.event_selected, 0, "Filter change must reset event_selected");
        assert_eq!(app.event_detail_scroll, 0, "Filter change must reset scroll");
        assert_eq!(app.event_focus, ListDetailFocus::List, "Filter change must reset focus");
    }
}
