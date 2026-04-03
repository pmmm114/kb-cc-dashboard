use chrono::{DateTime, Utc};
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

/// Formats a duration as a human-readable string.
/// - < 60s: "Xs"
/// - < 60m: "Xm Ys"
/// - >= 60m: "Xh Ym"
pub fn format_duration(duration: chrono::Duration) -> String {
    let total_secs = duration.num_seconds().max(0);
    if total_secs < 60 {
        format!("{}s", total_secs)
    } else if total_secs < 3600 {
        let m = total_secs / 60;
        let s = total_secs % 60;
        format!("{}m {}s", m, s)
    } else {
        let h = total_secs / 3600;
        let m = (total_secs % 3600) / 60;
        format!("{}h {}m", h, m)
    }
}

pub fn format_relative_time_dt(dt: &DateTime<Utc>) -> String {
    let diff = Utc::now() - *dt;
    let secs = diff.num_seconds();
    if secs <= 0 {
        "just now".to_string()
    } else if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
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
    fn format_relative_time_dt_seconds() {
        let dt = chrono::Utc::now() - chrono::Duration::seconds(30);
        let result = format_relative_time_dt(&dt);
        assert!(result.contains("s ago"), "expected 's ago', got: {result}");
    }

    #[test]
    fn format_relative_time_dt_minutes() {
        let dt = chrono::Utc::now() - chrono::Duration::seconds(300);
        let result = format_relative_time_dt(&dt);
        assert!(result.contains("m ago"), "expected 'm ago', got: {result}");
    }

    #[test]
    fn format_relative_time_dt_hours() {
        let dt = chrono::Utc::now() - chrono::Duration::seconds(7200);
        let result = format_relative_time_dt(&dt);
        assert!(result.contains("h ago"), "expected 'h ago', got: {result}");
    }

    #[test]
    fn format_relative_time_dt_days() {
        let dt = chrono::Utc::now() - chrono::Duration::seconds(259200);
        let result = format_relative_time_dt(&dt);
        assert!(result.contains("d ago"), "expected 'd ago', got: {result}");
    }

    #[test]
    fn format_relative_time_dt_future() {
        let dt = chrono::Utc::now() + chrono::Duration::seconds(100);
        let result = format_relative_time_dt(&dt);
        assert_eq!(result, "just now");
    }

    #[test]
    fn format_duration_seconds() {
        let d = chrono::Duration::seconds(45);
        assert_eq!(format_duration(d), "45s");
    }

    #[test]
    fn format_duration_minutes_and_seconds() {
        let d = chrono::Duration::seconds(154); // 2m 34s
        assert_eq!(format_duration(d), "2m 34s");
    }

    #[test]
    fn format_duration_hours_and_minutes() {
        let d = chrono::Duration::seconds(4500); // 1h 15m
        assert_eq!(format_duration(d), "1h 15m");
    }

    #[test]
    fn format_duration_zero() {
        let d = chrono::Duration::seconds(0);
        assert_eq!(format_duration(d), "0s");
    }

    #[test]
    fn format_duration_exactly_60s() {
        let d = chrono::Duration::seconds(60);
        assert_eq!(format_duration(d), "1m 0s");
    }

    #[test]
    fn format_duration_exactly_1h() {
        let d = chrono::Duration::seconds(3600);
        assert_eq!(format_duration(d), "1h 0m");
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
