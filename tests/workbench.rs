use codewhale_ratatui::{
    ContextItem, ContextRibbon, Paint, PaneHeader, WorkspaceFrame,
    testing::{Profile, render, text},
};
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn workspace_collapses_the_optional_dock_and_keeps_regions_disjoint() {
    let frame = WorkspaceFrame::new("project");
    let wide = Rect::new(7, 5, 112, 38);
    let areas = frame.areas(wide);
    let side = areas.side.expect("wide workspace has a dock");
    assert!(areas.main.intersection(side).is_empty());
    assert!(areas.main.intersection(areas.footer).is_empty());
    for rect in [areas.main, side, areas.footer] {
        assert_eq!(rect.intersection(wide), rect);
    }
    let narrow = frame.areas(Rect::new(7, 5, 40, 28));
    assert!(narrow.side.is_none());
    assert_eq!(narrow.main.width, 36);
    let huge_dock = WorkspaceFrame {
        side_width: u16::MAX,
        minimum_main_width: 0,
        ..frame
    };
    assert!(huge_dock.areas(Rect::new(0, 0, u16::MAX, 8)).side.is_none());
}

#[test]
fn ribbon_keeps_priority_and_reports_folded_context() {
    let ribbon = ContextRibbon::new(vec![
        ContextItem::new("repo", "reef").priority(3),
        ContextItem::new("route", "selected model").priority(0),
        ContextItem::new("branch", "main").priority(2),
    ]);
    let theme = Profile::DarkTrue.theme();
    let shown = text(&render(27, 1, |area, buf| ribbon.paint(area, buf, &theme)));
    assert!(shown.contains("repo reef"), "{shown}");
    assert!(shown.contains("+2 details"), "{shown}");
    assert!(!shown.contains("selected model"));
    let shown = text(&render(80, 1, |area, buf| ribbon.paint(area, buf, &theme)));
    assert!(shown.find("repo").unwrap() < shown.find("route").unwrap());
    assert!(!shown.contains("details"));
    let tiny = text(&render(10, 1, |area, buf| ribbon.paint(area, buf, &theme)));
    assert!(tiny.contains("+3 details"), "{tiny}");
}

#[test]
fn workbench_text_is_sanitized_and_all_surfaces_stay_in_their_bounds() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        let widgets: Vec<Box<dyn Paint>> = vec![
            Box::new(
                WorkspaceFrame::new("reef\u{202e}\u{1b}")
                    .branch("cafe\u{301}鲸")
                    .footer("safe\u{1b}"),
            ),
            Box::new(PaneHeader::new("reef\u{202e}\u{1b}").meta("meta")),
            Box::new(ContextRibbon::new(vec![ContextItem::new(
                "repo",
                "cafe\u{301}鲸\u{202e}",
            )])),
        ];
        for widget in &widgets {
            for buffer_area in [
                Rect::new(2, 2, 12, 8),
                Rect::new(u16::MAX - 12, u16::MAX - 8, 12, 8),
            ] {
                for (width, height) in [(0, 0), (1, 1), (4, 2), (20, 10)] {
                    let area = Rect::new(
                        buffer_area.x.saturating_add(3),
                        buffer_area.y.saturating_add(2),
                        width,
                        height,
                    );
                    let mut buf = Buffer::empty(buffer_area);
                    for cell in &mut buf.content {
                        cell.set_symbol("z");
                    }
                    let before = buf.clone();
                    widget.paint(area, &mut buf, &theme);
                    let clipped = area.intersection(buf.area);
                    for y in buf.area.y..buf.area.bottom() {
                        for x in buf.area.x..buf.area.right() {
                            if !clipped.contains((x, y).into()) {
                                assert_eq!(
                                    buf[(x, y)],
                                    before[(x, y)],
                                    "{} wrote outside requested area",
                                    profile.name()
                                );
                            }
                        }
                    }
                    assert!(!text(&buf).contains(['\u{202e}', '\u{1b}']));
                }
            }
        }
    }
}
