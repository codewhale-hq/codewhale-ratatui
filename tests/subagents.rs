use codewhale_ratatui::{
    AgentCard, CountBar, MotionMode, State, Subagent, SubagentControls, SubagentEvent,
    SubagentIntent, SubagentView, SubagentViewState, SubagentViewWords,
    testing::{self, Profile},
    whale_motion::{Activity, Context, Inputs, Presence},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    widgets::StatefulWidget,
};
use std::time::{Duration, Instant};

fn fixture(id: &str, status: State) -> Subagent<'static> {
    let mut agent = Subagent::new(
        id.to_owned(),
        AgentCard::new(format!("Agent {id}"), status)
            .task("Inspect session receipts and retain history")
            .role("Review")
            .route("owner model"),
    );
    agent.elapsed = Some(Duration::from_secs(83));
    agent.tokens = Some("12k tokens".into());
    agent.progress = Some(CountBar::new(3, 7).state(status).label("checks"));
    agent.events = vec![SubagentEvent::new(
        "00:10",
        State::Done,
        "Saved the receipt",
    )];
    agent.performance = Some(Inputs {
        presence: if status == State::Done {
            Presence::Done
        } else {
            Presence::Working
        },
        activity: Some(Activity {
            kind: Some("reading".into()),
            observed: true,
            ..Activity::default()
        }),
        context: Context {
            live: true,
            turn_id: Some(id.into()),
            status: (status == State::Done).then(|| "completed".into()),
            ..Context::default()
        },
    });
    agent
}
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn render(
    agents: &[Subagent<'_>],
    state: &mut SubagentViewState,
    profile: Profile,
    width: u16,
    height: u16,
) -> Buffer {
    testing::render(width, height, |area, buf| {
        SubagentView::new(agents, &profile.theme()).render(area, buf, state)
    })
}

#[test]
fn focus_follows_identity_through_reorder_and_removal_and_keeps_history() {
    let now = Instant::now();
    let mut agents = vec![
        fixture("a", State::Working),
        fixture("b", State::Done),
        fixture("c", State::NeedsYou),
    ];
    let mut state = SubagentViewState::default();
    state.update(&agents, now, MotionMode::Full);
    assert!(state.select(&agents, "b"));
    agents.swap(0, 1);
    state.update(&agents, now + Duration::from_secs(3600), MotionMode::Full);
    assert_eq!(state.selected_id(), Some("b"));
    assert_eq!(state.selected_index(), Some(0));
    let output = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 112, 38));
    assert!(output.contains("Agent b"));
    assert!(output.contains("Saved the receipt"));
    agents.remove(0);
    state.update(&agents, now, MotionMode::Full);
    assert_eq!(state.selected_id(), Some("a"));
    agents.clear();
    state.update(&agents, now, MotionMode::Full);
    assert_eq!(state.selected_id(), None);
    assert_eq!(
        state.next_frame_in(&agents, &Profile::DarkTrue.theme()),
        None
    );
}

#[test]
fn actions_are_capability_gated_bound_to_the_id_and_never_change_owner_state() {
    let mut agents = vec![fixture("a", State::Working), fixture("b", State::Failed)];
    let mut state = SubagentViewState::default();
    state.update(&agents, Instant::now(), MotionMode::Full);
    for code in [KeyCode::Enter, KeyCode::Char('m'), KeyCode::Char('x')] {
        assert_eq!(state.handle_key(&agents, key(code)), None);
    }
    agents[0].controls = SubagentControls {
        open: true,
        message: true,
        stop: true,
    };
    agents.swap(0, 1);
    assert_eq!(
        state.handle_key(&agents, key(KeyCode::Enter)),
        Some(SubagentIntent::Open("a".into()))
    );
    assert_eq!(
        state.handle_key(&agents, key(KeyCode::Char('m'))),
        Some(SubagentIntent::Message("a".into()))
    );
    assert_eq!(
        state.handle_key(&agents, key(KeyCode::Char('x'))),
        Some(SubagentIntent::Stop("a".into()))
    );
    assert_eq!(agents[1].card.status.state, State::Working);
    state.set_visible(false);
    assert_eq!(state.handle_key(&agents, key(KeyCode::Char('x'))), None);
    state.set_visible(true);
    let mut held = key(KeyCode::Char('x'));
    held.kind = KeyEventKind::Repeat;
    assert_eq!(state.handle_key(&agents, held), None);
    held.kind = KeyEventKind::Release;
    assert_eq!(state.handle_key(&agents, held), None);
    assert_eq!(
        state.handle_key(
            &agents,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)
        ),
        None
    );
}

