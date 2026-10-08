use std::time::{Duration, Instant};

use codewhale_ratatui::{
    PetStyle, Role, WhalePet,
    avatar_sprite::Sprite,
    testing::{self, Profile},
    whale_motion::{Activity, Context, Inputs, Presence, Stage, Tier},
};
use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::StatefulWidget};

fn observed(presence: Presence, kind: Option<&str>) -> Inputs {
    Inputs {
        presence,
        activity: kind.map(|kind| Activity {
            kind: Some(kind.into()),
            observed: true,
            parallel: Some(3.),
            ..Activity::default()
        }),
        context: Context {
            live: true,
            turn_id: Some("pet-test".into()),
            status: Some(
                if presence == Presence::Stuck {
                    "failed"
                } else {
                    "completed"
                }
                .into(),
            ),
            now_ms: Some(1000.),
            failed_at_ms: Some(0.),
        },
    }
}

fn frame(stage: &mut Stage, profile: Profile, cove: bool, style: PetStyle) -> Buffer {
    let theme = profile.theme();
    testing::render(76, 36, |area, buf| {
        buf.set_style(area, theme.bg(Role::Background));
        StatefulWidget::render(
            WhalePet::new(&theme).cove(cove).style(style),
            area,
            buf,
            stage,
        );
    })
}

#[test]
fn every_native_action_paints_color_and_the_actual_owner_caption() {
    let cases = [
        (Presence::Idle, None, "Resting"),
        (Presence::Listening, None, "Listening"),
        (Presence::Thinking, None, "Thinking"),
        (Presence::Working, Some("unknown"), "Working"),
        (Presence::Working, Some("reading"), "Reading"),
        (Presence::Working, Some("searching"), "Searching"),
        (Presence::Working, Some("editing"), "Editing"),
        (Presence::Working, Some("executing"), "Running a command"),
        (Presence::Working, Some("browsing"), "Browsing"),
        (Presence::Working, Some("responding"), "Replying"),
        (
            Presence::Working,
            Some("delegating"),
            "Working with 3 agents",
        ),
        (Presence::NeedsYou, None, "Needs you"),
        (Presence::Done, None, "Done"),
        (Presence::Stuck, None, "Stuck"),
        (Presence::Offline, None, "Asleep"),
        (Presence::Working, Some("computer"), "Using the computer"),
        (
            Presence::Working,
            Some("network"),
            "Calling a connected app",
        ),
    ];
    let mut distinct = std::collections::HashSet::new();
    for (presence, kind, words) in cases {
        let mut stage = Stage::new();
        stage.observe(Some("one"), observed(presence, kind), true);
        let buf = frame(&mut stage, Profile::DarkTrue, true, PetStyle::Color);
        assert!(testing::text(&buf).contains(words), "missing {words}");
        assert!(
            buf.content.iter().any(|c| matches!(c.symbol(), "▀" | "▄")),
            "no color art: {words}"
        );
        let art = WhalePet::art_area(buf.area);
        let mut cropped = Buffer::empty(art);
        for y in art.top()..art.bottom() {
            for x in art.left()..art.right() {
                cropped[(x, y)] = buf[(x, y)].clone();
            }
        }
        distinct.insert(testing::styled(&cropped, &Profile::DarkTrue.theme()));
    }
    assert_eq!(
        distinct.len(),
        17,
        "each authored action must have its own visible pose"
    );
}

#[test]
fn color_animation_and_water_interaction_share_the_host_clock() {
    let mut stage = Stage::new();
    stage.observe(Some("one"), observed(Presence::Idle, None), false);
    let now = Instant::now();
    stage.advance(now);
    let first = frame(&mut stage, Profile::DarkTrue, true, PetStyle::Color);
    stage.cove_observe(40., -30.);
    stage.cove_tap(30., 35.);
    stage.advance(now + Duration::from_millis(600));
    let advanced = frame(&mut stage, Profile::DarkTrue, true, PetStyle::Color);
    assert_ne!(first, advanced, "gaze, water and body should move");
    let f = stage.director().f;
    let repeated = frame(&mut stage, Profile::DarkTrue, true, PetStyle::Color);
    assert_eq!(
        advanced, repeated,
        "painting twice must sample the same frame"
    );
    assert_eq!(
        stage.director().f,
        f,
        "the component must never advance time"
    );
    assert_eq!(
        stage.cadence(Tier::Hero),
        Some(Duration::from_secs_f64(1. / 30.))
    );
}

#[test]
fn reduced_motion_stays_still_after_time_and_pointer_input() {
    let mut stage = Stage::new();
    stage.observe(
        Some("one"),
        observed(Presence::Working, Some("reading")),
        true,
    );
    let now = Instant::now();
    stage.advance(now);
    let first = frame(&mut stage, Profile::LightTrue, true, PetStyle::Color);
    stage.cove_observe(50., 30.);
    stage.advance(now + Duration::from_secs(120));
    assert!(
        first == frame(&mut stage, Profile::LightTrue, true, PetStyle::Color),
        "time and pointer gaze must not animate the reduced poster"
    );
    assert_eq!(stage.cadence(Tier::Hero), None);
    // Native reduced-motion taps may add a still ripple on explicit input.
    // They must never schedule or animate that ripple afterward.
    stage.cove_tap(-40., 45.);
    let tapped = frame(&mut stage, Profile::LightTrue, true, PetStyle::Color);
    stage.advance(now + Duration::from_secs(240));
    assert!(
        tapped == frame(&mut stage, Profile::LightTrue, true, PetStyle::Color),
        "an explicit reduced-motion ripple stays frozen"
    );
    stage.observe(
        Some("one"),
        observed(Presence::Working, Some("editing")),
        true,
    );
    assert_ne!(
        first,
        frame(&mut stage, Profile::LightTrue, true, PetStyle::Color)
    );
}

