use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::{App, ToolRecord};
use super::helpers::{detail_line, render_scroll_indicators};

/// Extracts the last path segment if the path looks like a worktree.
pub fn extract_worktree_suffix(path: &str) -> Option<&str> {
    if path.contains("/worktrees/") || path.contains("/tmp/claude-config-") {
        let trimmed = path.trim_end_matches('/');
        trimmed.rsplit('/').next().filter(|s| !s.is_empty())
    } else {
        None
    }
}

/// Formats tool records with counts and optional failure summary into an indented line.
/// Accepts any iterator over `&ToolRecord` to support both owned slices and sorted reference vecs.
pub fn format_tools_line<'a>(tools: impl Iterator<Item = &'a ToolRecord>) -> Line<'static> {
    let mut total_failures: usize = 0;
    let mut parts: Vec<String> = Vec::new();
    for t in tools {
        parts.push(format!("{} ({})", t.name, t.count));
        total_failures += t.failure_count;
    }
    if total_failures > 0 {
        parts.push(format!("[{} failed]", total_failures));
    }
    Line::from(Span::styled(
        format!("    {}", parts.join("  ")),
        Style::default().fg(Color::White),
    ))
}

/// Stub implementation for Sessions tab rendering.
/// T5 will fully rewrite this with 3-pane UI using session_records.
pub fn draw_sessions(f: &mut Frame, app: &App, list_area: Rect, detail_area: Rect) {
    draw_session_list(f, app, list_area);
    draw_session_detail(f, app, detail_area);
}

fn draw_session_list(f: &mut Frame, app: &App, area: Rect) {
    let records = app.visible_session_records();

    if records.is_empty() {
        let block = Block::default().borders(Borders::ALL).title("Sessions (0)");
        let msg = Paragraph::new("No sessions found \u{2014} waiting for data...")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let id_len = 8.min(r.session_id.len());
            let session_id_short = &r.session_id[..id_len];
            let status = if r.ended { "ended" } else { "active" };

            let dim = Style::default().fg(Color::DarkGray);
            let id_style = if !r.ended { Style::default().fg(Color::White) } else { dim };

            let line = Line::from(vec![
                Span::styled(format!("{:>2} ", i + 1), dim),
                Span::styled(session_id_short.to_string(), id_style),
                Span::raw(" "),
                Span::styled(format!("{:<14}", status), dim),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = format!("Sessions ({})", records.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let selected = app.session_selected.min(records.len().saturating_sub(1));
    let mut state = ListState::default();
    state.select(Some(selected));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_session_detail(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Session Detail");
    let records = app.visible_session_records();

    if records.is_empty() {
        f.render_widget(block, area);
        return;
    }

    let selected = app.session_selected.min(records.len().saturating_sub(1));
    let record = records[selected];

    let lines = vec![
        detail_line("Session ID", &record.session_id),
        detail_line("Segments", &record.prompt_segments.len().to_string()),
        detail_line("Agents", &record.agent_records.len().to_string()),
        detail_line("Status", if record.ended { "ended" } else { "active" }),
    ];

    let total_lines = lines.len();
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.session_detail_scroll as u16, 0));
    f.render_widget(paragraph, area);

    render_scroll_indicators(f, area, total_lines, app.session_detail_scroll);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::ConfigInventory;

    #[test]
    fn extract_worktree_suffix_matches_worktrees_path() {
        assert_eq!(
            extract_worktree_suffix("/some/path/worktrees/my-branch"),
            Some("my-branch")
        );
    }

    #[test]
    fn extract_worktree_suffix_matches_tmp_config() {
        assert_eq!(
            extract_worktree_suffix("/tmp/claude-config-test"),
            Some("claude-config-test")
        );
    }

    #[test]
    fn extract_worktree_suffix_returns_none_for_normal_path() {
        assert_eq!(extract_worktree_suffix("/home/user/project"), None);
    }

    #[test]
    fn format_tools_line_with_failures() {
        let tools = vec![
            ToolRecord {
                name: "Read".to_string(),
                count: 5,
                failure_count: 0,
            },
            ToolRecord {
                name: "Bash".to_string(),
                count: 3,
                failure_count: 2,
            },
        ];
        let line = format_tools_line(tools.iter());
        let text: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("Read (5)"));
        assert!(text.contains("Bash (3)"));
        assert!(text.contains("[2 failed]"));
    }

    #[test]
    fn format_tools_line_no_failures() {
        let tools = vec![ToolRecord {
            name: "Edit".to_string(),
            count: 2,
            failure_count: 0,
        }];
        let line = format_tools_line(tools.iter());
        let text: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("Edit (2)"));
        assert!(!text.contains("failed"));
    }
}
