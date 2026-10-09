//! Package Motion: easing, retargeting, the policy, the schedule and the
//! reference component painted at the start, in flight and settled.
//!
//! Nothing here sleeps or reads a clock while it runs: `t0` is taken once and
//! every other instant is `t0` plus a duration.

use std::time::{Duration, Instant};

use codewhale_ratatui::{
    FrameBudget, MOTION_FRAME_INTERVAL, MOTION_MIN_FRAME_INTERVAL, MotionChannel, MotionDemo,
    MotionDemoWords, MotionEasing, MotionMode, MotionPolicy, MotionSet, MotionStep, MotionTiming,
    Paint, Role, Theme,
    glyphs::{DONE, pick},
    testing::{self, Profile, WIDTHS},
};
use ratatui::{buffer::Buffer, layout::Rect};

const MODES: [MotionMode; 3] = [MotionMode::Full, MotionMode::Reduced, MotionMode::Still];

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn truecolor() -> Theme {
    Profile::DarkTrue.theme()
}

fn full() -> MotionPolicy {
    MotionPolicy::new(MotionMode::Full, &truecolor())
}

fn timing(duration_ms: u64, easing: MotionEasing) -> MotionTiming {
    MotionTiming::space(ms(duration_ms), easing)
}

/// Small deterministic generator, so the property loops need no dependency.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn unit(&mut self) -> f32 {
        (self.next() % 10_001) as f32 / 10_000.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

// ---------------------------------------------------------------- easing

#[test]
fn every_easing_starts_at_zero_ends_at_one_and_never_reverses() {
    for easing in [
        MotionEasing::Linear,
        MotionEasing::EaseOut,
        MotionEasing::EaseIn,
    ] {
        assert_eq!(easing.apply(0.0), 0.0, "{easing:?}");
        assert_eq!(easing.apply(1.0), 1.0, "{easing:?}");
        let mut last = 0.0;
        for i in 0..=1000 {
            let v = easing.apply(i as f32 / 1000.0);
            assert!(
                (0.0..=1.0).contains(&v),
                "{easing:?} left 0..=1 at {i}: {v}"
            );
            assert!(v >= last, "{easing:?} reversed at {i}: {last} then {v}");
            last = v;
        }
    }
}

#[test]
fn ease_out_arrives_early_and_ease_in_late() {
    assert!(MotionEasing::EaseOut.apply(0.5) > 0.5);
    assert!(MotionEasing::EaseIn.apply(0.5) < 0.5);
    assert_eq!(MotionEasing::Linear.apply(0.5), 0.5);
}

#[test]
fn easing_clamps_out_of_range_input_and_finishes_on_nan() {
    for easing in [
        MotionEasing::Linear,
        MotionEasing::EaseOut,
        MotionEasing::EaseIn,
    ] {
        assert_eq!(easing.apply(-3.0), 0.0);
        assert_eq!(easing.apply(7.0), 1.0);
        assert_eq!(easing.apply(f32::NEG_INFINITY), 0.0);
        assert_eq!(easing.apply(f32::INFINITY), 1.0);
        assert_eq!(easing.apply(f32::NAN), 1.0);
    }
}

#[test]
fn the_named_timings_are_the_design_tokens() {
    assert_eq!(MotionTiming::STATE.duration, ms(120));
    assert_eq!(MotionTiming::ARRIVE.duration, ms(180));
    assert_eq!(MotionTiming::EXIT.duration, ms(120));
    assert_eq!(MotionTiming::PANEL.duration, ms(340));
    assert_eq!(MotionTiming::EXIT.easing, MotionEasing::EaseIn);
    assert_eq!(MotionTiming::STATE.channel, MotionChannel::Tint);
    assert_eq!(MotionTiming::PANEL.channel, MotionChannel::Space);
}

// ------------------------------------------------------------------ steps

