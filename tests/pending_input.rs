use codewhale_ratatui::{
    ContextPreviewItem, ContextPreviewState, Paint, PendingInputAction, PendingInputItem,
    PendingInputPreview, PendingInputStatus,
    testing::{self, Profile, render},
};
use ratatui::layout::Rect;

fn fixture() -> PendingInputPreview<'static> {
    PendingInputPreview::new(
        vec![
            PendingInputItem::new("q-1", "First queued message", PendingInputStatus::Queued),
            PendingInputItem::new("s-1", "A steering note", PendingInputStatus::Steering),
            PendingInputItem::new("p-1", "A paused follow-up", PendingInputStatus::Paused),
            PendingInputItem::new("e-1", "An editing follow-up", PendingInputStatus::Editing),
            PendingInputItem::new("f-1", "Already on its way", PendingInputStatus::InFlight),
        ],
        vec![
            ContextPreviewItem::new("c-1", "notes.md", ContextPreviewState::Included),
            ContextPreviewItem::new("c-2", "draft.md", ContextPreviewState::Unconfirmed),
            ContextPreviewItem::new("c-3", "old.log", ContextPreviewState::Removable),
        ],
    )
    .selected("q-1")
}

#[test]
fn pending_input_keeps_the_rules_in_every_profile_and_width() {
    testing::assert_rules(6, |area, buf, theme| {
        fixture().paint(area, buf, theme);
    });
}

#[test]
fn an_empty_preview_asks_for_zero_rows_and_paints_nothing() {
    let empty = PendingInputPreview::new(Vec::new(), Vec::new());
    for profile in Profile::ALL {
        assert_eq!(Paint::height(&empty, 40, &profile.theme()), 0);
    }
    let theme = Profile::DarkTrue.theme();
    let buf = render(40, 3, |area, buf| empty.paint(area, buf, &theme));
    assert!(testing::text(&buf).trim().is_empty());
}

#[test]
fn the_one_row_narrow_view_keeps_an_action_discoverable() {
    let theme = Profile::DarkTrue.theme();
    let buf = render(40, 1, |area, buf| fixture().paint(area, buf, &theme));
    let shown = testing::text(&buf);
    assert!(shown.contains("Send now"), "{shown}");
    assert!(codewhale_ratatui::text::width(&shown) <= 40);
}

#[test]
fn status_words_and_context_states_are_explicit() {
    let theme = Profile::DarkTrue.theme();
    let buf = render(80, 8, |area, buf| fixture().paint(area, buf, &theme));
    let shown = testing::text(&buf);
    for status in PendingInputStatus::ALL {
        assert!(
            shown.contains(status.word()),
            "missing {}: {shown}",
            status.word()
        );
    }
    for state in ContextPreviewState::ALL {
        assert!(
            shown.contains(state.word()),
            "missing {}: {shown}",
            state.word()
        );
    }
    // No false delivered or verified claim anywhere.
    for banned in ["sent", "delivered", "verified", "failed"] {
        assert!(!shown.contains(banned), "{banned} in {shown}");
    }
}

#[test]
fn actions_are_metadata_and_in_flight_items_offer_none() {
    let preview = fixture();
    assert_eq!(
        preview.actions_for("q-1"),
        Some(PendingInputAction::ALL.as_slice())
    );
    assert_eq!(preview.selected_index(), Some(0));
    assert!(
        preview
            .actions_for("f-1")
            .is_some_and(|actions| actions.is_empty())
    );
    assert!(preview.actions_for("absent").is_none());
    assert_eq!(preview.actions(), PendingInputAction::ALL.to_vec());
    assert_eq!(preview.pending_count(), 5);
}

