use codewhale_ratatui::{
    AttentionItem, AttentionQueue, AttentionWords, KeyHint, KeyHints, Paint, State, glyphs,
    testing::{self, Profile, render, text},
};
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn priorities_are_stable_and_selection_uses_the_original_item_index() {
    let theme = Profile::DarkTrue.theme();
    let items = [
        AttentionItem::new("First", "builder", "First request", State::NeedsYou).priority(10),
        AttentionItem::new("Second", "reviewer", "Second request", State::Failed).priority(80),
        AttentionItem::new("Third", "verifier", "Third request", State::Unknown).priority(80),
    ];
    let queue = AttentionQueue::new(&items).selected(0).focused(true);
    assert_eq!(queue.order(), [1, 2, 0]);
    let buf = render(92, queue.height(92, &theme), |area, buf| {
        queue.paint(area, buf, &theme)
    });
    let shown = text(&buf);
    assert!(shown.find("Second").unwrap() < shown.find("Third").unwrap());
    assert!(shown.find("Third").unwrap() < shown.find("First").unwrap());
    let first = shown
        .lines()
        .find(|line| line.contains("First / builder"))
        .unwrap();
    assert!(first.starts_with(glyphs::SELECTION), "{shown}");
    assert!(shown.contains("Failed") && shown.contains("Unknown") && shown.contains("Needs you"));
}

#[test]
fn selected_shortcuts_are_quiet_until_a_request_is_selected() {
    let hints = KeyHints::new(vec![KeyHint::new("Enter", "open request")]);
    let theme = Profile::DarkTrue.theme();
    let item = AttentionItem::new(
        "Project",
        "Reviewer",
        "Choose the next step.",
        State::NeedsYou,
    )
    .action("Review change")
    .hints(&hints);
    let buf = render(80, item.height(80, &theme), |area, buf| {
        item.paint(area, buf, &theme)
    });
    assert!(text(&buf).contains("Review change"));
    assert!(!text(&buf).contains("Enter"));
    let item = item.selected(true).focused(true);
    let buf = render(80, item.height(80, &theme), |area, buf| {
        item.paint(area, buf, &theme)
    });
    assert!(text(&buf).contains("Enter open request"));
}

#[test]
fn clipped_scope_or_request_withholds_the_action_and_says_to_open_details() {
    let hints = KeyHints::new(vec![KeyHint::new("y", "allow once")]);
    let theme = Profile::DarkTrue.theme();
    for item in [
        AttentionItem::new(
            "Project",
            "Agent",
            "Read every line of this very long request before deciding what to permit.",
            State::NeedsYou,
        )
        .request_rows(1),
        AttentionItem::new(
            "An unusually long project identity",
            "Agent",
            "Short request",
            State::NeedsYou,
        ),
    ] {
        let item = item.action("Allow once").hints(&hints).selected(true);
        let buf = render(24, item.height(24, &theme), |area, buf| {
            item.paint(area, buf, &theme)
        });
        let shown = text(&buf);
        assert!(
            !shown.contains("Allow once") && !shown.contains("allow once"),
            "{shown}"
        );
        assert!(shown.contains("Open the full"), "{shown}");
    }
    let item = AttentionItem::new("Project", "Agent", "A short request", State::NeedsYou)
        .action("Allow once")
        .hints(&hints)
        .selected(true);
    let buf = render(80, 2, |area, buf| item.paint(area, buf, &theme));
    assert!(!text(&buf).contains("Allow once"));
    assert!(text(&buf).contains("Open the full"));
}

#[test]
fn overflow_counts_requests_and_keeps_the_selected_request_visible() {
    let theme = Profile::DarkTrue.theme();
    let items: Vec<_> = ["One", "Two", "Three"]
        .into_iter()
        .map(|project| {
            AttentionItem::new(project, "Agent", "Choose a next step.", State::NeedsYou)
                .action("Review request")
        })
        .collect();
    let queue = AttentionQueue::new(&items);
    let buf = render(80, 7, |area, buf| queue.paint(area, buf, &theme));
    assert!(text(&buf).contains("2 not fully shown"));
    let queue = queue.selected(2).focused(true);
    let buf = render(80, 7, |area, buf| queue.paint(area, buf, &theme));
    let shown = text(&buf);
    assert!(shown.contains("Three / Agent"), "{shown}");
    assert!(shown.contains("2 not fully shown"), "{shown}");
}

