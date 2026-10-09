use codewhale_ratatui::{
    InstrumentSurface, KeyHint, Paint, PickerState, Role, SessionList, SessionListWords,
    SessionRow, gallery,
    testing::{Profile, render, text},
};
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
};

fn actions() -> Vec<KeyHint> {
    vec![
        KeyHint::new("Enter", "resume"),
        KeyHint::new("/", "search"),
        KeyHint::new("Esc", "close"),
    ]
}

#[test]
fn instrument_geometry_matches_native_independent_margin_thresholds() {
    // views/mod.rs::render_underwater_surface: margins are independent,
    // horizontal at 44 columns, vertical + top body pad at 24 rows.
    let theme = Profile::DarkTrue.theme();
    let surface = InstrumentSurface::new("sessions");
    for (outer, shell, body) in [
        (
            Rect::new(0, 0, 43, 23),
            Rect::new(0, 0, 43, 23),
            Rect::new(1, 1, 41, 21),
        ),
        (
            Rect::new(0, 0, 44, 23),
            Rect::new(1, 0, 42, 23),
            Rect::new(2, 1, 40, 21),
        ),
        (
            Rect::new(0, 0, 43, 24),
            Rect::new(0, 1, 43, 22),
            Rect::new(1, 3, 41, 19),
        ),
        (
            Rect::new(5, 8, 100, 24),
            Rect::new(6, 9, 98, 22),
            Rect::new(7, 11, 96, 19),
        ),
    ] {
        let layout = surface.areas(outer, &theme);
        assert_eq!(layout.surface, shell);
        assert_eq!(layout.body, body);
        assert_eq!(layout.footer.height, 0);
    }
}

#[test]
fn native_action_rail_pads_keys_packs_whole_actions_and_wraps() {
    let theme = Profile::DarkTrue.theme();
    let mut body = Rect::default();
    let buffer = render(34, 4, |area, buf| {
        body = InstrumentSurface::draw_footer(area, buf, &actions(), &theme, false);
    });
    assert_eq!(body.height, 3);
    assert_eq!(
        text(&buffer).lines().last().unwrap(),
        " Enter resume  / search  Esc close"
    );
    let buffer = render(32, 4, |area, buf| {
        body = InstrumentSurface::draw_footer(area, buf, &actions(), &theme, true);
    });
    assert_eq!(body.height, 2, "short rooms spend no quiet gutter");
    let shown = text(&buffer);
    let lines: Vec<_> = shown.lines().collect();
    assert_eq!(lines[2].trim_end(), " Enter resume  / search");
    assert_eq!(lines[3].trim_end(), " Esc close");
    let buffer = render(32, 8, |area, buf| {
        body = InstrumentSurface::draw_footer(area, buf, &actions(), &theme, true);
    });
    assert_eq!(body.height, 5, "roomy rail reserves one quiet row");
    assert!(buffer[(1, 6)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(1, 6)].fg, theme.color(Role::Primary).unwrap());
    assert_eq!(buffer[(7, 6)].fg, theme.color(Role::Muted).unwrap());
    let expanded = [
        KeyHint::new("Ctrl+Enter", "继续当前任务并查看完整的执行结果"),
        KeyHint::new("Esc", "close"),
    ];
    for width in [3, 4, 8] {
        let buffer = render(width, 40, |area, buf| {
            body = InstrumentSurface::draw_footer(area, buf, &expanded, &theme, false);
        });
        let content: String = text(&buffer)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert_eq!(
            content,
            "Ctrl+Enter继续当前任务并查看完整的执行结果Escclose"
        );
        assert!(buffer[(1, body.height)].modifier.contains(Modifier::BOLD));
    }
}

#[test]
fn instrument_paints_native_two_rules_and_action_roles() {
    let theme = Profile::DarkTrue.theme();
    let surface = InstrumentSurface::new("sessions").actions(actions());
    let buffer = render(80, 24, |area, buf| {
        surface.draw(area, buf, &theme);
    });
    let shown = text(&buffer);
    let lines: Vec<_> = shown.lines().collect();
    assert!(lines[1].contains(" sessions "));
    assert_eq!(lines[22].matches('─').count(), 78);
    assert!(lines[21].contains(" Enter resume  / search  Esc close"));
    assert!(lines[0].trim().is_empty());
    assert!((0..80).all(|x| buffer[(x, 23)].symbol() == " "));
    assert_eq!(buffer[(2, 1)].fg, theme.color(Role::Primary).unwrap());
}

fn sessions() -> SessionList<'static> {
    SessionList::new(vec![
        SessionRow::new("abcdefgh-0123", "Review sign-in")
            .messages(1)
            .mode("Work")
            .updated("2026-10-01 14:12 (2m ago)")
            .current(true)
            .fork(true)
            .archived(true),
        SessionRow::new("ijklmnop-4567", "Session")
            .messages(2)
            .updated("2026-09-30 12:00 (1d ago)"),
    ])
}