#[test]
fn caller_text_is_sanitized_before_it_reaches_the_cells() {
    let item = PendingInputItem::new(
        "x",
        "rm -rf ~/\u{202E}txt.exe\u{1b}[31m\u{9b}31m",
        PendingInputStatus::Queued,
    );
    let preview = PendingInputPreview::new(vec![item], Vec::new());
    let theme = Profile::DarkTrue.theme();
    let buf = render(60, 3, |area, buf| preview.paint(area, buf, &theme));
    let shown = testing::text(&buf);
    assert!(shown.contains("rm -rf ~/txt.exe"), "{shown}");
    assert!(!shown.contains('\u{202e}'));
    assert!(!shown.contains('\u{1b}'));
    assert!(!shown.contains('\u{9b}'));
}

#[test]
fn cjk_and_long_lines_stay_inside_the_requested_width() {
    let preview = PendingInputPreview::new(
        vec![PendingInputItem::new(
            "cjk",
            "鲸鱼".repeat(30),
            PendingInputStatus::Queued,
        )],
        Vec::new(),
    );
    let theme = Profile::DarkTrue.theme();
    for width in [12u16, 24, 40] {
        let buf = render(width, 4, |area, buf| preview.paint(area, buf, &theme));
        for line in testing::text(&buf).lines() {
            assert!(
                codewhale_ratatui::text::width(line) <= usize::from(width),
                "{width}: {line}"
            );
        }
    }
}

#[test]
fn painting_outside_the_buffer_area_writes_nothing() {
    let preview = fixture();
    let theme = Profile::DarkTrue.theme();
    let buf = render(20, 4, |_, buf| {
        preview.paint(Rect::new(28, 9, 40, 6), buf, &theme);
    });
    assert!(testing::text(&buf).trim().is_empty());
}

#[test]
fn requested_height_bounds_the_painted_rows_and_the_rest_is_counted() {
    let preview = fixture();
    let theme = Profile::DarkTrue.theme();
    let buf = render(40, 5, |area, buf| preview.paint(area, buf, &theme));
    let shown = testing::text(&buf);
    assert!(shown.lines().count() <= 5);
    assert!(shown.contains("+5 more"), "{shown}");
    assert!(shown.contains("Send now"));
}

#[test]
fn row_limit_and_multiline_summary_keep_the_callers_intent() {
    let theme = Profile::DarkTrue.theme();
    let item = PendingInputItem::new("id", "first\nsecond", PendingInputStatus::Queued);
    let preview = PendingInputPreview::new(vec![item], vec![]).max_rows(0);
    let buf = render(60, 8, |area, buf| preview.paint(area, buf, &theme));
    assert!(testing::text(&buf).trim().is_empty());
    let preview = preview.max_rows(3);
    let buf = render(60, 8, |area, buf| preview.paint(area, buf, &theme));
    assert!(testing::text(&buf).contains("first second"));
    assert!(
        testing::text(&buf)
            .lines()
            .skip(3)
            .all(|line| line.trim().is_empty())
    );
}

fn native_fixture() -> codewhale_ratatui::PendingCard<'static> {
    use codewhale_ratatui::{PendingCard, PendingCardContext, PendingCardWords};
    let mut card = PendingCard::new(PendingCardWords::default());
    card.context.push(PendingCardContext {
        kind: "image".into(),
        label: "whale.png".into(),
        detail: Some("attached".into()),
        included: true,
        removable: true,
        selected: true,
    });
    card.sending.push("Please continue".into());
    card.queued.push("A follow-up".into());
    card
}

#[test]
fn native_card_keeps_profiles_and_height_on_the_shared_plan() {
    testing::assert_rules(8, |area, buf, theme| {
        native_fixture().paint(area, buf, theme)
    });
    let theme = Profile::DarkTrue.theme();
    for width in [0, 1, 2, 3] {
        let card = native_fixture();
        assert_eq!(card.height(width, &theme), 0);
        let buf = render(width, 5, |area, buf| card.paint(area, buf, &theme));
        assert!(testing::text(&buf).trim().is_empty());
    }
    let empty = codewhale_ratatui::PendingCard::new(Default::default());
    assert_eq!(empty.height(40, &theme), 0);
}