#[test]
fn caller_copy_is_localized_safe_and_keeps_authored_unicode_lines() {
    let words = AttentionWords {
        title: "Braucht dich\u{202e}".into(),
        requests: "Anfragen".into(),
        hidden: "Anfragen nicht vollständig sichtbar".into(),
        clipped: "Vollständige Anfrage öffnen".into(),
        ..AttentionWords::default()
    };
    let theme = Profile::DarkTrue.theme();
    let items = [AttentionItem::new(
        "鲸鱼\u{202e}",
        "cafe\u{301}",
        "第一行\n\nLast line\u{1b}",
        State::NeedsYou,
    )
    .status_word("Braucht dich\u{202e}")
    .timestamp("vor 2m\u{2066}")
    .action("Öffnen\u{202e}")
    .with_words(&words)];
    let queue = AttentionQueue::new(&items).with_words(&words);
    let buf = render(80, queue.height(80, &theme), |area, buf| {
        queue.paint(area, buf, &theme)
    });
    let shown = text(&buf);
    assert!(shown.contains("鲸鱼 / cafe\u{301}"));
    assert!(shown.contains("第一行") && shown.contains("Last line") && shown.contains("Öffnen"));
    assert!(shown.contains("Braucht dich") && shown.contains("1 Anfragen"));
    assert!(!shown.contains(['\u{202e}', '\u{2066}', '\u{1b}']));
    let blank = shown.lines().position(|line| line.trim() == "▏").unwrap();
    assert!(shown.lines().nth(blank - 1).unwrap().contains("第一行"));
    assert!(shown.lines().nth(blank + 1).unwrap().contains("Last line"));
}

#[test]
fn empty_and_selected_queues_follow_every_profile_and_frame_rule() {
    let items = [
        AttentionItem::new("Project", "Agent", "Choose the next step.", State::NeedsYou)
            .action("Review request"),
    ];
    testing::assert_rules(8, |area, buf, theme| {
        AttentionQueue::new(&items)
            .selected(0)
            .focused(true)
            .paint(area, buf, theme)
    });
    testing::assert_rules(10, |area, buf, theme| {
        AttentionQueue::new(&[]).paint(area, buf, theme)
    });
    for profile in Profile::ALL {
        let theme = profile.theme();
        let buf = render(64, 10, |area, buf| {
            AttentionQueue::new(&[]).paint(area, buf, &theme)
        });
        assert!(text(&buf).contains("No requests waiting"));
    }
}

#[test]
fn tiny_widths_and_partial_buffers_never_write_outside_the_owned_area() {
    let hints = KeyHints::new(vec![KeyHint::new("Enter", "open")]);
    let items = [AttentionItem::new(
        "鲸鱼",
        "Agent",
        "cafe\u{301} 👩\u{200d}💻\nsecond",
        State::NeedsYou,
    )
    .action("Open")
    .hints(&hints)];
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [0, 1, 2, 4, 12, 80] {
            for height in [0, 1, 2, 8] {
                for origin in [(0, 0), (9, 7), (30, 30)] {
                    let requested = Rect::new(origin.0, origin.1, width, height);
                    let queue = AttentionQueue::new(&items).selected(0).focused(true);
                    for component in [&queue as &dyn Paint, &items[0] as &dyn Paint] {
                        let mut buf = Buffer::empty(Rect::new(5, 4, 12, 8));
                        for cell in &mut buf.content {
                            cell.set_symbol("z");
                        }
                        let before = buf.clone();
                        component.paint(requested, &mut buf, &theme);
                        let clipped = requested.intersection(buf.area);
                        for y in buf.area.y..buf.area.bottom() {
                            for x in buf.area.x..buf.area.right() {
                                if !clipped.contains((x, y).into()) {
                                    assert_eq!(
                                        buf[(x, y)],
                                        before[(x, y)],
                                        "{} at {width}x{height}",
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
}

#[test]
fn saturated_coordinates_clip_long_unicode_status_footer_and_empty_copy() {
    let words = AttentionWords {
        title: "鲸鱼 inbox with a long localized heading".into(),
        hidden: "鲸鱼 requests not fully shown with long localized details".into(),
        empty_title: "鲸鱼 no requests with a long localized title".into(),
        empty_detail:
            "cafe\u{301} 鲸鱼 a long detail sentence that should wrap safely at the terminal edge"
                .into(),
        ..AttentionWords::default()
    };
    let hints = KeyHints::new(vec![KeyHint::new("Enter", "open full request")]);
    let items = [
        AttentionItem::new(
            "鲸鱼",
            "Reviewer",
            "cafe\u{301} 鲸鱼\nFull request",
            State::NeedsYou,
        )
        .status_word("Needs you with long localized state words 鲸鱼")
        .timestamp("just now with long localized metadata 鲸鱼")
        .action("Review request")
        .hints(&hints)
        .selected(true),
        AttentionItem::new("Second", "Agent", "Another request", State::NeedsYou),
    ];
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [1, 4, 80] {
            for height in [1, 2, 10] {
                let bounds = Rect::new(u16::MAX - 90, u16::MAX - 12, 90, 12);
                let requested = Rect::new(u16::MAX - width, u16::MAX - height, width, height);
                let queue = AttentionQueue::new(&items).selected(0).with_words(&words);
                let empty = AttentionQueue::new(&[]).with_words(&words);
                for component in [
                    &queue as &dyn Paint,
                    &empty as &dyn Paint,
                    &items[0] as &dyn Paint,
                ] {
                    let mut buf = Buffer::empty(bounds);
                    for cell in &mut buf.content {
                        cell.set_symbol("z");
                    }
                    let before = buf.clone();
                    component.paint(requested, &mut buf, &theme);
                    for y in bounds.y..bounds.bottom() {
                        for x in bounds.x..bounds.right() {
                            if !requested.contains((x, y).into()) {
                                assert_eq!(
                                    buf[(x, y)],
                                    before[(x, y)],
                                    "{} {width}x{height}",
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