#[test]
fn session_rows_match_native_grammar_labels_and_title_fallback() {
    let theme = Profile::DarkTrue.theme();
    let buffer = render(160, 5, |area, buf| sessions().paint(area, buf, &theme));
    let shown = text(&buffer);
    let lines: Vec<_> = shown.lines().map(str::trim_end).collect();
    assert_eq!(lines[0], "scope and sort · recent");
    assert_eq!(
        lines[1],
        "1. abcdefgh | Review sign-in | 1 msg | current | fork | archived | work | 2026-10-01 14:12 (2m ago)"
    );
    assert_eq!(
        lines[2],
        "2. ijklmnop | ijklmnop | 2 msgs | unknown | 2026-09-30 12:00 (1d ago)"
    );
    assert!(buffer[(0, 1)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(0, 1)].bg, theme.color(Role::Selected).unwrap());
}

#[test]
fn source_title_cap_and_ellipsis_are_conservative_and_grapheme_safe() {
    let theme = Profile::DarkTrue.theme();
    let list = SessionList::new(vec![SessionRow::new(
        "abcdefgh",
        "0123456789012345678901234567890123456789",
    )]);
    let shown = text(&render(160, 4, |area, buf| list.paint(area, buf, &theme)));
    assert!(
        shown.contains("abcdefgh | 0123456789012345678901234567... | 0 msgs"),
        "{shown}"
    );
    for width in 0..=12 {
        let list = SessionList::new(vec![SessionRow::new(
            "海🌊e\u{301}session",
            "🌊海e\u{301} notes",
        )])
        .query("🌊🌊🌊");
        let buffer = render(width, 4, |area, buf| list.paint(area, buf, &theme));
        for line in text(&buffer).lines() {
            assert!(codewhale_ratatui::text::width(line) <= usize::from(width));
        }
    }
}

#[test]
fn native_session_headers_and_empty_copy_have_correct_priority() {
    let theme = Profile::DarkTrue.theme();
    let list = sessions()
        .query("login")
        .rename("new title")
        .status("Renamed")
        .confirm_delete(true);
    let buffer = render(100, 6, |area, buf| list.paint(area, buf, &theme));
    let shown = text(&buffer);
    assert!(shown.lines().next().unwrap().starts_with("/login"));
    assert!(
        shown
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("Confirm delete (y/n)")
    );
    assert!(!shown.contains("Renamed") && !shown.contains("New title:"));
    assert_eq!(buffer[(0, 1)].fg, theme.color(Role::Attention).unwrap());
    let shown = text(&render(80, 4, |area, buf| {
        sessions().rename("New title").paint(area, buf, &theme)
    }));
    assert!(shown.starts_with("New title: New title_"));
    let shown = text(&render(80, 4, |area, buf| {
        SessionList::new(Vec::new()).paint(area, buf, &theme)
    }));
    assert!(shown.contains("No saved sessions yet."));
    assert!(shown.contains("Send a message to start one — it saves automatically."));
}

#[test]
fn session_scrolling_numbers_visible_shortcuts_and_reports_native_range() {
    let theme = Profile::DarkTrue.theme();
    let rows = (0..20)
        .map(|n| SessionRow::new(format!("row{n:05}"), format!("Session {n}")).messages(n))
        .collect();
    let list = SessionList::new(rows).state(PickerState::new(12));
    let area = Rect::new(4, 7, 80, 6);
    let hits = list.hitboxes(area, &theme);
    assert_eq!(
        hits.iter().map(|hit| hit.index).collect::<Vec<_>>(),
        [9, 10, 11, 12]
    );
    assert_eq!(hits[0].area, Rect::new(4, 8, 79, 1));
    assert_eq!(list.item_at(area, Position::new(10, 11), &theme), Some(12));
    assert_eq!(
        list.item_at(area, Position::new(83, 11), &theme),
        None,
        "rail is not an item"
    );
    let buffer = render(80, 6, |area, buf| list.paint(area, buf, &theme));
    let shown = text(&buffer);
    let lines: Vec<_> = shown.lines().collect();
    assert!(lines[1].starts_with("1. row00009"));
    assert!(lines[4].starts_with("4. row00012"));
    assert!(lines[5].starts_with("Showing 10-13 / 20"));
}

#[test]
fn only_nine_visible_rows_have_numeric_shortcuts() {
    let theme = Profile::DarkTrue.theme();
    let list = SessionList::new(
        (0..12)
            .map(|n| SessionRow::new(format!("row{n:05}"), "Title"))
            .collect(),
    );
    let shown = text(&render(80, 15, |area, buf| list.paint(area, buf, &theme)));
    let lines: Vec<_> = shown.lines().collect();
    assert!(lines[9].starts_with("9. row00008"));
    assert!(lines[10].starts_with("   row00009"));
    assert!(!shown.contains("10. row"));
}

