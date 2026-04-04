# Agent User-Testing Scenarios

10 task scenarios for evaluating the Claude Code Dashboard TUI via agent-based user testing.
Each scenario simulates a real Claude Code premium subscriber interacting with the dashboard.

---

## Scenario 1: Identify the Longest-Running Agent

**Persona**: Senior developer monitoring a long-running implementation session.
**Goal**: Find which agent ran the longest and report its type and duration.
**Task**: Look at the Sessions tab content. Identify all agents visible, determine which one ran the longest based on start/end times, and report the agent type and approximate duration.
**Success criteria**:
- Correctly identifies the Explore agent as the completed agent with a known duration (~25 seconds)
- Notes that the tdd-implementer agent is still active (no end time)
- Reports agent types accurately

## Scenario 2: Find the Session with the Most Tool Failures

**Persona**: DevOps engineer investigating reliability issues.
**Goal**: Determine which session and which tool had the highest failure count.
**Task**: Examine the dashboard snapshots to find tool failure information. Identify the session, agent, and tool name with failures.
**Success criteria**:
- Identifies session a1b2c3d4 as having tool failures
- Identifies Grep (1 failure in Explore agent) and Edit (1 failure in tdd-implementer)
- Notes that session e5f6a7b8 has zero failures

## Scenario 3: Navigate to a Specific Prompt Segment

**Persona**: Developer reviewing what happened during a past session.
**Goal**: Find the second prompt segment of the active session and describe its content.
**Task**: In the Sessions tab, locate the prompt segments for session a1b2c3d4. Identify the text of the second user prompt (segment index 1, since segment 0 is initialization).
**Success criteria**:
- Correctly identifies "Implement the user authentication module" as the first real user prompt
- Identifies "Add unit tests for the login handler" as the second user prompt
- Notes that the initialization segment is separate from user prompts

## Scenario 4: Assess Tab Navigation Visual Clarity

**Persona**: New user trying the dashboard for the first time.
**Goal**: Evaluate whether the tab bar clearly indicates which tab is active and what tabs are available.
**Task**: Look at all four snapshot renders (Sessions list, Sessions segment, Events, Config). Evaluate the visual clarity of the tab bar: Can you tell which tab is active? Are all tabs labeled? Is the visual distinction sufficient?
**Success criteria**:
- Identifies all three tabs: Sessions, Config, Events
- Assesses whether the active tab is visually distinguishable
- Provides concrete feedback on any ambiguity or improvement needed

## Scenario 5: Evaluate Event Log Readability

**Persona**: Developer debugging a tool invocation failure.
**Goal**: Determine if the Events tab provides enough information to debug a tool failure.
**Task**: Examine the Events tab snapshot. Can you identify: which events are shown, what order they appear in, whether failure events are visually distinct from success events, and whether there is enough detail to diagnose an issue?
**Success criteria**:
- Lists specific event types visible (SessionStart, UserPromptSubmit, PostToolUse, etc.)
- Assesses whether PostToolUseFailure stands out from PostToolUse
- Comments on information density and whether error details are accessible

## Scenario 6: Verify Config Tab Completeness

**Persona**: Platform engineer auditing their Claude Code configuration.
**Goal**: Check that the Config tab shows all expected configuration categories.
**Task**: Look at the Config tab snapshot. Verify that Agents, Skills, Rules, Hooks, and Plugins categories are all present. For any visible items, check if the displayed information is useful (name, description, model, etc.).
**Success criteria**:
- Confirms presence of all 5 categories (Agents, Skills, Rules, Hooks, Plugins)
- Reports which category is currently focused
- Evaluates whether item details are informative enough for an audit

## Scenario 7: Track Task Progress Across Segments

**Persona**: Team lead monitoring task completion status.
**Goal**: Find all tasks across prompt segments and report their completion status.
**Task**: Examine the Sessions tab snapshots to find task information. For each task, report its ID, which segment it belongs to, and whether it is completed.
**Success criteria**:
- Identifies T1 (completed, in segment for "Implement the user authentication module")
- Identifies T2 (not completed, in segment for "Add unit tests for the login handler")
- Notes the teammate name for T1 if visible (auth-worker)

## Scenario 8: Compare Active vs Ended Session

**Persona**: Developer checking if a previous session ended normally.
**Goal**: Determine the visual difference between an active session and an ended session.
**Task**: Look at the Sessions list snapshot. Identify which session is active and which has ended. Describe any visual indicators that distinguish them (e.g., color, label, icon, status text).
**Success criteria**:
- Correctly identifies a1b2c3d4 as active and e5f6a7b8 as ended
- Describes the visual indicators used to show session status
- Assesses whether the distinction is clear enough at a glance

## Scenario 9: Evaluate Information Density in Agent Details

**Persona**: Power user who wants maximum information without clutter.
**Goal**: Assess whether agent details show enough context without overwhelming the display.
**Task**: Examine agent information visible in the Sessions tab snapshots. For each agent, check what metadata is shown: type, CWD, tools used, tool counts, failure counts, loaded rules/skills, start/end times. Rate the information density.
**Success criteria**:
- Lists which agent metadata fields are visible vs hidden
- Assesses whether the 120x40 terminal size is sufficient for the information
- Identifies any critical information that is missing or any redundant information that wastes space

## Scenario 10: End-to-End Workflow Comprehension

**Persona**: New team member learning how the dashboard represents a Claude Code workflow.
**Goal**: Reconstruct the workflow story from the dashboard snapshots alone.
**Task**: Using all available snapshots, reconstruct the narrative: What sessions exist? What did the user ask in each? What agents were involved? What tools were used? What was the outcome? Tell the story of this dashboard state as if explaining to a colleague.
**Success criteria**:
- Reconstructs the full narrative: session a1b2c3d4 started, user asked for auth module, Explore agent investigated, then user asked for tests, tdd-implementer is working
- Mentions the ended session e5f6a7b8 as a separate completed session
- Demonstrates that the dashboard provides enough context to understand the workflow without external documentation
