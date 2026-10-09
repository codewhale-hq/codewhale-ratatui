//! Package Chrome: dialog, sheet, heading, tabs, toggle, segmented, keymap
//! and the folded key hints built from it.
//!
//! Unit tests of sizing, folding and keys, then `assert_rules` and an insta
//! snapshot for each component and state at 40, 80 and 120 columns.

use codewhale_ratatui::{
    Binding, Depth, Dialog, DialogWidth, Heading, KeyChord, KeyHint, KeyHints, KeyHintsWords,
    Keymap, Paint, Panel, Role, Segmented, SegmentedOutcome, SegmentedState, Sheet, SheetEdge, Tab,
    Tabs, TabsOutcome, TabsState, Theme, Toggle, ToggleOutcome, ToggleState, ToggleWords, centered,
    keys::Platform,
    testing::{self, Profile},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Paragraph, Widget, Wrap},
};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

const LINUX: Platform = Platform {
    macos: false,
    ascii: false,
};

// ---------------------------------------------------------------- fixtures

/// Work behind a dialog or a sheet: text over the full width of every row,
/// so anything the overlay fails to clear shows.
fn backdrop(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let line = "Edited summary.md and then ran the whole suite again for good measure";
    for row in 0..area.height {
        buf.set_stringn(
            area.x,
            area.y + row,
            line,
            usize::from(area.width),
            theme.fg(Role::Muted),
        );
    }
}

fn say(body: Rect, buf: &mut Buffer, theme: &Theme, text: &str) {
    Paragraph::new(text)
        .style(theme.fg(Role::Foreground))
        .wrap(Wrap { trim: true })
        .render(body, buf);
}

fn confirm_hints() -> KeyHints {
    KeyHints::new(vec![
        KeyHint::new("y", "stop"),
        KeyHint::new("n", "keep running"),
    ])
}

/// The keys of a settings screen, with priorities. Enter is the most
/// important, then Esc, then the arrows; saving is disabled.
fn settings_keymap() -> Keymap<&'static str> {
    Keymap::new()
        .with(Binding::pair(KeyCode::Up, KeyCode::Down, "move", "move").priority(200))
        .with(Binding::new(KeyCode::Enter, "change", "change").priority(250))
        .with(Binding::new(KeyChord::char('r'), "reset", "reset").priority(100))
        .with(Binding::new(KeyChord::char('/'), "search", "search").priority(120))
        .with(
            Binding::new(KeyChord::ctrl('s'), "save", "save")
                .priority(80)
                .disabled(),
        )
        .with(Binding::new(KeyCode::Esc, "close", "close").priority(240))
        .with(Binding::help(KeyChord::char('?'), "help"))
}

fn tab_items() -> Vec<Tab> {
    vec![
        Tab::new("General"),
        Tab::new("Appearance"),
        Tab::new("Agents").badge(3),
        Tab::new("Connections"),
        Tab::new("Privacy"),
        Tab::new("Advanced"),
    ]
}

fn sub(area: Rect, y: u16, height: u16) -> Rect {
    Rect {
        y: area.y + y,
        height,
        ..area
    }
    .intersection(area)
}

// ------------------------------------------------------------------ dialog

#[test]
fn a_dialog_is_centered_and_never_wider_than_the_terminal() {
    let theme = Profile::DarkTrue.theme();
    let hints = confirm_hints();
    for (width, area_width, expect) in [
        (DialogWidth::Narrow, 120, 48),
        (DialogWidth::Standard, 120, 60),
        (DialogWidth::Wide, 120, 72),
        (DialogWidth::Wide, 80, 72),
        // Clamped to the terminal with a two-cell margin each side.
        (DialogWidth::Wide, 60, 56),
        (DialogWidth::Narrow, 40, 36),
        (DialogWidth::Standard, 24, 20),
    ] {
        let area = Rect::new(0, 0, area_width, 30);
        let dialog = Dialog::new()
            .title("Stop?")
            .width(width)
            .body_rows(2)
            .hints(&hints);
        let rect = dialog.rect(area, &theme);
        assert_eq!(rect.width, expect, "{width:?} in {area_width}");
        assert_eq!(rect.x, (area_width - expect) / 2, "{width:?} is centered");
        assert_eq!(rect.y, (30 - rect.height) / 2, "{width:?} is centered");
    }
    assert!(DialogWidth::Narrow.cells() < DialogWidth::Standard.cells());
    assert!(DialogWidth::Standard.cells() < DialogWidth::Wide.cells());
}

#[test]
fn a_dialog_is_as_tall_as_its_body_needs_and_returns_the_body() {
    let theme = Profile::DarkTrue.theme();
    let hints = confirm_hints();
    let area = Rect::new(0, 0, 100, 40);
    for rows in [1, 2, 5, 12] {
        let dialog = Dialog::new().title("Stop?").body_rows(rows).hints(&hints);
        let mut buf = Buffer::empty(area);
        let body = dialog.draw(area, &mut buf, &theme);
        let rect = dialog.rect(area, &theme);
        assert!(
            body.height >= rows,
            "{rows} body rows asked, {body:?} given"
        );
        assert!(body.height <= rows + 2, "no slack beyond padding: {body:?}");
        assert!(rect.contains(body.as_position()), "{rect:?} holds {body:?}");
        assert!(body.right() <= rect.right() && body.bottom() <= rect.bottom());
        assert_eq!(
            dialog.height(100, &theme),
            rect.height,
            "height() agrees with rect()"
        );
    }
}

