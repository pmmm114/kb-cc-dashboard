#!/usr/bin/env bash
#
# user-test.sh — Agent-based user testing for Claude Code Dashboard
#
# Generates TUI snapshots, then dispatches Claude (opus) agents to evaluate
# the dashboard against predefined scenarios. Collects structured JSON feedback.
#
# Usage:
#   ./scripts/user-test.sh [--rounds N] [--agents N] [--dry-run]
#
# Flags:
#   --rounds N   Number of evaluation rounds (default: 1, max: 3)
#   --agents N   Number of scenarios per round (default: 10, max: 10)
#   --dry-run    Generate snapshots and print prompts without calling claude

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SCENARIOS_FILE="$SCRIPT_DIR/agent-scenarios.md"

# Defaults
ROUNDS=1
AGENTS=10
DRY_RUN=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --rounds)
            ROUNDS="$2"
            shift 2
            ;;
        --agents)
            AGENTS="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--rounds N] [--agents N] [--dry-run]"
            echo ""
            echo "  --rounds N   Number of evaluation rounds (default: 1, max: 3)"
            echo "  --agents N   Number of scenarios per round (default: 10, max: 10)"
            echo "  --dry-run    Generate snapshots and print prompts without calling claude"
            exit 0
            ;;
        *)
            echo "Error: Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

# Validate arguments
if [[ "$ROUNDS" -lt 1 || "$ROUNDS" -gt 3 ]]; then
    echo "Error: --rounds must be between 1 and 3" >&2
    exit 1
fi
if [[ "$AGENTS" -lt 1 || "$AGENTS" -gt 10 ]]; then
    echo "Error: --agents must be between 1 and 10" >&2
    exit 1
fi

# --- Dependency checks ---

check_command() {
    if ! command -v "$1" &>/dev/null; then
        echo "Error: Required command '$1' not found. Please install it." >&2
        exit 1
    fi
}

check_command cargo
check_command jq

if [[ "$DRY_RUN" == false ]]; then
    check_command claude
fi

# --- Snapshot generation ---

SNAPSHOT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/claude-dashboard-snapshots.XXXXXX")"
FEEDBACK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/claude-dashboard-feedback.XXXXXX")"

echo "=== Claude Code Dashboard: Agent User Testing ==="
echo "Rounds: $ROUNDS | Agents per round: $AGENTS | Dry run: $DRY_RUN"
echo "Snapshot dir: $SNAPSHOT_DIR"
echo "Feedback dir: $FEEDBACK_DIR"
echo ""

echo "--- Generating snapshots ---"

cd "$PROJECT_DIR"
export SNAPSHOT_OUTPUT_DIR="$SNAPSHOT_DIR"

# Run the dedicated snapshot dump test which writes 4 tab renders to disk.
# Cargo and eprintln! both write to stderr, so merge streams for grep.
SNAP_OUTPUT=$(SNAPSHOT_OUTPUT_DIR="$SNAPSHOT_DIR" cargo test --test snapshot_dump -- --ignored --nocapture 2>&1)
if ! echo "$SNAP_OUTPUT" | grep -q "All snapshots written"; then
    echo "Error: Snapshot generation failed." >&2
    echo "$SNAP_OUTPUT" >&2
    echo "Ensure tests/snapshot_dump.rs exists and compiles." >&2
    exit 1
fi