#[test]
fn native_localized_queue_copy_and_child_priority_survive_the_one_row_floor() {
    use codewhale_ratatui::{PendingCard, PendingCardWords};
    let theme = Profile::DarkTrue.theme();
    let mut card = PendingCard::new(PendingCardWords {
        queued_many_prefix: "共{count}条，下个：".into(),
        compact_controls: "发送 / 修改 / 丢弃".into(),
        ..Default::default()
    });
    card.queued
        .extend(["鲸鱼 e\u{301}".into(), "下一条".into()]);
    assert_eq!(card.height(60, &theme), 2);
    let two = render(60, 2, |area, buf| card.paint(area, buf, &theme));
    let shown = testing::text(&two);
    assert!(shown.contains("共2条，下个：鲸鱼 e\u{301}"), "{shown}");
    assert!(shown.contains("发送 / 修改 / 丢弃"), "{shown}");
    let one = render(60, 1, |area, buf| card.paint(area, buf, &theme));
    assert!(testing::text(&one).contains("发送 / 修改 / 丢弃"));
    assert!(!testing::text(&one).contains("鲸鱼"));
    card.priority_rows
        .extend(["子任务需要你 /agents".into(), "另一个任务需要你".into()]);
    assert_eq!(card.height(60, &theme), 4);
    let one = render(60, 1, |area, buf| card.paint(area, buf, &theme));
    assert!(testing::text(&one).contains("子任务需要你 /agents"));
    assert!(!testing::text(&one).contains("发送 / 修改"));
    let two = render(60, 2, |area, buf| card.paint(area, buf, &theme));
    assert!(testing::text(&two).contains("另一个任务需要你"));
}

#[test]
fn native_context_flags_and_exact_host_styles_remain_independent() {
    use codewhale_ratatui::{PendingCard, PendingCardContext, PendingCardStyles};
    use ratatui::style::{Color, Modifier, Style};
    let theme = Profile::DarkTrue.theme();
    let rgb = Color::Rgb;
    let styles = PendingCardStyles {
        input: Style::default()
            .fg(rgb(1, 2, 3))
            .add_modifier(Modifier::DIM),
        warning: Style::default().fg(rgb(4, 5, 6)),
        context_muted: Style::default().fg(rgb(7, 8, 9)),
        context_label: Style::default().fg(rgb(10, 11, 12)),
        selected: Style::default().fg(rgb(13, 14, 15)).bg(rgb(16, 17, 18)),
    };
    for included in [false, true] {
        for removable in [false, true] {
            for selected in [false, true] {
                let mut card = PendingCard::new(Default::default());
                card.styles = Some(styles);
                card.context.push(PendingCardContext {
                    kind: "image".into(),
                    label: "a.png".into(),
                    detail: Some("attached".into()),
                    included,
                    removable,
                    selected,
                });
                let buf = render(96, 3, |area, buf| card.paint(area, buf, &theme));
                let shown = testing::text(&buf);
                assert!(shown.contains("[image] a.png · attached"), "{shown}");
                assert_eq!(shown.contains("Backspace/Delete removes"), selected);
                assert_eq!(shown.contains("· removable"), !selected && removable);
                assert_eq!(buf[(2, 1)].symbol(), if selected { "▸" } else { "↳" });
                assert_eq!(
                    buf[(2, 1)].fg,
                    if selected {
                        rgb(13, 14, 15)
                    } else if included {
                        rgb(7, 8, 9)
                    } else {
                        rgb(4, 5, 6)
                    }
                );
                assert_eq!(
                    buf[(4, 1)].fg,
                    if selected {
                        rgb(13, 14, 15)
                    } else if included {
                        rgb(10, 11, 12)
                    } else {
                        rgb(7, 8, 9)
                    }
                );
                if selected {
                    assert_eq!(buf[(4, 1)].bg, rgb(16, 17, 18));
                    assert!(buf[(2, 1)].modifier.contains(Modifier::BOLD));
                    assert!(!buf[(4, 1)].modifier.contains(Modifier::BOLD));
                }
            }
        }
    }
}

