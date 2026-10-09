//! The current native whale and pet surface, sampled on the host's clock.
//! Activity and pod counts here are explicit catalogue fixtures.

use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use super::Entry;
use crate::{
    DotWhale, MotionMode, Paint, PetMode, PetModeState, State, StatusMark, Theme, WhalePet,
    whale_motion::{Activity, Context, Inputs, Presence, Stage},
};

#[derive(serde::Deserialize)]
struct DotWhaleSample {
    points: Vec<[f64; 2]>,
    materials: Vec<[f64; 4]>,
}

fn dot_whale_sample() -> &'static DotWhaleSample {
    static SAMPLE: OnceLock<DotWhaleSample> = OnceLock::new();
    SAMPLE.get_or_init(|| {
        serde_json::from_str(include_str!("../../assets/dot-whale-search.json"))
            .expect("valid dot-whale gallery fixture")
    })
}

fn pet(area: Rect, buf: &mut Buffer, theme: &Theme, presence: Presence, kind: Option<&str>) {
    let mut stage = Stage::new();
    stage.observe(
        Some("gallery-pet"),
        Inputs {
            presence,
            activity: kind.map(|kind| Activity {
                kind: Some(kind.into()),
                observed: true,
                parallel: (kind == "delegating").then_some(3.),
                ..Activity::default()
            }),
            context: Context {
                live: true,
                ..Context::default()
            },
        },
        false,
    );
    let now = Instant::now();
    stage.advance(now);
    // Sample an entered pose, never elapsed wall time or a new activity model.
    for step in 1..=36 {
        stage.advance(now + Duration::from_millis(step * 40));
    }
    WhalePet::new(theme).paint(area, buf, &mut stage);
}

fn pet_mode(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let mut state = PetModeState::default();
    state.set_visible(true);
    state.update(
        Some("gallery-pet-mode"),
        Inputs {
            presence: Presence::Idle,
            activity: None,
            context: Context::default(),
        },
        &[],
        Instant::now(),
        MotionMode::Still,
    );
    let mut view = PetMode::new(theme, StatusMark::new(State::Ready).word("Idle"));
    view.hints = "Pet surface - idle preview".into();
    view.render(area, buf, &mut state);
}

fn dot_whale(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let sample = dot_whale_sample();
    DotWhale::new(&sample.points, &sample.materials)
        .caption("Searching")
        .action_id(Some("grep_files"))
        .paint(area, buf, theme);
}

pub(crate) fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "pet-dot-search",
            width: 64,
            height: 28,
            draw: dot_whale,
        },
        Entry {
            name: "pet-cove",
            width: 64,
            height: 28,
            draw: |a, b, t| pet(a, b, t, Presence::Idle, None),
        },
        Entry {
            name: "pet-reading",
            width: 64,
            height: 28,
            draw: |a, b, t| pet(a, b, t, Presence::Working, Some("reading")),
        },
        Entry {
            name: "pet-needs-you",
            width: 64,
            height: 28,
            draw: |a, b, t| pet(a, b, t, Presence::NeedsYou, None),
        },
        Entry {
            name: "pet-pod",
            width: 64,
            height: 28,
            draw: |a, b, t| pet(a, b, t, Presence::Working, Some("delegating")),
        },
        Entry {
            name: "pet-mode",
            width: 100,
            height: 36,
            draw: pet_mode,
        },
    ]
}