#[test]
fn color_keeps_native_gradient_and_light_apertures_without_the_cove() {
    let mut stage = Stage::new();
    stage.observe(Some("one"), observed(Presence::Idle, None), true);
    for profile in [Profile::DarkTrue, Profile::LightTrue] {
        let buf = frame(&mut stage, profile, false, PetStyle::Color);
        let art = WhalePet::art_area(buf.area);
        let mut colors = std::collections::HashSet::new();
        for y in art.top()..art.bottom() {
            for x in art.left()..art.right() {
                let c = &buf[(x, y)];
                if matches!(c.symbol(), "▀" | "▄") {
                    colors.extend([c.fg, c.bg]);
                }
            }
        }
        assert!(
            colors.len() > 20,
            "the body must retain its diagonal gradient"
        );
        assert!(
            colors.contains(&Color::Rgb(250, 248, 245)),
            "native light eye/throat apertures"
        );
        assert_eq!(
            buf[(art.x, art.y)].symbol(),
            " ",
            "transparent surroundings"
        );
    }
}

#[test]
fn all_terminal_profiles_keep_state_words_and_capability_fallbacks() {
    let mut stage = Stage::new();
    stage.observe(Some("one"), observed(Presence::NeedsYou, None), true);
    for profile in Profile::ALL {
        for style in [PetStyle::Color, PetStyle::Braille] {
            let buf = frame(&mut stage, profile, true, style);
            assert!(
                testing::text(&buf).contains("Needs you"),
                "{} {style:?}",
                profile.name()
            );
            if !profile.theme().paints_grounds() || style == PetStyle::Braille {
                assert!(!buf.content.iter().any(|c| matches!(c.symbol(), "▀" | "▄")));
            }
            if !profile.theme().paints_grounds() {
                assert!(
                    !buf.content
                        .iter()
                        .any(|c| matches!(c.bg, Color::Rgb(..) | Color::Indexed(_)))
                );
            }
            if profile == Profile::Ascii {
                assert!(testing::text(&buf).is_ascii());
            }
            if matches!(profile, Profile::Dark256 | Profile::Light256) {
                assert!(
                    !buf.content
                        .iter()
                        .any(|c| matches!(c.fg, Color::Rgb(..)) || matches!(c.bg, Color::Rgb(..)))
                );
            }
        }
    }
}

#[test]
fn offsets_clipping_empty_and_small_rectangles_preserve_surroundings() {
    let theme = Profile::DarkTrue.theme();
    let mut stage = Stage::new();
    stage.observe(
        Some("one"),
        observed(Presence::Working, Some("editing")),
        true,
    );
    for area in [
        Rect::new(13, 7, 42, 24),
        Rect::new(55, 24, 40, 40),
        Rect::new(20, 10, 14, 3),
        Rect::new(30, 20, 0, 0),
        Rect::new(60, 30, 1, 1),
    ] {
        let mut buf = Buffer::empty(Rect::new(8, 5, 60, 32));
        for cell in &mut buf.content {
            cell.set_symbol(".");
        }
        let before = buf.clone();
        WhalePet::new(&theme)
            .words("正在编辑")
            .paint(area, &mut buf, &mut stage);
        let clip = area.intersection(buf.area);
        for y in buf.area.top()..buf.area.bottom() {
            for x in buf.area.left()..buf.area.right() {
                if !clip.contains((x, y).into()) {
                    assert_eq!(buf[(x, y)], before[(x, y)]);
                }
            }
        }
    }
}

#[test]
fn mouse_hit_mapping_uses_the_actual_bounded_art_rectangle() {
    for area in [
        Rect::new(0, 0, 88, 36),
        Rect::new(20, 8, 140, 60),
        Rect::new(0, 0, 0, 0),
    ] {
        let art = WhalePet::art_area(area);
        assert!(art.width <= 96);
        assert_eq!(art.width, art.height * 2);
        assert_eq!(WhalePet::point(area, art.right(), art.bottom()), None);
        if !art.is_empty() {
            let (x, y) = WhalePet::point(area, art.x, art.y).unwrap();
            assert!((-62. ..0.).contains(&x) && (-62. ..0.).contains(&y));
            let (x, y) = WhalePet::point(area, art.right() - 1, art.bottom() - 1).unwrap();
            assert!((0. ..62.).contains(&x) && (0. ..62.).contains(&y));
        }
    }
}

#[test]
fn live_image_adapter_refuses_invalid_or_unbounded_geometry() {
    assert!(Sprite::image(&[255; 16], 2, 2).is_ok());
    assert!(Sprite::image(&[255; 15], 2, 2).is_err());
    assert!(Sprite::image(&[], 0, 0).is_err());
    assert!(Sprite::image(&[], usize::MAX, 1).is_err());
    assert!(Sprite::image(&[], 2049, 1).is_err());
}

#[test]
fn pod_caption_keeps_real_counts_beyond_the_compact_whale_limit() {
    let mut input = observed(Presence::Working, Some("delegating"));
    input.activity.as_mut().unwrap().parallel = Some(700.);
    let mut stage = Stage::new();
    stage.observe(Some("one"), input, true);
    let buf = frame(&mut stage, Profile::DarkTrue, true, PetStyle::Color);
    assert!(testing::text(&buf).contains("Working with 700 agents"));
    assert_eq!(
        stage
            .director()
            .calves
            .iter()
            .filter(|c| c.value > 0.)
            .count(),
        3
    );
}
