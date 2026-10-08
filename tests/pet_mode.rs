use codewhale_ratatui::{
    AgentCard, MotionMode, PetMode, PetModeAreas, PetModeState, State, StatusMark, Subagent,
    SubagentControls, SubagentIntent,
    testing::{self, Profile},
    whale_motion::{Context, Inputs, Presence},
};
use ratatui::{layout::Rect, text::Line, widgets::StatefulWidget};
use std::time::{Duration, Instant};
fn inputs(presence: Presence) -> Inputs {
    Inputs {
        presence,
        activity: None,
        context: Context {
            turn_id: Some("actual-turn-id".into()),
            status: (presence == Presence::Done).then(|| "completed".into()),
            ..Default::default()
        },
    }
}
fn text(buf: &ratatui::buffer::Buffer) -> String {
    buf.content.iter().map(|cell| cell.symbol()).collect()
}
#[test]
fn compact_response_wins_over_scenery_and_long_output_reaches_the_end() {
    let theme = Profile::DarkTrue.theme();
    let output: Vec<_> = (0..70_002)
        .map(|n| Line::raw(format!("Response row {n}")))
        .collect();
    let mut state = PetModeState::default();
    state.set_visible(true);
    state.update(
        Some("session"),
        inputs(Presence::Done),
        &[],
        Instant::now(),
        MotionMode::Reduced,
    );
    state.scroll_end(&[], true);
    let buf = testing::render(40, 10, |area, buf| {
        let mut view = PetMode::new(&theme, StatusMark::new(State::Done));
        view.output = &output;
        view.render(area, buf, &mut state);
    });
    assert!(text(&buf).contains("Response row 70001"));
    assert!(
        PetModeAreas::new(Rect::new(0, 0, 40, 10), false, true, false)
            .pet
            .is_empty()
    );
}
#[test]
fn hidden_and_reduced_and_completed_surfaces_schedule_no_motion() {
    let theme = Profile::DarkTrue.theme();
    let now = Instant::now();
    let mut state = PetModeState::default();
    state.update(
        Some("session"),
        inputs(Presence::Working),
        &[],
        now,
        MotionMode::Full,
    );
    assert_eq!(state.next_frame_in(&[], &theme), None);
    state.set_visible(true);
    testing::render(100, 32, |area, buf| {
        PetMode::new(&theme, StatusMark::new(State::Working)).render(area, buf, &mut state)
    });
    state.update(
        Some("session"),
        inputs(Presence::Working),
        &[],
        now,
        MotionMode::Full,
    );
    assert!(state.next_frame_in(&[], &theme).is_some());
    state.update(
        Some("session"),
        inputs(Presence::Working),
        &[],
        now,
        MotionMode::Reduced,
    );
    assert_eq!(state.next_frame_in(&[], &theme), None);
    state.update(
        Some("session"),
        inputs(Presence::Done),
        &[],
        now,
        MotionMode::Full,
    );
    state.update(
        Some("session"),
        inputs(Presence::Done),
        &[],
        now + Duration::from_secs(2),
        MotionMode::Full,
    );
    assert_eq!(state.next_frame_in(&[], &theme), None);
}
#[test]
fn roster_open_is_an_intent_and_session_change_discards_selection_and_scroll() {
    let mut agent = Subagent::new("worker", AgentCard::new("Worker", State::Working));
    agent.controls = SubagentControls {
        open: true,
        ..Default::default()
    };
    let agents = [agent];
    let mut state = PetModeState::default();
    state.set_visible(true);
    state.update(
        Some("one"),
        inputs(Presence::Working),
        &agents,
        Instant::now(),
        MotionMode::Reduced,
    );
    assert_eq!(state.open_agent(&agents), None);
    state.toggle_agents(&agents);
    assert_eq!(
        state.open_agent(&agents),
        Some(SubagentIntent::Open("worker".into()))
    );
    state.output_scroll = 20;
    state.update(
        Some("two"),
        inputs(Presence::Idle),
        &[],
        Instant::now(),
        MotionMode::Reduced,
    );
    assert_eq!(state.selected_agent_id(), None);
    assert_eq!(state.output_scroll, 0);
    assert!(!state.focus_agents);
}
