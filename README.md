# Claude Code Dashboard

Real-time TUI for Claude Code session observability, built with Rust and [ratatui](https://github.com/ratatui/ratatui).

```
Plugin hooks ──> Unix socket ──> TUI
```

The dashboard connects to the [kb-cc-plugin](https://github.com/pmmm114/kb-cc-plugin) `dashboard` plugin via a Unix socket. Hook events (PreToolUse, PostToolUse, SubagentStop, Stop, etc.) are streamed to the TUI in real time, alongside session state from the Claude Code session directory.

## Installation

### Build from source

```bash
git clone https://github.com/pmmm114/kb-cc-dashboard.git
cd kb-cc-dashboard
cargo build --release
# Binary is at target/release/claude-dashboard
```

### Future

- `cargo install claude-dashboard`
- Pre-built binaries via GitHub Releases

## Plugin setup

The dashboard requires the `dashboard` plugin to be installed separately in Claude Code:

```bash
claude plugin install dashboard@kb-cc-plugin
```

See the [kb-cc-plugin repository](https://github.com/pmmm114/kb-cc-plugin) for plugin configuration details.

## Usage

```bash
claude-dashboard [--socket-path PATH] [--claude-dir PATH] [--session-dir PATH]
```

### CLI options

| Flag | Default | Description |
|------|---------|-------------|
| `--socket-path PATH` | `/tmp/claude-dashboard.sock` | Path to the Unix socket where the plugin sends hook events |
| `--claude-dir PATH` | `~/.claude` | Path to the Claude Code config directory (agents, skills, rules, hooks) |
| `--session-dir PATH` | `/tmp/claude-session` | Path to the directory containing session state JSON files |

### Socket path configuration

Both the TUI and the plugin must agree on the socket path:

- **TUI side**: pass `--socket-path /path/to/socket` when launching the dashboard
- **Plugin side**: set the socket path in the plugin's `userConfig` field (see the plugin README for details)

If both use the default (`/tmp/claude-dashboard.sock`), no extra configuration is needed.

## Tabs

### Sessions

Lists active Claude Code sessions with their current phase, active agents, tool usage, and task progress. Select a session to view its full detail panel including agent breakdown, tool counts, and active tasks.

### Config

Browses the Claude Code configuration inventory organized by category: Agents, Skills, Rules, Hooks, and Plugins. Navigate categories with arrow keys, select items to view their content.

### Events

Live feed of hook events received over the Unix socket. Events are displayed in chronological order with session ID, event name, tool name, and timestamp. Select an event to view its full JSON payload.

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` | Cycle through tabs (Sessions, Config, Events) |
| `Up` / `Down` | Navigate list items |
| `Left` / `Right` | Switch focus between list and detail panel (or between config categories) |
| `Enter` | Open detail view for the selected item |
| `Esc` | Return to list view from detail |
| `PageUp` / `PageDown` | Scroll detail panel |
| `G` / `End` | Jump to latest event (Events tab) |
| `q` | Quit |

## Dependencies

| Crate | Purpose |
|-------|---------|
| [ratatui](https://crates.io/crates/ratatui) | Terminal UI framework |
| [crossterm](https://crates.io/crates/crossterm) | Cross-platform terminal manipulation |
| [tokio](https://crates.io/crates/tokio) | Async runtime (socket listener, file watcher) |
| [clap](https://crates.io/crates/clap) | CLI argument parsing |
| [serde](https://crates.io/crates/serde) / [serde_json](https://crates.io/crates/serde_json) | JSON deserialization for hook events and session state |
| [notify](https://crates.io/crates/notify) | Filesystem watcher for session state changes |
| [chrono](https://crates.io/crates/chrono) | Timestamp formatting |

## License

MIT