#[test]
fn a_step_runs_from_its_start_to_its_target_and_stops() {
    let t0 = Instant::now();
    let mut step = MotionStep::at_rest(0.0);
    assert!(step.go(1.0, t0, timing(200, MotionEasing::Linear), full()));
    assert_eq!(step.at(t0), 0.0);
    assert!((step.at(t0 + ms(100)) - 0.5).abs() < 1e-5);
    assert_eq!(step.at(t0 + ms(200)), 1.0);
    assert_eq!(step.at(t0 + Duration::from_secs(60)), 1.0);
    assert!(!step.is_settled(t0 + ms(199)));
    assert!(step.is_settled(t0 + ms(200)));
    // A clock that reads earlier than the start holds the start value.
    assert_eq!(step.at(t0.checked_sub(ms(5)).unwrap_or(t0)), 0.0);
}

#[test]
fn a_new_step_rests_where_it_was_made() {
    let t0 = Instant::now();
    assert_eq!(MotionStep::default().at(t0), 0.0);
    assert_eq!(MotionStep::at_rest(0.25).at(t0), 0.25);
    assert_eq!(MotionStep::at_rest(9.0).target(), 1.0);
    assert_eq!(MotionStep::at_rest(f32::NAN).target(), 0.0);
    assert!(MotionStep::default().is_settled(t0));
}

#[test]
fn retargeting_mid_flight_does_not_jump() {
    let t0 = Instant::now();
    for easing in [
        MotionEasing::Linear,
        MotionEasing::EaseOut,
        MotionEasing::EaseIn,
    ] {
        let mut step = MotionStep::at_rest(0.0);
        step.go(1.0, t0, timing(200, easing), full());
        for cut in [10, 60, 100, 150, 199] {
            let mut flipped = step;
            let now = t0 + ms(cut);
            let before = flipped.at(now);
            assert!(flipped.go(0.0, now, timing(200, easing), full()));
            let after = flipped.at(now);
            assert!(
                (before - after).abs() < 1e-6,
                "{easing:?} jumped at {cut} ms: {before} to {after}"
            );
            // And it then heads the other way, to rest at the new target.
            let mut last = after;
            for i in 1..=40 {
                let v = flipped.at(now + ms(5 * i));
                assert!(v <= last + 1e-6, "{easing:?} turned back at {i}");
                last = v;
            }
            assert_eq!(flipped.at(now + ms(200)), 0.0);
        }
    }
}

#[test]
fn asking_for_the_target_it_already_has_does_not_restart_it() {
    let t0 = Instant::now();
    let mut step = MotionStep::at_rest(0.0);
    assert!(step.go(1.0, t0, MotionTiming::ARRIVE, full()));
    let mid = step.at(t0 + ms(90));
    assert!(!step.go(1.0, t0 + ms(90), MotionTiming::ARRIVE, full()));
    assert_eq!(step.at(t0 + ms(90)), mid);
    assert!(step.is_settled(t0 + ms(180)), "the end time did not move");
}

#[test]
fn unusable_targets_are_ignored_or_clamped() {
    let t0 = Instant::now();
    let mut step = MotionStep::at_rest(0.5);
    assert!(!step.go(f32::NAN, t0, MotionTiming::STATE, full()));
    assert!(!step.go(f32::INFINITY, t0, MotionTiming::STATE, full()));
    assert_eq!(step.target(), 0.5);
    assert!(step.go(40.0, t0, MotionTiming::STATE, full()));
    assert_eq!(step.target(), 1.0);
    assert!(step.go(-40.0, t0, MotionTiming::STATE, full()));
    assert_eq!(step.target(), 0.0);
}

#[test]
fn settle_stops_a_running_step_where_it_is_going() {
    let t0 = Instant::now();
    let mut step = MotionStep::at_rest(0.0);
    step.go(1.0, t0, MotionTiming::PANEL, full());
    assert!(!step.is_settled(t0 + ms(10)));
    step.settle();
    assert!(step.is_settled(t0 + ms(10)));
    assert_eq!(step.at(t0 + ms(10)), 1.0);
}

// ----------------------------------------------------------------- policy

#[test]
fn only_full_motion_animates() {
    let theme = truecolor();
    let policy = |mode| MotionPolicy::new(mode, &theme);
    assert!(policy(MotionMode::Full).animates());
    assert!(!policy(MotionMode::Reduced).animates());
    assert!(!policy(MotionMode::Still).animates());
    assert_eq!(policy(MotionMode::Reduced).mode(), MotionMode::Reduced);
    for channel in [MotionChannel::Space, MotionChannel::Tint] {
        assert!(policy(MotionMode::Full).allows(channel));
        assert!(!policy(MotionMode::Reduced).allows(channel));
        assert!(!policy(MotionMode::Still).allows(channel));
    }
}

