use codewhale_ratatui::{
    Paint, TuiInk, Workbar, WorkbarAgent, WorkbarOutcome, WorkbarPanel, WorkbarPlacement,
    WorkbarRow, WorkbarState, WorkbarTab, WorkbarTarget, WorkbarTone,
    testing::{self, Profile, render},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;

fn tabs() -> Vec<WorkbarTab> {
    WorkbarPanel::ORDER
        .into_iter()
        .map(|panel| WorkbarTab::new(panel).count(2))
        .collect()
}
fn fixture() -> Workbar {
    Workbar::new(
        WorkbarPanel::Tasks,
        vec![
            WorkbarRow::new("graph:one", "Read the current source")
                .mark("✓")
                .tone(WorkbarTone::Success)
                .detail("completed"),
            WorkbarRow::new("graph:two", "Extract the native dock")
                .mark("●")
                .tone(WorkbarTone::Live)
                .detail("in progress"),
            WorkbarRow::new("graph:three", "Document the API")
                .mark("○")
                .detail("pending"),
        ],
    )
    .tabs(tabs())
    .goal("Make Codewhale reusable")
    .selected("graph:two")
    .focused(true)
}

#[test]
fn dock_respects_terminal_profiles_and_widths() {
    testing::assert_rules(8, |area, buf, theme| fixture().paint(area, buf, theme));
}

#[test]
fn native_working_success_and_waiting_keep_their_distinct_source_inks() {
    let theme = Profile::DarkTrue.theme().tui();
    let rows = vec![
        WorkbarRow::new("working", "Working").tone(WorkbarTone::Live),
        WorkbarRow::new("success", "Completed").tone(WorkbarTone::Success),
        WorkbarRow::new("waiting", "Waiting").tone(WorkbarTone::Attention),
    ];
    let dock = Workbar::new(WorkbarPanel::Tasks, rows);
    let area = Rect::new(0, 0, 80, 6);
    let layout = dock.layout(area);
    let buf = render(80, 6, |area, buf| dock.paint(area, buf, &theme));
    for (row, ink) in [TuiInk::Working, TuiInk::Success, TuiInk::Warning]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            buf[(layout.content.x, layout.content.y + row as u16)].fg,
            theme.tui_ink(ink).fg.unwrap()
        );
    }
    assert_ne!(
        theme.tui_ink(TuiInk::Working).fg,
        theme.tui_ink(TuiInk::Success).fg
    );
}

#[test]
fn native_tab_names_order_and_bottom_divider_survive_extraction() {
    let theme = Profile::DarkTrue.theme();
    let buf = render(104, 7, |area, buf| fixture().paint(area, buf, &theme));
    let text = testing::text(&buf);
    let lines: Vec<_> = text.lines().collect();
    assert!(lines[0].chars().all(|value| value == '─'));
    let mut previous = 0;
    for panel in WorkbarPanel::ORDER {
        let at = lines[1].find(panel.label()).unwrap();
        assert!(at >= previous);
        previous = at;
    }
    assert!(lines[1].contains("Esc ×"));
    assert!(text.contains("Goal: Make Codewhale reusable"));
    assert!(text.contains("1 · ✓ Read the current source"));
}

#[test]
fn narrow_tabs_lose_counts_and_optional_tabs_before_the_active_tab() {
    let dock = fixture();
    let area = Rect::new(3, 4, 24, 7);
    let hits = dock.hitboxes(area);
    assert!(
        hits.iter()
            .any(|hit| hit.target == WorkbarTarget::Panel(WorkbarPanel::Tasks))
    );
    assert!(
        !hits
            .iter()
            .any(|hit| hit.target == WorkbarTarget::Panel(WorkbarPanel::Cost))
    );
    for hit in hits {
        assert_eq!(hit.area.intersection(area), hit.area);
    }
}

#[test]
fn goal_and_progress_share_only_the_native_wide_header() {
    let dock = fixture().progress("TODO 1/3; 2 left").max_height(16);
    assert_eq!(dock.layout(Rect::new(0, 0, 104, 8)).progress_height, 0);
    assert_eq!(dock.layout(Rect::new(0, 0, 40, 8)).progress_height, 1);
    assert_eq!(Paint::height(&dock, 104, &Profile::DarkTrue.theme()), 6);
    assert_eq!(Paint::height(&dock, 40, &Profile::DarkTrue.theme()), 7);
}

