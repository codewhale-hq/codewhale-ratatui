use std::time::Duration;

use codewhale_ratatui::{
    BubbleField, FishSchool, Habitat, HabitatDensity, Jellyfish, MotionMode, Paint, Role,
    testing::{Frame, Profile, assert_frames_keep_the_rules, frames_for, render, text},
};
use ratatui::{buffer::Buffer, layout::Rect};

fn fish_count(buf: &Buffer) -> usize {
    let shown = text(buf);
    ["><o>", "><>", "<o><", "<><"]
        .iter()
        .map(|body| shown.matches(body).count())
        .sum()
}

#[test]
fn density_is_bounded_and_a_school_retains_all_its_members() {
    let theme = Profile::Ascii.theme();
    for (density, count) in [
        (HabitatDensity::Sparse, 3),
        (HabitatDensity::Normal, 5),
        (HabitatDensity::Rich, 7),
    ] {
        let fish = FishSchool::new(Duration::ZERO, MotionMode::Full).density(density);
        let buf = render(96, 14, |area, buf| fish.paint(area, buf, &theme));
        assert_eq!(fish_count(&buf), count, "{}", text(&buf));
    }
    assert_eq!(
        HabitatDensity::Auto.for_area(Rect::new(0, 0, 40, 24)),
        HabitatDensity::Sparse
    );
    assert_eq!(
        HabitatDensity::Auto.for_area(Rect::new(0, 0, 80, 24)),
        HabitatDensity::Normal
    );
    assert_eq!(
        HabitatDensity::Auto.for_area(Rect::new(0, 0, 112, 30)),
        HabitatDensity::Rich
    );
}

#[test]
fn native_poses_are_real_dot_silhouettes_and_time_is_caller_owned() {
    let theme = Profile::DarkTrue.theme();
    let frame = |elapsed| {
        render(72, 12, |area, buf| {
            FishSchool::new(elapsed, MotionMode::Full)
                .density(HabitatDensity::Rich)
                .paint(area, buf, &theme);
            Jellyfish::new(elapsed, MotionMode::Full).paint(Rect::new(0, 0, 24, 7), buf, &theme);
        })
    };
    let a = frame(Duration::ZERO);
    let b = frame(Duration::from_millis(1_800));
    assert!(
        text(&a)
            .chars()
            .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
    );
    assert_eq!(a, frame(Duration::ZERO), "no private clock or RNG");
    assert_ne!(
        text(&a),
        text(&b),
        "caller time changes authored poses and travel"
    );
}

#[test]
fn jellyfish_clearance_withholds_the_whole_creature() {
    for profile in [Profile::DarkTrue, Profile::Ascii] {
        let theme = profile.theme();
        let jelly = Jellyfish::new(Duration::from_millis(1_800), MotionMode::Full);
        let empty = render(32, 9, |area, buf| jelly.paint(area, buf, &theme));
        assert!(empty.content.iter().any(|cell| cell.symbol() != " "));
        let mut blocked = Buffer::empty(empty.area);
        // Below the bell's three-row rectangle, in its one-row clearance.
        blocked[(15, 6)].set_symbol("X");
        let before = blocked.clone();
        jelly.paint(blocked.area, &mut blocked, &theme);
        assert_eq!(
            blocked,
            before,
            "{}: no detached dome or severed arm",
            profile.name()
        );
    }
}

#[test]
fn visitors_are_rare_unless_the_host_holds_a_visit_open() {
    let theme = Profile::Ascii.theme();
    let elapsed = Duration::from_secs(100);
    let visitor = Jellyfish::new(elapsed, MotionMode::Full).visitor(true);
    let absent = render(84, 24, |area, buf| visitor.paint(area, buf, &theme));
    assert!(text(&absent).trim().is_empty());
    let held = render(84, 24, |area, buf| {
        visitor.hold_visit(true).paint(area, buf, &theme)
    });
    assert!(!text(&held).trim().is_empty());
    let all = render(84, 24, |area, buf| {
        Habitat::new(elapsed, MotionMode::Full)
            .fish(false)
            .bubbles(false)
            .hold_jellyfish_visit(true)
            .paint(area, buf, &theme)
    });
    assert_eq!(all, held);
}

#[test]
fn filled_blank_overlays_are_protected_by_geometry_and_clearance() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        let baseline = render(112, 30, |area, buf| {
            buf.set_style(area, theme.bg(Role::Surface));
        });
        let mut covered = baseline.clone();
        Habitat::new(Duration::ZERO, MotionMode::Full)
            .density(HabitatDensity::Rich)
            .hold_jellyfish_visit(true)
            .protected(vec![covered.area])
            .paint(covered.area, &mut covered, &theme);
        assert_eq!(covered, baseline, "blank overlay is owned by its host");

        let visible = render(112, 30, |area, buf| {
            Habitat::new(Duration::ZERO, MotionMode::Full)
                .fish(false)
                .bubbles(false)
                .hold_jellyfish_visit(true)
                .paint(area, buf, &theme);
        });
        assert!(!text(&visible).trim().is_empty());
        // The visitor starts at local y=21 and ends before y=24. A blank
        // decision surface at y=24 still owns the row of clearance below it.
        let mut protected = Buffer::empty(visible.area);
        Habitat::new(Duration::ZERO, MotionMode::Full)
            .fish(false)
            .bubbles(false)
            .hold_jellyfish_visit(true)
            .exclude_rect(Rect::new(90, 24, 12, 2))
            .paint(protected.area, &mut protected, &theme);
        assert!(
            text(&protected).trim().is_empty(),
            "whole visitor gives way"
        );
    }
}