#[test]
fn a_dialog_clamps_to_a_short_terminal_and_keeps_a_row_of_margin() {
    let theme = Profile::DarkTrue.theme();
    let hints = confirm_hints();
    let dialog = Dialog::new().title("Stop?").body_rows(30).hints(&hints);
    let area = Rect::new(0, 0, 80, 12);
    let rect = dialog.rect(area, &theme);
    assert_eq!(rect.height, 10, "{rect:?}");
    assert_eq!(rect.y, 1);
}

#[test]
fn a_dialog_never_panics_and_never_leaves_its_area_at_any_size() {
    let hints = confirm_hints();
    for profile in Profile::ALL {
        let theme = profile.theme();
        for (w, h) in [
            (0, 0),
            (1, 1),
            (1, 0),
            (0, 1),
            (2, 2),
            (3, 7),
            (5, 3),
            (9, 4),
            (19, 5),
            (21, 6),
            (40, 3),
            (200, 80),
        ] {
            // Drawn in the middle of a larger buffer, so a stray cell
            // outside the area is caught.
            let area = Rect::new(2, 3, w, h);
            let mut buf = Buffer::empty(Rect::new(0, 0, 210, 90));
            for dialog in [
                Dialog::new(),
                Dialog::new()
                    .title("A rather long title for so small a box")
                    .hints(&hints),
                Dialog::new().width(DialogWidth::Wide).body_rows(u16::MAX),
            ] {
                let rect = dialog.rect(area, &theme);
                assert!(
                    rect.width <= area.width && rect.height <= area.height,
                    "{rect:?} in {area:?}"
                );
                if !rect.is_empty() {
                    assert!(
                        rect.x >= area.x
                            && rect.right() <= area.right()
                            && rect.y >= area.y
                            && rect.bottom() <= area.bottom(),
                        "{rect:?} escapes {area:?}"
                    );
                }
                let body = dialog.draw(area, &mut buf, &theme);
                assert!(body.width <= area.width && body.height <= area.height);
            }
            for y in 0..90 {
                for x in 0..210 {
                    if !area.contains((x, y).into()) {
                        assert_eq!(
                            buf[(x, y)].symbol(),
                            " ",
                            "{w}x{h} {} at {x},{y}",
                            profile.name()
                        );
                    }
                }
            }
        }
    }
}

/// Where grounds do not paint a dialog is told from the work behind it by
/// its edge alone: a complete box, in every such profile, with the work
/// behind it cleared.
#[test]
fn a_dialog_keeps_its_edge_where_grounds_collapse() {
    let hints = confirm_hints();
    for profile in [
        Profile::Ansi16,
        Profile::NoColor,
        Profile::Ascii,
        Profile::UnknownGround,
        Profile::Dark256,
        Profile::Light256,
        Profile::DarkTrue,
    ] {
        let theme = profile.theme();
        let area = Rect::new(0, 0, 70, 16);
        let mut buf = Buffer::empty(area);
        backdrop(area, &mut buf, &theme);
        let dialog = Dialog::new()
            .title("Stop the running workflow?")
            .body_rows(2)
            .hints(&hints);
        let rect = dialog.rect(area, &theme);
        dialog.draw(area, &mut buf, &theme);

        let (tl, tr, bl, br, h, v) = if theme.ascii() {
            ("+", "+", "+", "+", "-", "|")
        } else {
            ("┌", "┐", "└", "┘", "─", "│")
        };
        let sym = |x: u16, y: u16| buf[(x, y)].symbol().to_string();
        let (left, right, top, bottom) = (rect.x, rect.right() - 1, rect.y, rect.bottom() - 1);
        assert_eq!(sym(left, top), tl, "{}", profile.name());
        assert_eq!(sym(right, top), tr, "{}", profile.name());
        assert_eq!(sym(left, bottom), bl, "{}", profile.name());
        assert_eq!(sym(right, bottom), br, "{}", profile.name());
        for x in left + 1..right {
            assert_eq!(sym(x, top), h, "{} top edge at {x}", profile.name());
            assert_eq!(sym(x, bottom), h, "{} bottom edge at {x}", profile.name());
        }
        for y in top + 1..bottom {
            assert_eq!(sym(left, y), v, "{} left edge at {y}", profile.name());
            assert_eq!(sym(right, y), v, "{} right edge at {y}", profile.name());
        }
        // Nothing of the work behind shows through the box.
        let inside: String = (top + 1..bottom)
            .flat_map(|y| (left + 1..right).map(move |x| (x, y)))
            .map(|(x, y)| sym(x, y))
            .collect();
        assert!(!inside.contains("Edited"), "{}: {inside:?}", profile.name());
        // The edge is the strong one, never the quiet line.
        let edge_fg = buf[(left, top)].fg;
        if let Some(strong) = theme.color(Role::BorderStrong) {
            assert_eq!(edge_fg, strong, "{}", profile.name());
        }
        // And the box is the only thing at the edge: the cell just outside
        // still shows the work behind.
        assert!(sym(left - 1, top + 1) != " ", "{}", profile.name());
    }
}

#[test]
fn a_dialog_on_a_painted_ground_sits_on_the_surface() {
    let theme = Profile::DarkTrue.theme();
    let area = Rect::new(0, 0, 70, 16);
    let mut buf = Buffer::empty(area);
    let dialog = Dialog::new().title("Stop?").body_rows(2);
    let rect = dialog.rect(area, &theme);
    let body = dialog.draw(area, &mut buf, &theme);
    let inside = &buf[(body.x, body.y)];
    assert_eq!(inside.bg, theme.color(Role::Surface).unwrap());
    assert!(rect.contains(body.as_position()));
}

// ------------------------------------------------------------------- sheet