#[test]
fn native_default_height_and_compact_explicit_budget_are_preserved() {
    let dock = fixture().progress("TODO 1/3; 2 left");
    assert_eq!(Paint::height(&dock, 104, &Profile::DarkTrue.theme()), 5);
    assert_eq!(dock.height_for(104, 40, 3), 3);
    assert_eq!(dock.height_for(104, 40, 2), 0);
    assert_eq!(dock.explicit(false).height_for(104, 40, 3), 0);
}

#[test]
fn placements_follow_native_geometry_and_side_fallback() {
    let host = Rect::new(4, 2, 104, 28);
    for placement in [
        WorkbarPlacement::Bottom,
        WorkbarPlacement::Top,
        WorkbarPlacement::Left,
        WorkbarPlacement::Right,
    ] {
        let regions = fixture().placement(placement).regions(host);
        assert_eq!(regions.placement, placement);
        let dock = regions.dock.unwrap();
        assert!(regions.content.intersection(dock).is_empty());
        assert_eq!(dock.intersection(host), dock);
        assert_eq!(regions.content.intersection(host), regions.content);
        if placement.is_strip() {
            assert_eq!(regions.content.height + dock.height, host.height);
        } else {
            assert_eq!(regions.content.width + dock.width, host.width);
            assert!(regions.content.width >= 40);
        }
    }
    let narrow = fixture()
        .placement(WorkbarPlacement::Left)
        .regions(Rect::new(0, 0, 40, 20));
    assert_eq!(narrow.placement, WorkbarPlacement::Top);
    assert!(narrow.dock.is_some());
}

#[test]
fn off_and_automatic_empty_docks_reserve_and_paint_nothing() {
    let theme = Profile::DarkTrue.theme();
    for dock in [
        fixture().placement(WorkbarPlacement::Off),
        Workbar::new(WorkbarPanel::Tasks, vec![]).explicit(false),
    ] {
        assert_eq!(Paint::height(&dock, 80, &theme), 0);
        assert!(dock.regions(Rect::new(0, 0, 80, 20)).dock.is_none());
        let buf = render(80, 8, |area, buf| dock.paint(area, buf, &theme));
        assert!(testing::text(&buf).trim().is_empty());
    }
}

#[test]
fn explicit_empty_panel_uses_the_native_empty_message() {
    let theme = Profile::DarkTrue.theme();
    for panel in WorkbarPanel::ORDER {
        let dock = Workbar::new(panel, vec![]);
        let buf = render(80, 5, |area, buf| dock.paint(area, buf, &theme));
        assert!(testing::text(&buf).contains(panel.empty_label()));
    }
}

#[test]
fn overflow_count_and_hitboxes_use_the_same_scrolled_rows() {
    let rows = (0..10)
        .map(|index| WorkbarRow::new(format!("graph:{index}"), format!("step {index}")))
        .collect();
    let dock = Workbar::new(WorkbarPanel::Tasks, rows).offset(3);
    let area = Rect::new(0, 0, 40, 6);
    let layout = dock.layout(area);
    assert_eq!(layout.visible_rows, 3);
    assert_eq!(layout.offset, 3);
    let buf = render(40, 6, |area, buf| {
        dock.paint(area, buf, &Profile::DarkTrue.theme())
    });
    let text = testing::text(&buf);
    assert!(text.contains("step 3") && text.contains("step 5"));
    assert!(!text.contains("step 6"));
    assert!(text.contains("↓ 4 more"));
    assert_eq!(
        dock.target_at(area, layout.content.x, layout.content.y),
        Some(WorkbarTarget::Row("graph:3".into()))
    );
    assert!(
        dock.target_at(area, layout.content.x, layout.content.bottom() - 1)
            .is_none()
    );
}

