use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

const LABEL_WIDTH: usize = 18;

pub fn detail_line(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:<width$}", label, width = LABEL_WIDTH),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ])
}

pub fn format_relative_time(unix_ts: u64) -> String {
    if unix_ts == 0 {
        return "unknown".to_string();
    }
    let now = chrono::Utc::now().timestamp() as u64;
    let diff = now.saturating_sub(unix_ts);
    if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

/// Renders scroll indicators (up/down arrows) on a detail pane when content overflows.
pub fn render_scroll_indicators(f: &mut Frame, area: Rect, total_lines: usize, scroll_offset: usize) {
    let (has_above, has_below) = has_more_content(total_lines, area.height, scroll_offset);
    if has_above {
        let indicator = Span::styled(" \u{25b2} ", Style::default().fg(Color::DarkGray));
        f.render_widget(
            Paragraph::new(Line::from(indicator)),
            Rect::new(area.right().saturating_sub(4), area.y, 3, 1),
        );
    }
    if has_below {
        let indicator = Span::styled(" \u{25bc} more ", Style::default().fg(Color::DarkGray));
        f.render_widget(
            Paragraph::new(Line::from(indicator)),
            Rect::new(
                area.right().saturating_sub(8),
                area.bottom().saturating_sub(1),
                7,
                1,
            ),
        );
    }
}

/// Returns (has_more_above, has_more_below) for scroll indicators.
pub fn has_more_content(total_lines: usize, visible_height: u16, scroll_offset: usize) -> (bool, bool) {
    let visible = visible_height.saturating_sub(2) as usize; // subtract borders
    let has_more_above = scroll_offset > 0;
    let has_more_below = total_lines > scroll_offset + visible;
    (has_more_above, has_more_below)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_line_uses_label_width_18() {
        let line = detail_line("Phase", "idle");
        let spans = line.spans;
        assert_eq!(spans.len(), 2);
        // Label should be padded to 18 chars
        let label_text = spans[0].content.to_string();
        assert_eq!(label_text.len(), LABEL_WIDTH);
        assert!(label_text.starts_with("Phase"));
    }

    #[test]
    fn format_relative_time_seconds() {
        let now = chrono::Utc::now().timestamp() as u64;
        let result = format_relative_time(now - 30);
        assert!(result.contains("s ago"));
    }

    #[test]
    fn format_relative_time_minutes() {
        let now = chrono::Utc::now().timestamp() as u64;
        let result = format_relative_time(now - 120);
        assert!(result.contains("m ago"));
    }

    #[test]
    fn format_relative_time_hours() {
        let now = chrono::Utc::now().timestamp() as u64;
        let result = format_relative_time(now - 7200);
        assert!(result.contains("h ago"));
    }

    #[test]
    fn format_relative_time_days() {
        let now = chrono::Utc::now().timestamp() as u64;
        let result = format_relative_time(now - 172800);
        assert!(result.contains("d ago"));
    }

    #[test]
    fn format_relative_time_future_timestamp() {
        let now = chrono::Utc::now().timestamp() as u64;
        let result = format_relative_time(now + 1000);
        assert!(result.contains("0s ago"));
    }

    #[test]
    fn format_relative_time_zero_returns_unknown() {
        let result = format_relative_time(0);
        assert_eq!(result, "unknown");
    }

    #[test]
    fn has_more_content_no_scroll_short_content() {
        let (above, below) = has_more_content(5, 10, 0);
        assert!(!above);
        assert!(!below);
    }

    #[test]
    fn has_more_content_no_scroll_long_content() {
        // visible_height=10, borders=2, so visible=8. 20 lines > 8
        let (above, below) = has_more_content(20, 10, 0);
        assert!(!above);
        assert!(below);
    }

    #[test]
    fn has_more_content_scrolled_middle() {
        // 20 lines, height 10 (visible 8), scrolled 5 -> shows 5..13, more above and below
        let (above, below) = has_more_content(20, 10, 5);
        assert!(above);
        assert!(below);
    }

    #[test]
    fn has_more_content_scrolled_to_bottom() {
        // 20 lines, height 10 (visible 8), scrolled 12 -> shows 12..20, no more below
        let (above, below) = has_more_content(20, 10, 12);
        assert!(above);
        assert!(!below);
    }
}
