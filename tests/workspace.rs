use codewhale_ratatui::{
    AgentCard, Composer, Fleet, KeyHint, KeyHints, Message, Paint, State, ToolCard,
    testing::{Profile, render, text},
};
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn messages_keep_authored_lines_and_unicode_while_stripping_controls() {
    let theme = Profile::DarkTrue.theme();
    let message = Message::new("H\u{202e}unter", "first\n\ncafe\u{301} 鲸鱼\nlast\u{1b}");
    let buf = render(24, message.height(24, &theme), |area, buf| {
        message.paint(area, buf, &theme)
    });
    let shown = text(&buf);
    assert!(shown.contains("Hunter"));
    assert!(shown.contains("cafe\u{301} 鲸鱼"));
    assert_eq!(shown.lines().count(), 5);
    assert!(!shown.contains(['\u{202e}', '\u{1b}']));
}

#[test]
fn tool_output_omission_counts_source_lines_including_partly_visible_lines() {
    let theme = Profile::DarkTrue.theme();
    let card = ToolCard::new("read", "source.rs", State::Done)
        .output("abcdefghijk\nsecond\nthird")
        .omitted_lines(2);
    // The raised truecolor card has two cells of gutter. Its subject wraps
    // into two rows, so the title and subject leave only two output rows:
    // one row of the first logical line, and the count rail. All three
    // supplied lines are not fully shown, plus the host's two prior omissions.
    let buf = render(10, 5, |area, buf| card.paint(area, buf, &theme));
    let shown = text(&buf);
    assert!(
        shown.lines().last().unwrap().trim_start().starts_with("5 "),
        "{shown}"
    );
    // All three supplied lines fit at a wider width; only the two lines
    // already omitted by the host remain in the footer.
    let height = card.height(60, &theme);
    let buf = render(60, height, |area, buf| card.paint(area, buf, &theme));
    assert!(text(&buf).contains("2 output lines not fully shown"));
}

#[test]
fn composer_keeps_text_when_only_one_row_fits_and_wraps_hints() {
    let theme = Profile::DarkTrue.theme();
    let hints = KeyHints::new(vec![
        KeyHint::new("Enter", "send"),
        KeyHint::new("Esc", "cancel"),
    ]);
    let composer = Composer::new("Write the summary")
        .context("repo / branch")
        .hints(&hints);
    let buf = render(24, 1, |area, buf| composer.paint(area, buf, &theme));
    assert!(text(&buf).contains("Write the summary"));
    let height = composer.height(12, &theme);
    let buf = render(12, height, |area, buf| composer.paint(area, buf, &theme));
    let shown = text(&buf);
    assert!(shown.contains("Enter send"));
    assert!(shown.contains("Esc cancel"));
}

#[test]
fn a_fleet_shows_each_agents_own_data_and_clipped_count() {
    let theme = Profile::DarkTrue.theme();
    let fleet = Fleet::new(vec![
        AgentCard::new("Builder", State::Working)
            .route("route A")
            .task("Own task A"),
        AgentCard::new("Reviewer", State::NeedsYou)
            .route("route B")
            .task("Own task B"),
    ]);
    let buf = render(50, fleet.height(50, &theme), |area, buf| {
        fleet.paint(area, buf, &theme)
    });
    let shown = text(&buf);
    assert!(shown.contains("Builder  Working"));
    assert!(shown.contains("Reviewer  Needs you"));
    assert!(shown.contains("route A") && shown.contains("route B"));
    assert!(shown.contains("Own task A") && shown.contains("Own task B"));
    let buf = render(50, 3, |area, buf| fleet.paint(area, buf, &theme));
    assert!(text(&buf).contains("2 agents not fully shown"));
}

#[test]
fn every_workspace_surface_is_bounded_at_degenerate_sizes_and_buffer_edges() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [0, 1, 4, 20] {
            for height in [0, 1, 2, 8] {
                let area = Rect::new(4, 3, width, height);
                let surfaces: Vec<Box<dyn Paint>> = vec![
                    Box::new(Message::new("You", "cafe\u{301} 鲸鱼\nsecond")),
                    Box::new(
                        ToolCard::new("run", "verify", State::Unknown)
                            .output("鲸鱼\nsecond")
                            .omitted_lines(1),
                    ),
                    Box::new(Composer::new("鲸鱼\nsecond").context("repo / branch")),
                    Box::new(AgentCard::new("Worker", State::Working).task("鲸鱼\nsecond")),
                    Box::new(Fleet::new(vec![
                        AgentCard::new("Worker", State::NeedsYou).task("Own task"),
                    ])),
                ];
                for surface in surfaces {
                    let mut buf = Buffer::empty(Rect::new(2, 2, 10, 6));
                    for cell in &mut buf.content {
                        cell.set_symbol("z");
                    }
                    let before = buf.clone();
                    surface.paint(area, &mut buf, &theme);
                    let clipped = area.intersection(buf.area);
                    for y in buf.area.y..buf.area.bottom() {
                        for x in buf.area.x..buf.area.right() {
                            if !clipped.contains((x, y).into()) {
                                assert_eq!(
                                    buf[(x, y)],
                                    before[(x, y)],
                                    "{} at {width}x{height}: wrote outside area",
                                    profile.name()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
