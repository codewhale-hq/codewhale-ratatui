use std::time::Duration;

use ratatui::{buffer::Buffer, layout::Rect};

use codewhale_ratatui::{
    ChecksMark, ChecksState, ChecksWords, Count, KeyHint, MotionMode, Notice, NoticeTone, Paint,
    Role, Skeleton, glyphs, testing::Profile,
};

fn render_text(component: &impl Paint, width: u16, height: u16, profile: Profile) -> String {
    let theme = profile.theme();
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    component.paint(area, &mut buf, &theme);
    let mut out = String::new();
    for y in 0..height {
        let mut x = 0u16;
        while x < width {
            let sym = buf[(x, y)].symbol();
            out.push_str(sym);
            let step = codewhale_ratatui::text::width(sym).max(1) as u16;
            x = x.saturating_add(step);
        }
        if y + 1 < height {
            out.push('\n');
        }
    }
    out
}

#[test]
fn notice_tones_have_distinct_unicode_and_ascii_marks() {
    let mut unicode_marks = Vec::new();
    let mut ascii_marks = Vec::new();
    for tone in NoticeTone::ALL {
        let u = glyphs::pick(tone.glyph(), false);
        let a = glyphs::pick(tone.glyph(), true);
        assert!(!unicode_marks.contains(&u), "duplicate unicode mark {u}");
        assert!(!ascii_marks.contains(&a), "duplicate ascii mark {a}");
        assert!(a.is_ascii(), "ascii mark for {tone:?} must be ASCII");
        assert!(!tone.label().is_empty());
        unicode_marks.push(u);
        ascii_marks.push(a);
    }
}

#[test]
fn notice_renders_across_all_profiles_and_narrow_widths() {
    let notice = Notice::warning("Provider rate limit")
        .body("Switching to fallback route while quota recovers.")
        .action(KeyHint::new("r", "retry"))
        .dismissable(true);

    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in 0..=80u16 {
            let h = notice.height(width, &theme);
            let area = Rect::new(0, 0, width, h.max(1));
            let mut buf = Buffer::empty(area);
            notice.paint(area, &mut buf, &theme);
            if profile == Profile::Ascii {
                for cell in buf.content() {
                    assert!(
                        cell.symbol().is_ascii(),
                        "non-ASCII {:?} at width {width}",
                        cell.symbol()
                    );
                }
            }
        }
    }

    let dark = render_text(&notice, 60, 4, Profile::DarkTrue);
    assert!(dark.contains("Provider rate limit"));
    assert!(dark.contains("r retry"));
    assert!(dark.contains("Esc dismiss"));

    let ascii = render_text(&notice, 60, 4, Profile::Ascii);
    assert!(ascii.contains('|'));
    assert!(ascii.contains('*'));
    assert!(ascii.contains("Provider rate limit"));
}

#[test]
fn notice_paints_tonal_surface_when_grounds_differ() {
    let notice = Notice::info("Heads up");
    let theme = Profile::DarkTrue.theme();
    let area = Rect::new(0, 0, 24, 1);
    let mut buf = Buffer::empty(area);
    notice.paint(area, &mut buf, &theme);
    assert_eq!(buf[(5, 0)].bg, theme.bg(Role::Surface).bg.unwrap());

    let mono_theme = Profile::NoColor.theme();
    let mut mono_buf = Buffer::empty(area);
    notice.paint(area, &mut mono_buf, &mono_theme);
    assert_eq!(mono_buf[(0, 0)].symbol(), "▏");
}