# Verify snapshots exist
SNAP_COUNT=0
for snap in "$SNAPSHOT_DIR"/*.txt; do
    [[ -f "$snap" ]] && SNAP_COUNT=$((SNAP_COUNT + 1))
done

if [[ "$SNAP_COUNT" -eq 0 ]]; then
    echo "Error: No snapshots generated. Ensure the snapshot_dump test exists." >&2
    echo "Hint: Add the dump_snapshots test to tests/snapshot_dump.rs" >&2
    exit 1
fi

echo "  $SNAP_COUNT snapshots generated"
echo ""

# --- Parse scenarios ---

parse_scenarios() {
    local file="$1"
    local max="$2"
    local count=0
    local in_scenario=false
    local current_title=""
    local current_body=""

    while IFS= read -r line; do
        if [[ "$line" == "## Scenario "* ]]; then
            # Save previous scenario
            if [[ -n "$current_title" && "$count" -lt "$max" ]]; then
                count=$((count + 1))
                echo "SCENARIO_START:$count"
                echo "TITLE:$current_title"
                echo "$current_body"
                echo "SCENARIO_END:$count"
            fi
            current_title="${line#\#\# }"
            current_body=""
            in_scenario=true
        elif [[ "$in_scenario" == true ]]; then
            current_body+="$line"$'\n'
        fi
    done < "$file"

    # Last scenario
    if [[ -n "$current_title" && "$count" -lt "$max" ]]; then
        count=$((count + 1))
        echo "SCENARIO_START:$count"
        echo "TITLE:$current_title"
        echo "$current_body"
        echo "SCENARIO_END:$count"
    fi
}

# --- Build combined snapshot context ---

build_snapshot_context() {
    local context=""
    for snap in "$SNAPSHOT_DIR"/*.txt; do
        [[ -f "$snap" ]] || continue
        local name
        name="$(basename "$snap" .txt)"
        context+="=== Snapshot: $name ==="$'\n'
        context+="$(cat "$snap")"$'\n\n'
    done
    echo "$context"
}

SNAPSHOT_CONTEXT="$(build_snapshot_context)"

# --- JSON schema for structured output ---

JSON_SCHEMA='{
  "type": "object",
  "properties": {
    "severity": {
      "type": "string",
      "enum": ["critical", "major", "minor", "cosmetic"]
    },
    "title": {
      "type": "string"
    },
    "description": {
      "type": "string"
    },
    "reproduction_steps": {
      "type": "string"
    },
    "affected_tab": {
      "type": "string",
      "enum": ["Sessions", "Events", "Config", "All", "N/A"]
    }
  },
  "required": ["severity", "title", "description", "reproduction_steps", "affected_tab"]
}'

# --- Run evaluation rounds ---

COMBINED_FEEDBACK="$FEEDBACK_DIR/all_feedback.json"
echo "[]" > "$COMBINED_FEEDBACK"

for round in $(seq 1 "$ROUNDS"); do
    echo "=== Round $round of $ROUNDS ==="
    ROUND_FEEDBACK="$FEEDBACK_DIR/round_${round}.json"
    echo "[]" > "$ROUND_FEEDBACK"

    scenario_num=0
    while IFS= read -r line; do
        if [[ "$line" == "SCENARIO_START:"* ]]; then
            scenario_num="${line#SCENARIO_START:}"
            scenario_title=""
            scenario_body=""
        elif [[ "$line" == "TITLE:"* ]]; then
            scenario_title="${line#TITLE:}"
        elif [[ "$line" == "SCENARIO_END:"* ]]; then
            if [[ "$scenario_num" -gt "$AGENTS" ]]; then
                continue
            fi

            echo "  [$round/$scenario_num] $scenario_title"

            # Build the prompt
            PROMPT="You are a UX evaluator for a terminal-based dashboard (TUI) built with Ratatui.
You are examining rendered text snapshots of the dashboard at 120x40 terminal size.

Below are the current dashboard snapshots:

$SNAPSHOT_CONTEXT

---

Your evaluation task:

$scenario_title

$scenario_body

---

Based on your evaluation, produce a single JSON object describing any UX issue you found.
If you found no issues, report severity as \"cosmetic\" with title \"No issues found\".

Respond with ONLY the JSON object, no markdown fences, no explanation.
The JSON must have these exact fields:
- severity: one of \"critical\", \"major\", \"minor\", \"cosmetic\"
- title: short issue title
- description: detailed description of the issue
- reproduction_steps: how to observe the issue in the dashboard
- affected_tab: one of \"Sessions\", \"Events\", \"Config\", \"All\", \"N/A\""

            if [[ "$DRY_RUN" == true ]]; then
                echo "    [DRY RUN] Prompt length: ${#PROMPT} chars"
                echo ""
                echo "--- PROMPT PREVIEW (first 500 chars) ---"
                echo "${PROMPT:0:500}"
                echo "--- END PREVIEW ---"
                echo ""
            else
                # Invoke claude CLI with opus model
                RESPONSE=$(echo "$PROMPT" | claude --model opus -p --bare --output-format text 2>/dev/null || echo '{"severity":"cosmetic","title":"Agent invocation failed","description":"The claude CLI returned an error","reproduction_steps":"N/A","affected_tab":"N/A"}')

                # Extract JSON from response (handle potential wrapping text)
                JSON_RESPONSE=$(echo "$RESPONSE" | grep -o '{[^}]*}' | head -1 || echo "")

                if [[ -z "$JSON_RESPONSE" ]]; then
                    # Try the full response as JSON
                    if echo "$RESPONSE" | jq . &>/dev/null; then
                        JSON_RESPONSE="$RESPONSE"
                    else
                        JSON_RESPONSE="{\"severity\":\"cosmetic\",\"title\":\"Parse error\",\"description\":\"Could not parse agent response: $(echo "$RESPONSE" | head -5 | tr '"' "'" | tr '\n' ' ')\",\"reproduction_steps\":\"N/A\",\"affected_tab\":\"N/A\"}"
                    fi
                fi

                # Add metadata
                ENRICHED=$(echo "$JSON_RESPONSE" | jq \
                    --arg round "$round" \
                    --arg scenario "$scenario_num" \
                    --arg stitle "$scenario_title" \
                    '. + {round: ($round | tonumber), scenario: ($scenario | tonumber), scenario_title: $stitle}' 2>/dev/null || echo "$JSON_RESPONSE")

                # Append to round feedback
                jq --argjson item "$ENRICHED" '. + [$item]' "$ROUND_FEEDBACK" > "$ROUND_FEEDBACK.tmp" && mv "$ROUND_FEEDBACK.tmp" "$ROUND_FEEDBACK"

                echo "    Severity: $(echo "$ENRICHED" | jq -r '.severity' 2>/dev/null || echo 'unknown')"
                echo "    Title: $(echo "$ENRICHED" | jq -r '.title' 2>/dev/null || echo 'unknown')"
            fi
        else
            scenario_body+="$line"$'\n'
        fi
    done < <(parse_scenarios "$SCENARIOS_FILE" 10)

    if [[ "$DRY_RUN" == false ]]; then
        # Merge round feedback into combined
        jq -s '.[0] + .[1]' "$COMBINED_FEEDBACK" "$ROUND_FEEDBACK" > "$COMBINED_FEEDBACK.tmp" && mv "$COMBINED_FEEDBACK.tmp" "$COMBINED_FEEDBACK"

        # Check exit condition: any Critical or Major issues this round?
        CRIT_MAJOR_COUNT=$(jq '[.[] | select(.round == '"$round"' and (.severity == "critical" or .severity == "major"))] | length' "$COMBINED_FEEDBACK" 2>/dev/null || echo "0")

        echo ""
        echo "  Round $round summary: $CRIT_MAJOR_COUNT critical/major issues"

        if [[ "$CRIT_MAJOR_COUNT" -eq 0 && "$round" -gt 1 ]]; then
            echo ""
            echo "=== Convergence reached: No new Critical or Major issues in round $round ==="
            break
        fi

        if [[ "$round" -ge "$ROUNDS" ]]; then
            echo ""
            echo "=== Max rounds ($ROUNDS) reached ==="
        fi
    fi

    echo ""
done

# --- Output results ---

if [[ "$DRY_RUN" == true ]]; then
    echo "=== Dry run complete ==="
    echo "Snapshots saved to: $SNAPSHOT_DIR"
    echo "No feedback collected (dry-run mode)"
else
    TOTAL=$(jq 'length' "$COMBINED_FEEDBACK")
    CRITICAL=$(jq '[.[] | select(.severity == "critical")] | length' "$COMBINED_FEEDBACK")
    MAJOR=$(jq '[.[] | select(.severity == "major")] | length' "$COMBINED_FEEDBACK")
    MINOR=$(jq '[.[] | select(.severity == "minor")] | length' "$COMBINED_FEEDBACK")
    COSMETIC=$(jq '[.[] | select(.severity == "cosmetic")] | length' "$COMBINED_FEEDBACK")

    echo "=== Final Results ==="
    echo "Total feedback items: $TOTAL"
    echo "  Critical: $CRITICAL"
    echo "  Major:    $MAJOR"
    echo "  Minor:    $MINOR"
    echo "  Cosmetic: $COSMETIC"
    echo ""
    echo "Feedback saved to: $COMBINED_FEEDBACK"
    echo ""
    echo "To create GitHub issues from this feedback, run:"
    echo "  ./scripts/triage-issues.sh $COMBINED_FEEDBACK"
fi
