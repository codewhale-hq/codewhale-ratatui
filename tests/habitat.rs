use std::time::Duration;

use codewhale_ratatui::{
    BubbleField, DistantWhale, FishSchool, Habitat, HabitatDensity, Jellyfish, MarineSnow,
    MotionMode, Paint, Role, Shoal,
    testing::{Frame, Profile, assert_frames_keep_the_rules, frames_for, render, text},
    text::width as display_width,
};
use ratatui::{
    buffer::{Buffer, CellDiffOption},
    layout::Rect,
    style::{Color, Style},
};

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
        (HabitatDensity::Rich, 6),
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
            .shoal(false)
            .snow(false)
            .whale(false)
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
                .shoal(false)
                .snow(false)
                .whale(false)
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
            .shoal(false)
            .snow(false)
            .whale(false)
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
            Shoal::new(Duration::from_millis(20_000), motion).paint(buf.area, &mut buf, &theme);
            Jellyfish::new(Duration::ZERO, motion).paint(buf.area, &mut buf, &theme);
            BubbleField::new(Duration::ZERO, motion).paint(buf.area, &mut buf, &theme);
            MarineSnow::new(Duration::from_millis(20_000), motion)
                .paint(buf.area, &mut buf, &theme);
            DistantWhale::new(Duration::from_millis(20_000), motion)
                .hold_visit(true)
                .paint(buf.area, &mut buf, &theme);
            Habitat::new(Duration::from_millis(40_000), motion)
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
        for ms in [0u64, 1_800, 15_000, 40_000, 70_000] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 112, 30));
            for row in [19, 22, 26, 28] {
                buf.set_stringn(
                    0,
                    row,
                    "Protected scope / cafe\u{301} 鲸鱼",
                    112,
                    theme.fg(Role::Foreground),
                );
                buf.set_stringn(85, row, "鲸鱼", 20, theme.fg(Role::Accent));
            }
            buf[(50, 27)].set_diff_option(CellDiffOption::Skip);
            let before = buf.clone();
            Habitat::new(Duration::from_millis(ms), MotionMode::Full)
                .density(HabitatDensity::Rich)
                .hold_jellyfish_visit(true)
                .paint(buf.area, &mut buf, &theme);
            for y in 0..30 {
                for x in 0..112 {
                    let cell = &before[(x, y)];
                    if cell.symbol() != " " || cell.diff_option == CellDiffOption::Skip {
                        let span = display_width(cell.symbol()).max(1) as u16;
                        let right = x.saturating_add(span - 1);
                        for yy in y.saturating_sub(1)..=(y + 1).min(29) {
                            for xx in x.saturating_sub(2)..=(right + 2).min(111) {
                                assert_eq!(
                                    buf[(xx, yy)],
                                    before[(xx, yy)],
                                    "{} at {ms}ms cell ({xx},{yy}) intruded on ({x},{y})",
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

#[test]
fn the_full_ocean_keeps_clear_of_text_and_its_new_creatures_appear() {
    let theme = Profile::DarkTrue.theme();
    let line = "a line of prose the ocean must never touch";
    let mut painted_any = false;
    for ms in [0u64, 13_000, 40_000, 90_000] {
        let elapsed = Duration::from_millis(ms);
        let mut buf = Buffer::empty(Rect::new(0, 0, 120, 32));
        for row in [4u16, 5, 18] {
            buf.set_stringn(2, row, line, 60, theme.fg(Role::Foreground));
        }
        let before = buf.clone();
        Habitat::new(elapsed, MotionMode::Full)
            .density(HabitatDensity::Rich)
            .paint(buf.area, &mut buf, &theme);
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if buf[(x, y)] == before[(x, y)] {
                    continue;
                }
                painted_any = true;
                for (dx, dy) in (-2i32..=2).flat_map(|dx| (-1i32..=1).map(move |dy| (dx, dy))) {
                    let (nx, ny) = (i32::from(x) + dx, i32::from(y) + dy);
                    if nx < 0 || ny < 0 || nx >= 120 || ny >= 32 {
                        continue;
                    }
                    assert_eq!(
                        before[(nx as u16, ny as u16)].symbol(),
                        " ",
                        "life at ({x}, {y}) touches text at {ms} ms:\n{}",
                        text(&buf)
                    );
                }
            }
        }
    }
    assert!(painted_any);

    let shown = |paint: &dyn Fn(Rect, &mut Buffer)| {
        let buf = render(120, 32, |area, buf| paint(area, buf));
        text(&buf)
            .chars()
            .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
    };
    let t = Duration::from_millis(20_000);
    assert!(shown(
        &|a, b| Shoal::new(t, MotionMode::Full).paint(a, b, &theme)
    ));
    assert!(shown(
        &|a, b| MarineSnow::new(t, MotionMode::Full).paint(a, b, &theme)
    ));
    assert!(shown(
        &|a, b| DistantWhale::new(t, MotionMode::Full).paint(a, b, &theme)
    ));
    assert!(!shown(&|a, b| {
        DistantWhale::new(t, MotionMode::Reduced).paint(a, b, &theme)
    }));
    assert!(!shown(&|a, b| {
        MarineSnow::new(t, MotionMode::Full)
            .presence(0.0)
            .paint(a, b, &theme)
    }));
}

#[test]
fn glow_respects_color_depth_unknown_ground_and_invisible_presence() {
    let rgb_water = Color::Rgb(10, 30, 50);
    for profile in [
        Profile::Dark256,
        Profile::Light256,
        Profile::UnknownGround,
        Profile::Ansi16,
        Profile::NoColor,
        Profile::Ascii,
    ] {
        let theme = profile.theme();
        for ink in [None, Some(Color::Rgb(120, 200, 255))] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 112, 30));
            buf.set_style(buf.area, Style::default().bg(rgb_water));
            Habitat::new(Duration::from_millis(40_000), MotionMode::Full)
                .density(HabitatDensity::Rich)
                .hold_jellyfish_visit(true)
                .ink(ink)
                .paint(buf.area, &mut buf, &theme);
            assert!(
                buf.content
                    .iter()
                    .all(|cell| !matches!(cell.fg, Color::Rgb(..)) && cell.bg == rgb_water),
                "{} emitted an RGB foreground or mutated bg",
                profile.name()
            );

            let mut themed = Buffer::empty(Rect::new(0, 0, 112, 30));
            themed.set_style(themed.area, theme.bg(Role::Background));
            Habitat::new(Duration::from_millis(40_000), MotionMode::Full)
                .density(HabitatDensity::Rich)
                .hold_jellyfish_visit(true)
                .ink(ink)
                .paint(themed.area, &mut themed, &theme);
            let frame = Frame::new("habitat-depth", profile, themed);
            assert!(
                frame.violations().is_empty(),
                "{}: {:?}",
                frame.label(),
                frame.violations()
            );
        }
    }

    for profile in [Profile::DarkTrue, Profile::LightTrue] {
        let theme = profile.theme();
        let baseline = render(120, 32, |area, buf| {
            buf.set_style(area, theme.bg(Role::Background));
        });
        let mut faint = baseline.clone();
        Habitat::new(Duration::from_millis(40_000), MotionMode::Full)
            .density(HabitatDensity::Rich)
            .hold_jellyfish_visit(true)
            .presence(0.05)
            .paint(faint.area, &mut faint, &theme);
        assert_eq!(
            faint,
            baseline,
            "{}: presence(0.05) painted cells",
            profile.name()
        );

        for presence in [0.20f32, 1.0] {
            let mut painted = baseline.clone();
            Habitat::new(Duration::from_millis(40_000), MotionMode::Full)
                .density(HabitatDensity::Rich)
                .hold_jellyfish_visit(true)
                .presence(presence)
                .paint(painted.area, &mut painted, &theme);
            for y in 0..painted.area.height {
                for x in 0..painted.area.width {
                    if painted[(x, y)] != baseline[(x, y)] {
                        assert_ne!(painted[(x, y)].symbol(), " ");
                        assert_ne!(painted[(x, y)].fg, painted[(x, y)].bg);
                    }
                }
            }
        }
    }
}

#[test]
fn distant_whale_hold_visit_never_flips_mid_crossing_and_guards_edges() {
    let theme = Profile::DarkTrue.theme();
    assert!(
        !DistantWhale::new(Duration::from_millis(15_000), MotionMode::Full)
            .hold_visit(true)
            .visiting(Rect::new(0, 0, 0, 20), &theme)
    );
    for presence in [0.0f32, 0.05] {
        assert!(
            !DistantWhale::new(Duration::from_millis(15_000), MotionMode::Full)
                .hold_visit(true)
                .presence(presence)
                .visiting(Rect::new(0, 0, 72, 20), &theme)
        );
    }

    let ultrawide = Rect::new(0, 0, 260, 24);
    assert!(!DistantWhale::new(Duration::ZERO, MotionMode::Full).visiting(ultrawide, &theme));
    let mut saw_visit = false;
    let mut saw_rest = false;
    for secs in (0..400u64).step_by(5) {
        if DistantWhale::new(Duration::from_secs(secs), MotionMode::Full)
            .visiting(ultrawide, &theme)
        {
            saw_visit = true;
        } else {
            saw_rest = true;
        }
    }
    assert!(saw_visit && saw_rest);

    let mut prev_bounds: Option<(u16, u16)> = None;
    for range in [0u64..=250_000, 1_055_000..=1_070_000] {
        prev_bounds = None;
        for ms in range.step_by(100) {
            let buf = render(72, 12, |area, buf| {
                DistantWhale::new(Duration::from_millis(ms), MotionMode::Full)
                    .hold_visit(true)
                    .paint(area, buf, &theme);
            });
            let mut bounds: Option<(u16, u16)> = None;
            for y in 0..12 {
                for x in 0..72 {
                    if buf[(x, y)].symbol() != " " {
                        bounds = Some(match bounds {
                            Some((lo, hi)) => (lo.min(x), hi.max(x)),
                            None => (x, x),
                        });
                    }
                }
            }
            if let (Some((p_lo, p_hi)), Some((lo, hi))) = (prev_bounds, bounds) {
                assert!(
                    lo.abs_diff(p_lo) <= 1 && hi.abs_diff(p_hi) <= 1,
                    "whale jumped at {ms}ms: ({p_lo}..={p_hi}) -> ({lo}..={hi})"
                );
            }
            prev_bounds = bounds;
        }
    }
    assert!(prev_bounds.is_some());

    let has_glyphs = |buf: &Buffer, mut rows: std::ops::Range<u16>| {
        rows.any(|y| (0..buf.area.width).any(|x| buf[(x, y)].symbol() != " "))
    };
    let t = Duration::from_millis(40_000);
    let whale_only = render(112, 30, |area, buf| {
        Habitat::new(t, MotionMode::Full)
            .fish(false)
            .shoal(false)
            .bubbles(false)
            .snow(false)
            .paint(area, buf, &theme);
    });
    assert!(has_glyphs(&whale_only, 21..26));
    assert!(!has_glyphs(&whale_only, 16..20));

    let whale_blocked = render(112, 30, |area, buf| {
        Habitat::new(t, MotionMode::Full)
            .fish(false)
            .shoal(false)
            .bubbles(false)
            .snow(false)
            .exclude_rect(Rect::new(0, 21, 112, 5))
            .paint(area, buf, &theme);
    });
    assert!(!has_glyphs(&whale_blocked, 21..26));
    assert!(has_glyphs(&whale_blocked, 16..20));

    let both = render(112, 30, |area, buf| {
        Habitat::new(t, MotionMode::Full)
            .fish(false)
            .shoal(false)
            .bubbles(false)
            .snow(false)
            .hold_jellyfish_visit(true)
            .paint(area, buf, &theme);
    });
    assert!(has_glyphs(&both, 21..26));
    assert!(has_glyphs(&both, 16..20));
}
