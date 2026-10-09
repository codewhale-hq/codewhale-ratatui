use std::time::Duration;

use codewhale_ratatui::{
    MotionMode, Paint, Role, Theme, VerificationSpinner, duration, glyphs, spin,
    testing::{self, Profile, render, text},
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

#[test]
fn verification_uses_the_engine_tick_table_after_the_shared_earned_delay() {
    let expected = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    assert_eq!(VerificationSpinner::FRAMES, expected);
    for elapsed in [Duration::ZERO, ms(1), ms(399)] {
        assert_eq!(
            VerificationSpinner::frame(elapsed, MotionMode::Full, false),
            spin::PENDING_FRAME
        );
    }
    for index in 0..24_u64 {
        let start = ms(400 + index * 200);
        let tick = expected[index as usize % expected.len()];
        assert_eq!(
            VerificationSpinner::frame(start, MotionMode::Full, false),
            tick
        );
        assert_eq!(
            VerificationSpinner::frame(start + ms(199), MotionMode::Full, false),
            tick
        );
        assert_ne!(tick, spin::frame(start, MotionMode::Full, false));
    }
    let elapsed = Duration::MAX;
    let index = (elapsed - spin::EARN_DELAY).as_millis() / spin::FRAME_INTERVAL.as_millis();
    assert_eq!(
        VerificationSpinner::frame(elapsed, MotionMode::Full, false),
        expected[(index % 8) as usize]
    );
}

#[test]
fn the_host_gets_the_exact_next_boundary_without_a_clock_or_polling_loop() {
    for (elapsed, remaining) in [(0, 400), (399, 1), (400, 200), (599, 1), (600, 200)] {
        assert_eq!(
            VerificationSpinner::next_frame_in(ms(elapsed), MotionMode::Full),
            Some(ms(remaining))
        );
    }
    for mode in [MotionMode::Reduced, MotionMode::Still] {
        for elapsed in [Duration::ZERO, ms(399), ms(400), Duration::MAX] {
            assert_eq!(VerificationSpinner::next_frame_in(elapsed, mode), None);
            for ascii in [false, true] {
                assert_eq!(
                    VerificationSpinner::frame(elapsed, mode, ascii),
                    glyphs::pick(glyphs::CURRENT, ascii)
                );
            }
        }
    }
}

#[test]
fn ascii_frames_rotate_without_borrowing_the_attention_marker() {
    let expected = ["-", "\\", "|", "/"];
    for index in 0..16_u64 {
        let frame = VerificationSpinner::frame(ms(400 + index * 200), MotionMode::Full, true);
        assert_eq!(frame, expected[index as usize % expected.len()]);
        assert!(frame.is_ascii());
        assert_ne!(frame, glyphs::pick(glyphs::ATTENTION, true));
    }
    assert_eq!(
        VerificationSpinner::frame(ms(399), MotionMode::Full, true),
        ">"
    );
    let theme = Profile::Ascii.theme();
    let indicator = VerificationSpinner::new("Verifying result", ms(1400), MotionMode::Full);
    let buf = render(40, 1, |area, buf| indicator.paint(area, buf, &theme));
    let shown = text(&buf);
    assert!(shown.is_ascii());
    assert!(shown.contains("Verifying result - 1 s"));
    assert_eq!(buf[(0, 0)].symbol(), "\\");
}

#[test]
fn the_caller_owns_the_verb_and_elapsed_words_are_shared_measured_facts() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        for mode in [MotionMode::Full, MotionMode::Reduced, MotionMode::Still] {
            for elapsed in [ms(999), ms(1000), Duration::from_secs(246)] {
                let indicator = VerificationSpinner::new("Verificando el resultado", elapsed, mode);
                let spans = indicator.spans(&theme);
                let shown: String = spans.iter().map(|span| span.content.as_ref()).collect();
                assert!(shown.contains("Verificando el resultado"));
                assert_eq!(spans[0].style, theme.fg(Role::Live));
                if elapsed < Duration::from_secs(1) {
                    assert_eq!(spans.len(), 3);
                } else {
                    assert!(shown.ends_with(&duration(elapsed)), "{shown}");
                    assert_eq!(spans.len(), 5);
                }
                if mode != MotionMode::Full {
                    assert!(shown.starts_with(glyphs::pick(glyphs::CURRENT, theme.ascii())));
                }
            }
        }
    }
}