#[test]
fn a_sheet_anchors_to_its_edge_and_respects_its_maximum() {
    let area = Rect::new(2, 1, 80, 24);
    let bottom = Sheet::new().size(8);
    assert_eq!(bottom.rect(area), Rect::new(2, 17, 80, 8));
    assert_eq!(
        Sheet::new().edge(SheetEdge::Top).size(6).rect(area),
        Rect::new(2, 1, 80, 6)
    );
    assert_eq!(
        Sheet::new().edge(SheetEdge::Left).size(30).rect(area),
        Rect::new(2, 1, 30, 24)
    );
    assert_eq!(
        Sheet::new().edge(SheetEdge::Right).size(30).rect(area),
        Rect::new(52, 1, 30, 24)
    );
    // The maximum wins over what was asked for, and the terminal over both.
    assert_eq!(Sheet::new().size(20).max_size(9).rect(area).height, 9);
    assert_eq!(Sheet::new().size(99).rect(area).height, 24);
    // Unset, a sheet takes half the terminal.
    assert_eq!(Sheet::new().rect(area).height, 12);
    assert_eq!(SheetEdge::default(), SheetEdge::Bottom);
}

#[test]
fn a_sheet_never_panics_at_any_size() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        let hints = confirm_hints();
        for edge in [
            SheetEdge::Bottom,
            SheetEdge::Top,
            SheetEdge::Left,
            SheetEdge::Right,
        ] {
            for (w, h) in [(0, 0), (1, 1), (0, 4), (4, 0), (3, 3), (9, 5), (80, 24)] {
                let area = Rect::new(1, 1, w, h);
                let mut buf = Buffer::empty(Rect::new(0, 0, 90, 30));
                let sheet = Sheet::new()
                    .edge(edge)
                    .title("Settings")
                    .aside("3 changed")
                    .hints(&hints);
                let rect = sheet.rect(area);
                assert!(rect.width <= area.width && rect.height <= area.height);
                let body = sheet.draw(area, &mut buf, &theme);
                assert!(body.width <= area.width && body.height <= area.height);
                for y in 0..30 {
                    for x in 0..90 {
                        if !area.contains((x, y).into()) {
                            assert_eq!(buf[(x, y)].symbol(), " ", "{edge:?} {w}x{h} at {x},{y}");
                        }
                    }
                }
            }
        }
    }
}

/// A sheet is told from the work behind it by its ground where grounds
/// differ, and by an edge everywhere else.
#[test]
fn a_sheet_is_always_distinguishable_from_the_work_behind_it() {
    for profile in Profile::ALL {
        for theme in [profile.theme(), profile.theme().without_base_ground()] {
            let area = Rect::new(0, 0, 40, 12);
            let mut buf = Buffer::empty(area);
            backdrop(area, &mut buf, &theme);
            let sheet = Sheet::new().title("Settings").size(6);
            let rect = sheet.rect(area);
            sheet.draw(area, &mut buf, &theme);
            let edged = ["┌", "+"].contains(&buf[(rect.x, rect.y)].symbol());
            let grounded = theme.grounds_differ(Role::Surface, Role::Background);
            assert!(
                edged || grounded,
                "{}: neither an edge nor its own ground",
                profile.name()
            );
            assert!(sheet.rect(area).y + sheet.rect(area).height == area.bottom());
        }
    }
}

#[test]
fn panel_body_is_what_draw_returns() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        let hints = confirm_hints();
        for depth in [Depth::Deep, Depth::Stage, Depth::Raised, Depth::Overlay] {
            for (w, h) in [(0, 0), (3, 3), (9, 5), (30, 7), (60, 20)] {
                let area = Rect::new(1, 2, w, h);
                let panel = Panel::new(depth)
                    .title("Title")
                    .aside("aside")
                    .hints(&hints);
                let mut buf = Buffer::empty(Rect::new(0, 0, 70, 30));
                assert_eq!(panel.body(area, &theme), panel.draw(area, &mut buf, &theme));
            }
        }
    }
}

// ------------------------------------------------------------------ keymap

fn verbs(hints: &KeyHints) -> Vec<String> {
    hints.items.iter().map(|h| h.verb.to_string()).collect()
}