#[test]
fn all_ambient_life_disappears_under_reduced_or_still_motion() {
    for motion in [MotionMode::Reduced, MotionMode::Still] {
        for profile in Profile::ALL {
            let theme = profile.theme();
            let mut buf = Buffer::empty(Rect::new(0, 0, 112, 30));
            buf.set_stringn(2, 20, "鲸鱼 cafe\u{301}", 30, theme.fg(Role::Foreground));
            let before = buf.clone();
            FishSchool::new(Duration::ZERO, motion).paint(buf.area, &mut buf, &theme);
            Jellyfish::new(Duration::ZERO, motion).paint(buf.area, &mut buf, &theme);
            BubbleField::new(Duration::ZERO, motion).paint(buf.area, &mut buf, &theme);
            Habitat::new(Duration::ZERO, motion)
                .hold_jellyfish_visit(true)
                .paint(buf.area, &mut buf, &theme);
            assert_eq!(buf, before);
        }
    }
}

#[test]
fn existing_text_and_wide_graphemes_keep_their_cells_and_clearance() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        let mut buf = Buffer::empty(Rect::new(0, 0, 112, 30));
        for row in [19, 22, 26, 28] {
            buf.set_stringn(
                0,
                row,
                "Protected scope / cafe\u{301} 鲸鱼",
                112,
                theme.fg(Role::Foreground),
            );
            buf.set_stringn(85, row, "鲸鱼", 20, theme.fg(Role::Primary));
        }
        let before = buf.clone();
        Habitat::new(Duration::from_millis(1_800), MotionMode::Full)
            .density(HabitatDensity::Rich)
            .hold_jellyfish_visit(true)
            .paint(buf.area, &mut buf, &theme);
        for y in 0..30 {
            for x in 0..112 {
                // Every authored cell and its immediate horizontal/vertical
                // clearance remain intact. Wide glyph continuation cells
                // are included by the surrounding nonblank-cell check.
                if before[(x, y)].symbol() != " " {
                    for yy in y.saturating_sub(1)..=(y + 1).min(29) {
                        for xx in x.saturating_sub(1)..=(x + 1).min(111) {
                            assert_eq!(buf[(xx, yy)], before[(xx, yy)]);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_profile_keeps_the_design_rules_and_ascii_is_visible() {
    let frames = frames_for(
        "habitat",
        &Profile::ALL,
        &[40, 80, 112],
        30,
        |area, buf, theme| {
            Habitat::new(Duration::from_millis(1_500), MotionMode::Full)
                .density(HabitatDensity::Rich)
                .hold_jellyfish_visit(true)
                .paint(area, buf, theme);
        },
    );
    assert_frames_keep_the_rules(&frames);
    for frame in frames {
        assert!(!frame.text().trim().is_empty(), "{}", frame.label());
        if frame.profile == Profile::Ascii {
            assert!(frame.text().is_ascii());
            assert!(fish_count(&frame.buf) > 0);
        }
    }
}

#[test]
fn tiny_partial_and_saturated_buffer_edges_are_bounded() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        for origin in [(2, 2), (u16::MAX - 26, u16::MAX - 18)] {
            for width in [0, 1, 4, 24] {
                for height in [0, 1, 3, 16] {
                    let bounds = Rect::new(origin.0, origin.1, 24, 16);
                    let requested = Rect::new(origin.0 + 2, origin.1 + 1, width, height);
                    let clipped = requested.intersection(bounds);
                    let mut buf = Buffer::empty(bounds);
                    for y in bounds.y..bounds.bottom() {
                        for x in bounds.x..bounds.right() {
                            if !clipped.contains((x, y).into()) {
                                buf[(x, y)].set_symbol("z");
                            }
                        }
                    }
                    let before = buf.clone();
                    Habitat::new(Duration::MAX, MotionMode::Full)
                        .density(HabitatDensity::Rich)
                        .hold_jellyfish_visit(true)
                        .paint(requested, &mut buf, &theme);
                    Jellyfish::new(Duration::MAX, MotionMode::Full)
                        .paint(requested, &mut buf, &theme);
                    for y in bounds.y..bounds.bottom() {
                        for x in bounds.x..bounds.right() {
                            if !clipped.contains((x, y).into()) {
                                assert_eq!(buf[(x, y)], before[(x, y)]);
                            }
                        }
                    }
                    let frame = Frame::new("bounded marine life", profile, buf);
                    assert!(frame.violations().is_empty(), "{}", frame.label());
                }
            }
        }
    }
}
