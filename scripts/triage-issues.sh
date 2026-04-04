#!/usr/bin/env bash
#
# triage-issues.sh — Deduplicate agent feedback and create GitHub issues
#
# Reads JSON feedback produced by user-test.sh, groups by similarity
# (title + severity match), and creates GitHub issues via gh CLI.
#
# Usage:
#   ./scripts/triage-issues.sh <feedback.json> [--dry-run]
#
# Flags:
#   --dry-run   Print what would be created without calling gh

set -euo pipefail

# --- Argument parsing ---

FEEDBACK_FILE=""
DRY_RUN=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 <feedback.json> [--dry-run]"
            echo ""
            echo "  feedback.json   Path to the JSON feedback file from user-test.sh"
            echo "  --dry-run       Print what would be created without calling gh"
            exit 0
            ;;
        *)
            if [[ -z "$FEEDBACK_FILE" ]]; then
                FEEDBACK_FILE="$1"
            else
                echo "Error: Unexpected argument: $1" >&2
                exit 1
            fi
            shift
            ;;
    esac
done

if [[ -z "$FEEDBACK_FILE" ]]; then
    echo "Error: No feedback file specified." >&2
    echo "Usage: $0 <feedback.json> [--dry-run]" >&2
    exit 1
fi

if [[ ! -f "$FEEDBACK_FILE" ]]; then
    echo "Error: Feedback file not found: $FEEDBACK_FILE" >&2
    exit 1
fi

# --- Dependency checks ---

check_command() {
    if ! command -v "$1" &>/dev/null; then
        echo "Error: Required command '$1' not found. Please install it." >&2
        exit 1
    fi
}

check_command jq

if [[ "$DRY_RUN" == false ]]; then
    check_command gh
    # Verify gh auth
    if ! gh auth status &>/dev/null; then
        echo "Error: gh CLI is not authenticated. Run 'gh auth login' first." >&2
        exit 1
    fi
fi

# --- Validate JSON ---

if ! jq empty "$FEEDBACK_FILE" 2>/dev/null; then
    echo "Error: Invalid JSON in feedback file" >&2
    exit 1
fi

TOTAL_ITEMS=$(jq 'length' "$FEEDBACK_FILE")
if [[ "$TOTAL_ITEMS" -eq 0 ]]; then
    echo "No feedback items to process."
    exit 0
fi

echo "=== Triage: Processing $TOTAL_ITEMS feedback items ==="
echo ""

# --- Deduplicate by title + severity ---
#
# Group items by (severity, title) pair. For each unique pair:
# - Use the first item's description and reproduction_steps
# - Count how many agents reported the same issue
# - Collect all affected tabs

DEDUP_FILE="$(mktemp "${TMPDIR:-/tmp}/claude-triage-dedup.XXXXXX")"

jq '
  group_by(.severity + "|" + .title)
  | map({
      severity: .[0].severity,
      title: .[0].title,
      description: .[0].description,
      reproduction_steps: .[0].reproduction_steps,
      affected_tabs: ([.[].affected_tab] | unique | join(", ")),
      report_count: length,
      rounds: ([.[].round] | unique | sort)
    })
  | sort_by(
      if .severity == "critical" then 0
      elif .severity == "major" then 1
      elif .severity == "minor" then 2
      else 3 end
    )
' "$FEEDBACK_FILE" > "$DEDUP_FILE"

UNIQUE_COUNT=$(jq 'length' "$DEDUP_FILE")
echo "Deduplicated: $TOTAL_ITEMS items -> $UNIQUE_COUNT unique issues"
echo ""

# --- Severity counts ---

CRITICAL=$(jq '[.[] | select(.severity == "critical")] | length' "$DEDUP_FILE")
MAJOR=$(jq '[.[] | select(.severity == "major")] | length' "$DEDUP_FILE")
MINOR=$(jq '[.[] | select(.severity == "minor")] | length' "$DEDUP_FILE")
COSMETIC=$(jq '[.[] | select(.severity == "cosmetic")] | length' "$DEDUP_FILE")

echo "Severity breakdown:"
echo "  Critical: $CRITICAL"
echo "  Major:    $MAJOR"
echo "  Minor:    $MINOR"
echo "  Cosmetic: $COSMETIC"
echo ""

# --- Skip "No issues found" entries ---