#[test]
fn color_steps_run_at_truecolor_only() {
    for profile in Profile::ALL {
        let policy = MotionPolicy::new(MotionMode::Full, &profile.theme());
        let truecolor = matches!(
            profile,
            Profile::DarkTrue | Profile::DarkGraphite | Profile::LightTrue
        );
        assert_eq!(
            policy.allows(MotionChannel::Tint),
            truecolor,
            "{}",
            profile.name()
        );
        assert!(policy.allows(MotionChannel::Space), "{}", profile.name());
    }
}

#[test]
fn a_step_the_policy_forbids_jumps_to_its_target() {
    let t0 = Instant::now();
    for mode in [MotionMode::Reduced, MotionMode::Still] {
        let policy = MotionPolicy::new(mode, &truecolor());
        let mut step = MotionStep::at_rest(0.0);
        assert!(step.go(1.0, t0, MotionTiming::PANEL, policy));
        assert_eq!(step.at(t0), 1.0, "{mode:?}");
        assert!(step.is_settled(t0), "{mode:?}");
        assert_eq!(step.reveal(t0, policy, 20), 20, "{mode:?}");
        assert_eq!(step.next_frame_at(t0, policy), None, "{mode:?}");
    }
    // A color step at 256 colors is skipped, a spatial one still runs.
    let policy = MotionPolicy::new(MotionMode::Full, &Profile::Dark256.theme());
    let (mut tint, mut space) = (MotionStep::default(), MotionStep::default());
    tint.go(1.0, t0, MotionTiming::STATE, policy);
    space.go(1.0, t0, MotionTiming::PANEL, policy);
    assert!(tint.is_settled(t0));
    assert!(!space.is_settled(t0));
}

#[test]
fn a_mode_changed_mid_flight_leaves_no_ghost() {
    let t0 = Instant::now();
    let mut step = MotionStep::default();
    step.go(1.0, t0, MotionTiming::PANEL, full());
    let now = t0 + ms(50);
    assert!(step.reveal(now, full(), 40) < 40);
    let still = MotionPolicy::new(MotionMode::Still, &truecolor());
    assert_eq!(step.reveal(now, still, 40), 40, "shows the end state");
    assert_eq!(step.next_frame_at(now, still), None, "and asks for nothing");
}

// ---------------------------------------------------------------- helpers

#[test]
fn offset_and_reveal_map_progress_to_columns() {
    let t0 = Instant::now();
    let mut step = MotionStep::default();
    step.go(1.0, t0, timing(100, MotionEasing::Linear), full());
    assert_eq!(step.offset(t0, full(), 4, 14), 4);
    assert_eq!(step.offset(t0 + ms(50), full(), 4, 14), 9);
    assert_eq!(step.offset(t0 + ms(100), full(), 4, 14), 14);
    // Moving left works the same way.
    assert_eq!(step.offset(t0 + ms(50), full(), 20, 10), 15);
    assert_eq!(step.reveal(t0, full(), 30), 0);
    assert_eq!(step.reveal(t0 + ms(50), full(), 30), 15);
    assert_eq!(step.reveal(t0 + ms(100), full(), 30), 30);
    assert_eq!(step.reveal(t0 + ms(50), full(), 0), 0);
}

#[test]
fn a_reveal_never_exceeds_its_width_and_never_shrinks_going_out() {
    let t0 = Instant::now();
    let mut step = MotionStep::default();
    step.go(1.0, t0, MotionTiming::PANEL, full());
    let mut last = 0;
    for i in 0..=80 {
        let shown = step.reveal(t0 + ms(5 * i), full(), 37);
        assert!(shown <= 37 && shown >= last, "{shown} after {last}");
        last = shown;
    }
    assert_eq!(last, 37);
    assert_eq!(step.reveal(t0, full(), u16::MAX), 0);
    assert_eq!(
        step.reveal(t0 + Duration::from_secs(1), full(), u16::MAX),
        u16::MAX
    );
}