/// `Ctrl+E` and `Ctrl+Shift+E` are different chords with different hints, so a
/// keymap that declares both must fire both. Shift was stripped before the
/// comparison, so an event `Char('E')` with `CONTROL | SHIFT` fired whichever
/// of the two was declared first.
#[test]
fn ctrl_e_and_ctrl_shift_e_are_two_chords() {
    let ctrl_e = KeyChord::ctrl('e');
    let ctrl_shift_e = KeyChord::new(
        KeyCode::Char('e'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    let plain = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
    // Terminals report the shifted letter either way.
    let shifted = [
        KeyEvent::new(
            KeyCode::Char('E'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ),
        KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ),
    ];
    assert!(ctrl_e.matches(&plain));
    assert!(!ctrl_shift_e.matches(&plain));
    for event in &shifted {
        assert!(ctrl_shift_e.matches(event), "{event:?}");
        assert!(!ctrl_e.matches(event), "{event:?} is not Ctrl+E");
    }
    // The hints differ, so the bindings must too.
    assert_eq!(ctrl_e.label(LINUX), "Ctrl+E");
    assert_eq!(ctrl_shift_e.label(LINUX), "Ctrl+Shift+E");

    // Declared in either order, each chord fires its own action.
    let both = Keymap::new()
        .with(Binding::new(ctrl_e, "export", "export"))
        .with(Binding::new(ctrl_shift_e, "export all", "export-all"));
    let reversed = Keymap::new()
        .with(Binding::new(ctrl_shift_e, "export all", "export-all"))
        .with(Binding::new(ctrl_e, "export", "export"));
    for map in [&both, &reversed] {
        assert_eq!(map.lookup(&plain), Some(&"export"));
        for event in &shifted {
            assert_eq!(map.lookup(event), Some(&"export-all"), "{event:?}");
        }
    }
    // The same holds for Alt and Super.
    for modifier in [KeyModifiers::ALT, KeyModifiers::SUPER] {
        let bare = KeyChord::new(KeyCode::Char('e'), modifier);
        let with_shift = KeyChord::new(KeyCode::Char('e'), modifier | KeyModifiers::SHIFT);
        let event = KeyEvent::new(KeyCode::Char('E'), modifier | KeyModifiers::SHIFT);
        assert!(with_shift.matches(&event) && !bare.matches(&event));
    }
}

/// What the fix must not cost: Shift is still folded where it only says the
/// character is shifted, and BackTab still carries it.
#[test]
fn shift_is_still_folded_without_a_command_modifier() {
    let question = KeyChord::char('?');
    for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
        assert!(question.matches(&KeyEvent::new(KeyCode::Char('?'), modifiers)));
    }
    let upper = KeyChord::char('E');
    assert!(upper.matches(&KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT)));
    assert!(upper.matches(&KeyEvent::new(KeyCode::Char('E'), KeyModifiers::NONE)));
    assert!(!upper.matches(&KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)));
    let back_tab = KeyChord::plain(KeyCode::BackTab);
    assert!(back_tab.matches(&KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)));
    assert!(back_tab.matches(&KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)));
    // A letter held with Ctrl still arrives in either case.
    assert!(KeyChord::ctrl('o').matches(&KeyEvent::new(KeyCode::Char('O'), KeyModifiers::CONTROL)));
}

#[test]
fn a_keymap_looks_keys_up_and_skips_what_is_disabled() {
    let map = settings_keymap();
    assert_eq!(map.lookup(&key(KeyCode::Enter)), Some(&"change"));
    assert_eq!(map.lookup(&key(KeyCode::Down)), Some(&"move"));
    assert_eq!(map.lookup(&key(KeyCode::Up)), Some(&"move"));
    assert_eq!(map.lookup(&key(KeyCode::Char('?'))), Some(&"help"));
    assert_eq!(
        map.lookup(&KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT)),
        Some(&"help")
    );
    assert_eq!(
        map.lookup(&KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
        None
    );
    assert_eq!(map.lookup(&key(KeyCode::Char('x'))), None);
    let mut release = key(KeyCode::Enter);
    release.kind = KeyEventKind::Release;
    assert_eq!(map.lookup(&release), None);
}

#[test]
fn a_wide_row_shows_every_hint_in_declaration_order_and_no_more() {
    let words = KeyHintsWords::default();
    let hints = settings_keymap().hints(LINUX, 120, &words);
    assert_eq!(
        verbs(&hints),
        ["move", "change", "reset", "search", "save", "close"]
    );
    let theme = Profile::DarkTrue.theme();
    let lines = hints.lines(120, &theme);
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0].to_string(),
        "↑↓ move · Enter change · r reset · / search · Ctrl+S save · Esc close"
    );
    assert!(!hints.items[4].enabled, "the disabled binding shows dimmed");
}

#[test]
fn a_narrow_row_drops_the_lowest_priority_first_and_says_more() {
    let words = KeyHintsWords::default();
    let theme = Profile::DarkTrue.theme();
    let map = settings_keymap();
    let at = |width: u16| {
        let hints = map.hints(LINUX, width, &words);
        let lines = hints.lines(width, &theme);
        assert_eq!(lines.len(), 1, "one row at {width}");
        assert!(lines[0].width() <= usize::from(width), "fits at {width}");
        lines[0].to_string()
    };
    assert_eq!(
        at(80),
        "↑↓ move · Enter change · r reset · / search · Ctrl+S save · Esc close"
    );
    assert_eq!(
        at(60),
        "↑↓ move · Enter change · / search · Esc close · ? more"
    );
    assert_eq!(at(40), "Enter change · Esc close · ? more");
    assert_eq!(at(21), "Enter change · ? more");
    assert_eq!(at(20), "Enter change");
}

#[test]
fn folding_keeps_the_highest_and_drops_exactly_the_lowest_at_every_width() {
    let words = KeyHintsWords::default();
    let map = settings_keymap();
    let priority = |verb: &str| {
        map.bindings()
            .iter()
            .position(|b| b.verb == verb)
            .map(|i| (map.bindings()[i].priority, i))
            .unwrap()
    };
    let all = ["move", "change", "reset", "search", "save", "close"];
    for width in 1..=130u16 {
        let hints = map.hints(LINUX, width, &words);
        let shown = verbs(&hints);
        let kept: Vec<&str> = shown
            .iter()
            .map(String::as_str)
            .filter(|v| *v != "more")
            .collect();
        let dropped: Vec<&str> = all.iter().copied().filter(|v| !kept.contains(v)).collect();

        assert!(
            kept.contains(&"change"),
            "the highest is kept at {width}: {shown:?}"
        );
        // What is dropped is the lowest: every dropped binding ranks below
        // every kept one (priority, then the later declaration).
        let rank = |v: &str| {
            let (p, i) = priority(v);
            (std::cmp::Reverse(p), i)
        };
        for d in &dropped {
            for k in &kept {
                assert!(rank(k) < rank(d), "{d} dropped but {k} kept at {width}");
            }
        }
        // Declaration order is stable.
        let order: Vec<usize> = kept.iter().map(|v| priority(v).1).collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{kept:?} at {width}");
        // `? more` appears exactly when something was dropped and there is
        // room for it beside the one hint that must stay.
        let more = shown.last().is_some_and(|v| v == "more");
        if more {
            assert!(!dropped.is_empty(), "more with nothing dropped at {width}");
            assert_eq!(hints.items.last().unwrap().keys, "?");
        }
        if dropped.is_empty() {
            assert!(!more, "more with nothing dropped at {width}");
        }
        // Never wider than the row, unless it is the one hint that stays.
        let theme = Profile::DarkTrue.theme();
        let lines = hints.lines(width, &theme);
        if hints.items.len() > 1 {
            assert_eq!(lines.len(), 1, "wrapped at {width}: {shown:?}");
            assert!(lines[0].width() <= usize::from(width));
        }
    }
}