#[test]
fn ambiguous_and_empty_ids_cannot_receive_an_intent() {
    let agents = vec![
        fixture("same", State::Working),
        fixture("same", State::Done),
        fixture("", State::Working),
    ];
    let mut state = SubagentViewState::default();
    state.update(&agents, Instant::now(), MotionMode::Full);
    assert!(!state.select(&agents, "same"));
    assert!(!state.select(&agents, ""));
    assert_eq!(state.selected_id(), None);
    assert_eq!(state.handle_key(&agents, key(KeyCode::Enter)), None);
}

#[test]
fn navigation_reaches_attention_and_offscreen_workers() {
    let agents: Vec<_> = (0..30)
        .map(|i| {
            fixture(
                &format!("{i:02}"),
                if i == 22 {
                    State::NeedsYou
                } else if i == 27 {
                    State::Failed
                } else {
                    State::Working
                },
            )
        })
        .collect();
    let mut state = SubagentViewState::default();
    state.update(&agents, Instant::now(), MotionMode::Full);
    state.handle_key(&agents, key(KeyCode::Tab));
    assert_eq!(state.selected_id(), Some("22"));
    let output = testing::text(&render(&agents, &mut state, Profile::Ascii, 48, 20));
    assert!(output.contains("Agent 22"));
    assert!(output.contains("/ 30"));
    state.handle_key(&agents, key(KeyCode::Tab));
    assert_eq!(state.selected_id(), Some("27"));
    state.handle_key(&agents, key(KeyCode::BackTab));
    assert_eq!(state.selected_id(), Some("22"));
    state.handle_key(&agents, key(KeyCode::End));
    assert_eq!(state.selected_id(), Some("29"));
    state.handle_key(&agents, key(KeyCode::Home));
    assert_eq!(state.selected_id(), Some("00"));
}

#[test]
fn all_profiles_keep_state_words_and_observed_counts_without_inferred_completion() {
    let mut agent = fixture("a", State::Failed);
    agent.performance = None;
    agent.progress = Some(CountBar::new(9, 7).state(State::Failed).label("checks"));
    let agents = [agent];
    for profile in Profile::ALL {
        let mut state = SubagentViewState::default();
        state.update(&agents, Instant::now(), MotionMode::Reduced);
        let frame = testing::Frame::new(
            "subagents",
            profile,
            render(&agents, &mut state, profile, 112, 38),
        );
        assert!(frame.violations().is_empty(), "{:?}", frame.violations());
        assert!(frame.text().contains("Failed"));
        assert!(frame.text().contains("9 of 7 checks"));
        assert!(frame.text().contains("No activity animation reported"));
        assert!(!frame.text().contains("1 done"));
    }
}

#[test]
fn native_pet_and_phase_marks_move_but_painting_same_time_is_idempotent() {
    let agents = [fixture("a", State::Working)];
    let start = Instant::now();
    let mut state = SubagentViewState::default();
    state.update(&agents, start, MotionMode::Full);
    let first = render(&agents, &mut state, Profile::DarkTrue, 112, 38);
    assert_eq!(
        first,
        render(&agents, &mut state, Profile::DarkTrue, 112, 38)
    );
    assert!(
        state
            .next_frame_in(&agents, &Profile::DarkTrue.theme())
            .is_some()
    );
    state.update(
        &agents,
        start + Duration::from_millis(950),
        MotionMode::Full,
    );
    let second = render(&agents, &mut state, Profile::DarkTrue, 112, 38);
    assert_ne!(first, second);
    assert_eq!(
        second,
        render(&agents, &mut state, Profile::DarkTrue, 112, 38)
    );
}

