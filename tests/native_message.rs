use codewhale_ratatui::{
    Message, Paint, Role, glyphs,
    testing::{Profile, render, text},
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

#[test]
fn native_message_puts_body_beside_the_marker_and_uses_dim_continuations() {
    let theme = Profile::DarkTrue.theme().tui();
    let message = Message::native("First line.\n\nSecond line.");
    assert_eq!(message.height(20, &theme), 3);
    let buf = render(20, 3, |area, buf| message.paint(area, buf, &theme));
    assert_eq!(text(&buf), "● First line.\n▏\n▏ Second line.");
    assert_eq!(buf[(0, 0)].fg, theme.fg(Role::Primary).fg.unwrap());
    assert!(buf[(0, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(0, 1)].fg, theme.fg(Role::Dim).fg.unwrap());
    assert_eq!(buf[(2, 2)].fg, theme.fg(Role::Foreground).fg.unwrap());
}

#[test]
fn authored_heading_remains_available_and_markers_set_the_native_indent() {
    let theme = Profile::DarkTrue.theme();
    let heading = Message::new("Codewhale", "Body");
    let buf = render(24, heading.height(24, &theme), |area, buf| {
        heading.paint(area, buf, &theme)
    });
    assert_eq!(text(&buf), "● Codewhale\n▏ Body");
    let native = Message::native("First\nSecond").marker(">>");
    let buf = render(24, native.height(24, &theme), |area, buf| {
        native.paint(area, buf, &theme)
    });
    assert_eq!(text(&buf), ">> First\n▏  Second");
}

#[test]
fn empty_assistant_stream_fragments_do_not_leave_a_ghost_marker() {
    let theme = Profile::DarkTrue.theme();
    for content in ["", " \n\t\r\n", "\u{202e}\u{2066}\u{1b}\n"] {
        let message = Message::native(content);
        assert_eq!(message.height(40, &theme), 0);
        let mut buf = Buffer::filled(Rect::new(0, 0, 40, 3), ratatui::buffer::Cell::new("z"));
        let before = buf.clone();
        message.paint(buf.area, &mut buf, &theme);
        assert_eq!(buf, before);
    }
    // An intentional user entry still has its mark, and real prose that starts
    // with a blank logical line retains that line and the continuation rail.
    let user = Message::native("").marker(glyphs::USER);
    let buf = render(12, user.height(12, &theme), |area, buf| {
        user.paint(area, buf, &theme)
    });
    assert_eq!(text(&buf), "▎");
    let leading = Message::native("\nBody");
    let buf = render(12, leading.height(12, &theme), |area, buf| {
        leading.paint(area, buf, &theme)
    });
    assert_eq!(text(&buf), "●\n▏ Body");
}

#[test]
fn native_wrapping_reserves_source_gutter_and_preserves_unicode_and_lines() {
    let theme = Profile::DarkTrue.theme();
    let message = Message::native("one two three");
    // message.rs reserves marker width + two before wrapping; decoration
    // itself uses marker width + one, leaving the native trailing slack.
    assert_eq!(message.height(12, &theme), 2);
    let buf = render(12, 2, |area, buf| message.paint(area, buf, &theme));
    assert_eq!(text(&buf), "● one two\n▏ three");
    let unicode = Message::native("  cafe\u{301} 鲸鱼\n\nlast\u{202e}\u{1b}");
    let buf = render(14, unicode.height(14, &theme), |area, buf| {
        unicode.paint(area, buf, &theme)
    });
    assert_eq!(text(&buf), "●   cafe\u{301} 鲸鱼\n▏\n▏ last");
    assert!(!text(&buf).contains(['\u{202e}', '\u{1b}']));
}

#[test]
fn ascii_only_changes_native_chrome_and_no_color_keeps_words() {
    for (profile, expected) in [
        (Profile::Ascii, ". cafe\u{301} 鲸鱼\n| second"),
        (Profile::NoColor, "● cafe\u{301} 鲸鱼\n▏ second"),
    ] {
        let theme = profile.theme();
        let message = Message::native("cafe\u{301} 鲸鱼\nsecond");
        let buf = render(24, message.height(24, &theme), |area, buf| {
            message.paint(area, buf, &theme)
        });
        assert_eq!(text(&buf), expected);
        assert!(buf.content.iter().all(|cell| {
            cell.fg == ratatui::style::Color::Reset && cell.bg == ratatui::style::Color::Reset
        }));
    }
}

#[test]
fn native_messages_clip_at_tiny_offset_and_maximum_coordinate_viewports() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [0, 1, 2, 4, 12, 40] {
            for height in [0, 1, 2, 5] {
                for (bounds, area) in [
                    (Rect::new(2, 2, 12, 8), Rect::new(4, 3, width, height)),
                    (
                        Rect::new(u16::MAX - 5, u16::MAX - 5, 5, 5),
                        Rect::new(u16::MAX - 3, u16::MAX - 4, width, height),
                    ),
                ] {
                    let mut buf = Buffer::empty(bounds);
                    for cell in &mut buf.content {
                        cell.set_symbol("z");
                    }
                    let before = buf.clone();
                    Message::native("cafe\u{301} 鲸鱼👩‍💻 and prose\nsecond\nthird")
                        .paint(area, &mut buf, &theme);
                    let clipped = area.intersection(bounds);
                    for y in bounds.y..bounds.bottom() {
                        for x in bounds.x..bounds.right() {
                            if !clipped.contains((x, y).into()) {
                                assert_eq!(buf[(x, y)], before[(x, y)], "{}", profile.name());
                            }
                        }
                    }
                }
            }
        }
        let message = Message::native("first\nsecond\nthird");
        let buf = render(20, 1, |area, buf| message.paint(area, buf, &theme));
        assert_eq!(
            text(&buf),
            format!("{} first", glyphs::pick(glyphs::CURRENT, theme.ascii()))
        );
        assert_eq!(message.height(0, &theme), 0);
        // The final representable column receives the end of a full-width
        // heading; native wrapping still retains its deliberate slack cell.
        let bounds = Rect::new(u16::MAX - 8, u16::MAX - 2, 8, 2);
        let mut buf = Buffer::empty(bounds);
        Message::new("abcdef", "body").paint(bounds, &mut buf, &theme);
        assert_eq!(buf[(u16::MAX - 1, u16::MAX - 2)].symbol(), "f");
        let mut buf = Buffer::empty(bounds);
        Message::native("abcdef").paint(bounds, &mut buf, &theme);
        assert_eq!(
            text(&buf),
            format!(
                "{} abcde\n{}f",
                glyphs::pick(glyphs::CURRENT, theme.ascii()),
                glyphs::pick(glyphs::TRANSCRIPT_RAIL, theme.ascii()),
            )
        );
    }
}