#[test]
fn native_copy_is_localizable_and_controls_are_removed() {
    let theme = Profile::Ascii.theme();
    let words = SessionListWords {
        current: "active".into(),
        message: "message".into(),
        messages: "messages".into(),
        ..SessionListWords::default()
    };
    let list = SessionList::new(vec![
        SessionRow::new("abcdefgh", "海\u{202e} notes\u{1b}")
            .current(true)
            .messages(1),
    ])
    .words(words)
    .query("海\u{202e} search\u{7}");
    let shown = text(&render(100, 4, |area, buf| list.paint(area, buf, &theme)));
    assert!(shown.contains("海 notes | 1 message | active"));
    assert!(!shown.contains('\u{202e}') && !shown.contains('\u{1b}') && !shown.contains('\u{7}'));
    assert!(shown.starts_with("/海 search"));
}

#[test]
fn extracted_parts_and_recipes_stay_inside_clipped_buffers() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        for (width, height) in [(0, 0), (1, 1), (2, 2), (8, 3), (32, 7), (80, 24)] {
            let buffer_area = Rect::new(7, 5, width, height);
            let outer = Rect::new(0, 0, 160, 40);
            let mut buffer = Buffer::empty(buffer_area);
            InstrumentSurface::new("Room 🌊\u{202e}")
                .actions(actions())
                .draw(outer, &mut buffer, &theme);
            sessions().paint(outer, &mut buffer, &theme);
            for entry in gallery::entries().into_iter().filter(|entry| {
                entry.name.starts_with("view-")
                    || entry.name == "instrument-surface"
                    || entry.name == "session-list"
            }) {
                let mut buffer = Buffer::empty(buffer_area);
                (entry.draw)(outer, &mut buffer, &theme);
            }
        }
        let buffer_area = Rect::new(7, 5, 40, 14);
        let paint_area = Rect::new(12, 8, 24, 7);
        let mut buffer = Buffer::filled(buffer_area, ratatui::buffer::Cell::new("~"));
        InstrumentSurface::new("sessions")
            .actions(actions())
            .draw(paint_area, &mut buffer, &theme);
        for y in buffer_area.y..buffer_area.bottom() {
            for x in buffer_area.x..buffer_area.right() {
                if !paint_area.contains(Position::new(x, y)) {
                    assert_eq!(buffer[(x, y)].symbol(), "~");
                }
            }
        }
    }
}

#[test]
fn native_view_recipes_cover_the_public_families_and_ascii_ui() {
    let entries = gallery::entries();
    for name in [
        "instrument-surface",
        "session-list",
        "view-sessions",
        "view-settings",
        "view-commands",
        "view-models",
        "view-providers",
        "view-theme",
        "view-mode",
        "view-status",
        "view-file-picker",
        "view-fleet-dock",
        "view-jobs-dock",
        "view-files-dock",
        "view-context-dock",
        "view-git-dock",
        "view-cost-dock",
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("missing {name}"));
        let theme = Profile::Ascii.theme();
        let buffer = render(entry.width, entry.height, |area, buf| {
            (entry.draw)(area, buf, &theme)
        });
        assert!(text(&buffer).is_ascii(), "{name} has authored non-ASCII UI");
    }
}

#[test]
fn view_guide_inventories_every_current_modal_kind_once() {
    let expected = [
        "PetHabitat",
        "Approval",
        "Elevation",
        "UserInput",
        "CommandPalette",
        "Help",
        "SubAgents",
        "Pager",
        "LiveTranscript",
        "SessionPicker",
        "Config",
        "ModelPicker",
        "ProviderPicker",
        "ModePicker",
        "FleetRoster",
        "FleetSetup",
        "FleetList",
        "FleetDetail",
        "HotbarSetup",
        "SetupWizard",
        "FilePicker",
        "StatusPicker",
        "FeedbackPicker",
        "ThemePicker",
        "ContextMenu",
        "ContextInspector",
        "SkillsManager",
        "Extensions",
        "WorktreeManager",
        "WorkflowsManager",
        "Automations",
        "LaunchResumeConfirm",
        "RouterSetup",
    ];
    let guide = include_str!("../VIEWS.md");
    let inventory = guide
        .split("<!-- modal-inventory:start -->")
        .nth(1)
        .expect("inventory start")
        .split("<!-- modal-inventory:end -->")
        .next()
        .unwrap();
    let actual: Vec<_> = inventory
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| line.split('`').nth(1).unwrap())
        .collect();
    assert_eq!(actual, expected);
    for kind in expected {
        assert!(
            inventory
                .lines()
                .any(|line| line.starts_with(&format!("| `{kind}` |"))
                    && line.contains(".rs")
                    && line.contains("`"))
        );
    }
}