#[test]
fn reduced_still_hidden_and_settled_rosters_schedule_no_frames() {
    let mut agents = [fixture("a", State::Working)];
    let start = Instant::now();
    for motion in [MotionMode::Reduced, MotionMode::Still] {
        let mut state = SubagentViewState::default();
        state.update(&agents, start, motion);
        let first = render(&agents, &mut state, Profile::DarkTrue, 112, 38);
        state.update(&agents, start + Duration::from_secs(90), motion);
        assert_eq!(
            first,
            render(&agents, &mut state, Profile::DarkTrue, 112, 38)
        );
        assert_eq!(
            state.next_frame_in(&agents, &Profile::DarkTrue.theme()),
            None
        );
    }
    let mut state = SubagentViewState::default();
    state.update(&agents, start, MotionMode::Full);
    render(&agents, &mut state, Profile::DarkTrue, 112, 38);
    state.set_visible(false);
    assert_eq!(
        state.next_frame_in(&agents, &Profile::DarkTrue.theme()),
        None
    );
    state.set_visible(true);
    agents[0] = fixture("a", State::Done);
    state.update(&agents, start + Duration::from_secs(1), MotionMode::Full);
    render(&agents, &mut state, Profile::DarkTrue, 112, 38);
    assert!(
        state
            .next_frame_in(&agents, &Profile::DarkTrue.theme())
            .is_some()
    );
    state.update(&agents, start + Duration::from_secs(3), MotionMode::Full);
    render(&agents, &mut state, Profile::DarkTrue, 112, 38);
    assert_eq!(
        state.next_frame_in(&agents, &Profile::DarkTrue.theme()),
        None
    );
}

#[test]
fn compact_peek_and_event_scrolling_retain_the_selected_worker() {
    let mut agent = fixture("a", State::Working);
    agent.events = (0..30)
        .map(|i| SubagentEvent::new(format!("{i:02}"), State::Done, format!("receipt-{i:02}")))
        .collect();
    let agents = [agent];
    let mut state = SubagentViewState::default();
    state.update(&agents, Instant::now(), MotionMode::Reduced);
    let roster = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 48, 26));
    assert!(!roster.contains("receipt-29"));
    state.handle_key(&agents, key(KeyCode::Char(' ')));
    let latest = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 48, 26));
    assert!(latest.contains("receipt-29"));
    state.handle_key(&agents, key(KeyCode::PageUp));
    let older = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 48, 26));
    assert!(!older.contains("receipt-29"));
    assert!(older.contains("receipt-24"));
    state.handle_key(&agents, key(KeyCode::PageDown));
    assert_eq!(
        latest,
        testing::text(&render(&agents, &mut state, Profile::DarkTrue, 48, 26))
    );
    assert_eq!(state.selected_id(), Some("a"));
}

#[test]
fn caller_text_and_localized_copy_are_safe_and_keep_unicode_graphemes() {
    let mut agent = fixture("a", State::NeedsYou);
    agent.card.title = "鲸鱼\u{202e}\u{1b}[31m".into();
    agent.card.task = "cafe\u{301} / 等待批准\u{07}".into();
    agent.events = vec![SubagentEvent::new(
        "12:00\u{2066}",
        State::NeedsYou,
        "许可\u{202e}确认",
    )];
    let agents = [agent];
    let mut state = SubagentViewState::default();
    state.update(&agents, Instant::now(), MotionMode::Reduced);
    let theme = Profile::LightTrue.theme();
    let buf = testing::render(112, 38, |area, buf| {
        SubagentView::new(&agents, &theme)
            .words(SubagentViewWords {
                title: "代理\u{202e}".into(),
                summary: Some("等待你的确认".into()),
                activity: "活动".into(),
                ..SubagentViewWords::default()
            })
            .render(area, buf, &mut state)
    });
    let text = testing::text(&buf);
    assert!(text.contains("等待你的确认"));
    assert!(text.contains("许可确认"));
    for banned in ['\u{202e}', '\u{1b}', '\u{07}', '\u{2066}'] {
        assert!(!text.contains(banned));
    }
}

