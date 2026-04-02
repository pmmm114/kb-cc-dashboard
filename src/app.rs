use crate::config::ConfigInventory;
use crate::event::{EventKind, HookEvent};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet, VecDeque};

const MAX_EVENTS: usize = 500;

#[derive(Clone, Debug)]
pub struct ToolRecord {
    pub name: String,
    pub count: usize,
    pub failure_count: usize,
}

#[derive(Clone, Debug)]
pub struct TaskInfo {
    pub task_id: String,
    pub teammate_name: Option<String>,
    pub completed: bool,
}

pub type AgentId = u64;

#[derive(Clone, Debug)]
pub struct AgentRecord {
    pub id: AgentId,
    pub agent_type: String,
    pub cwd: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub context: AgentContext,
    pub tools: Vec<ToolRecord>,
}

impl AgentRecord {
    pub fn is_active(&self) -> bool {
        self.ended_at.is_none()
    }
}

#[derive(Clone, Debug, Default)]
pub struct AgentContext {
    pub agent_definitions: Vec<String>,
    pub skills: Vec<String>,
    pub rules: Vec<String>,
    pub memory: Vec<String>,
    pub other: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct PromptSegment {
    pub prompt_text: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub agents: Vec<AgentId>,
    pub orchestrator_tools: Vec<ToolRecord>,
    pub tasks: Vec<TaskInfo>,
}

#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub session_id: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_event_at: DateTime<Utc>,
    pub ended: bool,
    pub agent_records: Vec<AgentRecord>,
    pub prompt_segments: Vec<PromptSegment>,
    pub next_agent_id: AgentId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionFocus {
    List,
    Segment,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionCategory {
    AgentDefinition,
    Skill,
    Rule,
    Memory,
    Other,
}

pub fn classify_instruction_path(path: &str) -> InstructionCategory {
    if path.contains("/agents/") && path.ends_with(".md") {
        InstructionCategory::AgentDefinition
    } else if path.contains("/skills/") && path.contains("SKILL.md") {
        InstructionCategory::Skill
    } else if path.contains("/rules/") && path.ends_with(".md") {
        InstructionCategory::Rule
    } else if path.ends_with("CLAUDE.md") {
        InstructionCategory::Memory
    } else {
        InstructionCategory::Other
    }
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
    pub session_focus: SessionFocus,
    pub event_focus: ListDetailFocus,

    // Per-tab detail panel scroll offsets
    pub config_detail_scroll: usize,
    pub session_detail_scroll: usize,
    pub event_detail_scroll: usize,

    // Per-session event indexing
    pub session_events: HashMap<String, VecDeque<HookEvent>>,
    pub active_session_ids: HashSet<String>,
    pub events_session_filter: Option<String>,

    // Event-sourced session data
    pub session_records: HashMap<String, SessionRecord>,
    pub session_segment_selected: usize,
}

impl App {
    pub fn new(config: ConfigInventory) -> Self {
        Self {
            active_tab: Tab::Sessions,
            config,
            events: VecDeque::new(),
            session_selected: 0,
            config_category: ConfigCategory::Agents,
            config_item_selected: 0,
            config_focus: ConfigFocus::Category,
            event_selected: 0,
            event_auto_scroll: true,
            should_quit: false,
            session_focus: SessionFocus::List,
            event_focus: ListDetailFocus::List,
            config_detail_scroll: 0,
            session_detail_scroll: 0,
            event_detail_scroll: 0,
            session_events: HashMap::new(),
            active_session_ids: HashSet::new(),
            events_session_filter: None,
            session_records: HashMap::new(),
            session_segment_selected: 0,
        }
    }

    pub fn push_event(&mut self, event: HookEvent) {
        let sid = event.session_id.clone();
        let now = event.received_at;

        // --- Old aggregation (kept temporarily for T3 cleanup) ---

        // Track active session (old)
        self.active_session_ids.insert(sid.clone());

        // Index event per session (old)
        let session_queue = self.session_events.entry(sid.clone()).or_default();
        session_queue.push_back(event.clone());
        if session_queue.len() > MAX_EVENTS {
            session_queue.pop_front();
        }

        // --- Event-sourced session_records ---

        // BUG-2 fix: skip re-activation if session already ended (except SessionStart)
        let already_ended = self
            .session_records
            .get(&sid)
            .map(|r| r.ended)
            .unwrap_or(false);
        if already_ended && event.kind() != EventKind::SessionStart {
            // Still add to flat event queue below, but do not modify session_records
        } else {
            // Get or create the session record (creates segment zero)
            self.get_or_create_session(&sid, now);

            match event.kind() {
                EventKind::UserPromptSubmit => {
                    let prompt = event
                        .payload
                        .get("prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    // Close previous segment
                    if let Some(seg) = self.current_segment_mut(&sid) {
                        if seg.ended_at.is_none() {
                            seg.ended_at = Some(now);
                        }
                    }

                    // Create new segment
                    let new_seg = PromptSegment {
                        prompt_text: prompt,
                        started_at: now,
                        ended_at: None,
                        agents: Vec::new(),
                        orchestrator_tools: Vec::new(),
                        tasks: Vec::new(),
                    };
                    if let Some(record) = self.session_records.get_mut(&sid) {
                        record.prompt_segments.push(new_seg);
                    }
                }
                EventKind::SubagentStart => {
                    if let Some(agent_type) =
                        event.payload.get("agent_type").and_then(|v| v.as_str())
                    {
                        let cwd = event
                            .payload
                            .get("cwd")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        if let Some(record) = self.session_records.get_mut(&sid) {
                            let agent_id = record.next_agent_id;
                            record.next_agent_id += 1;
                            let agent = AgentRecord {
                                id: agent_id,
                                agent_type: agent_type.to_string(),
                                cwd,
                                started_at: now,
                                ended_at: None,
                                context: AgentContext::default(),
                                tools: Vec::new(),
                            };
                            record.agent_records.push(agent);
                            // Add AgentId to current segment
                            if let Some(seg) = record.prompt_segments.last_mut() {
                                seg.agents.push(agent_id);
                            }
                        }
                    }
                }
                EventKind::InstructionsLoaded => {
                    if let Some(file_path) =
                        event.payload.get("file_path").and_then(|v| v.as_str())
                    {
                        let category = classify_instruction_path(file_path);
                        let display_name = std::path::Path::new(file_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(file_path)
                            .to_string();
                        let agent_ctx = event
                            .payload
                            .get("agent_context_type")
                            .and_then(|v| v.as_str());
                        let agent_cwd = event
                            .payload
                            .get("cwd")
                            .and_then(|v| v.as_str());

                        let agent_id =
                            self.find_agent_for_routing(&sid, agent_ctx, agent_cwd);

                        if let Some(aid) = agent_id {
                            if let Some(agent) = self.agent_record_mut(&sid, aid) {
                                match category {
                                    InstructionCategory::AgentDefinition => {
                                        agent.context.agent_definitions.push(display_name);
                                    }
                                    InstructionCategory::Skill => {
                                        agent.context.skills.push(display_name);
                                    }
                                    InstructionCategory::Rule => {
                                        agent.context.rules.push(display_name);
                                    }
                                    InstructionCategory::Memory => {
                                        agent.context.memory.push(display_name);
                                    }
                                    InstructionCategory::Other => {
                                        agent.context.other.push(display_name);
                                    }
                                }
                            }
                        }
                        // If no matching agent, the instruction goes to
                        // the orchestrator context — which is segment-level.
                        // (Handled by the old aggregation for now; T5 will
                        // add orchestrator context to segments.)
                    }
                }
                EventKind::PostToolUse => {
                    // BUG-1 fix: only PostToolUse increments count (NOT PreToolUse)
                    if let Some(tool_name) =
                        event.payload.get("tool_name").and_then(|v| v.as_str())
                    {
                        let agent_ctx = event
                            .payload
                            .get("agent_context_type")
                            .and_then(|v| v.as_str());
                        let agent_cwd = event
                            .payload
                            .get("cwd")
                            .and_then(|v| v.as_str());
                        let agent_id =
                            self.find_agent_for_routing(&sid, agent_ctx, agent_cwd);

                        if let Some(aid) = agent_id {
                            if let Some(agent) = self.agent_record_mut(&sid, aid) {
                                let tr = Self::get_or_create_tool(&mut agent.tools, tool_name);
                                tr.count += 1;
                            }
                        } else {
                            // Orchestrator tool
                            if let Some(seg) = self.current_segment_mut(&sid) {
                                let tr =
                                    Self::get_or_create_tool(&mut seg.orchestrator_tools, tool_name);
                                tr.count += 1;
                            }
                        }
                    }
                }
                EventKind::PostToolUseFailure => {
                    if let Some(tool_name) =
                        event.payload.get("tool_name").and_then(|v| v.as_str())
                    {
                        let agent_ctx = event
                            .payload
                            .get("agent_context_type")
                            .and_then(|v| v.as_str());
                        let agent_cwd = event
                            .payload
                            .get("cwd")
                            .and_then(|v| v.as_str());
                        let agent_id =
                            self.find_agent_for_routing(&sid, agent_ctx, agent_cwd);

                        if let Some(aid) = agent_id {
                            if let Some(agent) = self.agent_record_mut(&sid, aid) {
                                let tr = Self::get_or_create_tool(&mut agent.tools, tool_name);
                                tr.count += 1;
                                tr.failure_count += 1;
                            }
                        } else {
                            if let Some(seg) = self.current_segment_mut(&sid) {
                                let tr =
                                    Self::get_or_create_tool(&mut seg.orchestrator_tools, tool_name);
                                tr.count += 1;
                                tr.failure_count += 1;
                            }
                        }
                    }
                }
                EventKind::PreToolUse => {
                    // BUG-1 fix: PreToolUse does NOT increment tool count
                    // Event is stored in the flat queue only
                }
                EventKind::SubagentStop => {
                    if let Some(agent_type) =
                        event.payload.get("agent_type").and_then(|v| v.as_str())
                    {
                        let cwd = event
                            .payload
                            .get("cwd")
                            .and_then(|v| v.as_str());
                        // Find matching active agent and set ended_at (preserve data)
                        if let Some(record) = self.session_records.get_mut(&sid) {
                            if let Some(agent) = record
                                .agent_records
                                .iter_mut()
                                .rev()
                                .find(|a| {
                                    a.is_active()
                                        && a.agent_type == agent_type
                                        && a.cwd.as_deref() == cwd
                                })
                            {
                                agent.ended_at = Some(now);
                            }
                        }
                    }
                }
                EventKind::Stop => {
                    // Close current segment
                    if let Some(seg) = self.current_segment_mut(&sid) {
                        if seg.ended_at.is_none() {
                            seg.ended_at = Some(now);
                        }
                    }
                }
                EventKind::SessionEnd | EventKind::StopFailure => {
                    // Close current segment
                    if let Some(seg) = self.current_segment_mut(&sid) {
                        if seg.ended_at.is_none() {
                            seg.ended_at = Some(now);
                        }
                    }
                    // Force-close all active agents
                    if let Some(record) = self.session_records.get_mut(&sid) {
                        for agent in &mut record.agent_records {
                            if agent.ended_at.is_none() {
                                agent.ended_at = Some(now);
                            }
                        }
                        record.ended = true;
                    }
                    self.active_session_ids.remove(&sid);
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
                    if let Some(seg) = self.current_segment_mut(&sid) {
                        seg.tasks.push(TaskInfo {
                            task_id,
                            teammate_name,
                            completed: false,
                        });
                    }
                }
                EventKind::TaskCompleted => {
                    if let Some(task_id) = event.payload.get("task_id").and_then(|v| v.as_str()) {
                        if let Some(seg) = self.current_segment_mut(&sid) {
                            if let Some(task) =
                                seg.tasks.iter_mut().rfind(|t| t.task_id == task_id)
                            {
                                task.completed = true;
                            }
                        }
                    }
                }
                EventKind::SessionStart => {
                    // Session already created by get_or_create_session above.
                    // SessionStart can re-open an ended session (handled by the
                    // already_ended check above allowing SessionStart through).
                }
                _ => {}
            }
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

    pub fn on_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => self.on_esc(),
            KeyCode::Tab => {
                self.active_tab = self.active_tab.next();
                // Reset target tab's focus and scroll — except Sessions (preserves drill-down)
                match self.active_tab {
                    Tab::Sessions => {
                        // Tab switch does NOT reset session_focus (design requirement)
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
                    Tab::Sessions if self.session_focus == SessionFocus::Detail => {
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
                    Tab::Sessions if self.session_focus == SessionFocus::Detail => {
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
                match self.session_focus {
                    SessionFocus::Detail => {
                        self.session_focus = SessionFocus::Segment;
                        self.session_detail_scroll = 0;
                    }
                    SessionFocus::Segment => {
                        self.session_focus = SessionFocus::List;
                        self.session_segment_selected = 0;
                    }
                    SessionFocus::List => {} // No-op at root
                }
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
                SessionFocus::List => {
                    self.session_selected = self.session_selected.saturating_sub(1);
                    self.session_segment_selected = 0;
                    self.session_detail_scroll = 0;
                }
                SessionFocus::Segment => {
                    self.session_segment_selected = self.session_segment_selected.saturating_sub(1);
                    self.session_detail_scroll = 0;
                }
                SessionFocus::Detail => {
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
                SessionFocus::List => {
                    let count = self.visible_session_records().len();
                    if count > 0 {
                        self.session_selected =
                            (self.session_selected + 1).min(count - 1);
                        self.session_segment_selected = 0;
                        self.session_detail_scroll = 0;
                    }
                }
                SessionFocus::Segment => {
                    // Bounds checked against selected session's segment count
                    if let Some(record) = self.selected_session_record() {
                        let seg_count = record.prompt_segments.len();
                        if seg_count > 0 {
                            self.session_segment_selected =
                                (self.session_segment_selected + 1).min(seg_count - 1);
                            self.session_detail_scroll = 0;
                        }
                    }
                }
                SessionFocus::Detail => {
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
                match self.session_focus {
                    SessionFocus::Detail => {
                        self.session_focus = SessionFocus::Segment;
                        self.session_detail_scroll = 0;
                    }
                    SessionFocus::Segment => {
                        self.session_focus = SessionFocus::List;
                    }
                    SessionFocus::List => {}
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
                match self.session_focus {
                    SessionFocus::List => self.session_focus = SessionFocus::Segment,
                    SessionFocus::Segment => self.session_focus = SessionFocus::Detail,
                    SessionFocus::Detail => {}
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
                // Enter acts as Right on Sessions tab (same as Config pattern)
                self.navigate_right();
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

    /// Get or create a SessionRecord for the given session_id.
    /// Creates segment zero if the session is new.
    pub fn get_or_create_session(
        &mut self,
        sid: &str,
        received_at: DateTime<Utc>,
    ) -> &mut SessionRecord {
        if !self.session_records.contains_key(sid) {
            let record = SessionRecord {
                session_id: sid.to_string(),
                first_seen_at: received_at,
                last_event_at: received_at,
                ended: false,
                agent_records: Vec::new(),
                prompt_segments: vec![PromptSegment {
                    prompt_text: "(session initialization)".to_string(),
                    started_at: received_at,
                    ended_at: None,
                    agents: Vec::new(),
                    orchestrator_tools: Vec::new(),
                    tasks: Vec::new(),
                }],
                next_agent_id: 0,
            };
            self.session_records.insert(sid.to_string(), record);
        }
        let record = self.session_records.get_mut(sid).unwrap();
        record.last_event_at = received_at;
        record
    }

    /// Returns the last (current) segment in the session, if any.
    pub fn current_segment_mut(&mut self, sid: &str) -> Option<&mut PromptSegment> {
        self.session_records
            .get_mut(sid)
            .and_then(|r| r.prompt_segments.last_mut())
    }

    /// Find a matching agent for routing events.
    /// Prefers active agents over completed, matches by agent_type then cwd.
    pub fn find_agent_for_routing(
        &self,
        sid: &str,
        agent_context_type: Option<&str>,
        _cwd: Option<&str>,
    ) -> Option<AgentId> {
        let act = agent_context_type?;
        let record = self.session_records.get(sid)?;

        // Prefer active agents, search from newest to oldest
        let active_match = record
            .agent_records
            .iter()
            .rev()
            .find(|a| a.is_active() && a.agent_type == act)
            .map(|a| a.id);
        if active_match.is_some() {
            return active_match;
        }

        // Fallback to completed agents
        record
            .agent_records
            .iter()
            .rev()
            .find(|a| a.agent_type == act)
            .map(|a| a.id)
    }

    /// Get a mutable reference to an AgentRecord by id.
    fn agent_record_mut(&mut self, sid: &str, agent_id: AgentId) -> Option<&mut AgentRecord> {
        self.session_records
            .get_mut(sid)
            .and_then(|r| r.agent_records.iter_mut().find(|a| a.id == agent_id))
    }

    /// Returns session records sorted: active (not ended) first by last_event_at desc,
    /// then ended by last_event_at desc.
    pub fn visible_session_records(&self) -> Vec<&SessionRecord> {
        let mut active: Vec<&SessionRecord> = self
            .session_records
            .values()
            .filter(|r| !r.ended)
            .collect();
        active.sort_by(|a, b| b.last_event_at.cmp(&a.last_event_at));

        let mut ended: Vec<&SessionRecord> = self
            .session_records
            .values()
            .filter(|r| r.ended)
            .collect();
        ended.sort_by(|a, b| b.last_event_at.cmp(&a.last_event_at));

        active.extend(ended);
        active
    }

    /// Returns the currently selected session record (based on visible_session_records order)
    pub fn selected_session_record(&self) -> Option<&SessionRecord> {
        let records = self.visible_session_records();
        records.get(self.session_selected).copied()
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
        assert!(app.events.is_empty());
        assert_eq!(app.session_selected, 0);
        assert_eq!(app.config_category, ConfigCategory::Agents);
        assert_eq!(app.config_focus, ConfigFocus::Category);
        assert!(app.event_auto_scroll);
        assert!(!app.should_quit);
        assert_eq!(app.session_focus, SessionFocus::List);
        assert_eq!(app.event_focus, ListDetailFocus::List);
        assert_eq!(app.config_detail_scroll, 0);
        assert_eq!(app.session_detail_scroll, 0);
        assert_eq!(app.event_detail_scroll, 0);
        // New event-sourced session fields
        assert!(app.session_records.is_empty());
        assert_eq!(app.session_segment_selected, 0);
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
        // Create two session records via events
        app.push_event(make_event_with_session("SessionStart", "sess-a"));
        app.push_event(make_event_with_session("SessionStart", "sess-b"));

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

    // --- Per-session event indexing tests ---

    #[test]
    fn new_app_has_empty_session_tracking_fields() {
        let app = App::new(ConfigInventory::default());
        assert!(app.session_events.is_empty());
        assert!(app.active_session_ids.is_empty());
        assert!(app.events_session_filter.is_none());
        assert!(app.session_records.is_empty());
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
    fn enter_transitions_session_list_to_segment() {
        let mut app = App::new(ConfigInventory::default());
        assert_eq!(app.session_focus, SessionFocus::List);

        app.on_key(make_key(KeyCode::Enter));
        assert_eq!(app.session_focus, SessionFocus::Segment);
    }

    #[test]
    fn esc_transitions_session_detail_to_segment() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Detail;
        app.session_detail_scroll = 5;

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.session_focus, SessionFocus::Segment);
        assert_eq!(app.session_detail_scroll, 0);
    }

    #[test]
    fn esc_noop_at_session_list() {
        let mut app = App::new(ConfigInventory::default());
        assert_eq!(app.session_focus, SessionFocus::List);

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.session_focus, SessionFocus::List);
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
    fn tab_switch_does_not_reset_session_focus() {
        let mut app = App::new(ConfigInventory::default());
        // Start on Sessions, move to Detail
        app.session_focus = SessionFocus::Detail;
        app.session_detail_scroll = 10;

        // Tab to Config, then back through Events to Sessions
        app.on_key(make_key(KeyCode::Tab)); // -> Config
        app.on_key(make_key(KeyCode::Tab)); // -> Events
        app.on_key(make_key(KeyCode::Tab)); // -> Sessions
        assert_eq!(app.session_focus, SessionFocus::Detail, "Tab switch must preserve session focus");
        assert_eq!(app.session_detail_scroll, 10, "Tab switch must preserve session scroll");
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
        app.session_focus = SessionFocus::Detail;
        assert_eq!(app.session_detail_scroll, 0);

        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(app.session_detail_scroll, 5);

        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(app.session_detail_scroll, 10);
    }

    #[test]
    fn page_up_decrements_session_scroll_with_saturation() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Detail;
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
        // Create two sessions via events
        app.push_event(make_event_with_session("SessionStart", "s1"));
        app.push_event(make_event_with_session("SessionStart", "s2"));
        app.session_selected = 1;
        app.session_detail_scroll = 10;

        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.session_detail_scroll, 0);
    }

    #[test]
    fn session_navigate_down_in_detail_scrolls() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Detail;
        app.session_detail_scroll = 0;

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.session_detail_scroll, 1);
    }

    #[test]
    fn session_navigate_up_in_detail_scrolls() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Detail;
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
    fn right_transitions_session_list_to_segment() {
        let mut app = App::new(ConfigInventory::default());
        assert_eq!(app.session_focus, SessionFocus::List);

        app.on_key(make_key(KeyCode::Right));
        assert_eq!(app.session_focus, SessionFocus::Segment);
    }

    #[test]
    fn left_transitions_session_detail_to_segment() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Detail;
        app.session_detail_scroll = 5;

        app.on_key(make_key(KeyCode::Left));
        assert_eq!(app.session_focus, SessionFocus::Segment);
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
        assert_eq!(app.session_focus, SessionFocus::List);
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
        assert_eq!(app.session_focus, SessionFocus::List);
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
        app.session_focus = SessionFocus::Detail;
        app.on_key(make_key(KeyCode::Enter));
        assert_eq!(app.session_focus, SessionFocus::Detail);
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

    // --- T1: New data model type tests ---

    #[test]
    fn agent_record_is_active_when_ended_at_is_none() {
        let record = AgentRecord {
            id: 1,
            agent_type: "planner".to_string(),
            cwd: None,
            started_at: Utc::now(),
            ended_at: None,
            context: AgentContext::default(),
            tools: vec![],
        };
        assert!(record.is_active());
    }

    #[test]
    fn agent_record_is_not_active_when_ended_at_is_some() {
        let record = AgentRecord {
            id: 1,
            agent_type: "planner".to_string(),
            cwd: None,
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            context: AgentContext::default(),
            tools: vec![],
        };
        assert!(!record.is_active());
    }

    #[test]
    fn classify_agents_md_as_agent_definition() {
        assert_eq!(
            classify_instruction_path("/home/user/.claude/agents/planner.md"),
            InstructionCategory::AgentDefinition
        );
    }

    #[test]
    fn classify_skill_md_as_skill() {
        assert_eq!(
            classify_instruction_path("/home/user/.claude/skills/commit-convention/SKILL.md"),
            InstructionCategory::Skill
        );
    }

    #[test]
    fn classify_rules_md_as_rule() {
        assert_eq!(
            classify_instruction_path("/home/user/.claude/rules/code-quality.md"),
            InstructionCategory::Rule
        );
    }

    #[test]
    fn classify_nested_rules_md_as_rule() {
        assert_eq!(
            classify_instruction_path("/home/user/.claude/rules/config/eval-quality.md"),
            InstructionCategory::Rule
        );
    }

    #[test]
    fn classify_claude_md_as_memory() {
        assert_eq!(
            classify_instruction_path("/home/user/.claude/CLAUDE.md"),
            InstructionCategory::Memory
        );
    }

    #[test]
    fn classify_project_claude_md_as_memory() {
        assert_eq!(
            classify_instruction_path("/home/user/project/.claude/CLAUDE.md"),
            InstructionCategory::Memory
        );
    }

    #[test]
    fn classify_unknown_path_as_other() {
        assert_eq!(
            classify_instruction_path("/some/random/file.txt"),
            InstructionCategory::Other
        );
    }

    #[test]
    fn session_focus_equality() {
        assert_eq!(SessionFocus::List, SessionFocus::List);
        assert_eq!(SessionFocus::Segment, SessionFocus::Segment);
        assert_eq!(SessionFocus::Detail, SessionFocus::Detail);
        assert_ne!(SessionFocus::List, SessionFocus::Segment);
        assert_ne!(SessionFocus::Segment, SessionFocus::Detail);
    }

    #[test]
    fn session_focus_is_copy() {
        let a = SessionFocus::List;
        let b = a; // Copy
        assert_eq!(a, b);
    }

    #[test]
    fn agent_context_default_is_empty() {
        let ctx = AgentContext::default();
        assert!(ctx.agent_definitions.is_empty());
        assert!(ctx.skills.is_empty());
        assert!(ctx.rules.is_empty());
        assert!(ctx.memory.is_empty());
        assert!(ctx.other.is_empty());
    }

    // --- New event-sourced session_records tests (T2) ---

    #[test]
    fn push_event_creates_session_record_on_first_event() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("PreToolUse", "s1"));

        assert!(app.session_records.contains_key("s1"));
        let record = app.session_records.get("s1").unwrap();
        assert_eq!(record.session_id, "s1");
        assert!(!record.ended);
    }

    #[test]
    fn push_event_creates_segment_zero() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("SessionStart", "s1"));

        let record = app.session_records.get("s1").unwrap();
        assert_eq!(record.prompt_segments.len(), 1);
        assert_eq!(record.prompt_segments[0].prompt_text, "(session initialization)");
        assert!(record.prompt_segments[0].ended_at.is_none());
    }

    #[test]
    fn push_event_user_prompt_creates_new_segment() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("SessionStart", "s1"));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"hello"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        assert_eq!(record.prompt_segments.len(), 2);
        assert_eq!(record.prompt_segments[1].prompt_text, "hello");
        assert!(record.prompt_segments[1].ended_at.is_none());
    }

    #[test]
    fn push_event_user_prompt_closes_previous_segment() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("SessionStart", "s1"));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"first"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"second"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        assert_eq!(record.prompt_segments.len(), 3);
        // Segment zero and first prompt should be closed
        assert!(record.prompt_segments[0].ended_at.is_some());
        assert!(record.prompt_segments[1].ended_at.is_some());
        // Current segment should be open
        assert!(record.prompt_segments[2].ended_at.is_none());
    }

    #[test]
    fn push_event_subagent_start_creates_agent_record() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("SessionStart", "s1"));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner","cwd":"/tmp/work"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        assert_eq!(record.agent_records.len(), 1);
        assert_eq!(record.agent_records[0].agent_type, "planner");
        assert_eq!(record.agent_records[0].cwd, Some("/tmp/work".to_string()));
        assert!(record.agent_records[0].is_active());
        assert_eq!(record.agent_records[0].id, 0);
        assert_eq!(record.next_agent_id, 1);
        // Agent should be linked to current segment
        assert_eq!(record.prompt_segments[0].agents, vec![0]);
    }

    #[test]
    fn push_event_subagent_stop_preserves_agent_data() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        ));
        // Add a tool use to the agent
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read","agent_context_type":"planner"}"#,
        ));
        // Now stop the agent
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStop","session_id":"s1","agent_type":"planner"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        assert_eq!(record.agent_records.len(), 1);
        assert!(!record.agent_records[0].is_active());
        assert!(record.agent_records[0].ended_at.is_some());
        // Tools should be preserved
        assert_eq!(record.agent_records[0].tools.len(), 1);
        assert_eq!(record.agent_records[0].tools[0].name, "Read");
        assert_eq!(record.agent_records[0].tools[0].count, 1);
    }

    #[test]
    fn push_event_post_tool_use_increments_count() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Edit","agent_context_type":"tdd"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Edit","agent_context_type":"tdd"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        let edit = record.agent_records[0]
            .tools
            .iter()
            .find(|t| t.name == "Edit")
            .unwrap();
        assert_eq!(edit.count, 2);
        assert_eq!(edit.failure_count, 0);
    }

    #[test]
    fn push_event_pre_tool_use_does_not_increment() {
        // BUG-1 regression test
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PreToolUse","session_id":"s1","tool_name":"Read","agent_context_type":"tdd"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        // PreToolUse should NOT create any tool record in the new model
        assert!(
            record.agent_records[0].tools.is_empty(),
            "PreToolUse must not increment tool count (BUG-1 fix)"
        );
    }

    #[test]
    fn push_event_session_end_does_not_reactivate() {
        // BUG-2 regression test
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("SessionStart", "s1"));
        app.push_event(make_event_with_session("SessionEnd", "s1"));

        // Verify ended
        assert!(app.session_records.get("s1").unwrap().ended);

        // Now send another event — should NOT re-activate
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read"}"#,
        ));

        assert!(
            app.session_records.get("s1").unwrap().ended,
            "Session must remain ended after receiving events (BUG-2 fix)"
        );
    }

    #[test]
    fn push_event_session_end_force_closes_agents() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd"}"#,
        ));
        app.push_event(make_event_with_session("SessionEnd", "s1"));

        let record = app.session_records.get("s1").unwrap();
        assert!(record.ended);
        for agent in &record.agent_records {
            assert!(
                agent.ended_at.is_some(),
                "Agent {} should be force-closed on SessionEnd",
                agent.agent_type
            );
        }
    }

    #[test]
    fn push_event_instructions_loaded_classifies_to_agent_context() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        ));
        // Load various instruction types with agent context
        app.push_event(make_test_event(
            r#"{"hook_event_name":"InstructionsLoaded","session_id":"s1","file_path":"/home/.claude/agents/planner.md","agent_context_type":"planner"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"InstructionsLoaded","session_id":"s1","file_path":"/home/.claude/skills/commit-convention/SKILL.md","agent_context_type":"planner"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"InstructionsLoaded","session_id":"s1","file_path":"/home/.claude/rules/workflow.md","agent_context_type":"planner"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"InstructionsLoaded","session_id":"s1","file_path":"/home/.claude/CLAUDE.md","agent_context_type":"planner"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        let ctx = &record.agent_records[0].context;
        assert_eq!(ctx.agent_definitions, vec!["planner.md"]);
        assert_eq!(ctx.skills, vec!["SKILL.md"]);
        assert_eq!(ctx.rules, vec!["workflow.md"]);
        assert_eq!(ctx.memory, vec!["CLAUDE.md"]);
    }

    #[test]
    fn push_event_task_created_in_current_segment() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"do something"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"TaskCreated","session_id":"s1","task_id":"T1","teammate_name":"agent-a"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        let last_seg = record.prompt_segments.last().unwrap();
        assert_eq!(last_seg.tasks.len(), 1);
        assert_eq!(last_seg.tasks[0].task_id, "T1");
        assert!(!last_seg.tasks[0].completed);
    }

    #[test]
    fn push_event_segment_zero_absorbs_pre_prompt_events() {
        let mut app = App::new(ConfigInventory::default());
        // Send InstructionsLoaded and tool events before any UserPromptSubmit
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SessionStart","session_id":"s1"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        // Only segment zero should exist
        assert_eq!(record.prompt_segments.len(), 1);
        assert_eq!(record.prompt_segments[0].prompt_text, "(session initialization)");
        // The orchestrator tool should be in segment zero
        assert_eq!(record.prompt_segments[0].orchestrator_tools.len(), 1);
        assert_eq!(record.prompt_segments[0].orchestrator_tools[0].name, "Read");
    }

    #[test]
    fn push_event_session_end_events_do_not_create_new_segments() {
        // BUG-2 extended: events after SessionEnd should not create segments or agents
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("SessionStart", "s1"));
        app.push_event(make_event_with_session("SessionEnd", "s1"));

        let seg_count_before = app.session_records.get("s1").unwrap().prompt_segments.len();
        let agent_count_before = app.session_records.get("s1").unwrap().agent_records.len();

        // These events should be ignored on an ended session
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"late"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        assert!(record.ended, "Session must remain ended");
        assert_eq!(
            record.prompt_segments.len(),
            seg_count_before,
            "No new segments should be created after SessionEnd"
        );
        assert_eq!(
            record.agent_records.len(),
            agent_count_before,
            "No new agents should be created after SessionEnd"
        );
    }

    // --- SessionFocus 3-level navigation tests ---

    #[test]
    fn session_focus_navigation_list_to_segment() {
        let mut app = App::new(ConfigInventory::default());
        assert_eq!(app.session_focus, SessionFocus::List);

        app.on_key(make_key(KeyCode::Right));
        assert_eq!(app.session_focus, SessionFocus::Segment);
    }

    #[test]
    fn session_focus_navigation_segment_to_detail() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Segment;

        app.on_key(make_key(KeyCode::Right));
        assert_eq!(app.session_focus, SessionFocus::Detail);
    }

    #[test]
    fn session_focus_esc_from_detail_to_segment() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Detail;
        app.session_detail_scroll = 5;

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.session_focus, SessionFocus::Segment);
        assert_eq!(app.session_detail_scroll, 0);
    }

    #[test]
    fn session_focus_esc_from_segment_to_list() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Segment;
        app.session_segment_selected = 3;

        app.on_key(make_key(KeyCode::Esc));
        assert_eq!(app.session_focus, SessionFocus::List);
        assert_eq!(app.session_segment_selected, 0);
    }

    #[test]
    fn session_focus_left_from_detail_to_segment() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Detail;

        app.on_key(make_key(KeyCode::Left));
        assert_eq!(app.session_focus, SessionFocus::Segment);
    }

    #[test]
    fn session_focus_left_from_segment_to_list() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Segment;

        app.on_key(make_key(KeyCode::Left));
        assert_eq!(app.session_focus, SessionFocus::List);
    }

    #[test]
    fn session_focus_navigate_up_in_segment() {
        let mut app = App::new(ConfigInventory::default());
        // Create a session with multiple segments
        app.push_event(make_test_event(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"first"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"second"}"#,
        ));
        app.session_focus = SessionFocus::Segment;
        app.session_segment_selected = 2;

        app.on_key(make_key(KeyCode::Up));
        assert_eq!(app.session_segment_selected, 1);
    }

    #[test]
    fn session_focus_navigate_down_in_segment() {
        let mut app = App::new(ConfigInventory::default());
        // Create a session with multiple segments
        app.push_event(make_test_event(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"first"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"second"}"#,
        ));
        app.session_focus = SessionFocus::Segment;
        app.session_segment_selected = 0;

        app.on_key(make_key(KeyCode::Down));
        assert_eq!(app.session_segment_selected, 1);
    }

    #[test]
    fn tab_switch_preserves_session_focus() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Segment;
        app.session_segment_selected = 2;

        // Tab away and back
        app.on_key(make_key(KeyCode::Tab)); // -> Config
        app.on_key(make_key(KeyCode::Tab)); // -> Events
        app.on_key(make_key(KeyCode::Tab)); // -> Sessions

        assert_eq!(
            app.session_focus, SessionFocus::Segment,
            "Tab switch must NOT reset session focus"
        );
        assert_eq!(
            app.session_segment_selected, 2,
            "Tab switch must NOT reset segment selection"
        );
    }

    #[test]
    fn visible_session_records_sorts_active_first() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("SessionStart", "s1"));
        app.push_event(make_event_with_session("SessionStart", "s2"));
        app.push_event(make_event_with_session("SessionEnd", "s1"));

        let records = app.visible_session_records();
        assert_eq!(records.len(), 2);
        // s2 is active, should be first
        assert_eq!(records[0].session_id, "s2");
        assert_eq!(records[1].session_id, "s1");
    }

    #[test]
    fn push_event_stop_failure_ends_session() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        ));
        app.push_event(make_event_with_session("StopFailure", "s1"));

        let record = app.session_records.get("s1").unwrap();
        assert!(record.ended);
        assert!(record.agent_records[0].ended_at.is_some());
    }

    #[test]
    fn push_event_post_tool_failure_increments_both_counts() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUseFailure","session_id":"s1","tool_name":"Bash","agent_context_type":"tdd"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        let bash = record.agent_records[0]
            .tools
            .iter()
            .find(|t| t.name == "Bash")
            .unwrap();
        assert_eq!(bash.count, 1);
        assert_eq!(bash.failure_count, 1);
    }

    #[test]
    fn push_event_orchestrator_tool_goes_to_segment() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_event_with_session("SessionStart", "s1"));
        // Tool use without agent context goes to orchestrator (segment level)
        app.push_event(make_test_event(
            r#"{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        let seg = &record.prompt_segments[0];
        assert_eq!(seg.orchestrator_tools.len(), 1);
        assert_eq!(seg.orchestrator_tools[0].name, "Read");
        assert_eq!(seg.orchestrator_tools[0].count, 1);
    }

    #[test]
    fn push_event_stop_closes_current_segment() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","prompt":"test"}"#,
        ));
        app.push_event(make_event_with_session("Stop", "s1"));

        let record = app.session_records.get("s1").unwrap();
        let last_seg = record.prompt_segments.last().unwrap();
        assert!(last_seg.ended_at.is_some());
        // Session should NOT be ended on Stop (only on SessionEnd)
        assert!(!record.ended);
    }

    #[test]
    fn push_event_multiple_agents_get_unique_ids() {
        let mut app = App::new(ConfigInventory::default());
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd","cwd":"/a"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"tdd","cwd":"/b"}"#,
        ));

        let record = app.session_records.get("s1").unwrap();
        assert_eq!(record.agent_records[0].id, 0);
        assert_eq!(record.agent_records[1].id, 1);
        assert_eq!(record.agent_records[2].id, 2);
        assert_eq!(record.next_agent_id, 3);
    }

    #[test]
    fn find_agent_for_routing_prefers_active() {
        let mut app = App::new(ConfigInventory::default());
        // Start and stop one planner
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        ));
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStop","session_id":"s1","agent_type":"planner"}"#,
        ));
        // Start another planner
        app.push_event(make_test_event(
            r#"{"hook_event_name":"SubagentStart","session_id":"s1","agent_type":"planner"}"#,
        ));

        let agent_id = app.find_agent_for_routing("s1", Some("planner"), None);
        // Should return the active one (id=1), not the completed one (id=0)
        assert_eq!(agent_id, Some(1));
    }

    #[test]
    fn page_down_only_works_in_session_detail() {
        let mut app = App::new(ConfigInventory::default());
        app.session_focus = SessionFocus::Segment;

        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(app.session_detail_scroll, 0, "PageDown should not work in Segment focus");

        app.session_focus = SessionFocus::Detail;
        app.on_key(make_key(KeyCode::PageDown));
        assert_eq!(app.session_detail_scroll, 5, "PageDown should work in Detail focus");
    }
}