#[test]
fn a_color_step_is_exact_at_its_ends_and_a_blend_between() {
    let t0 = Instant::now();
    let theme = truecolor();
    let mut step = MotionStep::default();
    step.go(1.0, t0, MotionTiming::ARRIVE, full());
    let at = |ms_in| step.fg(t0 + ms(ms_in), full(), &theme, Role::Muted, Role::Live);
    assert_eq!(at(0), theme.fg(Role::Muted));
    assert_eq!(at(180), theme.fg(Role::Live));
    let mid = at(60);
    assert_ne!(mid, theme.fg(Role::Muted));
    assert_ne!(mid, theme.fg(Role::Live));
    // Mid-way is never a raw color the theme could not have produced: it is
    // a mix, so each channel lies between the two roles' channels.
    let (a, b, m) = (theme.fg(Role::Muted).fg, theme.fg(Role::Live).fg, mid.fg);
    let rgb = |c: Option<ratatui::style::Color>| match c {
        Some(ratatui::style::Color::Rgb(r, g, b)) => [r, g, b],
        other => panic!("expected truecolor, got {other:?}"),
    };
    for ((a, b), m) in rgb(a).into_iter().zip(rgb(b)).zip(rgb(m)) {
        assert!(m >= a.min(b) && m <= a.max(b), "{m} outside {a}..{b}");
    }
}

#[test]
fn a_color_step_snaps_where_it_cannot_blend() {
    let t0 = Instant::now();
    for profile in Profile::ALL {
        let theme = profile.theme();
        let own = MotionPolicy::new(MotionMode::Full, &theme);
        if own.allows(MotionChannel::Tint) {
            continue;
        }
        let mut step = MotionStep::default();
        step.go(1.0, t0, MotionTiming::ARRIVE, full());
        for ms_in in [0, 30, 89, 90, 150, 180] {
            let now = t0 + ms(ms_in);
            // Under this terminal's own policy the step never ran: Live.
            assert_eq!(
                step.fg(now, own, &theme, Role::Muted, Role::Live),
                theme.fg(Role::Live),
                "{} at {ms_in} ms",
                profile.name()
            );
            // Even when a truecolor policy had it in flight, painting for
            // this terminal picks one of the two roles and invents no color.
            let got = step.fg(now, full(), &theme, Role::Muted, Role::Live);
            assert!(
                got == theme.fg(Role::Muted) || got == theme.fg(Role::Live),
                "{} at {ms_in} ms painted {got:?}",
                profile.name()
            );
        }
    }
}

#[test]
fn a_ground_that_does_not_paint_is_never_blended_through() {
    let t0 = Instant::now();
    let theme = truecolor().without_base_ground();
    let mut step = MotionStep::default();
    step.go(1.0, t0, MotionTiming::ARRIVE, full());
    let mid = step.bg(
        t0 + ms(60),
        full(),
        &theme,
        Role::Background,
        Role::Selected,
    );
    assert!(
        mid == theme.bg(Role::Background) || mid == theme.bg(Role::Selected),
        "{mid:?}"
    );
    let settled = step.bg(
        t0 + ms(180),
        full(),
        &theme,
        Role::Background,
        Role::Selected,
    );
    assert_eq!(settled, theme.bg(Role::Selected));
    // Two painted grounds do blend.
    let blended = step.bg(
        t0 + ms(60),
        full(),
        &truecolor(),
        Role::Surface,
        Role::Selected,
    );
    assert_ne!(blended, truecolor().bg(Role::Surface));
    assert_ne!(blended, truecolor().bg(Role::Selected));
}

// ------------------------------------------------------------------- set

#[test]
fn a_set_keeps_named_motions() {
    let t0 = Instant::now();
    let mut set = MotionSet::new();
    assert_eq!(set.step("slide").target(), 0.0, "unknown names rest at 0");
    assert!(set.go("slide", 1.0, t0, MotionTiming::ARRIVE, full()));
    assert!(set.go("reveal", 1.0, t0, MotionTiming::PANEL, full()));
    assert!(!set.go("slide", 1.0, t0 + ms(10), MotionTiming::ARRIVE, full()));
    assert_eq!(set.step("slide").target(), 1.0);
    assert!(!set.is_settled(t0 + ms(200)), "reveal is still running");
    assert!(set.is_settled(t0 + ms(340)));
    set.go("slide", 0.0, t0 + ms(400), MotionTiming::EXIT, full());
    assert!(!set.is_settled(t0 + ms(410)));
}