#[test]
fn fleet_degrades_columns_in_native_order_without_fabricating_usage() {
    let facts = WorkbarAgent::new(
        "builder",
        "running",
        "Extract native work dock and its reusable API",
    )
    .elapsed_seconds(61)
    .tokens(111_900);
    let dock = Workbar::new(
        WorkbarPanel::Fleet,
        vec![
            WorkbarRow::new("worker:a", "Builder")
                .mark("●")
                .agent(facts),
        ],
    );
    let wide = render(104, 5, |area, buf| {
        dock.paint(area, buf, &Profile::DarkTrue.theme())
    });
    assert!(testing::text(&wide).contains("1m 01s"));
    assert!(testing::text(&wide).contains("111.9k tokens"));
    let narrow = render(40, 5, |area, buf| {
        dock.paint(area, buf, &Profile::DarkTrue.theme())
    });
    assert!(testing::text(&narrow).contains("Extract native work dock"));
    assert!(!testing::text(&narrow).contains("111.9k"));
    let unknown = Workbar::new(
        WorkbarPanel::Fleet,
        vec![
            WorkbarRow::new("worker:a", "Builder").agent(WorkbarAgent::new(
                "builder",
                "running",
                "Extract the native work dock",
            )),
        ],
    );
    let buf = render(104, 5, |area, buf| {
        unknown.paint(area, buf, &Profile::DarkTrue.theme())
    });
    assert!(!testing::text(&buf).contains("tokens"));
    assert!(!testing::text(&buf).contains("0s"));
}

#[test]
fn caller_text_and_fleet_facts_are_sanitized_and_off_buffer_is_safe() {
    let dock = Workbar::new(
        WorkbarPanel::Fleet,
        vec![
            WorkbarRow::new("id", "Builder\u{202e}\u{1b}").agent(WorkbarAgent::new(
                "builder",
                "running",
                "first\nsecond\u{2066}",
            )),
        ],
    )
    .goal("goal\ntext\u{202e}");
    let buf = render(80, 8, |area, buf| {
        dock.paint(area, buf, &Profile::DarkTrue.theme())
    });
    let text = testing::text(&buf);
    assert!(text.contains("first second") && text.contains("goal text"));
    assert!(!text.contains('\u{202e}') && !text.contains('\u{1b}') && !text.contains('\u{2066}'));
    let outside = render(20, 3, |_, buf| {
        dock.paint(Rect::new(30, 10, 40, 6), buf, &Profile::DarkTrue.theme())
    });
    assert!(testing::text(&outside).trim().is_empty());
}

#[test]
fn keys_skip_headings_dispatch_row_ids_and_return_typing_to_composer() {
    let rows = vec![
        WorkbarRow::new("header", "Tasks").selectable(false),
        WorkbarRow::new("a", "First"),
        WorkbarRow::new("b", "Second"),
    ];
    let mut state = WorkbarState::default();
    assert_eq!(
        state.handle_key(
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT),
            &rows,
            1
        ),
        WorkbarOutcome::Changed
    );
    assert_eq!(state.selected.as_deref(), Some("a"));
    state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &rows, 1);
    assert_eq!(state.selected.as_deref(), Some("b"));
    assert_eq!(state.offset, 2);
    assert_eq!(
        state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &rows, 1),
        WorkbarOutcome::Activate("b".into())
    );
    assert_eq!(
        state.handle_key(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            &rows,
            1
        ),
        WorkbarOutcome::ReleaseFocus
    );
    assert!(!state.focused);
}

#[test]
fn cycle_order_release_events_and_hidden_focus_match_native_routing() {
    let mut state = WorkbarState::default();
    assert_eq!(
        state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL), &[], 0),
        WorkbarOutcome::Panel(WorkbarPanel::Fleet)
    );
    assert_eq!(
        state.handle_key(
            KeyEvent::new(
                KeyCode::BackTab,
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            ),
            &[],
            0
        ),
        WorkbarOutcome::Panel(WorkbarPanel::Tasks)
    );
    assert_eq!(
        state.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT), &[], 0),
        WorkbarOutcome::Ignored
    );
    assert!(!state.focused);
    let mut key = KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL);
    key.kind = KeyEventKind::Release;
    assert_eq!(state.handle_key(key, &[], 2), WorkbarOutcome::Ignored);
    assert_eq!(state.panel, WorkbarPanel::Tasks);
}
