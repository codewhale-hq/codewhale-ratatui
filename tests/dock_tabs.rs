use codewhale_ratatui::{DockTabRow, DockTabStyles, DockTabTarget, WorkbarPanel, WorkbarTab};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

fn styles() -> DockTabStyles {
    DockTabStyles {
        idle: Style::default().fg(Color::Rgb(13, 29, 47)),
        active: Style::default()
            .fg(Color::Rgb(61, 79, 97))
            .bg(Color::Rgb(23, 37, 53))
            .add_modifier(Modifier::BOLD),
        hovered: Style::default()
            .fg(Color::Green)
            .bg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED),
        close: Style::default().fg(Color::Cyan),
        close_hovered: Style::default()
            .fg(Color::Yellow)
            .bg(Color::Magenta)
            .add_modifier(Modifier::UNDERLINED),
    }
}
fn seeded() -> Buffer {
    let mut buffer = Buffer::empty(Rect::new(2, 3, 135, 8));
    for cell in &mut buffer.content {
        cell.set_symbol("~").set_style(
            Style::default()
                .bg(Color::Rgb(11, 23, 37))
                .add_modifier(Modifier::ITALIC),
        );
    }
    buffer
}

#[test]
fn native_dock_plan_paints_exact_styles_only_inside_its_action_boxes() {
    let tabs = [
        WorkbarTab::new(WorkbarPanel::Tasks).count(7),
        WorkbarTab::new(WorkbarPanel::Fleet).count(12),
    ];
    let styles = styles();
    let row = DockTabRow {
        tabs: &tabs,
        active: WorkbarPanel::Tasks,
        bottom: true,
        close: " Esc × ".into(),
        hovered: Some(DockTabTarget::Close),
        pressed: Some(WorkbarPanel::Fleet),
        styles,
    };
    let area = Rect::new(7, 5, 80, 3);
    let mut expected = seeded();
    Paragraph::new(Line::from(Span::styled(" Tasks 7 ", styles.active)))
        .render(Rect::new(8, 6, 9, 1), &mut expected);
    Paragraph::new(Line::from(Span::styled(" Fleet 12 ", styles.active)))
        .render(Rect::new(19, 6, 10, 1), &mut expected);
    Paragraph::new(Line::from(Span::styled(" Esc × ", styles.close_hovered)))
        .render(Rect::new(80, 6, 7, 1), &mut expected);
    let mut actual = seeded();
    (&row).render(area, &mut actual);
    assert_eq!(actual, expected);
    assert_eq!(
        row.plan(area).hitboxes(),
        vec![
            (
                DockTabTarget::Panel(WorkbarPanel::Tasks),
                Rect::new(8, 6, 9, 1)
            ),
            (
                DockTabTarget::Panel(WorkbarPanel::Fleet),
                Rect::new(19, 6, 10, 1)
            ),
            (DockTabTarget::Close, Rect::new(80, 6, 7, 1)),
        ]
    );
}

#[test]
fn dock_sheds_counts_then_rightmost_inactive_tabs_and_never_invents_esc() {
    let tabs = WorkbarPanel::ORDER.map(|panel| WorkbarTab::new(panel).count(usize::MAX));
    for active in WorkbarPanel::ORDER {
        for width in 0..=128 {
            let row = DockTabRow {
                tabs: &tabs,
                active,
                bottom: false,
                close: " x ".into(),
                hovered: None,
                pressed: None,
                styles: styles(),
            };
            let area = Rect::new(7, 5, width, 1);
            let plan = row.plan(area);
            assert_eq!(plan.close, " x ");
            let boxes = plan.hitboxes();
            for (_, rect) in &boxes {
                assert_eq!(rect.intersection(area), *rect);
            }
            for pair in boxes.windows(2) {
                assert!(pair[0].1.right() <= pair[1].1.x);
            }
            if width >= 20 {
                assert!(plan.tabs.iter().any(|tab| tab.tab.panel == active));
            }
            if width == 0 {
                assert!(boxes.is_empty());
            }
            if width == 40 {
                assert!(
                    plan.tabs
                        .iter()
                        .all(|tab| !tab.label.contains(char::is_numeric))
                );
            }
        }
    }
}

#[test]
fn missing_active_tab_and_hovered_tab_use_the_same_paint_and_target_plan() {
    let row = DockTabRow {
        tabs: &[],
        active: WorkbarPanel::Context,
        bottom: false,
        close: " × ".into(),
        hovered: Some(DockTabTarget::Panel(WorkbarPanel::Context)),
        pressed: None,
        styles: styles(),
    };
    let area = Rect::new(7, 5, 40, 1);
    let mut actual = seeded();
    (&row).render(area, &mut actual);
    let plan = row.plan(area);
    assert_eq!(plan.tabs.len(), 1);
    assert_eq!(plan.tabs[0].label, " Context ");
    assert!(actual[(8, 5)].modifier.contains(Modifier::BOLD));
    assert!(!actual[(8, 5)].modifier.contains(Modifier::UNDERLINED));
    assert_eq!(
        plan.hitboxes()[0].0,
        DockTabTarget::Panel(WorkbarPanel::Context)
    );
}