#[test]
fn input_settles_every_motion_at_once() {
    let t0 = Instant::now();
    let mut set = MotionSet::new();
    set.go("a", 1.0, t0, MotionTiming::PANEL, full());
    set.go("b", 1.0, t0, MotionTiming::ARRIVE, full());
    set.settle_all();
    assert!(set.is_settled(t0));
    assert_eq!(set.next_frame_at(t0, full()), None);
    assert_eq!(set.step("a").at(t0), 1.0);
    assert_eq!(set.step("b").at(t0), 1.0);
}

// -------------------------------------------------------------- schedule

#[test]
fn an_idle_set_asks_for_no_redraws() {
    let t0 = Instant::now();
    let set = MotionSet::new();
    for second in 0..=5 {
        for mode in MODES {
            let policy = MotionPolicy::new(mode, &truecolor());
            assert_eq!(
                set.next_frame_at(t0 + Duration::from_secs(second), policy),
                None
            );
        }
    }
}

#[test]
fn a_live_set_asks_for_the_next_frame_then_nothing_once_settled() {
    let t0 = Instant::now();
    let mut set = MotionSet::new();
    set.go("panel", 1.0, t0, MotionTiming::PANEL, full());
    assert_eq!(
        set.next_frame_at(t0, full()),
        Some(t0 + MOTION_FRAME_INTERVAL)
    );
    assert!(set.next_frame_at(t0 + ms(339), full()).is_some());
    assert_eq!(set.next_frame_at(t0 + ms(340), full()), None);
    assert_eq!(set.next_frame_at(t0 + Duration::from_secs(5), full()), None);
}

/// Drive the loop the way a host does: draw, ask, advance to the answer.
/// Returns the instants frames were drawn at.
fn run(set: &MotionSet, t0: Instant, policy: MotionPolicy) -> Vec<Instant> {
    let mut drawn = vec![t0];
    let mut now = t0;
    while let Some(next) = set.next_frame_at(now, policy) {
        assert!(next > now, "a frame due now or earlier would spin");
        now = next;
        drawn.push(now);
        assert!(drawn.len() < 1000, "never settled");
    }
    drawn
}

#[test]
fn frames_are_never_closer_than_the_cap_and_the_last_one_is_settled() {
    let t0 = Instant::now();
    for interval in [ms(1), MOTION_FRAME_INTERVAL, ms(33), ms(100)] {
        let mut set = MotionSet::new().with_frame_interval(interval);
        set.go("a", 1.0, t0, MotionTiming::PANEL, full());
        set.go("b", 1.0, t0 + ms(40), MotionTiming::ARRIVE, full());
        let cap = interval.max(MOTION_MIN_FRAME_INTERVAL);
        let drawn = run(&set, t0, full());
        for pair in drawn.windows(2) {
            assert!(
                pair[1] - pair[0] >= cap,
                "{interval:?}: {:?}",
                pair[1] - pair[0]
            );
        }
        let last = *drawn.last().expect("at least the first frame");
        assert!(set.is_settled(last));
        assert_eq!(set.step("a").at(last), 1.0);
        // Bounded: a 340 ms step needs about 340 / cap frames, not more.
        let bound = (ms(340).as_micros() / cap.as_micros()) as usize + 3;
        assert!(
            drawn.len() <= bound,
            "{} frames at {interval:?}",
            drawn.len()
        );
    }
}

#[test]
fn a_cap_faster_than_the_floor_is_raised_to_it() {
    let t0 = Instant::now();
    let mut set = MotionSet::new().with_frame_interval(Duration::ZERO);
    set.go("a", 1.0, t0, MotionTiming::PANEL, full());
    assert_eq!(
        set.next_frame_at(t0, full()),
        Some(t0 + MOTION_MIN_FRAME_INTERVAL)
    );
}