#[test]
fn native_wrapped_body_caps_overflow_without_fragmenting_long_tokens() {
    use codewhale_ratatui::{PendingCard, PendingCardWords};
    let theme = Profile::DarkTrue.theme();
    let mut card = PendingCard::new(PendingCardWords {
        sending_prefix: "发送：".into(),
        ..Default::default()
    });
    card.sending.push("one\ntwo\nthree\nfour\nfive".into());
    assert_eq!(card.height(40, &theme), 5);
    let shown = testing::text(&render(40, 5, |area, buf| card.paint(area, buf, &theme)));
    assert!(shown.contains("发送：one"));
    assert!(shown.contains("three"));
    assert!(!shown.contains("four"));
    assert!(shown.lines().last().unwrap().trim() == "…");
    card.sending =
        vec!["https://example.test/a/very/long/indivisible/token/that/stays/on/one/row".into()];
    assert_eq!(card.height(12, &theme), 2);
    let shown = testing::text(&render(12, 4, |area, buf| card.paint(area, buf, &theme)));
    assert!(!shown.contains('…'), "long token should clip once: {shown}");
    card.words.sending_prefix = "".into();
    for body in ["① ① ①", "1\u{20e3} 1\u{20e3} 1\u{20e3}"] {
        card.sending = vec![body.into()];
        assert_eq!(
            card.height(4, &theme),
            4,
            "native wrapping must keep the two-cell terminal contract: {body}"
        );
    }
}

#[test]
fn native_sanitization_is_measured_once_and_clipped_paint_preserves_other_cells() {
    use codewhale_ratatui::{PendingCard, PendingCardWords};
    use ratatui::{
        buffer::Buffer,
        style::{Color, Modifier, Style},
    };
    let theme = Profile::DarkTrue.theme();
    let mut unsafe_card = PendingCard::new(PendingCardWords {
        sending_prefix: "发送：".into(),
        ..Default::default()
    });
    unsafe_card
        .sending
        .push("one\t two\nthree\u{202e}\u{0007}".into());
    let mut safe_card = unsafe_card.clone();
    safe_card.sending = vec!["one two\nthree".into()];
    assert_eq!(unsafe_card.height(18, &theme), safe_card.height(18, &theme));
    let draw = |card: &PendingCard<'_>| {
        let mut buf = Buffer::empty(Rect::new(3, 5, 24, 8));
        buf.set_style(
            buf.area,
            Style::default()
                .bg(Color::Rgb(41, 42, 43))
                .add_modifier(Modifier::UNDERLINED),
        );
        let before = buf.clone();
        card.paint(Rect::new(3, 5, 18, 3), &mut buf, &theme);
        for y in 5..13 {
            for x in 3..27 {
                if x >= 21 || y >= 8 {
                    assert_eq!(buf[(x, y)], before[(x, y)]);
                }
            }
        }
        buf
    };
    assert_eq!(draw(&unsafe_card), draw(&safe_card));
    let mut buf = Buffer::empty(Rect::new(3, 5, 24, 8));
    let before = buf.clone();
    unsafe_card.paint(Rect::new(40, 50, 10, 4), &mut buf, &theme);
    assert_eq!(buf, before);

    let mut wide = PendingCard::new(PendingCardWords {
        sending_prefix: "".into(),
        ..Default::default()
    });
    wide.sending.push("你好".into());
    let mut buf = Buffer::empty(Rect::new(3, 5, 4, 2));
    buf.set_style(
        buf.area,
        Style::default()
            .bg(Color::Rgb(41, 42, 43))
            .add_modifier(Modifier::UNDERLINED),
    );
    let before = buf.clone();
    wide.paint(buf.area, &mut buf, &theme);
    assert_eq!(buf[(3, 6)].symbol(), "你");
    assert_eq!(buf[(5, 6)].symbol(), "好");
    // Paragraph preserves the native host's prior hidden-cell styles;
    // Buffer::set_line would reset these continuation cells instead.
    assert_eq!(buf[(4, 6)], before[(4, 6)]);
    assert_eq!(buf[(6, 6)], before[(6, 6)]);
}
