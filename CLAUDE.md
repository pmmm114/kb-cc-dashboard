# CLAUDE.md — kb-cc-dashboard

## Design constraints

- **No state.json dependency** — all session/agent/tool data is derived from the event stream. Do NOT add filesystem reads for hook state files.
- **No "workflow" or "phase" concept** — these are custom hook concepts from kb-cc-plugin's orchestration, not standard Claude Code platform events. Do not model them.
- **Event-sourced agents** — on SubagentStop, set `ended_at` on the AgentRecord. Never delete agent records; completed agents must remain queryable.
- **Segment zero** — the first PromptSegment ("(session initialization)") absorbs all events that arrive before the first `UserPromptSubmit`. This is intentional, not a fallback.
- **AgentId is monotonic per session** — `next_agent_id: u64` increments and is never reused/recycled, even after agents end.

## Event routing rules

- **Only PostToolUse counts** — PreToolUse is intentionally not counted toward tool usage. Both Post variants (success + failure) increment counts.
- **Agent matching priority** — when routing events to agents: prefer active (no `ended_at`) over completed, then match by `agent_type` + CWD. Unmatched events go to the current segment's orchestrator fields.

## Plugin interaction

The dashboard plugin (`kb-cc-plugin/dashboard`) enriches tool events with `agent_context_type` by tracking an agent stack per session. The dashboard uses this field for agent-tool routing.

- **Known gap**: `InstructionsLoaded` events are NOT enriched by the plugin. Instructions without `agent_context_type` fall to orchestrator context.
- **macOS symlink trap**: `/tmp` resolves to `/private/tmp` on macOS. CWD matching in both plugin enrichment and dashboard agent routing can silently break if paths are not canonicalized.

## Known regressions (do not reintroduce)

- **BUG-1**: Double counting — tool events counted twice under certain routing paths.
- **BUG-2**: Session re-activation — ended sessions incorrectly marked active on late events.