#[test]
fn reduced_and_still_schedule_nothing_even_for_a_running_step() {
    let t0 = Instant::now();
    let mut set = MotionSet::new();
    set.go("a", 1.0, t0, MotionTiming::PANEL, full());
    for mode in [MotionMode::Reduced, MotionMode::Still] {
        let policy = MotionPolicy::new(mode, &truecolor());
        assert_eq!(set.next_frame_at(t0, policy), None, "{mode:?}");
        // Started under that policy, it never ran at all.
        let mut fresh = MotionSet::new();
        fresh.go("a", 1.0, t0, MotionTiming::PANEL, policy);
        assert!(fresh.is_settled(t0), "{mode:?}");
    }
}

#[test]
fn a_color_step_at_256_colors_schedules_nothing() {
    let t0 = Instant::now();
    let policy = MotionPolicy::new(MotionMode::Full, &Profile::Dark256.theme());
    let mut set = MotionSet::new();
    set.go("ink", 1.0, t0, MotionTiming::ARRIVE, policy);
    assert_eq!(set.next_frame_at(t0, policy), None);
    set.go("slide", 1.0, t0, MotionTiming::PANEL, policy);
    assert!(set.next_frame_at(t0, policy).is_some());
}

#[test]
fn the_soonest_of_several_sets_wins_and_none_means_none() {
    let t0 = Instant::now();
    let mut a = MotionSet::new();
    let mut b = MotionSet::new().with_frame_interval(ms(100));
    a.go("x", 1.0, t0, MotionTiming::PANEL, full());
    b.go("x", 1.0, t0, MotionTiming::PANEL, full());
    let due = codewhale_ratatui::soonest_frame([
        a.next_frame_at(t0, full()),
        b.next_frame_at(t0, full()),
    ]);
    assert_eq!(due, Some(t0 + MOTION_FRAME_INTERVAL));
    let idle = MotionSet::new();
    assert_eq!(
        codewhale_ratatui::soonest_frame([idle.next_frame_at(t0, full()), None]),
        None
    );
    assert_eq!(codewhale_ratatui::soonest_frame([]), None);
}

#[test]
fn a_frame_budget_keeps_the_earliest_request_and_idle_asks_for_nothing() {
    let t0 = Instant::now();
    let mut budget = FrameBudget::new();
    assert_eq!(budget.next_frame_in(), None);
    budget.request(None);
    budget.request_at(t0, None);
    assert_eq!(budget.next_frame_in(), None);
    budget.request(Some(ms(150)));
    budget.request(Some(ms(40)));
    budget.request(Some(ms(90)));
    assert_eq!(budget.next_frame_in(), Some(ms(40)));
    // A due instant already past asks for a frame now.
    budget.request_at(t0 + ms(500), Some(t0));
    assert_eq!(budget.next_frame_in(), Some(Duration::ZERO));
    let mut set = MotionSet::new();
    set.go("a", 1.0, t0, MotionTiming::PANEL, full());
    let mut budget = FrameBudget::new();
    budget.request_at(t0, set.next_frame_at(t0, full()));
    assert_eq!(budget.next_frame_in(), Some(MOTION_FRAME_INTERVAL));
}

#[test]
fn a_frame_budget_allows_one_spinner_and_one_horizon() {
    let mut budget = FrameBudget::new();
    assert!(budget.claim_spinner());
    assert!(!budget.claim_spinner());
    assert!(!budget.claim_spinner());
    assert!(budget.claim_horizon());
    assert!(!budget.claim_horizon());
    // A new frame starts with a fresh budget.
    assert!(FrameBudget::new().claim_spinner());
}

// -------------------------------------------------------------- property