#[test]
fn without_a_help_binding_nothing_says_more() {
    let map = Keymap::new()
        .with(Binding::new(KeyCode::Enter, "select", ()).priority(200))
        .with(Binding::new(KeyCode::Esc, "cancel", ()).priority(100))
        .with(Binding::new(KeyChord::char('d'), "delete", ()).priority(50));
    let hints = KeyHints::from_keymap(&map, LINUX, 20, &KeyHintsWords::default());
    assert_eq!(verbs(&hints), ["select"]);
}

#[test]
fn the_word_beside_the_help_key_is_the_hosts() {
    let words = KeyHintsWords {
        more: "mehr".into(),
    };
    let hints = settings_keymap().hints(LINUX, 40, &words);
    let last = hints.items.last().unwrap();
    assert_eq!((last.keys.as_ref(), last.verb.as_ref()), ("?", "mehr"));
}

#[test]
fn equal_priorities_drop_the_later_declaration_first() {
    let map = Keymap::new()
        .with(Binding::new(KeyCode::Enter, "open", ()).priority(10))
        .with(Binding::new(KeyChord::char('a'), "first", ()).priority(10))
        .with(Binding::new(KeyChord::char('b'), "second", ()).priority(10))
        .with(Binding::help(KeyChord::char('?'), ()));
    // "Enter open · a first · ? more" is 29 cells.
    let hints = map.hints(LINUX, 29, &KeyHintsWords::default());
    assert_eq!(verbs(&hints), ["open", "first", "more"]);
}

#[test]
fn ascii_hints_keep_ascii_keys_and_two_space_separators() {
    let ascii = Platform {
        macos: false,
        ascii: true,
    };
    let hints = settings_keymap().hints(ascii, 120, &KeyHintsWords::default());
    assert_eq!(hints.items[0].keys, "Up/Down");
    let theme = Profile::Ascii.theme();
    let line = hints.lines(120, &theme)[0].to_string();
    assert!(line.is_ascii(), "{line}");
    assert!(line.starts_with("Up/Down move  Enter change"), "{line}");
}

#[test]
fn long_hints_preserve_styled_graphemes_and_painted_actions() {
    let theme = Profile::DarkTrue.theme();
    let hints = KeyHints::new(vec![
        KeyHint::new("Enter", "select a very long action"),
        KeyHint::new("鲸鱼", "cafe\u{301}"),
    ]);
    let lines = hints.lines(4, &theme);
    assert!(lines.iter().all(|line| line.width() <= 4));
    let text: String = lines.iter().map(ToString::to_string).collect();
    assert_eq!(text, "Enter select a very long action鲸鱼 cafe\u{301}");
}

// ------------------------------------------------------------------ toggle

#[test]
fn toggle_keys_flip_report_and_respect_disabled() {
    let mut toggle = Toggle::new("Show tool details", false);
    assert_eq!(
        toggle.handle_key(key(KeyCode::Char(' '))),
        ToggleOutcome::Toggled(true)
    );
    assert_eq!(
        toggle.handle_key(key(KeyCode::Enter)),
        ToggleOutcome::Toggled(false)
    );
    assert_eq!(
        toggle.handle_key(key(KeyCode::Char('x'))),
        ToggleOutcome::Ignored
    );
    let mut release = key(KeyCode::Enter);
    release.kind = KeyEventKind::Release;
    assert_eq!(toggle.handle_key(release), ToggleOutcome::Ignored);
    let chord = KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL);
    assert_eq!(toggle.handle_key(chord), ToggleOutcome::Ignored);
    assert!(!toggle.state.on);

    let mut locked = Toggle::new("Telemetry", true).disabled("set by your admin");
    assert_eq!(
        locked.handle_key(key(KeyCode::Enter)),
        ToggleOutcome::Ignored
    );
    assert!(locked.state.on);
    let mut state = ToggleState::default();
    assert_eq!(
        state.handle_key(key(KeyCode::Enter), true),
        ToggleOutcome::Toggled(true)
    );
}

#[test]
fn toggle_words_are_supplied_by_the_host() {
    let words = ToggleWords {
        on: "Ein".into(),
        off: "Aus".into(),
    };
    let theme = Profile::DarkTrue.theme();
    let toggle = Toggle::new("Telemetrie", true).with_words(&words);
    let buf = testing::render(40, 1, |area, buf| toggle.paint(area, buf, &theme));
    assert!(
        testing::text(&buf).ends_with("● Ein"),
        "{}",
        testing::text(&buf)
    );
}

// --------------------------------------------------------------- segmented