ACTIONABLE_FILE="$(mktemp "${TMPDIR:-/tmp}/claude-triage-actionable.XXXXXX")"
jq '[.[] | select(.title != "No issues found")]' "$DEDUP_FILE" > "$ACTIONABLE_FILE"
ACTIONABLE_COUNT=$(jq 'length' "$ACTIONABLE_FILE")

if [[ "$ACTIONABLE_COUNT" -eq 0 ]]; then
    echo "All feedback items are 'No issues found'. Nothing to create."
    rm -f "$DEDUP_FILE" "$ACTIONABLE_FILE"
    exit 0
fi

echo "Actionable issues: $ACTIONABLE_COUNT"
echo ""

# --- Create GitHub issues ---

CREATED_COUNT=0
CREATED_ISSUES=()

for i in $(seq 0 $((ACTIONABLE_COUNT - 1))); do
    SEVERITY=$(jq -r ".[$i].severity" "$ACTIONABLE_FILE")
    TITLE=$(jq -r ".[$i].title" "$ACTIONABLE_FILE")
    DESCRIPTION=$(jq -r ".[$i].description" "$ACTIONABLE_FILE")
    REPRO_STEPS=$(jq -r ".[$i].reproduction_steps" "$ACTIONABLE_FILE")
    AFFECTED_TABS=$(jq -r ".[$i].affected_tabs" "$ACTIONABLE_FILE")
    REPORT_COUNT=$(jq -r ".[$i].report_count" "$ACTIONABLE_FILE")
    ROUNDS=$(jq -r ".[$i].rounds | join(\", \")" "$ACTIONABLE_FILE")

    # Capitalize first letter of severity for title
    SEVERITY_CAP="$(echo "${SEVERITY:0:1}" | tr '[:lower:]' '[:upper:]')${SEVERITY:1}"

    ISSUE_TITLE="[UX/$SEVERITY_CAP] $TITLE"

    ISSUE_BODY="## Description

$DESCRIPTION

## Reproduction Steps

$REPRO_STEPS

## Details

- **Severity**: $SEVERITY
- **Affected tab(s)**: $AFFECTED_TABS
- **Reported by**: $REPORT_COUNT agent(s) across round(s) $ROUNDS
- **Source**: Automated agent-based UX evaluation

---
*Generated by \`scripts/user-test.sh\` + \`scripts/triage-issues.sh\`*"

    # Map severity to label
    LABELS="ux,$SEVERITY"

    echo "--- Issue $((i + 1))/$ACTIONABLE_COUNT ---"
    echo "  Title:    $ISSUE_TITLE"
    echo "  Severity: $SEVERITY"
    echo "  Reports:  $REPORT_COUNT"
    echo "  Labels:   $LABELS"

    if [[ "$DRY_RUN" == true ]]; then
        echo "  [DRY RUN] Would create issue"
        echo ""
    else
        # Create the issue via gh CLI
        ISSUE_URL=$(gh issue create \
            --title "$ISSUE_TITLE" \
            --body "$ISSUE_BODY" \
            --label "$LABELS" \
            2>&1) || {
            echo "  Warning: Failed to create issue. Labels may not exist." >&2
            echo "  Retrying without labels..." >&2
            ISSUE_URL=$(gh issue create \
                --title "$ISSUE_TITLE" \
                --body "$ISSUE_BODY" \
                2>&1) || {
                echo "  Error: Failed to create issue even without labels" >&2
                continue
            }
        }

        echo "  Created: $ISSUE_URL"
        CREATED_COUNT=$((CREATED_COUNT + 1))
        CREATED_ISSUES+=("$ISSUE_URL")
        echo ""
    fi
done

# --- Summary ---

echo ""
echo "=== Triage Summary ==="
if [[ "$DRY_RUN" == true ]]; then
    echo "Dry run: $ACTIONABLE_COUNT issues would be created"
else
    echo "Created $CREATED_COUNT GitHub issues out of $ACTIONABLE_COUNT actionable"
    if [[ ${#CREATED_ISSUES[@]} -gt 0 ]]; then
        echo ""
        echo "Created issues:"
        for url in "${CREATED_ISSUES[@]}"; do
            echo "  - $url"
        done
    fi
fi
echo ""

# Exit condition info for user-test.sh integration
if [[ "$CRITICAL" -gt 0 || "$MAJOR" -gt 0 ]]; then
    echo "Status: Critical/Major issues found — another round recommended"
    exit 2  # Special exit code: issues found
else
    echo "Status: No critical/major issues — convergence possible"
    exit 0
fi