#[test]
fn any_sequence_of_steps_stays_finite_in_range_continuous_and_ends() {
    let mut rng = Lcg(0x5EED);
    let t0 = Instant::now();
    let theme = truecolor();
    for case in 0..3000 {
        let mode = MODES[rng.below(3) as usize];
        let policy = MotionPolicy::new(mode, &theme);
        let mut set = MotionSet::new();
        let mut step = MotionStep::default();
        let mut now = t0;
        for _ in 0..1 + rng.below(6) {
            now += ms(rng.below(300));
            let target = match rng.below(12) {
                0 => f32::NAN,
                1 => f32::INFINITY,
                2 => -rng.unit() * 5.0,
                3 => 1.0 + rng.unit() * 5.0,
                _ => rng.unit(),
            };
            let easing = [
                MotionEasing::Linear,
                MotionEasing::EaseOut,
                MotionEasing::EaseIn,
            ][rng.below(3) as usize];
            let timing = if rng.below(2) == 0 {
                MotionTiming::space(ms(rng.below(500)), easing)
            } else {
                MotionTiming::tint(ms(rng.below(500)), easing)
            };
            let before = step.at(now);
            step.go(target, now, timing, policy);
            set.go("m", target, now, timing, policy);
            let after = step.at(now);
            assert!(before.is_finite() && after.is_finite(), "case {case}");
            if policy.allows(timing.channel) && !timing.duration.is_zero() {
                assert!(
                    (before - after).abs() < 1e-5,
                    "case {case}: {before} jumped to {after}"
                );
            }
            for probe in [0, 1, 17, 90, 250, 499, 500, 10_000] {
                let t = now + ms(probe);
                let v = step.at(t);
                assert!((0.0..=1.0).contains(&v), "case {case}: {v} at +{probe} ms");
                assert!(
                    (0.0..=1.0).contains(&step.target()),
                    "case {case}: target {}",
                    step.target()
                );
                let width = rng.below(200) as u16;
                assert!(step.reveal(t, policy, width) <= width, "case {case}");
                let (from, to) = (rng.below(300) as u16, rng.below(300) as u16);
                let col = step.offset(t, policy, from, to);
                assert!(col >= from.min(to) && col <= from.max(to), "case {case}");
                // Painting never panics for any pair of roles.
                let _ = step.fg(t, policy, &theme, Role::Muted, Role::Live);
                let _ = step.bg(t, policy, &theme, Role::Surface, Role::Selected);
            }
        }
        // It ends: the host's loop reaches a draw with nothing left to ask.
        let drawn = run(&set, now, policy);
        let last = *drawn.last().expect("a first frame");
        assert!(set.is_settled(last), "case {case}");
        assert!(drawn.len() < 100, "case {case}: {} frames", drawn.len());
        assert_eq!(step.at(last), step.target(), "case {case}");
        assert_eq!(
            set.step("m").at(last),
            set.step("m").target(),
            "case {case}"
        );
    }
}

// ------------------------------------------------------------ the demo

/// Paint the demo `after` its state change to done under `mode`.
fn demo(t0: Instant, mode: MotionMode, after: Duration) -> impl Fn(Rect, &mut Buffer, &Theme) {
    move |area, buf, theme| {
        let policy = MotionPolicy::new(mode, theme);
        let mut motions = MotionSet::new();
        MotionDemo::start(&mut motions, true, t0, policy);
        MotionDemo::new(&motions, t0 + after, policy, true).paint(area, buf, theme);
    }
}

fn buffers(t0: Instant, mode: MotionMode, after: Duration) -> Vec<Buffer> {
    testing::frames_for("demo", &Profile::ALL, &WIDTHS, 3, demo(t0, mode, after))
        .into_iter()
        .map(|f| f.buf)
        .collect()
}

#[test]
fn reduced_and_still_paint_exactly_the_end_state_of_full() {
    let t0 = Instant::now();
    let end = buffers(t0, MotionMode::Full, ms(1000));
    for mode in [MotionMode::Reduced, MotionMode::Still] {
        for after in [0, 1, 60, 100, 179, 340, 1000] {
            let got = buffers(t0, mode, ms(after));
            for (i, (got, want)) in got.iter().zip(&end).enumerate() {
                assert_eq!(
                    got, want,
                    "{mode:?} +{after} ms differs from Full's end (frame {i})"
                );
            }
        }
    }
}

#[test]
fn full_differs_from_the_end_state_while_in_flight_and_agrees_once_settled() {
    let t0 = Instant::now();
    let end = buffers(t0, MotionMode::Full, ms(1000));
    assert_eq!(buffers(t0, MotionMode::Full, ms(340)), end);
    for after in [0, 40, 100, 179] {
        assert_ne!(buffers(t0, MotionMode::Full, ms(after)), end, "+{after} ms");
    }
    // At 256 colors the ink does not blend, but the marker still slides.
    let dark256 = |after| {
        testing::frames_for(
            "demo",
            &[Profile::Dark256],
            &[80],
            3,
            demo(t0, MotionMode::Full, ms(after)),
        )
    };
    assert_ne!(dark256(60)[0].buf, dark256(1000)[0].buf);
}

