use codewhale_ratatui::{
    AgentCard, MotionMode, PetMode, PetModeAreas, PetModeState, State, StatusMark, Subagent,
    SubagentControls, SubagentIntent,
    testing::{self, Profile},
    whale_motion::{Context, Inputs, Presence},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
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
fn streamed_output_follows_without_displacing_a_reader() {
    let theme = Profile::DarkTrue.theme();
    let now = Instant::now();
    let mut state = PetModeState::default();
    state.set_visible(true);
    state.update(
        Some("session"),
        inputs(Presence::Working),
        &[],
        now,
        MotionMode::Reduced,
    );
    let paint = |state: &mut PetModeState, count| {
        let output: Vec<_> = (0..count)
            .map(|n| Line::raw(format!("Stream row {n}")))
            .collect();
        text(&testing::render(40, 12, |area, buf| {
            let mut view = PetMode::new(&theme, StatusMark::new(State::Working));
            view.output = &output;
            view.render(area, buf, state);
        }))
    };
    assert!(paint(&mut state, 20).contains("Stream row 19"));
    assert!(paint(&mut state, 30).contains("Stream row 29"));
    state.scroll(&[], -2);
    let top = state.output_scroll;
    assert!(!paint(&mut state, 40).contains("Stream row 39"));
    assert_eq!(state.output_scroll, top);
    state.scroll_end(&[], false);
    assert!(paint(&mut state, 41).contains("Stream row 0"));
    state.scroll_end(&[], true);
    assert!(paint(&mut state, 42).contains("Stream row 41"));
    state.scroll(&[], -1);
    state.scroll(&[], 1);
    assert!(paint(&mut state, 43).contains("Stream row 42"));
    assert!(state.handle_mouse(
        &[],
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 1,
            row: 5,
            modifiers: KeyModifiers::NONE,
        }
    ));
    let top = state.output_scroll;
    assert!(!paint(&mut state, 44).contains("Stream row 43"));
    assert_eq!(state.output_scroll, top);
    state.reset_output();
    assert!(paint(&mut state, 45).contains("Stream row 44"));
    state.scroll_end(&[], false);
    let mut next = inputs(Presence::Working);
    next.context.turn_id = Some("next-turn".into());
    state.update(Some("session"), next, &[], now, MotionMode::Reduced);
    assert!(paint(&mut state, 10).contains("Stream row 9"));
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
    for (width, height) in [(40, 10), (80, 24), (15, 40)] {
        let plan = PetModeAreas::new(Rect::new(0, 0, width, height), false, true, false);
        assert!(plan.pet.is_empty());
        assert_eq!(plan.output.height, height - 4);
    }
    for height in 0..=4 {
        let plan = PetModeAreas::new(Rect::new(0, 0, 40, height), false, false, false);
        assert!(plan.header.bottom() <= plan.footer.y);
    }
    let one = testing::render(40, 1, |area, buf| {
        PetMode::new(&theme, StatusMark::new(State::Done)).render(area, buf, &mut state);
    });
    assert!(text(&one).contains("Done"));
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
    // A retained parent completion cannot freeze current background work.
    let mut working = inputs(Presence::Working);
    working.context.status = Some("completed".into());
    state.update(Some("session"), working, &[], now, MotionMode::Full);
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
    for frame in 1..=80 {
        state.update(
            Some("session"),
            inputs(Presence::Done),
            &[],
            now + Duration::from_millis(frame * 50),
            MotionMode::Full,
        );
    }
    assert_eq!(state.next_frame_in(&[], &theme), None);
}
#[test]
fn roster_open_is_an_intent_and_session_change_discards_selection_and_scroll() {
    let mut agent = Subagent::new("worker", AgentCard::new("Worker", State::Working));
    agent.controls = SubagentControls {
        open: true,
        ..Default::default()
    };
    agent.controls.message = true;
    agent.controls.stop = true;
    let mut agents = vec![agent];
    agents.extend((1..5).map(|id| {
        Subagent::new(
            format!("worker-{id}"),
            AgentCard::new(format!("Worker {id}"), State::Working),
        )
    }));
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
    assert_eq!(
        state.handle_key(&agents, KeyEvent::from(KeyCode::Char('m'))),
        Some(SubagentIntent::Message("worker".into()))
    );
    assert_eq!(
        state.handle_key(&agents, KeyEvent::from(KeyCode::Char('x'))),
        Some(SubagentIntent::Stop("worker".into()))
    );
    state.scroll(&agents, 0);
    assert_eq!(state.selected_agent_id(), Some("worker"));
    state.scroll(&agents, 3);
    assert_eq!(state.selected_agent_id(), Some("worker-3"));
    state.scroll(&agents, -2);
    assert_eq!(state.selected_agent_id(), Some("worker-1"));
    assert_eq!(
        state.handle_key(&agents, KeyEvent::from(KeyCode::Char('x'))),
        None
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

#[test]
fn only_the_canonical_new_completed_turn_schedules_a_flourish() {
    let theme = Profile::DarkTrue.theme();
    let now = Instant::now();
    let mut state = PetModeState::default();
    state.set_visible(true);
    state.update(
        Some("session"),
        inputs(Presence::Working),
        &[],
        now,
        MotionMode::Full,
    );
    testing::render(100, 32, |area, buf| {
        PetMode::new(&theme, StatusMark::new(State::Working)).render(area, buf, &mut state);
    });
    let mut failed = inputs(Presence::Done);
    failed.context.status = Some("failed".into());
    state.update(Some("session"), failed, &[], now, MotionMode::Full);
    assert_eq!(state.next_frame_in(&[], &theme), None);
    let mut anonymous = inputs(Presence::Done);
    anonymous.context.turn_id = None;
    state.update(Some("session"), anonymous, &[], now, MotionMode::Full);
    assert_eq!(state.next_frame_in(&[], &theme), None);
    let done = inputs(Presence::Done);
    state.update(Some("session"), done.clone(), &[], now, MotionMode::Full);
    assert!(state.next_frame_in(&[], &theme).is_some());
    for frame in 1..=80 {
        state.update(
            Some("session"),
            done.clone(),
            &[],
            now + Duration::from_millis(frame * 50),
            MotionMode::Full,
        );
    }
    assert_eq!(state.next_frame_in(&[], &theme), None);
    state.update(
        Some("session"),
        inputs(Presence::Listening),
        &[],
        now + Duration::from_secs(4),
        MotionMode::Full,
    );
    state.update(
        Some("session"),
        done.clone(),
        &[],
        now + Duration::from_secs(4),
        MotionMode::Full,
    );
    assert_eq!(state.next_frame_in(&[], &theme), None);
    let mut next_done = done;
    next_done.context.turn_id = Some("next-actual-turn".into());
    state.update(
        Some("session"),
        next_done,
        &[],
        now + Duration::from_secs(4),
        MotionMode::Full,
    );
    assert!(state.next_frame_in(&[], &theme).is_some());
}

#[test]
fn populated_roster_and_response_remain_readable_at_wide_and_compact_sizes() {
    let theme = Profile::DarkTrue.theme();
    let agent = Subagent::new(
        "fixture-worker",
        AgentCard::new("Fixture worker", State::NeedsYou),
    );
    let agents = [agent];
    let output = [Line::raw("Fixture response")];
    for (width, height) in [(40, 12), (80, 24), (120, 40)] {
        let mut state = PetModeState::default();
        state.set_visible(true);
        state.update(
            Some("fixture-session"),
            inputs(Presence::NeedsYou),
            &agents,
            Instant::now(),
            MotionMode::Reduced,
        );
        let visible = text(&testing::render(width, height, |area, buf| {
            let mut view = PetMode::new(&theme, StatusMark::new(State::NeedsYou));
            view.agents = &agents;
            view.output = &output;
            view.render(area, buf, &mut state);
        }));
        assert!(visible.contains("Fixture response"));
        if width >= 100 {
            assert!(visible.contains("Fixture worker"));
        } else {
            state.toggle_agents(&agents);
            let focused = testing::render(width, height, |area, buf| {
                let mut view = PetMode::new(&theme, StatusMark::new(State::NeedsYou));
                view.agents = &agents;
                view.output = &output;
                view.render(area, buf, &mut state);
            });
            assert!(text(&focused).contains("Fixture worker"));
        }
    }
}

#[test]
fn clicking_reply_changes_focus_without_opening_or_messaging_an_agent() {
    let theme = Profile::Ascii.theme();
    let agents = [Subagent::new(
        "worker",
        AgentCard::new("Worker", State::Working),
    )];
    let mut state = PetModeState::default();
    state.set_visible(true);
    state.update(
        Some("session"),
        inputs(Presence::Working),
        &agents,
        Instant::now(),
        MotionMode::Still,
    );
    state.focus_agents = true;
    let output = [Line::raw("actual reply")];
    testing::render(140, 40, |area, buf| {
        let mut view = PetMode::new(&theme, StatusMark::new(State::Working));
        view.agents = &agents;
        view.output = &output;
        view.render(area, buf, &mut state);
    });
    let output = PetModeAreas::new(Rect::new(0, 0, 140, 40), true, true, true).output;
    assert!(state.handle_mouse(
        &agents,
        MouseEvent {
            kind: MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: output.x,
            row: output.y,
            modifiers: KeyModifiers::NONE,
        }
    ));
    assert!(!state.focus_agents);
    assert!(
        state
            .handle_key(&agents, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .is_none()
    );
}

#[test]
fn focused_reading_and_clipped_hints_stay_legible_across_terminal_profiles() {
    for profile in [
        Profile::DarkTrue,
        Profile::LightTrue,
        Profile::NoColor,
        Profile::Ascii,
    ] {
        let theme = profile.theme();
        let agents = [Subagent::new(
            "worker",
            AgentCard::new("Worker", State::Working),
        )];
        let output: Vec<_> = (1..=90)
            .map(|row| Line::raw(format!("Reply row {row}")))
            .collect();
        let mut state = PetModeState::default();
        state.set_visible(true);
        state.update(
            Some("session"),
            inputs(Presence::Working),
            &agents,
            Instant::now(),
            MotionMode::Still,
        );
        for focus_agents in [false, true] {
            state.focus_agents = focus_agents;
            let buf = testing::render(140, 40, |area, buf| {
                let mut view = PetMode::new(&theme, StatusMark::new(State::Working));
                view.agents = &agents;
                view.agent_words.title = "Agents".into();
                view.output = &output;
                view.output_title = "Reply".into();
                view.hints = "Esc message · Tab switch view".into();
                view.pane_hints = "Up/Down select · Enter transcript".into();
                view.render(area, buf, &mut state);
            });
            let marker = if theme.ascii() { "> " } else { "› " };
            let rendered = text(&buf);
            assert!(rendered.contains(&format!(
                "{marker}{}",
                if focus_agents { "Agents" } else { "Reply" }
            )));
            assert!(rendered.contains("68-90 / 90"));
            assert!(rendered.contains("Reply row 90"));
            let plan = PetModeAreas::new(buf.area, true, true, focus_agents);
            assert_eq!(
                buf[(plan.agents.x - 1, plan.agents.y)].symbol(),
                if theme.ascii() { "|" } else { "│" }
            );
            assert!(rendered.contains("Esc message"));
            assert!(rendered.contains("Enter transcript"));
        }
        let buf = testing::render(12, 14, |area, buf| {
            let mut view = PetMode::new(&theme, StatusMark::new(State::NeedsYou).word("Needs you"));
            view.title = "A title that cannot fit".into();
            view.hints = "Esc message · Tab switch view".into();
            view.pane_hints = "Up select".into();
            view.render(area, buf, &mut state);
        });
        let rendered = text(&buf);
        assert!(rendered.contains("Needs you"));
        assert!(rendered.contains("Up select"));
        assert!(rendered.contains(if theme.ascii() { "..." } else { "…" }));
    }
}