#[test]
fn clipping_tiny_empty_offset_and_coordinate_limit_buffers_are_safe() {
    let agents = [fixture("a", State::Working)];
    for bounds in [
        Rect::new(7, 9, 120, 40),
        Rect::new(u16::MAX - 120, u16::MAX - 40, 120, 40),
        Rect::new(7, 9, 0, 40),
        Rect::new(7, 9, 120, 0),
    ] {
        for request in [
            bounds,
            Rect::new(bounds.x, bounds.y, 1, 1),
            Rect::new(bounds.x, bounds.y, 12, 7),
            Rect::new(0, 0, 1, 1),
        ] {
            let mut state = SubagentViewState::default();
            state.update(&agents, Instant::now(), MotionMode::Reduced);
            let mut buf = Buffer::filled(bounds, Cell::new("."));
            let before = buf.clone();
            SubagentView::new(&agents, &Profile::DarkTrue.theme())
                .render(request, &mut buf, &mut state);
            let allowed = bounds.intersection(request);
            for y in bounds.y..bounds.bottom() {
                for x in bounds.x..bounds.right() {
                    if !allowed.contains((x, y).into()) {
                        assert_eq!(buf[(x, y)], before[(x, y)]);
                    }
                }
            }
        }
    }
    for (width, height) in [
        (0, 0),
        (1, 1),
        (2, 5),
        (26, 10),
        (48, 26),
        (87, 40),
        (88, 14),
        (112, 38),
    ] {
        let mut state = SubagentViewState::default();
        state.update(&agents, Instant::now(), MotionMode::Reduced);
        render(&agents, &mut state, Profile::Ascii, width, height);
    }
}

#[test]
fn missing_telemetry_stays_missing_and_unknown_is_not_working_or_done() {
    let agents = [Subagent::new(
        "u",
        AgentCard::new("Unknown worker", State::Unknown).task("No owner status yet"),
    )];
    let mut state = SubagentViewState::default();
    state.update(&agents, Instant::now(), MotionMode::Full);
    let output = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 112, 38));
    assert!(output.contains("Unknown"));
    assert!(!output.contains("tokens"));
    assert!(!output.contains("of 0"));
    assert_eq!(
        state.next_frame_in(&agents, &Profile::DarkTrue.theme()),
        None
    );
}

#[test]
fn live_native_pet_keeps_every_terminal_profiles_color_contract() {
    let agents = [fixture("a", State::Working)];
    for profile in Profile::ALL {
        let mut state = SubagentViewState::default();
        state.update(&agents, Instant::now(), MotionMode::Reduced);
        let frame = testing::Frame::new(
            "subagent pet",
            profile,
            render(&agents, &mut state, profile, 112, 38),
        );
        assert!(frame.violations().is_empty(), "{:?}", frame.violations());
        assert!(frame.text().contains("Working"));
        if !profile.theme().paints_grounds() {
            assert!(
                frame
                    .buf
                    .content()
                    .iter()
                    .all(|c| c.bg == ratatui::style::Color::Reset)
            );
        }
    }
}

#[test]
fn compact_details_keep_reported_usage_and_authored_elapsed_fallback() {
    let mut agents = [fixture("a", State::Working)];
    let mut state = SubagentViewState::default();
    state.update(&agents, Instant::now(), MotionMode::Reduced);
    state.handle_key(&agents, key(KeyCode::Right));
    let output = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 48, 26));
    assert!(output.contains("1m 23s / 12k tokens"));
    agents[0].elapsed = None;
    agents[0].card.elapsed = Some("owner time unavailable".into());
    let output = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 48, 26));
    assert!(output.contains("owner time unavailable / 12k tokens"));
}