#[test]
fn caller_unicode_is_preserved_in_every_profile_while_controls_and_bidi_are_removed() {
    let indicator = VerificationSpinner::new(
        "Ve\u{202e}rify\u{1b} cafe\u{301} 鲸鱼\u{2066}",
        ms(1400),
        MotionMode::Full,
    );
    for profile in Profile::ALL {
        let theme = profile.theme();
        let buf = render(80, 1, |area, buf| indicator.paint(area, buf, &theme));
        let shown = text(&buf);
        assert!(shown.contains("Verify"));
        assert!(!shown.contains(['\u{202e}', '\u{1b}', '\u{2066}']));
        assert!(shown.contains("cafe\u{301} 鲸鱼"));
        assert_eq!(
            buf[(0, 0)].symbol(),
            VerificationSpinner::frame(ms(1400), MotionMode::Full, theme.ascii())
        );
    }
}

#[test]
fn one_cell_and_narrow_rows_keep_the_tick_and_never_split_graphemes() {
    let indicator = VerificationSpinner::new("鲸鱼 cafe\u{301}", ms(400), MotionMode::Full);
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [1, 2, 3, 4, 5, 8, 12, 40] {
            let buf = render(width, 2, |area, buf| indicator.paint(area, buf, &theme));
            assert_eq!(
                buf[(0, 0)].symbol(),
                VerificationSpinner::frame(ms(400), MotionMode::Full, theme.ascii())
            );
            for cell in &buf.content {
                assert!(!cell.symbol().starts_with('\u{301}'));
            }
            assert!(
                buf.content[usize::from(width)..]
                    .iter()
                    .all(|cell| cell.symbol() == " ")
            );
        }
    }
}

#[test]
fn offset_and_off_buffer_areas_preserve_every_untouched_cell_and_style() {
    let indicator =
        VerificationSpinner::new("Checking 鲸鱼 cafe\u{301}", ms(1400), MotionMode::Full);
    let bounds = Rect::new(7, 5, 12, 3);
    for profile in Profile::ALL {
        let theme = profile.theme();
        for area in [
            bounds,
            Rect::new(5, 4, 9, 3),
            Rect::new(9, 6, 40, 3),
            Rect::new(0, 0, 4, 4),
            Rect::new(7, 5, 0, 1),
            Rect::new(7, 5, 12, 0),
            Rect::new(0, 0, u16::MAX, u16::MAX),
        ] {
            let mut before = Buffer::empty(bounds);
            before.set_style(bounds, theme.fg(Role::Muted).add_modifier(Modifier::ITALIC));
            for cell in &mut before.content {
                cell.set_symbol("z");
            }
            let clipped = area.intersection(bounds);
            let mut after = before.clone();
            let mut expected = before.clone();
            indicator.paint(area, &mut after, &theme);
            indicator.paint(clipped, &mut expected, &theme);
            assert_eq!(after, expected, "{}: {area:?}", profile.name());
            for y in bounds.y..bounds.bottom() {
                for x in bounds.x..bounds.right() {
                    if clipped.is_empty() || y != clipped.y || x < clipped.x || x >= clipped.right()
                    {
                        assert_eq!(after[(x, y)], before[(x, y)]);
                    }
                }
            }
        }
    }
}

#[test]
fn the_last_representable_column_preserves_unicode_and_matches_local_paint() {
    let indicator = VerificationSpinner::new("Check 鲸鱼 cafe\u{301}", ms(1400), MotionMode::Full);
    let bounds = Rect::new(u16::MAX - 12, u16::MAX - 3, 12, 3);
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [1, 2, 5, 12, 80] {
            let mut near_limit = Buffer::empty(bounds);
            let mut local = Buffer::empty(Rect::new(0, 0, 12, 3));
            indicator.paint(
                Rect::new(bounds.x, bounds.y, width, 3),
                &mut near_limit,
                &theme,
            );
            indicator.paint(Rect::new(0, 0, width, 3), &mut local, &theme);
            assert_eq!(
                near_limit.content,
                local.content,
                "{}: {width}",
                profile.name()
            );
        }
    }
}

#[test]
fn verification_follows_the_kit_rules_in_every_terminal_profile_and_width() {
    fn paint(area: Rect, buf: &mut Buffer, theme: &Theme) {
        VerificationSpinner::new("Verifying terminal layout", ms(1400), MotionMode::Full)
            .paint(area, buf, theme);
    }
    testing::assert_rules(1, paint);
}