#[test]
fn the_demo_keeps_the_frame_rules_at_every_moment() {
    let t0 = Instant::now();
    for mode in MODES {
        for after in [0, 60, 100, 500] {
            testing::assert_rules(3, demo(t0, mode, ms(after)));
        }
    }
}

#[test]
fn the_demo_says_its_state_in_a_mark_and_a_word_at_every_moment() {
    let t0 = Instant::now();
    for after in [0, 60, 500] {
        for frame in testing::frames_for(
            "demo",
            &Profile::ALL,
            &WIDTHS,
            3,
            demo(t0, MotionMode::Full, ms(after)),
        ) {
            let first = frame.text().lines().next().unwrap_or_default().to_string();
            let mark = pick(DONE, frame.profile == Profile::Ascii);
            assert!(first.starts_with(mark), "{}: {first}", frame.label());
            assert!(first.contains("Done"), "{}: {first}", frame.label());
        }
    }
}

#[test]
fn the_demo_clips_to_its_area_and_cleans_caller_words() {
    let t0 = Instant::now();
    let words = MotionDemoWords {
        working: "Working".into(),
        done: "Done\u{202e}gnihton".into(),
        slide: "A very long selected row label that cannot fit".into(),
        detail: "日本語の長い説明文が続きます and then some more words to clip".into(),
    };
    for width in [1u16, 2, 5, 12, 40] {
        for height in [0u16, 1, 2, 3, 5] {
            let theme = truecolor();
            let policy = MotionPolicy::new(MotionMode::Full, &theme);
            let mut motions = MotionSet::new();
            MotionDemo::start(&mut motions, true, t0, policy);
            for after in [0, 60, 1000] {
                let area = Rect::new(0, 0, width, height);
                let mut buf = Buffer::empty(area);
                MotionDemo::new(&motions, t0 + ms(after), policy, true)
                    .with_words(words.clone())
                    .paint(area, &mut buf, &theme);
                assert!(!testing::text(&buf).contains('\u{202e}'));
            }
        }
    }
}

#[test]
fn the_demo_is_three_rows() {
    let set = MotionSet::new();
    let theme = truecolor();
    let policy = MotionPolicy::new(MotionMode::Full, &theme);
    assert_eq!(
        MotionDemo::new(&set, Instant::now(), policy, false).height(40, &theme),
        3
    );
}

#[test]
fn the_demo_working_state_is_at_rest() {
    // Not started: working, marker at home, nothing revealed, no frames due.
    let t0 = Instant::now();
    let set = MotionSet::new();
    let theme = truecolor();
    let policy = MotionPolicy::new(MotionMode::Full, &theme);
    let buf = testing::render(40, 3, |area, buf| {
        MotionDemo::new(&set, t0, policy, false).paint(area, buf, &theme);
    });
    let text = testing::text(&buf);
    let rows: Vec<&str> = text.lines().collect();
    assert!(rows[0].starts_with("● Working"), "{rows:?}");
    assert!(rows[1].starts_with("▸ Selected"), "{rows:?}");
    assert_eq!(rows.get(2).copied().unwrap_or(""), "");
    assert_eq!(set.next_frame_at(t0, policy), None);
}

#[test]
fn snapshot_at_the_start() {
    let t0 = Instant::now();
    insta::assert_snapshot!(
        "demo-start",
        testing::snapshot(3, demo(t0, MotionMode::Full, Duration::ZERO))
    );
}

#[test]
fn snapshot_in_flight() {
    let t0 = Instant::now();
    insta::assert_snapshot!(
        "demo-mid-flight",
        testing::snapshot(3, demo(t0, MotionMode::Full, ms(100)))
    );
}

#[test]
fn snapshot_settled() {
    let t0 = Instant::now();
    insta::assert_snapshot!(
        "demo-settled",
        testing::snapshot(3, demo(t0, MotionMode::Full, ms(500)))
    );
}

#[test]
fn snapshot_reduced_is_the_settled_frame() {
    let t0 = Instant::now();
    let reduced = testing::snapshot(3, demo(t0, MotionMode::Reduced, ms(100)));
    assert_eq!(
        reduced,
        testing::snapshot(3, demo(t0, MotionMode::Full, ms(500)))
    );
}