#[test]
fn empty_roster_keeps_its_message_after_compact_peek() {
    let agents = [fixture("a", State::Working)];
    let mut state = SubagentViewState::default();
    state.update(&agents, Instant::now(), MotionMode::Full);
    state.handle_key(&agents, key(KeyCode::Right));
    state.update(&[], Instant::now(), MotionMode::Full);
    let output = testing::text(&render(&[], &mut state, Profile::DarkTrue, 48, 26));
    assert!(output.contains("No agents reported"));
    assert_eq!(state.next_frame_in(&[], &Profile::DarkTrue.theme()), None);
}

#[test]
fn each_working_and_checking_onset_earns_the_native_pending_delay() {
    let mut agents = vec![fixture("a", State::Ready)];
    agents[0].performance = None;
    let mut state = SubagentViewState::default();
    let start = Instant::now();
    state.update(&agents, start, MotionMode::Full);
    render(&agents, &mut state, Profile::DarkTrue, 48, 26);
    let onset = start + Duration::from_secs(10);
    agents[0].card.status = codewhale_ratatui::StatusMark::new(State::Working);
    state.update(&agents, onset, MotionMode::Full);
    let output = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 48, 26));
    assert!(output.contains(&format!(
        "{} Working",
        codewhale_ratatui::spin::PENDING_FRAME
    )));
    state.update(
        &agents,
        onset + Duration::from_millis(399),
        MotionMode::Full,
    );
    assert_eq!(
        state.next_frame_in(&agents, &Profile::DarkTrue.theme()),
        Some(Duration::from_millis(1))
    );
    state.update(
        &agents,
        onset + Duration::from_millis(401),
        MotionMode::Full,
    );
    let output = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 48, 26));
    assert!(output.contains(&format!("{} Working", codewhale_ratatui::spin::FRAMES[0])));
    agents[0].checking = true;
    state.update(&agents, onset + Duration::from_secs(2), MotionMode::Full);
    let output = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 48, 26));
    assert!(output.contains(&format!(
        "{} Working",
        codewhale_ratatui::spin::PENDING_FRAME
    )));
    agents.push(fixture("b", State::Working));
    state.update(&agents, onset + Duration::from_secs(3), MotionMode::Full);
    let output = testing::text(&render(&agents, &mut state, Profile::DarkTrue, 48, 26));
    assert!(
        output
            .lines()
            .nth(7)
            .unwrap()
            .contains(codewhale_ratatui::spin::PENDING_FRAME)
    );
}

#[test]
fn appended_wrapped_receipts_do_not_move_a_paused_history_viewport() {
    let mut agents = [fixture("a", State::Working)];
    agents[0].events = (0..30)
        .map(|i| SubagentEvent::new(format!("{i:02}"), State::Done, format!("receipt-{i:02}")))
        .collect();
    let mut state = SubagentViewState::default();
    state.update(&agents, Instant::now(), MotionMode::Reduced);
    state.handle_key(&agents, key(KeyCode::Right));
    render(&agents, &mut state, Profile::Ascii, 48, 26);
    state.handle_key(&agents, key(KeyCode::PageUp));
    let older = render(&agents, &mut state, Profile::Ascii, 48, 26);
    agents[0].events.push(SubagentEvent::new(
        "30",
        State::Working,
        "NEW-RECEIPT ".repeat(20),
    ));
    assert_eq!(older, render(&agents, &mut state, Profile::Ascii, 48, 26));
    for _ in 0..10 {
        state.handle_key(&agents, key(KeyCode::PageDown));
    }
    let latest = testing::text(&render(&agents, &mut state, Profile::Ascii, 48, 26));
    assert!(latest.contains("NEW-RECEIPT"));
    agents[0]
        .events
        .push(SubagentEvent::new("31", State::Done, "latest receipt"));
    assert!(
        testing::text(&render(&agents, &mut state, Profile::Ascii, 48, 26))
            .contains("latest receipt")
    );
}
