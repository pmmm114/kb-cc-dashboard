use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::app::{App, ConfigFocus, ListDetailFocus, SessionFocus, Tab};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),   // content area
            Constraint::Length(1), // status bar
        ])
        .split(f.area());

    draw_tab_bar(f, app, chunks[0]);
    draw_content(f, app, chunks[1]);
    draw_status_bar(f, app, chunks[2]);
}

fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let titles = [Tab::Sessions, Tab::Config, Tab::Events]
        .iter()
        .map(|t| t.label())
        .collect::<Vec<_>>();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Claude Code Dashboard"),
        )
        .select(match app.active_tab {
            Tab::Sessions => 0,
            Tab::Config => 1,
            Tab::Events => 2,
        })
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

fn draw_content(f: &mut Frame, app: &App, area: Rect) {
    match app.active_tab {
        Tab::Sessions => {
            crate::tabs::sessions::draw_sessions(f, app, area);
        }
        Tab::Config => {
            crate::tabs::config::draw_config(f, app, area);
        }
        Tab::Events => {
            let (list_area, detail_area) = split_list_detail(area);
            crate::tabs::events::draw_events(f, app, list_area, detail_area);
        }
    }
}

fn hint_key(label: &str) -> Span<'_> {
    Span::styled(label, Style::default().fg(Color::Yellow))
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![hint_key("[q]"), Span::raw(" Quit  "), hint_key("[Tab]"), Span::raw(" Switch  ")];

    match app.active_tab {
        Tab::Sessions => match app.session_focus {
            SessionFocus::List => {
                spans.extend([hint_key("[up/dn]"), Span::raw(" Select  "), hint_key("[Enter/\u{2192}]"), Span::raw(" Segments  ")]);
            }
            SessionFocus::Segment => {
                spans.extend([
                    hint_key("[up/dn]"), Span::raw(" Select  "),
                    hint_key("[Enter/\u{2192}]"), Span::raw(" Detail  "),
                    hint_key("[Esc/\u{2190}]"), Span::raw(" Back  "),
                ]);
            }
            SessionFocus::Detail => {
                spans.extend([
                    hint_key("[up/dn]"), Span::raw(" Scroll  "),
                    hint_key("[Esc/\u{2190}]"), Span::raw(" Back  "),
                    hint_key("[PgUp/Dn]"), Span::raw(" Page  "),
                ]);
            }
        },
        Tab::Events => match app.event_focus {
            ListDetailFocus::List => {
                spans.extend([
                    hint_key("[up/dn]"), Span::raw(" Select  "),
                    hint_key("[Enter]"), Span::raw(" Detail  "),
                    hint_key("[f]"), Span::raw(" Filter  "),
                ]);
            }
            ListDetailFocus::Detail => {
                spans.extend([
                    hint_key("[up/dn]"), Span::raw(" Scroll  "),
                    hint_key("[Esc]"), Span::raw(" Back  "),
                    hint_key("[PgUp/Dn]"), Span::raw(" Page  "),
                ]);
            }
        },
        Tab::Config => match app.config_focus {
            ConfigFocus::Category => {
                spans.extend([
                    hint_key("[up/dn]"), Span::raw(" Select  "),
                    hint_key("[Enter/\u{2192}]"), Span::raw(" Items  "),
                ]);
            }
            ConfigFocus::Item => {
                spans.extend([
                    hint_key("[up/dn]"), Span::raw(" Select  "),
                    hint_key("[Enter/\u{2192}]"), Span::raw(" Detail  "),
                    hint_key("[Esc/\u{2190}]"), Span::raw(" Back  "),
                ]);
            }
            ConfigFocus::Detail => {
                spans.extend([
                    hint_key("[up/dn]"), Span::raw(" Scroll  "),
                    hint_key("[Esc/\u{2190}]"), Span::raw(" Back  "),
                    hint_key("[PgUp/Dn]"), Span::raw(" Page  "),
                ]);
            }
        },
    }

    spans.push(Span::styled(
        format!("Config: {}", app.config.total_items()),
        Style::default().fg(Color::DarkGray),
    ));

    let hints = Line::from(spans);
    f.render_widget(Paragraph::new(hints), area);
}