#[test]
fn skeleton_shimmers_only_in_full_motion_and_stays_ascii_safe() {
    let full_a = render_text(
        &Skeleton::text(3).motion(MotionMode::Full).tick(0),
        30,
        3,
        Profile::DarkTrue,
    );
    let full_b = render_text(
        &Skeleton::text(3).motion(MotionMode::Full).tick(6),
        30,
        3,
        Profile::DarkTrue,
    );
    assert_ne!(
        full_a, full_b,
        "Full motion shimmer should advance with tick"
    );
    assert!(full_b.contains('▒'));

    for mode in [MotionMode::Reduced, MotionMode::Still] {
        let a = render_text(
            &Skeleton::text(3).motion(mode).tick(0),
            30,
            3,
            Profile::DarkTrue,
        );
        let b = render_text(
            &Skeleton::text(3)
                .motion(mode)
                .elapsed(Duration::from_secs(5)),
            30,
            3,
            Profile::DarkTrue,
        );
        assert_eq!(a, b, "{mode:?} must stay strictly static");
        assert!(!a.contains('▒'));
    }

    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in 0..=80u16 {
            for skel in [
                Skeleton::rows(2).tick(3),
                Skeleton::text(4).motion(MotionMode::Reduced),
            ] {
                let area = Rect::new(0, 0, width, skel.height(width, &theme));
                let mut buf = Buffer::empty(area);
                skel.paint(area, &mut buf, &theme);
                if profile == Profile::Ascii {
                    for cell in buf.content() {
                        assert!(cell.symbol().is_ascii());
                    }
                }
            }
        }
    }
}

#[test]
fn count_formats_zero_capped_and_paints_cleanly() {
    assert_eq!(Count::new(0).to_string(), "0");
    assert_eq!(Count::new(42).to_string(), "42");
    assert_eq!(Count::new(99).max(99).to_string(), "99");
    assert_eq!(Count::new(100).max(99).to_string(), "99+");
    assert_eq!(Count::new(120).max(99).width(), 3);

    for profile in Profile::ALL {
        let theme = profile.theme();
        let badge = Count::new(150).max(99).role(Role::Attention);
        for width in 0..=80u16 {
            let area = Rect::new(0, 0, width, 1);
            let mut buf = Buffer::empty(area);
            badge.paint(area, &mut buf, &theme);
        }
    }

    let rendered = render_text(&Count::new(150).max(99), 6, 1, Profile::DarkTrue);
    assert!(rendered.starts_with("99+"));
}

#[test]
fn checks_mark_has_distinct_marks_roles_and_words() {
    let mut unicode_marks = Vec::new();
    let mut ascii_marks = Vec::new();
    let words = ChecksWords::default();
    for state in ChecksState::ALL {
        let u = glyphs::pick(state.glyph(), false);
        let a = glyphs::pick(state.glyph(), true);
        assert!(!unicode_marks.contains(&u), "duplicate unicode mark {u}");
        assert!(!ascii_marks.contains(&a), "duplicate ascii mark {a}");
        assert!(a.is_ascii());
        assert_eq!(state.word(), words.get(state));
        unicode_marks.push(u);
        ascii_marks.push(a);
    }

    assert_eq!(ChecksState::Passing.role(), Role::Live);
    assert_eq!(ChecksState::Failing.role(), Role::Danger);
    assert_eq!(ChecksState::Pending.role(), Role::Attention);
    assert_eq!(ChecksState::Skipped.role(), Role::Muted);

    let mark = ChecksMark::new(ChecksState::Failing).count(3);
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in 0..=80u16 {
            let area = Rect::new(0, 0, width, 1);
            let mut buf = Buffer::empty(area);
            mark.paint(area, &mut buf, &theme);
            if profile == Profile::Ascii {
                for cell in buf.content() {
                    assert!(cell.symbol().is_ascii());
                }
            }
        }
    }

    let full = render_text(&mark, 24, 1, Profile::DarkTrue);
    assert!(full.contains("✕ Failing (3)"));
    let ascii = render_text(&mark, 24, 1, Profile::Ascii);
    assert!(ascii.contains("X Failing (3)"));

    let custom = ChecksMark::with_words(
        ChecksState::Passing,
        &ChecksWords {
            passing: "通过".into(),
            ..ChecksWords::default()
        },
    );
    let zh = render_text(&custom, 16, 1, Profile::DarkTrue);
    assert!(zh.contains("✓ 通过"));
}
