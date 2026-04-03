# Claude Code Dashboard

Real-time TUI for Claude Code session observability.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)

```
Claude Code hooks ──> dashboard plugin ──> Unix socket ──> Dashboard TUI
```

## What it does

Claude Code에서 발생하는 모든 hook 이벤트를 실시간으로 수신하여 터미널에서 시각화합니다.

- **Sessions** — 세션별 에이전트 트리, 로드된 컨텍스트(Rules, Skills, Agent 정의), 도구 사용량, 태스크 진행 상황
- **Config** — Claude Code 설정 인벤토리 브라우저 (Agents, Skills, Rules, Hooks, Plugins)
- **Events** — 실시간 hook 이벤트 피드 + JSON 페이로드 상세 뷰

## Quick Start

### 1. 대시보드 빌드

```bash
git clone https://github.com/pmmm114/kb-cc-dashboard.git
cd kb-cc-dashboard
cargo build --release
```

바이너리 위치: `target/release/claude-dashboard`

### 2. 플러그인 설치

대시보드는 [kb-cc-plugin](https://github.com/pmmm114/kb-cc-plugin)의 `dashboard` 플러그인에서 이벤트를 수신합니다.

```bash
claude plugin install dashboard@kb-cc-plugin
```

### 3. 실행

```bash
# 터미널 1: 대시보드 실행
./target/release/claude-dashboard

# 터미널 2: Claude Code 사용 — 이벤트가 자동으로 대시보드에 표시됩니다
claude
```

대시보드와 플러그인 모두 기본 소켓 경로 `/tmp/claude-dashboard.sock`을 사용합니다. 변경이 필요하면 양쪽 모두 동일한 경로로 설정하세요.

## Usage

```bash
claude-dashboard [OPTIONS]
```

| 옵션 | 기본값 | 설명 |
|------|--------|------|
| `--socket-path PATH` | `/tmp/claude-dashboard.sock` | 이벤트 수신 Unix 소켓 경로 |
| `--claude-dir PATH` | `~/.claude` | Claude Code 설정 디렉토리 경로 |

## Guide

### Sessions 탭 (3-pane drill-down)

세션 리스트 → Prompt Segments → 에이전트 트리로 구성된 3단계 탐색 구조입니다.

**세션 리스트** (좌측)
- `◉` 깜빡임 — 이벤트 수신 중 (live)
- `●` 초록 고정 — 활성 상태 (active)
- `○` 회색 — 종료됨 (inactive)

**Prompt Segments** (중앙)

사용자가 입력한 프롬프트 단위로 에이전트 활동을 그룹핑합니다. 각 세그먼트는 하나의 `UserPromptSubmit` 이벤트에 대응합니다.

```
  3 ● "리팩터링 해줘"            now
  2 ✓ "테스트 추가해줘"          5m
  1 ✓ "버그 수정해줘"            15m
```

**에이전트 트리** (우측)

선택한 세그먼트에서 실행된 에이전트와 그 컨텍스트를 트리 구조로 보여줍니다.

```
 ▾ planner (completed, 1m 12s)
 │  ├─ Context
 │  │  ├─ Agent: planner.md
 │  │  ├─ Skills: gh-cli
 │  │  └─ Rules: workflow.md, investigation.md
 │  └─ Tools: Read ×12, Grep ×5, Glob ×3
 │
 ▾ tdd-implementer @worktree-T1 (◉ 1m 54s)
    ├─ Context
    │  ├─ Agent: tdd-implementer.md
    │  └─ Rules: code-quality.md, workflow.md
    ├─ Tools: Read ×8, Edit ×3 [1 failed], Bash ×5
    └─ Task: T2 (login flow)
```

### Config 탭 (3-pane browser)

`~/.claude/` 디렉토리의 설정 파일을 카테고리별로 탐색합니다. Agents, Skills, Rules, Hooks, Plugins 5개 카테고리.

### Events 탭 (2-pane feed)

실시간 hook 이벤트 스트림. 카테고리 아이콘과 에이전트 트리 라인으로 시각적 계층을 제공합니다.

**이벤트 리스트** (좌측)

```
 ⚡ 12:34:56 PostToolUse        Read (orchestrator)
 ◆ 12:34:58 SubagentStart      tdd-implementer
 │ ⚡ 12:35:01 PostToolUse      Bash (tdd-implementer)
 │ ⚡ 12:35:05 PostToolUse      Edit (tdd-implementer)
 ◆ 12:35:10 SubagentStop       tdd-implementer
```

- 카테고리 아이콘: ⚡ Tool, ◆ Agent, ▶ User, ◻ Task, ⚙ System, ✖ Error
- `│` 트리 라인 — 에이전트 스코프 내 이벤트를 들여쓰기
- 에러 이벤트(StopFailure, PostToolUseFailure) — 빨간 배경 강조
- Sessions 탭에서 `Enter` → Events 탭으로 전환 + 해당 세션 필터 자동 적용

**구조화 디테일** (우측)

이벤트 선택 시 3섹션 구조로 페이로드를 표시합니다:

```
── PostToolUse ──
  Tool:             Read
  Agent Context:    tdd-implementer
  File:             src/app.rs
  Duration:         45

── Extra Fields ──
  custom_field:     hello

── Raw JSON ──
  { "tool_name": "Read", ... }
```

- Structured — 이벤트 타입별 주요 필드 요약
- Extra Fields — 플러그인 커스텀 필드 자동 감지
- Raw JSON — 전체 페이로드 (항상 표시)

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` | 탭 전환 (Sessions → Config → Events) |
| `Up` / `Down` | 항목 선택 또는 스크롤 |
| `Left` / `Right` | 패인 간 이동 |
| `Enter` | Sessions: 이벤트 탭으로 전환 + 필터 / 기타: 다음 패인 진입 |
| `Esc` | 이전 패인으로 복귀 |
| `PageUp` / `PageDown` | 디테일 패인 페이지 스크롤 |
| `f` | 세션별 이벤트 필터 순환 (Events 탭) |
| `G` / `End` | 최신 이벤트로 이동 (Events 탭) |
| `q` | 종료 |

## License

MIT