#[test]
fn segmented_keys_move_jump_and_stop_at_the_ends() {
    let mut control = Segmented::new(["Full", "Reduced", "Still"], SegmentedState::new(0));
    assert_eq!(
        control.handle_key(key(KeyCode::Left)),
        SegmentedOutcome::Ignored
    );
    assert_eq!(
        control.handle_key(key(KeyCode::Right)),
        SegmentedOutcome::Selected(1)
    );
    assert_eq!(
        control.handle_key(key(KeyCode::Char('3'))),
        SegmentedOutcome::Selected(2)
    );
    assert_eq!(
        control.handle_key(key(KeyCode::Right)),
        SegmentedOutcome::Ignored
    );
    assert_eq!(
        control.handle_key(key(KeyCode::Char('4'))),
        SegmentedOutcome::Ignored
    );
    assert_eq!(
        control.handle_key(key(KeyCode::Char('1'))),
        SegmentedOutcome::Selected(0)
    );
    assert_eq!(control.state.selected, 0);
    let mut release = key(KeyCode::Right);
    release.kind = KeyEventKind::Release;
    assert_eq!(control.handle_key(release), SegmentedOutcome::Ignored);
    let mut locked = Segmented::new(["A", "B"], SegmentedState::new(0)).disabled("locked");
    assert_eq!(
        locked.handle_key(key(KeyCode::Right)),
        SegmentedOutcome::Ignored
    );
}

#[test]
fn segmented_shows_a_bracketed_selection_or_the_compact_form() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        let control = Segmented::new(["Full", "Reduced", "Still"], SegmentedState::new(1));
        let at = |width: u16| {
            testing::text(&testing::render(width, 1, |area, buf| {
                control.paint(area, buf, &theme)
            }))
        };
        assert_eq!(at(40), " Full  [Reduced]  Still", "{}", profile.name());
        assert_eq!(at(20), "Reduced (2 of 3)", "{}", profile.name());
    }
}

// -------------------------------------------------------------------- tabs

#[test]
fn tabs_keys_move_wrap_and_scroll_the_selection_into_view() {
    let items = tab_items();
    let mut state = TabsState::new(0);
    for _ in 0..items.len() - 1 {
        let mut tabs = Tabs::new(&items, state);
        assert!(matches!(
            tabs.handle_key(key(KeyCode::Right), 36),
            TabsOutcome::Selected(_)
        ));
        state = tabs.state;
        assert!(tabs.visible(36).contains(&state.selected));
    }
    assert_eq!(state.selected, items.len() - 1);
    assert!(state.offset > 0, "the strip scrolled");
    let mut tabs = Tabs::new(&items, state);
    assert_eq!(
        tabs.handle_key(key(KeyCode::Right), 36),
        TabsOutcome::Ignored
    );
    assert_eq!(
        tabs.handle_key(key(KeyCode::Tab), 36),
        TabsOutcome::Selected(0)
    );
    assert_eq!(
        tabs.state.offset, 0,
        "wrapping to the first tab scrolls back"
    );
    let back = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(
        tabs.handle_key(back, 36),
        TabsOutcome::Selected(items.len() - 1)
    );
}

#[test]
fn the_selected_tab_is_on_screen_at_every_width_and_selection() {
    let items = tab_items();
    for profile in [Profile::DarkTrue, Profile::NoColor, Profile::Ascii] {
        let theme = profile.theme();
        for width in testing::WIDTHS {
            for selected in 0..items.len() {
                let tabs = Tabs::new(&items, TabsState::new(selected));
                let buf = testing::render(width, 2, |area, buf| tabs.paint(area, buf, &theme));
                let text = testing::text(&buf);
                let label = items[selected].label.as_ref();
                assert!(
                    text.lines().next().unwrap().contains(label),
                    "{width} {selected}: {text}"
                );
                // The underline sits under the selected label, as wide as it.
                let rule = if theme.ascii() { '-' } else { '─' };
                let second = text.lines().nth(1).unwrap_or_default();
                assert!(
                    second.chars().filter(|c| *c == rule).count() >= label.chars().count().min(4),
                    "{width} {selected}: {second:?}"
                );
            }
        }
    }
}

#[test]
fn a_narrow_strip_marks_what_it_hides_with_a_count() {
    let items = tab_items();
    let theme = Profile::DarkTrue.theme();
    let tabs = Tabs::new(&items, TabsState::new(0));
    let first = testing::text(&testing::render(34, 2, |area, buf| {
        tabs.paint(area, buf, &theme)
    }));
    assert!(
        first.lines().next().unwrap().trim_end().ends_with("more ›"),
        "{first}"
    );
    let tabs = Tabs::new(&items, TabsState::new(5));
    let last = testing::text(&testing::render(34, 2, |area, buf| {
        tabs.paint(area, buf, &theme)
    }));
    assert!(last.lines().next().unwrap().starts_with("‹ "), "{last}");
}

#[test]
fn folded_tabs_keep_a_selected_unicode_label_and_its_badge() {
    let items = [
        Tab::new("General"),
        Tab::new("Connections"),
        Tab::new("鲸鱼 cafe\u{301}").badge(2),
        Tab::new("Privacy"),
    ];
    let theme = Profile::DarkTrue.theme();
    let tabs = Tabs::new(&items, TabsState::new(2));
    let text = testing::text(&testing::render(24, 2, |area, buf| {
        tabs.paint(area, buf, &theme)
    }));
    assert!(text.contains("鲸鱼 cafe\u{301} 2"), "{text}");
}

// ----------------------------------------------------------------- heading

#[test]
fn headings_and_panels_do_not_lose_titles_to_oversized_asides() {
    let theme = Profile::DarkTrue.theme();
    let buf = testing::render(10, 5, |area, buf| {
        Panel::new(Depth::Stage)
            .title("Settings")
            .aside("an aside much wider than the area")
            .draw(area, buf, &theme);
    });
    assert!(testing::text(&buf).contains("Settings"));
    let buf = testing::render(8, 1, |area, buf| {
        Heading::new("Settings")
            .meta("a very long aside")
            .paint(area, buf, &theme)
    });
    assert_eq!(testing::text(&buf), "Settings");
    for area in [Rect::new(3, 4, 0, 4), Rect::new(3, 4, 4, 0)] {
        assert_eq!(centered(area, 50, 30, 10, 8), area);
    }
}