pub fn split_list_detail(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);
    (chunks[0], chunks[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigInventory;
    use crate::test_helpers::test_utils::buffer_to_string;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_app(app: &App, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn draw_does_not_panic_with_default_app() {
        let app = App::new(ConfigInventory::default());
        let _buf = render_app(&app, 80, 24);
    }

    #[test]
    fn draw_shows_tab_title() {
        let app = App::new(ConfigInventory::default());
        let buf = render_app(&app, 80, 24);
        let content = buffer_to_string(&buf);
        assert!(
            content.contains("Claude Code Dashboard"),
            "Expected dashboard title in output"
        );
    }

    #[test]
    fn draw_shows_sessions_tab_label() {
        let app = App::new(ConfigInventory::default());
        let buf = render_app(&app, 80, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Sessions"), "Expected Sessions tab label");
    }

    #[test]
    fn draw_shows_all_tab_labels() {
        let app = App::new(ConfigInventory::default());
        let buf = render_app(&app, 80, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Sessions"));
        assert!(content.contains("Config"));
        assert!(content.contains("Events"));
    }

    #[test]
    fn draw_shows_quit_hint() {
        let app = App::new(ConfigInventory::default());
        let buf = render_app(&app, 80, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("[q]"), "Expected quit hint");
        assert!(content.contains("Quit"), "Expected Quit label");
    }

    #[test]
    fn draw_shows_content_panels_for_each_tab() {
        for tab in [Tab::Sessions, Tab::Config, Tab::Events] {
            let mut app = App::new(ConfigInventory::default());
            app.active_tab = tab;
            let _buf = render_app(&app, 80, 24);
        }
    }

    #[test]
    fn split_list_detail_returns_two_rects() {
        let area = Rect::new(0, 0, 100, 50);
        let (left, right) = split_list_detail(area);
        assert!(left.width > 0);
        assert!(right.width > 0);
        assert_eq!(left.width + right.width, area.width);
    }

    #[test]
    fn draw_handles_small_terminal() {
        let app = App::new(ConfigInventory::default());
        let _buf = render_app(&app, 20, 5);
    }

    #[test]
    fn status_bar_shows_filter_hint_on_events_tab() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        let buf = render_app(&app, 100, 24);
        let content = buffer_to_string(&buf);
        assert!(
            content.contains("[f]") && content.contains("Filter"),
            "Expected '[f] Filter' hint on Events tab, got:\n{}",
            content
        );
    }

    #[test]
    fn status_bar_hides_filter_hint_on_events_detail() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_focus = crate::app::ListDetailFocus::Detail;
        let buf = render_app(&app, 100, 24);
        let content = buffer_to_string(&buf);
        assert!(
            !content.contains("Filter"),
            "Expected no 'Filter' hint on Events Detail focus, got:\n{}",
            content
        );
    }

    #[test]
    fn status_bar_hides_filter_hint_on_other_tabs() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Sessions;
        let buf = render_app(&app, 100, 24);
        let content = buffer_to_string(&buf);
        assert!(
            !content.contains("Filter"),
            "Expected no 'Filter' hint on Sessions tab, got:\n{}",
            content
        );
    }

    #[test]
    fn status_bar_sessions_list_hints() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Sessions;
        app.session_focus = crate::app::SessionFocus::List;
        let buf = render_app(&app, 120, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("[q]") && content.contains("Quit"), "sessions list: missing [q] Quit");
        assert!(content.contains("[Tab]") && content.contains("Switch"), "sessions list: missing [Tab] Switch");
        assert!(content.contains("Select"), "sessions list: missing Select");
        assert!(content.contains("Segments"), "sessions list: missing Segments hint");
        assert!(!content.contains("Filter"), "sessions list: should not show Filter");
        assert!(!content.contains("PgUp"), "sessions list: should not show PgUp/Dn");
    }

    #[test]
    fn status_bar_sessions_segment_hints() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Sessions;
        app.session_focus = crate::app::SessionFocus::Segment;
        let buf = render_app(&app, 120, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Select"), "sessions segment: missing Select");
        assert!(content.contains("Detail"), "sessions segment: missing Detail hint");
        assert!(content.contains("Back"), "sessions segment: missing Back");
        assert!(!content.contains("PgUp"), "sessions segment: should not show PgUp/Dn");
    }

    #[test]
    fn status_bar_sessions_detail_hints() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Sessions;
        app.session_focus = crate::app::SessionFocus::Detail;
        let buf = render_app(&app, 120, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Scroll"), "sessions detail: missing Scroll");
        assert!(content.contains("Back"), "sessions detail: missing Back");
        assert!(content.contains("PgUp"), "sessions detail: missing PgUp/Dn");
    }

    #[test]
    fn status_bar_events_list_hints() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_focus = crate::app::ListDetailFocus::List;
        let buf = render_app(&app, 120, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Select"), "events list: missing Select");
        assert!(content.contains("[Enter]") && content.contains("Detail"), "events list: missing [Enter] Detail");
        assert!(content.contains("[f]") && content.contains("Filter"), "events list: missing [f] Filter");
        assert!(!content.contains("PgUp"), "events list: should not show PgUp/Dn");
    }

    #[test]
    fn status_bar_events_detail_hints() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Events;
        app.event_focus = crate::app::ListDetailFocus::Detail;
        let buf = render_app(&app, 120, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Scroll"), "events detail: missing Scroll");
        assert!(content.contains("[Esc]") && content.contains("Back"), "events detail: missing [Esc] Back");
        assert!(content.contains("PgUp"), "events detail: missing PgUp/Dn");
        assert!(!content.contains("Filter"), "events detail: should not show Filter");
    }

    #[test]
    fn status_bar_config_category_hints() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = crate::app::ConfigFocus::Category;
        let buf = render_app(&app, 120, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Select"), "config category: missing Select");
        assert!(content.contains("Items"), "config category: missing Items label");
        assert!(!content.contains("PgUp"), "config category: should not show PgUp/Dn");
    }

    #[test]
    fn status_bar_config_item_hints() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = crate::app::ConfigFocus::Item;
        let buf = render_app(&app, 120, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Select"), "config item: missing Select");
        assert!(content.contains("Detail"), "config item: missing Detail label");
        assert!(content.contains("Back"), "config item: missing Back");
        assert!(!content.contains("PgUp"), "config item: should not show PgUp/Dn");
    }

    #[test]
    fn status_bar_config_detail_hints() {
        let mut app = App::new(ConfigInventory::default());
        app.active_tab = Tab::Config;
        app.config_focus = crate::app::ConfigFocus::Detail;
        let buf = render_app(&app, 120, 24);
        let content = buffer_to_string(&buf);
        assert!(content.contains("Scroll"), "config detail: missing Scroll");
        assert!(content.contains("Back"), "config detail: missing Back");
        assert!(content.contains("PgUp"), "config detail: missing PgUp/Dn");
        assert!(!content.contains("[Enter]"), "config detail: should not show [Enter]");
    }
}