// --------------------------------------------------- nothing leaves its area

#[test]
fn chrome_paints_only_inside_its_area_at_narrow_widths() {
    let items = tab_items();
    let hints = confirm_hints();
    for profile in Profile::ALL {
        let theme: Theme = profile.theme();
        for width in [0, 1, 4, 9, 20] {
            let components: Vec<Box<dyn Paint>> = vec![
                Box::new(Heading::new("Heading").meta("meta")),
                Box::new(Tabs::new(&items, TabsState::new(3)).focused(true)),
                Box::new(Toggle::new("Label", true).focused(true).disabled("why")),
                Box::new(Segmented::new(["Full", "Reduced"], SegmentedState::new(1))),
                Box::new(Dialog::new().title("Title").body_rows(3).hints(&hints)),
                Box::new(Sheet::new().title("Title").hints(&hints)),
            ];
            for component in components {
                let mut buf = Buffer::empty(Rect::new(0, 0, 40, 18));
                let area = Rect::new(3, 4, width, 8);
                component.paint(area, &mut buf, &theme);
                for y in 0..18 {
                    for x in 0..40 {
                        if !area.contains((x, y).into()) {
                            assert_eq!(buf[(x, y)].symbol(), " ", "outside {area:?} at {x},{y}");
                        }
                    }
                }
            }
        }
    }
}

// ------------------------------------------------- rules and snapshots

fn heading_paint(area: Rect, buf: &mut Buffer, theme: &Theme) {
    Heading::new("Settings")
        .meta("saved to this project")
        .paint(sub(area, 0, 1), buf, theme);
    Heading::new("Appearance")
        .section()
        .meta("3 changed")
        .paint(sub(area, 2, 2), buf, theme);
    Heading::new("Defaults")
        .sub()
        .paint(sub(area, 4, 1), buf, theme);
    Heading::new("A page title far too long for a narrow terminal to hold")
        .meta("12 items")
        .paint(sub(area, 6, 1), buf, theme);
}

fn heading_cjk(area: Rect, buf: &mut Buffer, theme: &Theme) {
    Heading::new("鲸鱼的设置页面标题非常非常长，终端放不下").paint(sub(area, 0, 1), buf, theme);
    Heading::new("设置")
        .section()
        .meta("已修改 3 项")
        .paint(sub(area, 1, 1), buf, theme);
}

fn tabs_start(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = tab_items();
    Tabs::new(&items, TabsState::new(2))
        .focused(true)
        .paint(area, buf, theme);
}

fn tabs_scrolled(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = tab_items();
    let mut state = TabsState::new(4);
    Tabs::new(&items, state).scroll_into_view(&mut state, area.width);
    Tabs::new(&items, state).paint(area, buf, theme);
}

fn tabs_end(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = tab_items();
    let mut state = TabsState::new(5);
    Tabs::new(&items, state).scroll_into_view(&mut state, area.width);
    Tabs::new(&items, state).paint(area, buf, theme);
}

fn tabs_one_row(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = tab_items();
    Tabs::new(&items[..3], TabsState::new(1)).paint(sub(area, 0, 1), buf, theme);
}

fn toggles(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rows = [
        Toggle::new("Show tool details", true),
        Toggle::new("Reduced motion", false).focused(true),
        Toggle::new("Share usage data", false).disabled("set by your admin"),
        Toggle::new("Show reasoning as it streams in the transcript", true).focused(true),
    ];
    for (i, toggle) in rows.iter().enumerate() {
        toggle.paint(sub(area, i as u16, 1), buf, theme);
    }
}

fn toggle_cjk(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let words = ToggleWords {
        on: "开".into(),
        off: "关".into(),
    };
    Toggle::new("鲸鱼模式", true)
        .with_words(&words)
        .paint(sub(area, 0, 1), buf, theme);
    Toggle::new("显示工具详情，并在对话记录里展开推理过程", false)
        .with_words(&words)
        .focused(true)
        .paint(sub(area, 1, 1), buf, theme);
}

fn segmenteds(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let motion = ["Full", "Reduced", "Still"];
    Segmented::new(motion, SegmentedState::new(1)).paint(sub(area, 0, 1), buf, theme);
    Segmented::new(["Light", "Dark", "System"], SegmentedState::new(0))
        .focused(true)
        .paint(sub(area, 1, 1), buf, theme);
    Segmented::new(motion, SegmentedState::new(2))
        .disabled("set by your admin")
        .paint(sub(area, 2, 1), buf, theme);
    Segmented::new(["On", "Off"], SegmentedState::new(0)).paint(sub(area, 3, 1), buf, theme);
    let narrow = Rect {
        width: area.width.min(14),
        ..area
    };
    Segmented::new(motion, SegmentedState::new(1)).paint(sub(narrow, 4, 1), buf, theme);
    Segmented::new(
        ["Light", "Dark", "System", "High contrast", "Auto"],
        SegmentedState::new(3),
    )
    .paint(sub(area, 5, 1), buf, theme);
}

fn keymap_hints(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let platform = Platform::current(theme.ascii());
    let map = settings_keymap();
    for (row, width) in [area.width, 34.min(area.width), 22.min(area.width)]
        .into_iter()
        .enumerate()
    {
        let rect = Rect {
            y: area.y + row as u16,
            width,
            height: 1,
            ..area
        };
        map.hints(platform, width, &KeyHintsWords::default())
            .paint(rect, buf, theme);
    }
}

fn dialog_narrow(area: Rect, buf: &mut Buffer, theme: &Theme) {
    backdrop(area, buf, theme);
    let hints = confirm_hints();
    let body = Dialog::new()
        .title("Stop the running workflow?")
        .width(DialogWidth::Narrow)
        .body_rows(2)
        .hints(&hints)
        .draw(area, buf, theme);
    say(
        body,
        buf,
        theme,
        "2 agents are still working. Their edits so far are kept.",
    );
}

fn dialog_wide(area: Rect, buf: &mut Buffer, theme: &Theme) {
    backdrop(area, buf, theme);
    let hints = KeyHints::new(vec![
        KeyHint::new("y", "delete branch"),
        KeyHint::new("n", "keep it"),
        KeyHint::new("d", "show what would be deleted"),
    ]);
    let body = Dialog::new()
        .title("Delete branch feat/kit-buildout and everything built on it")
        .width(DialogWidth::Wide)
        .body_rows(3)
        .hints(&hints)
        .draw(area, buf, theme);
    say(
        body,
        buf,
        theme,
        "This cannot be undone. Unpushed commits on the branch are lost.",
    );
}

fn dialog_cjk(area: Rect, buf: &mut Buffer, theme: &Theme) {
    backdrop(area, buf, theme);
    let hints = KeyHints::new(vec![
        KeyHint::new("y", "停止"),
        KeyHint::new("n", "继续运行"),
    ]);
    let body = Dialog::new()
        .title("要停止正在运行的工作流吗？")
        .width(DialogWidth::Narrow)
        .body_rows(2)
        .hints(&hints)
        .draw(area, buf, theme);
    say(
        body,
        buf,
        theme,
        "仍有两个智能体在工作。它们到目前为止的改动会保留。",
    );
}

fn dialog_bare(area: Rect, buf: &mut Buffer, theme: &Theme) {
    backdrop(area, buf, theme);
    let body = Dialog::new().body_rows(1).draw(area, buf, theme);
    say(body, buf, theme, "A dialog with no title and no hints.");
}

fn sheet_bottom(area: Rect, buf: &mut Buffer, theme: &Theme) {
    backdrop(area, buf, theme);
    let hints = KeyHints::new(vec![
        KeyHint::new(
            codewhale_ratatui::keys::pair_label(
                KeyCode::Up,
                KeyCode::Down,
                Platform::current(theme.ascii()),
            ),
            "move",
        ),
        KeyHint::new("Enter", "change"),
        KeyHint::new("Esc", "close"),
    ]);
    let body = Sheet::new()
        .title("Settings")
        .aside("3 changed")
        .size(10)
        .max_size(9)
        .hints(&hints)
        .draw(area, buf, theme);
    say(
        body,
        buf,
        theme,
        "Theme       Shoreline\nMotion      Reduced\nTool detail Collapsed",
    );
}

fn sheet_right(area: Rect, buf: &mut Buffer, theme: &Theme) {
    backdrop(area, buf, theme);
    let body = Sheet::new()
        .edge(SheetEdge::Right)
        .title("Agents")
        .aside("2 running")
        .size(30)
        .draw(area, buf, theme);
    say(
        body,
        buf,
        theme,
        "reviewer is reading the diff\nbuilder is editing tabs.rs",
    );
}

/// `assert_rules` over every profile at 40, 80 and 120, and a snapshot of
/// `DarkTrue` styled plus `NoColor` and `Ascii` as text.
macro_rules! checked {
    ($test:ident, $name:literal, $height:expr, $paint:expr) => {
        #[test]
        fn $test() {
            testing::assert_rules($height, $paint);
            insta::assert_snapshot!($name, testing::snapshot($height, $paint));
        }
    };
}

/// Caller text may be any script, so the ASCII profile (which must draw
/// ASCII only) is left out of the rule check; the snapshot still records it.
macro_rules! checked_cjk {
    ($test:ident, $name:literal, $height:expr, $paint:expr) => {
        #[test]
        fn $test() {
            let profiles: Vec<Profile> = Profile::ALL
                .into_iter()
                .filter(|p| *p != Profile::Ascii)
                .collect();
            let frames = testing::frames_for("", &profiles, &testing::WIDTHS, $height, $paint);
            testing::assert_frames_keep_the_rules(&frames);
            insta::assert_snapshot!($name, testing::snapshot($height, $paint));
        }
    };
}

checked!(heading, "heading", 7, heading_paint);
checked_cjk!(heading_in_cjk, "heading-cjk", 2, heading_cjk);
checked!(tabs_at_the_start, "tabs-start", 2, tabs_start);
checked!(tabs_scrolled_both_ways, "tabs-scrolled", 2, tabs_scrolled);
checked!(tabs_at_the_end, "tabs-end", 2, tabs_end);
checked!(tabs_in_one_row, "tabs-one-row", 1, tabs_one_row);
checked!(toggle, "toggle", 4, toggles);
checked_cjk!(toggle_in_cjk, "toggle-cjk", 2, toggle_cjk);
checked!(segmented, "segmented", 6, segmenteds);
checked!(keymap_folded_hints, "keymap-hints", 3, keymap_hints);
checked!(dialog_narrow_with_hints, "dialog-narrow", 12, dialog_narrow);
checked!(dialog_wide_with_long_hints, "dialog-wide", 14, dialog_wide);
checked_cjk!(dialog_cjk_text, "dialog-cjk", 12, dialog_cjk);
checked!(dialog_without_title_or_hints, "dialog-bare", 8, dialog_bare);
checked!(sheet_on_the_bottom, "sheet-bottom", 12, sheet_bottom);
checked!(sheet_on_the_right, "sheet-right", 12, sheet_right);
