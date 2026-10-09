//! Package Lists: fuzzy matching, List, Picker (query, tabs, preview,
//! no-match) and the empty state. Behavior first, then the frame rules and
//! snapshots at 40, 80 and 120 columns.

use std::borrow::Cow;
use std::cell::RefCell;

use codewhale_ratatui::{
    EmptyMark, EmptyState, List, ListOutcome, ListRow, ListRowState, ListState, Paint, Picker,
    PickerItem, PickerMatches, PickerOutcome, PickerState, PickerTabs, PickerWords, Theme,
    fuzzy_score, rank, rank_matches,
    testing::{self, Profile},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

// ---------------------------------------------------------------- fuzzy

#[test]
fn a_match_needs_every_query_character_in_order() {
    let m = fuzzy_score("abc", "aXbXc").expect("subsequence");
    assert_eq!(m.positions, [0, 2, 4]);
    assert!(fuzzy_score("acb", "abc").is_none(), "order matters");
    assert!(fuzzy_score("abcd", "abc").is_none(), "every character");
    assert!(fuzzy_score("x", "").is_none());
    assert!(fuzzy_score("aa", "a").is_none(), "a character is used once");
}

#[test]
fn matching_ignores_case_and_query_whitespace() {
    assert!(fuzzy_score("SHORE", "shoreline").is_some());
    assert!(fuzzy_score("shore", "SHORELINE").is_some());
    assert!(fuzzy_score("sh ore", "shoreline").is_some());
    assert_eq!(
        fuzzy_score("sh ore ", "shoreline").unwrap().positions.len(),
        5
    );
    assert!(
        fuzzy_score("É", "école").is_some(),
        "non-ASCII case folds too"
    );
    // Typing the case the candidate has scores a little higher.
    let exact = fuzzy_score("Shore", "Shoreline").unwrap().score;
    let folded = fuzzy_score("shore", "Shoreline").unwrap().score;
    assert!(exact > folded);
}

#[test]
fn an_empty_query_matches_everything_with_nothing_to_highlight() {
    for query in ["", "   "] {
        let m = fuzzy_score(query, "anything").unwrap();
        assert!(m.positions.is_empty());
        assert_eq!(m.score, 0);
        assert_eq!(rank(query, ["b", "a", "c"]), [0, 1, 2], "original order");
    }
    assert_eq!(fuzzy_score("", ""), fuzzy_score("", "x"));
}

#[test]
fn positions_are_ascending_char_indices_of_the_query() {
    let cases = [
        ("rs", "src/main.rs"),
        ("sm", "src/main.rs"),
        ("gS", "getState"),
        ("鱼录", "鲸鱼记录"),
        ("ab", "ab ab ab"),
        ("ÉÉ", "éé"),
    ];
    for (query, candidate) in cases {
        let m = fuzzy_score(query, candidate).unwrap_or_else(|| panic!("{query} in {candidate}"));
        let chars: Vec<char> = candidate.chars().collect();
        let wanted: Vec<char> = query
            .chars()
            .filter(|c| !c.is_whitespace())
            .flat_map(char::to_lowercase)
            .collect();
        assert_eq!(m.positions.len(), wanted.len(), "{query} in {candidate}");
        assert!(m.positions.windows(2).all(|w| w[0] < w[1]));
        for (&at, want) in m.positions.iter().zip(wanted) {
            let got = chars[at].to_lowercase().next().unwrap();
            assert_eq!(got, want, "{query} in {candidate} at {at}");
        }
    }
}

#[test]
fn unicode_and_cjk_candidates_match_by_character() {
    let m = fuzzy_score("鱼录", "鲸鱼记录").unwrap();
    assert_eq!(m.positions, [1, 3], "char indices, not bytes");
    assert!(fuzzy_score("录鱼", "鲸鱼记录").is_none());
    let m = fuzzy_score("café", "Café Noir").unwrap();
    assert_eq!(m.positions, [0, 1, 2, 3]);
    assert_eq!(rank("鲸", ["Shoreline", "鲸鱼工作区", "Reef"]), [1]);
}

#[test]
fn obvious_cases_rank_the_way_a_person_expects() {
    // A prefix beats a scatter.
    assert_eq!(rank("main", ["domain.rs", "main.rs", "mxaxixn"]), [1, 0, 2]);
    // The start of a word beats the middle of one.
    assert_eq!(rank("bar", ["foobar", "foo_bar", "bar"]), [2, 1, 0]);
    // A camelCase hump counts as a word start.
    assert_eq!(rank("ps", ["popups", "PickerState"]), [1, 0]);
    // After a path separator counts too.
    assert_eq!(rank("main", ["domain/x", "src/main.rs"]), [1, 0]);
    // A run of consecutive characters beats the same characters apart.
    assert_eq!(rank("pick", ["p-i-c-k", "picker"]), [1, 0]);
    // The whole candidate beats a longer one that contains it.
    assert_eq!(rank("plan", ["Planning", "Plan"]), [1, 0]);
}

#[test]
fn ranking_is_stable_deterministic_and_drops_non_matches() {
    assert_eq!(
        rank("a", ["a", "a", "a"]),
        [0, 1, 2],
        "ties keep their order"
    );
    let items = ["Shoreline", "Reef", "Shoreline light", "Deep sea"];
    assert_eq!(rank("shore", items), rank("shore", items));
    assert_eq!(rank("zzz", items), Vec::<usize>::new());
    let hits = rank_matches("shore", items);
    assert_eq!(
        hits.iter().map(|h| h.index).collect::<Vec<_>>(),
        rank("shore", items)
    );
    assert!(hits.windows(2).all(|w| w[0].score >= w[1].score));
    assert_eq!(hits[0].positions, [0, 1, 2, 3, 4]);
    // Owned and borrowed candidates both work.
    let owned: Vec<String> = items.iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(rank("shore", &owned), rank("shore", items));
}

// ---------------------------------------------------------------- empty state

fn text_of(
    width: u16,
    height: u16,
    profile: Profile,
    paint: impl Fn(Rect, &mut Buffer, &Theme),
) -> String {
    let theme = profile.theme();
    testing::text(&testing::render(width, height, |a, b| paint(a, b, &theme)))
}

/// The cell column where `needle` starts in a painted row (not its byte).
fn column_of(row: &str, needle: &str) -> u16 {
    let byte = row
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} in {row}"));
    u16::try_from(row[..byte].chars().count()).unwrap()
}

fn has_braille(s: &str) -> bool {
    s.chars().any(|c| ('\u{2800}'..='\u{28ff}').contains(&c))
}

fn empty_state() -> EmptyState<'static> {
    EmptyState::new("No workflow runs yet")
        .body("Runs appear here once a workflow starts, with what each one changed.")
        .action("Start one with /workflow, or press n")
}

#[test]
fn the_empty_state_says_what_is_true_and_the_one_next_step() {
    let shown = text_of(64, 14, Profile::DarkTrue, |a, b, t| {
        empty_state().paint(a, b, t)
    });
    assert!(shown.contains("No workflow runs yet"), "{shown}");
    assert!(shown.contains("Runs appear here"), "{shown}");
    assert!(shown.contains("Start one with /workflow"), "{shown}");
    assert!(has_braille(&shown), "the whale has room at 64x14:\n{shown}");
    // Centred: the title's left margin is within a cell of its right.
    let title = shown.lines().find(|l| l.contains("No workflow")).unwrap();
    let left = title.len() - title.trim_start().len();
    let right = 64 - left - title.trim().chars().count();
    assert!(left.abs_diff(right) <= 1, "{left} {right}");
}

#[test]
fn the_empty_state_degrades_with_room_and_in_ascii() {
    // No rows for the whale at 40x6: a glyph before the title instead.
    let small = text_of(40, 6, Profile::DarkTrue, |a, b, t| {
        empty_state().paint(a, b, t)
    });
    assert!(!has_braille(&small), "{small}");
    assert!(small.contains("○ No workflow runs yet"), "{small}");
    // ASCII has no honest Braille: glyph in ASCII, even with all the room.
    let ascii = text_of(80, 20, Profile::Ascii, |a, b, t| {
        empty_state().paint(a, b, t)
    });
    assert!(ascii.is_ascii() && !has_braille(&ascii), "{ascii}");
    assert!(ascii.contains("o No workflow runs yet"), "{ascii}");
    // One row: the title survives, then nothing else.
    let one = text_of(40, 1, Profile::NoColor, |a, b, t| {
        empty_state().paint(a, b, t)
    });
    assert!(one.contains("No workflow runs yet"), "{one}");
    // The mark is a parameter.
    let none = text_of(64, 14, Profile::DarkTrue, |a, b, t| {
        empty_state().mark(EmptyMark::None).paint(a, b, t);
    });
    assert!(!has_braille(&none) && !none.contains('○'), "{none}");
    let glyph = text_of(64, 14, Profile::DarkTrue, |a, b, t| {
        empty_state().mark(EmptyMark::Glyph).paint(a, b, t);
    });
    assert!(!has_braille(&glyph) && glyph.contains('○'), "{glyph}");
}

#[test]
fn the_empty_state_never_overflows_its_area() {
    for profile in Profile::ALL {
        for width in 0..=44 {
            for height in 0..=16 {
                // A bigger buffer than the area: nothing may land outside it.
                let theme = profile.theme();
                let mut buf = Buffer::empty(Rect::new(0, 0, 50, 20));
                let area = Rect::new(3, 2, width, height);
                empty_state().paint(area, &mut buf, &theme);
                for y in 0..20 {
                    for x in 0..50 {
                        let inside = area.contains((x, y).into());
                        if !inside {
                            assert_eq!(
                                buf[(x, y)].symbol(),
                                " ",
                                "{} {width}x{height} at {x},{y}",
                                profile.name()
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn translated_empty_state_text_is_control_safe() {
    let empty = EmptyState::new("鲸鱼\u{202e}").action("下一步\u{1b}[31m");
    for profile in Profile::ALL {
        let theme = profile.theme();
        let b = testing::render(24, 3, |a, b| empty.paint(a, b, &theme));
        let shown = testing::text(&b);
        assert!(!shown.contains('\u{202e}') && !shown.contains('\u{1b}'));
        assert!(shown.contains("鲸鱼"), "{shown}");
    }
}

#[test]
fn the_empty_state_reports_a_height_that_fits() {
    let theme = Profile::DarkTrue.theme();
    let wanted = empty_state().height(64, &theme);
    assert!(
        text_of(64, wanted, Profile::DarkTrue, |a, b, t| empty_state()
            .paint(a, b, t))
        .contains("Start one with /workflow")
    );
    let ascii = Profile::Ascii.theme();
    assert!(
        empty_state().height(64, &ascii) < wanted,
        "no whale rows in ASCII"
    );
}

// ---------------------------------------------------------------- list

/// A row that is one line, or two, and may be disabled.
struct Row {
    name: &'static str,
    lines: u16,
    disabled: bool,
}

impl ListRow for Row {
    fn height(&self, _width: u16) -> u16 {
        self.lines
    }

    fn is_disabled(&self) -> bool {
        self.disabled
    }

    fn paint_row(&self, area: Rect, buf: &mut Buffer, theme: &Theme, state: ListRowState) {
        buf.set_stringn(
            area.x,
            area.y,
            self.name,
            usize::from(area.width),
            state.ink(theme),
        );
        for dy in 1..area.height {
            buf.set_stringn(
                area.x,
                area.y + dy,
                "  and more",
                usize::from(area.width),
                state.ink(theme),
            );
        }
    }
}

fn rows(n: usize) -> Vec<Row> {
    (0..n)
        .map(|i| Row {
            name: Box::leak(format!("row {i}").into_boxed_str()),
            lines: 1,
            disabled: false,
        })
        .collect()
}

fn viewport(height: u16) -> Rect {
    Rect::new(0, 0, 30, height)
}

/// A tiny deterministic generator, so the property tests need no crate.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % below.max(1)
    }
}

#[test]
fn the_selection_is_always_visible_and_in_range() {
    let mut rng = Lcg(7);
    let keys = [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Home,
        KeyCode::End,
    ];
    for len in 0..=14usize {
        for height in 1..=6u16 {
            for lines in 1..=2u16 {
                let mut rs = rows(len);
                // Some rows disabled, never all.
                for (i, r) in rs.iter_mut().enumerate() {
                    r.lines = lines;
                    r.disabled = len > 1 && i % 3 == 1;
                }
                let mut state = ListState::default();
                state.settle(len, |i| !rs[i].disabled);
                for _ in 0..60 {
                    let code = keys[rng.next(keys.len() as u64) as usize];
                    state.handle_key(key(code), &rs, viewport(height));
                    if len == 0 {
                        assert_eq!((state.selected, state.offset), (0, 0));
                        continue;
                    }
                    assert!(state.selected < len, "in range");
                    assert!(
                        !rs[state.selected].disabled,
                        "never rests on a disabled row"
                    );
                    let offset = state.visible_offset(len, height, |i| rs[i].lines);
                    assert!(offset <= state.selected, "{len} {height} {lines}");
                    let used: u16 = (offset..=state.selected).map(|i| rs[i].lines).sum();
                    assert!(
                        used <= height.max(lines),
                        "selected row is fully visible: len {len} height {height} lines {lines} \
                         selected {} offset {offset}",
                        state.selected
                    );
                    assert_eq!(state.offset, offset, "the stored offset is the painted one");
                }
            }
        }
    }
}

#[test]
fn a_stale_selection_is_pulled_into_range() {
    let mut state = ListState::new(99);
    state.offset = 50;
    assert_eq!(
        state.visible_offset(10, 4, |_| 1),
        6,
        "last row on the last line"
    );
    let mut state = ListState::new(99);
    state.settle(3, |_| true);
    assert_eq!(state.selected, 2);
    state.settle(0, |_| true);
    assert_eq!((state.selected, state.offset), (0, 0));
}

#[test]
fn pages_clamp_and_home_end_reach_the_ends() {
    let rs = rows(20);
    let mut s = ListState::default();
    assert_eq!(
        s.handle_key(key(KeyCode::PageDown), &rs, viewport(5)),
        ListOutcome::Moved
    );
    assert_eq!(s.selected, 5, "a page is the rows in view");
    s.handle_key(key(KeyCode::PageDown), &rs, viewport(5));
    assert_eq!(s.selected, 10);
    s.handle_key(key(KeyCode::End), &rs, viewport(5));
    assert_eq!((s.selected, s.offset), (19, 15));
    s.handle_key(key(KeyCode::PageDown), &rs, viewport(5));
    assert_eq!(s.selected, 19, "pages clamp; they do not wrap");
    s.handle_key(key(KeyCode::PageUp), &rs, viewport(5));
    assert_eq!(s.selected, 14);
    s.handle_key(key(KeyCode::Home), &rs, viewport(5));
    assert_eq!((s.selected, s.offset), (0, 0));
    s.handle_key(key(KeyCode::PageUp), &rs, viewport(5));
    assert_eq!(s.selected, 0);
    // Arrows wrap.
    s.handle_key(key(KeyCode::Up), &rs, viewport(5));
    assert_eq!((s.selected, s.offset), (19, 15));
    s.handle_key(key(KeyCode::Down), &rs, viewport(5));
    assert_eq!((s.selected, s.offset), (0, 0));
}

#[test]
fn navigation_skips_disabled_rows() {
    let mut rs = rows(5);
    for i in [0, 2, 4] {
        rs[i].disabled = true;
    }
    let mut s = ListState::default();
    s.settle(5, |i| !rs[i].disabled);
    assert_eq!(s.selected, 1, "settles on the first enabled row");
    s.handle_key(key(KeyCode::Down), &rs, viewport(5));
    assert_eq!(s.selected, 3);
    s.handle_key(key(KeyCode::Down), &rs, viewport(5));
    assert_eq!(s.selected, 1, "wraps past the disabled ends");
    s.handle_key(key(KeyCode::Up), &rs, viewport(5));
    assert_eq!(s.selected, 3);
    s.handle_key(key(KeyCode::Home), &rs, viewport(5));
    assert_eq!(s.selected, 1);
    s.handle_key(key(KeyCode::End), &rs, viewport(5));
    assert_eq!(s.selected, 3);
    // Enter and Space on a disabled row do nothing.
    s.selected = 2;
    assert_eq!(
        s.handle_key(key(KeyCode::Enter), &rs, viewport(5)),
        ListOutcome::Ignored
    );
    assert_eq!(
        s.handle_key(key(KeyCode::Char(' ')), &rs, viewport(5)),
        ListOutcome::Ignored
    );
    s.selected = 3;
    assert_eq!(
        s.handle_key(key(KeyCode::Enter), &rs, viewport(5)),
        ListOutcome::Chose(3)
    );
    assert_eq!(
        s.handle_key(key(KeyCode::Char(' ')), &rs, viewport(5)),
        ListOutcome::Toggled(3)
    );

    // Nothing enabled: nothing to move to, nothing to choose.
    let all: Vec<Row> = (0..3)
        .map(|_| Row {
            name: "x",
            lines: 1,
            disabled: true,
        })
        .collect();
    let mut s = ListState::new(1);
    s.handle_key(key(KeyCode::Down), &all, viewport(3));
    assert_eq!(s.selected, 1);
    assert_eq!(
        s.handle_key(key(KeyCode::Enter), &all, viewport(3)),
        ListOutcome::Ignored
    );
}

#[test]
fn zero_and_one_row_lists_are_safe() {
    let none: Vec<Row> = Vec::new();
    let mut s = ListState::default();
    for code in [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageDown,
        KeyCode::Enter,
    ] {
        assert_eq!(
            s.handle_key(key(code), &none, viewport(4)),
            ListOutcome::Ignored
        );
    }
    assert_eq!(
        s.handle_key(key(KeyCode::Esc), &none, viewport(4)),
        ListOutcome::Cancelled
    );
    assert_eq!(s, ListState::default());

    let one = rows(1);
    for code in [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
    ] {
        assert_eq!(
            s.handle_key(key(code), &one, viewport(4)),
            ListOutcome::Moved
        );
        assert_eq!((s.selected, s.offset), (0, 0));
    }
    assert_eq!(
        s.handle_key(key(KeyCode::Enter), &one, viewport(4)),
        ListOutcome::Chose(0)
    );
    assert_eq!(
        s.handle_key(key(KeyCode::Char('q')), &one, viewport(4)),
        ListOutcome::Ignored
    );
    let mut release = key(KeyCode::Down);
    release.kind = KeyEventKind::Release;
    assert_eq!(
        s.handle_key(release, &rs_of(3), viewport(4)),
        ListOutcome::Ignored
    );
    // Painting either into any size is fine.
    for profile in Profile::ALL {
        let theme = profile.theme();
        for (w, h) in [(0, 0), (1, 1), (2, 3), (5, 1), (40, 4)] {
            let _ = testing::render(w, h, |a, b| {
                List::new(&none, ListState::default()).paint(a, b, &theme);
                List::new(&one, ListState::default()).paint(a, b, &theme);
            });
        }
    }
}

fn rs_of(n: usize) -> Vec<Row> {
    rows(n)
}

#[test]
fn the_selected_row_has_a_mark_in_every_profile_and_a_ground_only_where_grounds_paint() {
    let rs = rows(3);
    for profile in Profile::ALL {
        let theme = profile.theme();
        let buf = testing::render(20, 3, |a, b| {
            List::new(&rs, ListState::new(1)).paint(a, b, &theme)
        });
        let shown = testing::text(&buf);
        let marker = if theme.ascii() { ">" } else { "▸" };
        let line = shown.lines().nth(1).unwrap();
        assert!(
            line.starts_with(&format!("{marker} row 1")),
            "{}: {shown}",
            profile.name()
        );
        for other in [0, 2] {
            assert!(
                shown.lines().nth(other).unwrap().starts_with("  row"),
                "{}",
                profile.name()
            );
        }
        // Bold label: a cue that survives NO_COLOR.
        assert!(
            buf[(3, 1)].modifier.contains(Modifier::BOLD),
            "{}",
            profile.name()
        );
        assert!(!buf[(3, 0)].modifier.contains(Modifier::BOLD));
        let grounded = buf[(10, 1)].bg != ratatui::style::Color::Reset;
        assert_eq!(grounded, theme.paints_grounds(), "{}", profile.name());
        assert_eq!(buf[(10, 0)].bg, ratatui::style::Color::Reset);
    }
}

#[test]
fn the_scrollbar_shows_only_when_rows_are_hidden() {
    let theme = Profile::DarkTrue.theme();
    let fits = rows(4);
    let hidden = rows(12);
    let right_column = |rs: &[Row], height: u16| {
        let state = {
            let mut s = ListState::new(rs.len() - 1);
            s.scroll_into_view(rs.len(), height, |_| 1);
            s
        };
        let buf = testing::render(20, height, |a, b| List::new(rs, state).paint(a, b, &theme));
        (0..height)
            .map(|y| buf[(19, y)].symbol().to_owned())
            .collect::<String>()
    };
    assert_eq!(right_column(&fits, 5).trim(), "");
    let rail = right_column(&hidden, 5);
    assert!(rail.contains('█') && rail.contains('│'), "{rail}");
    let ascii = Profile::Ascii.theme();
    let buf = testing::render(20, 5, |a, b| {
        List::new(&hidden, ListState::new(11)).paint(a, b, &ascii)
    });
    assert!(testing::text(&buf).contains('#'));
    // The rail never covers a row's text: rows are two cells narrower.
    let wide = [
        Row {
            name: "0123456789012345678901234567",
            lines: 1,
            disabled: false,
        },
        Row {
            name: "b",
            lines: 1,
            disabled: false,
        },
    ];
    let buf = testing::render(12, 1, |a, b| {
        List::new(&wide, ListState::default()).paint(a, b, &theme)
    });
    assert_eq!(
        buf[(11, 0)].symbol(),
        "█",
        "the whole bar is one cell, two rows in one line"
    );
    assert_eq!(buf[(10, 0)].symbol(), " ", "with a cell of air");
}

#[test]
fn tall_rows_scroll_whole_and_click_to_their_row() {
    let mut rs = rows(5);
    for r in &mut rs {
        r.lines = 2;
    }
    let theme = Profile::DarkTrue.theme();
    // Five lines hold two whole rows and a line of a third: scrolling to the
    // last row shows the last two whole, never half of one above them.
    let mut s = ListState::new(4);
    s.scroll_into_view(5, 5, |i| rs[i].lines);
    assert_eq!(s.offset, 3);
    let area = Rect::new(0, 0, 30, 5);
    let buf = testing::render(30, 5, |a, b| List::new(&rs, s).paint(a, b, &theme));
    let shown = testing::text(&buf);
    assert!(
        shown.contains("row 3") && shown.contains("row 4") && shown.contains("and more"),
        "{shown}"
    );
    assert!(!shown.contains("row 2"), "{shown}");

    // From the top, the third row is clipped to one line and still clickable.
    let top = ListState::new(1);
    let list = List::new(&rs, top);
    assert_eq!(list.row_at(area, 3, 0), Some(0));
    assert_eq!(
        list.row_at(area, 3, 1),
        Some(0),
        "the second line is the same row"
    );
    assert_eq!(list.row_at(area, 3, 2), Some(1));
    assert_eq!(
        list.row_at(area, 3, 4),
        Some(2),
        "a clipped row still takes clicks"
    );
    assert_eq!(list.row_at(area, 40, 0), None);
    assert_eq!(list.height(30, &theme), 10);
    let shown = text_of(30, 5, Profile::NoColor, |a, b, t| list.paint(a, b, t));
    assert!(
        shown.contains("row 2") && !shown.contains("row 3"),
        "{shown}"
    );
}

/// A custom row may report any height. One of `u16::MAX` lines after a
/// one-line row used to overflow the running `y` (a debug panic, a wrapped
/// position in release); the row is clipped to what the viewport has left.
#[test]
fn a_row_reporting_u16_max_lines_is_clipped_not_overflowed() {
    let mut rs = rows(3);
    rs[1].lines = u16::MAX;
    let theme = Profile::DarkTrue.theme();
    for height in [1u16, 2, 5, 12] {
        let area = Rect::new(0, 0, 30, height);
        for selected in 0..3 {
            let state = ListState::new(selected);
            let list = List::new(&rs, state);
            let shown = text_of(30, height, Profile::NoColor, |a, b, t| list.paint(a, b, t));
            assert!(
                shown.contains("row"),
                "selected {selected}, height {height}: {shown}"
            );
            // The selected row is on screen, wherever the huge row sits.
            assert!(
                (0..height).any(|line| list.row_at(area, 3, line) == Some(selected)),
                "selected {selected}, height {height}: {shown}"
            );
            assert_eq!(list.row_at(area, 3, height), None);
        }
    }
    // Starting at the top, the one-line row is followed by the huge one, which
    // fills the rest of the viewport.
    let list = List::new(&rs, ListState::new(0));
    let area = Rect::new(0, 0, 30, 5);
    assert_eq!(list.row_at(area, 3, 0), Some(0));
    assert_eq!(list.row_at(area, 3, 1), Some(1));
    assert_eq!(list.row_at(area, 3, 4), Some(1));
    let buf = testing::render(30, 5, |a, b| list.paint(a, b, &theme));
    let shown = testing::text(&buf);
    assert!(
        shown.contains("row 0") && shown.contains("row 1") && !shown.contains("row 2"),
        "{shown}"
    );
    assert_eq!(list.height(30, &theme), u16::MAX, "the total saturates");
}

#[test]
fn an_empty_list_paints_its_empty_state() {
    let none: Vec<Row> = Vec::new();
    let shown = text_of(40, 5, Profile::NoColor, |a, b, t| {
        List::new(&none, ListState::default())
            .empty(EmptyState::new("No saved sessions").action("Start one with a message"))
            .paint(a, b, t);
    });
    assert!(
        shown.contains("No saved sessions") && shown.contains("Start one"),
        "{shown}"
    );
    let bare = text_of(40, 5, Profile::NoColor, |a, b, t| {
        List::new(&none, ListState::default()).paint(a, b, t);
    });
    assert_eq!(bare.trim(), "");
}

#[test]
fn plain_string_rows_are_lists_too() {
    let words = ["Theme", "Motion", "鲸鱼工作区的一个非常长的名字"];
    let shown = text_of(24, 3, Profile::Ascii, |a, b, t| {
        List::new(&words, ListState::new(2)).paint(a, b, t);
    });
    assert!(
        shown.lines().nth(2).unwrap().starts_with("> 鲸鱼"),
        "{shown}"
    );
    assert!(shown.lines().nth(2).unwrap().contains("..."), "{shown}");
}

// ---------------------------------------------------------------- picker

fn themes() -> Vec<PickerItem> {
    vec![
        PickerItem::new("Shoreline").detail("Graphite ground, follows the terminal"),
        PickerItem::new("Shoreline light").detail("Paper ground"),
        PickerItem::new("Deep sea").detail("Navy ground, blue ombre"),
        PickerItem::new("Graphite").detail("The exact token grounds"),
        PickerItem::new("Reef").detail("Warm dark ground"),
        PickerItem::new("Lighthouse")
            .detail("High contrast light")
            .disabled("Needs a light terminal"),
    ]
}

fn tab_labels() -> Vec<Cow<'static, str>> {
    vec!["All".into(), "Dark".into(), "Light".into()]
}

/// Which tab each of `themes()` is in.
fn tab_of() -> Vec<Option<usize>> {
    vec![Some(1), Some(2), Some(1), Some(1), Some(1), Some(2)]
}

#[test]
fn filtering_ranks_and_keeps_original_indices() {
    let items = themes();
    let m = PickerMatches::rank(&items, "shore", None);
    let order: Vec<usize> = m.iter().map(|m| m.index).collect();
    assert_eq!(
        order,
        [0, 1],
        "Shoreline before Shoreline light; others dropped"
    );
    assert_eq!(m.get(0).unwrap().positions, [0, 1, 2, 3, 4]);
    assert_eq!(m.len(), 2);
    // An empty query keeps everything in order, with nothing to highlight.
    let all = PickerMatches::rank(&items, "", None);
    assert_eq!(all.len(), 6);
    assert!(
        all.iter()
            .enumerate()
            .all(|(i, m)| m.index == i && m.positions.is_empty())
    );
    // No match.
    assert!(PickerMatches::rank(&items, "zebra", None).is_empty());
}

#[test]
fn a_detail_match_ranks_below_every_label_match_and_highlights_nothing() {
    let items = [
        PickerItem::new("Alpha").detail("mentions beta here"),
        PickerItem::new("Beta"),
        PickerItem::new("Betamax"),
    ];
    let m = PickerMatches::rank(&items, "beta", None);
    let order: Vec<usize> = m.iter().map(|m| m.index).collect();
    assert_eq!(order, [1, 2, 0]);
    assert!(m.get(2).unwrap().positions.is_empty());
}

#[test]
fn tabs_filter_by_category_and_all_shows_everything() {
    let items = themes();
    let labels = tab_labels();
    let of = tab_of();
    let dark = PickerTabs::new(&labels, 1).assign(&of).all(0);
    let order = |m: &PickerMatches| m.iter().map(|m| m.index).collect::<Vec<_>>();
    assert_eq!(
        order(&PickerMatches::rank(&items, "", Some(&dark))),
        [0, 2, 3, 4]
    );
    let light = PickerTabs::new(&labels, 2).assign(&of).all(0);
    assert_eq!(
        order(&PickerMatches::rank(&items, "", Some(&light))),
        [1, 5]
    );
    let all = PickerTabs::new(&labels, 0).assign(&of).all(0);
    assert_eq!(order(&PickerMatches::rank(&items, "", Some(&all))).len(), 6);
    // The query and the tab combine.
    assert_eq!(
        order(&PickerMatches::rank(&items, "shore", Some(&light))),
        [1]
    );
    // An item with no tab is in every tab.
    let some = [Some(1), None, Some(2), None, None, None];
    let dark = PickerTabs::new(&labels, 1).assign(&some);
    assert_eq!(
        order(&PickerMatches::rank(&items, "", Some(&dark))),
        [0, 1, 3, 4, 5]
    );
}

fn query_picker<'a>(
    items: &'a [PickerItem],
    matches: &'a PickerMatches,
    query: &'a str,
    state: PickerState,
) -> Picker<'a> {
    Picker::new(items, state)
        .query(query, query.chars().count())
        .matches(matches)
}

#[test]
fn matched_characters_are_underlined_so_the_match_shows_without_color() {
    let items = themes();
    let matches = PickerMatches::rank(&items, "sho", None);
    for profile in [
        Profile::NoColor,
        Profile::Ascii,
        Profile::Ansi16,
        Profile::DarkTrue,
    ] {
        let theme = profile.theme();
        let picker = query_picker(&items, &matches, "sho", PickerState::new(0));
        let buf = testing::render(60, 5, |a, b| picker.paint(a, b, &theme));
        let shown = testing::text(&buf);
        let row = shown.lines().nth(1).unwrap();
        let at = column_of(row, "Shoreline");
        for dx in 0..9 {
            let cell = &buf[(at + dx, 1)];
            let under = cell.modifier.contains(Modifier::UNDERLINED);
            assert_eq!(under, dx < 3, "{} column {dx}: {row}", profile.name());
        }
        // The other row's "sho" is underlined too, and nothing else on it.
        let second = shown.lines().nth(2).unwrap();
        let at = column_of(second, "Shoreline light");
        assert!(buf[(at, 2)].modifier.contains(Modifier::UNDERLINED));
        assert!(!buf[(at + 3, 2)].modifier.contains(Modifier::UNDERLINED));
    }
}

#[test]
fn the_query_line_shows_the_query_a_placeholder_and_the_count() {
    let items = themes();
    let matches = PickerMatches::rank(&items, "shore", None);
    let picker = query_picker(&items, &matches, "shore", PickerState::new(0));
    let shown = text_of(50, 4, Profile::NoColor, |a, b, t| picker.paint(a, b, t));
    let line = shown.lines().next().unwrap();
    assert!(line.starts_with("› shore"), "{shown}");
    assert!(line.ends_with("2/6"), "{shown}");
    let empty = Picker::new(&items, PickerState::new(0)).query("", 0);
    let shown = text_of(50, 4, Profile::NoColor, |a, b, t| empty.paint(a, b, t));
    assert!(
        shown
            .lines()
            .next()
            .unwrap()
            .starts_with("› Type to filter"),
        "{shown}"
    );
    // ASCII swaps the prompt.
    let shown = text_of(50, 4, Profile::Ascii, |a, b, t| empty.paint(a, b, t));
    assert!(
        shown
            .lines()
            .next()
            .unwrap()
            .starts_with("/ Type to filter"),
        "{shown}"
    );
    // A long query keeps its end, and says it was cut.
    let long = "the quick brown fox jumps over the lazy dog";
    let none = PickerMatches::default();
    let picker = Picker::new(&items, PickerState::new(0))
        .query(long, long.len())
        .matches(&none);
    let shown = text_of(24, 4, Profile::NoColor, |a, b, t| picker.paint(a, b, t));
    let line = shown.lines().next().unwrap();
    assert!(
        line.starts_with("› …") && line.ends_with("lazy dog"),
        "{shown}"
    );
    // The cursor goes where the caller says, in cells.
    let picker = Picker::new(&items, PickerState::new(0)).query("shore", 2);
    let at = picker.cursor(Rect::new(4, 1, 40, 6)).unwrap();
    assert_eq!((at.x, at.y), (4 + 2 + 2, 1));
    assert_eq!(
        Picker::new(&items, PickerState::new(0)).cursor(Rect::new(0, 0, 40, 6)),
        None
    );
    // The cursor cell is reversed, visible in every profile.
    let theme = Profile::NoColor.theme();
    let buf = testing::render(30, 3, |a, b| {
        Picker::new(&items, PickerState::new(0))
            .query("ab", 1)
            .paint(a, b, &theme)
    });
    assert!(buf[(3, 0)].modifier.contains(Modifier::REVERSED));
    assert!(!buf[(2, 0)].modifier.contains(Modifier::REVERSED));
}

#[test]
fn the_cursor_follows_a_long_query_cut_to_its_end() {
    let items = themes();
    let long = "the quick brown fox jumps over the lazy dog";
    let area = Rect::new(0, 0, 24, 4);
    let theme = Profile::NoColor.theme();
    let picker = Picker::new(&items, PickerState::new(0)).query(long, long.chars().count());
    let at = picker.cursor(area).unwrap();
    let buf = testing::render(24, 4, |a, b| picker.paint(a, b, &theme));
    assert!(
        buf[(at.x, at.y)].modifier.contains(Modifier::REVERSED),
        "the reversed cell is the cursor's cell"
    );
    // At the end of the text, one cell past its last character.
    assert_eq!(buf[(at.x - 1, at.y)].symbol(), "g");
}

#[test]
fn painting_into_an_area_larger_than_the_buffer_clips() {
    let items = themes();
    let none: Vec<Row> = Vec::new();
    for profile in Profile::ALL {
        let theme = profile.theme();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
        let big = Rect::new(2, 1, 60, 20);
        empty_state().paint(big, &mut buf, &theme);
        List::new(&rows(30), ListState::new(25)).paint(big, &mut buf, &theme);
        List::new(&none, ListState::default()).paint(big, &mut buf, &theme);
        Picker::new(&items, PickerState::new(2))
            .query("x", 1)
            .paint(big, &mut buf, &theme);
    }
}

#[test]
fn no_matches_is_an_empty_state_not_a_count() {
    let items = themes();
    let matches = PickerMatches::rank(&items, "zebra", None);
    let picker = query_picker(&items, &matches, "zebra", PickerState::new(0));
    let shown = text_of(60, 8, Profile::DarkTrue, |a, b, t| picker.paint(a, b, t));
    assert!(shown.contains("No matches for “zebra”"), "{shown}");
    assert!(shown.contains("Edit the search"), "{shown}");
    assert!(!shown.contains("0/6"), "no count: {shown}");
    let ascii = text_of(60, 8, Profile::Ascii, |a, b, t| picker.paint(a, b, t));
    assert!(ascii.contains("No matches for \"zebra\""), "{ascii}");
    assert!(ascii.is_ascii());
    // The words are parameters.
    let words = PickerWords {
        placeholder: "Escribe para filtrar".into(),
        no_matches: "Sin resultados para".into(),
        no_matches_action: "Cambia la búsqueda".into(),
        empty: "Nada que elegir".into(),
    };
    let shown = text_of(60, 8, Profile::NoColor, |a, b, t| {
        picker.words(&words).paint(a, b, t)
    });
    assert!(
        shown.contains("Sin resultados para “zebra”") && shown.contains("Cambia la búsqueda"),
        "{shown}"
    );
    let nothing: [PickerItem; 0] = [];
    let shown = text_of(60, 8, Profile::NoColor, |a, b, t| {
        Picker::new(&nothing, PickerState::default())
            .words(&words)
            .paint(a, b, t);
    });
    assert!(shown.contains("Nada que elegir"), "{shown}");
    let shown = text_of(60, 8, Profile::NoColor, |a, b, t| {
        Picker::new(&nothing, PickerState::default()).paint(a, b, t);
    });
    assert!(shown.contains("Nothing to choose from"), "{shown}");
}

#[test]
fn the_preview_sits_beside_the_list_and_collapses_when_narrow() {
    let items = themes();
    let matches = PickerMatches::rank(&items, "", None);
    let seen = RefCell::new(Vec::new());
    let preview = |area: Rect, buf: &mut Buffer, _: &Theme, item: &PickerItem, index: usize| {
        seen.borrow_mut().push((area, index));
        buf.set_stringn(
            area.x,
            area.y,
            format!("PREVIEW {}", item.label),
            usize::from(area.width),
            ratatui::style::Style::default(),
        );
    };
    let picker = Picker::new(&items, PickerState::new(2))
        .matches(&matches)
        .preview(&preview);

    let theme = Profile::NoColor.theme();
    let wide = testing::text(&testing::render(80, 6, |a, b| picker.paint(a, b, &theme)));
    assert!(
        wide.contains("PREVIEW Deep sea"),
        "the selected item: {wide}"
    );
    assert!(
        wide.contains('│'),
        "a rule between the list and the pane: {wide}"
    );
    assert_eq!(
        seen.borrow().last().map(|s| s.1),
        Some(2),
        "the item's index"
    );
    let layout = picker.layout(Rect::new(0, 0, 80, 6));
    let (list, pane) = (layout.list, layout.preview.expect("room for the pane"));
    assert!(list.right() < pane.x && pane.right() <= 80 && list.width >= 20 && pane.width >= 20);
    assert_eq!(layout.separator.unwrap().x, list.right() + 1);

    // Below the threshold there is no pane and the list takes the width.
    seen.borrow_mut().clear();
    let narrow = picker.layout(Rect::new(0, 0, 40, 6));
    assert!(narrow.preview.is_none() && narrow.separator.is_none());
    assert_eq!(narrow.list.width, 40);
    let text = testing::text(&testing::render(40, 6, |a, b| picker.paint(a, b, &theme)));
    assert!(
        !text.contains("PREVIEW") && seen.borrow().is_empty(),
        "{text}"
    );
    // The threshold is the picker's to move.
    assert!(
        picker
            .preview_min_width(30)
            .layout(Rect::new(0, 0, 50, 6))
            .preview
            .is_some()
    );
    assert!(picker.layout(Rect::new(0, 0, 55, 6)).preview.is_none());
    assert!(picker.layout(Rect::new(0, 0, 56, 6)).preview.is_some());
    // With nothing shown there is nothing to preview.
    let none = PickerMatches::default();
    assert!(
        picker
            .matches(&none)
            .layout(Rect::new(0, 0, 80, 6))
            .preview
            .is_none()
    );
}

#[test]
fn tabs_are_painted_with_a_mark_and_scroll_to_keep_the_active_one() {
    let items = themes();
    let labels = tab_labels();
    let of = tab_of();
    let tabs = PickerTabs::new(&labels, 1).assign(&of).all(0);
    let matches = PickerMatches::rank(&items, "", Some(&tabs));
    let picker = Picker::new(&items, PickerState::new(0))
        .tabs(tabs)
        .matches(&matches);
    let shown = text_of(50, 5, Profile::NoColor, |a, b, t| picker.paint(a, b, t));
    assert_eq!(
        shown.lines().next().unwrap(),
        "All  ● Dark  Light",
        "{shown}"
    );
    let shown = text_of(50, 5, Profile::Ascii, |a, b, t| picker.paint(a, b, t));
    assert_eq!(
        shown.lines().next().unwrap(),
        "All  . Dark  Light",
        "{shown}"
    );
    // Too many tabs for the width: the active one stays, the rest say more.
    let many: Vec<Cow<'static, str>> = (0..12).map(|i| format!("Category {i}").into()).collect();
    let tabs = PickerTabs::new(&many, 6);
    let picker = Picker::new(&items, PickerState::new(0)).tabs(tabs);
    let shown = text_of(40, 5, Profile::NoColor, |a, b, t| picker.paint(a, b, t));
    let line = shown.lines().next().unwrap();
    assert!(
        line.contains("● Category 6") && line.starts_with('…') && line.ends_with('…'),
        "{line}"
    );
    assert!(line.chars().count() <= 40);
}

#[test]
fn small_areas_give_the_list_a_row_before_the_tabs_and_the_query() {
    let items = themes();
    let labels = tab_labels();
    let picker = Picker::new(&items, PickerState::new(0))
        .query("x", 1)
        .tabs(PickerTabs::new(&labels, 0));
    let l = picker.layout(Rect::new(0, 0, 40, 5));
    assert_eq!(
        (
            l.tabs.map(|r| r.y),
            l.query.map(|r| r.y),
            l.list.y,
            l.list.height
        ),
        (Some(0), Some(1), 2, 3)
    );
    let l = picker.layout(Rect::new(0, 0, 40, 2));
    assert_eq!(
        (l.tabs, l.query.map(|r| r.y), l.list.y, l.list.height),
        (None, Some(0), 1, 1)
    );
    let l = picker.layout(Rect::new(0, 0, 40, 1));
    assert_eq!((l.tabs, l.query, l.list.height), (None, None, 1));
    let l = picker.layout(Rect::new(0, 0, 40, 0));
    assert!(l.list.is_empty());
    for profile in Profile::ALL {
        let theme = profile.theme();
        for (w, h) in [(0, 0), (1, 1), (3, 2), (20, 2), (40, 1), (80, 9)] {
            let _ = testing::render(w, h, |a, b| picker.paint(a, b, &theme));
        }
    }
}

#[test]
fn with_a_query_line_letters_and_editing_keys_belong_to_the_query() {
    let items = themes();
    let matches = PickerMatches::rank(&items, "", None);
    let picker = query_picker(&items, &matches, "", PickerState::default());
    let mut s = PickerState::default();
    let mut press = |k: KeyEvent| s.handle_picker_key(k, &picker, 4);
    for code in [
        KeyCode::Char('a'),
        KeyCode::Char('j'),
        KeyCode::Char('k'),
        KeyCode::Char(' '),
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::F(2),
    ] {
        assert_eq!(press(key(code)), PickerOutcome::Query, "{code:?}");
    }
    assert_eq!(
        press(ctrl('a')),
        PickerOutcome::Query,
        "editing chords go to the editor"
    );
    assert_eq!(press(key(KeyCode::Down)), PickerOutcome::Moved);
    assert_eq!(press(key(KeyCode::Up)), PickerOutcome::Moved);
    assert_eq!(press(key(KeyCode::PageDown)), PickerOutcome::Moved);
    assert_eq!(press(key(KeyCode::Enter)), PickerOutcome::Chose(4));
    assert_eq!(
        press(ctrl(' ')),
        PickerOutcome::Toggled(4),
        "Space is text, Ctrl+Space toggles"
    );
    assert_eq!(press(key(KeyCode::Esc)), PickerOutcome::Cancelled);
    let mut release = key(KeyCode::Char('a'));
    release.kind = KeyEventKind::Release;
    assert_eq!(press(release), PickerOutcome::Ignored);
    // Tab with no tabs is the host's.
    assert_eq!(press(key(KeyCode::Tab)), PickerOutcome::Ignored);
}

#[test]
fn without_a_query_line_the_picker_keeps_the_letters_out_and_home_end_move() {
    let items = themes();
    let picker = Picker::new(&items, PickerState::default());
    let mut s = PickerState::default();
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Char('j')), &picker, 4),
        PickerOutcome::Ignored
    );
    assert_eq!(
        s.handle_picker_key(key(KeyCode::End), &picker, 4),
        PickerOutcome::Moved
    );
    assert_eq!(
        s.selected, 4,
        "the last enabled row (Lighthouse is disabled)"
    );
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Home), &picker, 4),
        PickerOutcome::Moved
    );
    assert_eq!(s.selected, 0);
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Char(' ')), &picker, 4),
        PickerOutcome::Toggled(0)
    );
}

#[test]
fn picker_navigation_skips_disabled_rows_and_refuses_to_choose_them() {
    let items = [
        PickerItem::new("a").disabled("no"),
        PickerItem::new("b"),
        PickerItem::new("c").disabled("no"),
        PickerItem::new("d"),
    ];
    let picker = Picker::new(&items, PickerState::default());
    let mut s = PickerState::new(1);
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Down), &picker, 4),
        PickerOutcome::Moved
    );
    assert_eq!(s.selected, 3);
    s.handle_picker_key(key(KeyCode::Down), &picker, 4);
    assert_eq!(s.selected, 1, "wraps past the disabled first row");
    s.handle_picker_key(key(KeyCode::Up), &picker, 4);
    assert_eq!(s.selected, 3);
    s.selected = 2;
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Enter), &picker, 4),
        PickerOutcome::Ignored
    );
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Char(' ')), &picker, 4),
        PickerOutcome::Ignored
    );
    // The old key handler keeps its behavior for hosts that use it.
    let mut old = PickerState::new(2);
    assert_eq!(
        old.handle_key(key(KeyCode::Enter), 4, 4),
        PickerOutcome::Chose(2)
    );
}

#[test]
fn tabs_change_with_tab_keys_wrap_and_reset_the_selection() {
    let items = themes();
    let labels = tab_labels();
    let of = tab_of();
    let at = |active: usize| PickerTabs::new(&labels, active).assign(&of).all(0);
    let m = PickerMatches::rank(&items, "", Some(&at(1)));
    let picker = Picker::new(&items, PickerState::default())
        .tabs(at(1))
        .matches(&m);
    let mut s = PickerState::new(2);
    s.offset = 1;
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Tab), &picker, 4),
        PickerOutcome::Tab(2)
    );
    assert_eq!(
        s,
        PickerState::default(),
        "a new tab starts at its first row"
    );
    let last = Picker::new(&items, PickerState::default()).tabs(at(2));
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Tab), &last, 4),
        PickerOutcome::Tab(0),
        "wraps"
    );
    assert_eq!(
        s.handle_picker_key(key(KeyCode::BackTab), &picker, 4),
        PickerOutcome::Tab(0)
    );
    let first = Picker::new(&items, PickerState::default()).tabs(at(0));
    assert_eq!(
        s.handle_picker_key(key(KeyCode::BackTab), &first, 4),
        PickerOutcome::Tab(2),
        "wraps back"
    );
    let shift_tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT);
    assert_eq!(
        s.handle_picker_key(shift_tab, &picker, 4),
        PickerOutcome::Tab(0)
    );
    // Without a query line the arrows change tabs; with one they belong to the cursor.
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Right), &picker, 4),
        PickerOutcome::Tab(2)
    );
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Left), &picker, 4),
        PickerOutcome::Tab(0)
    );
    let typing = picker.query("", 0);
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Right), &typing, 4),
        PickerOutcome::Query
    );
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Tab), &typing, 4),
        PickerOutcome::Tab(2)
    );
    // One tab has nowhere to go.
    let one = [Cow::Borrowed("Only")];
    let single = Picker::new(&items, PickerState::default()).tabs(PickerTabs::new(&one, 0));
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Tab), &single, 4),
        PickerOutcome::Ignored
    );
}

#[test]
fn an_empty_picker_still_cancels_and_a_stale_selection_is_clamped() {
    let items = themes();
    let none = PickerMatches::default();
    let picker = query_picker(&items, &none, "zebra", PickerState::default());
    let mut s = PickerState::new(4);
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Down), &picker, 4),
        PickerOutcome::Ignored
    );
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Enter), &picker, 4),
        PickerOutcome::Ignored
    );
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Char('z')), &picker, 4),
        PickerOutcome::Query
    );
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Esc), &picker, 4),
        PickerOutcome::Cancelled
    );
    // The filter shrank under a selection past its end.
    let two = PickerMatches::rank(&items, "shore", None);
    let picker = query_picker(&items, &two, "shore", PickerState::default());
    let mut s = PickerState::new(5);
    assert_eq!(
        s.handle_picker_key(key(KeyCode::Enter), &picker, 4),
        PickerOutcome::Chose(1)
    );
    s.reset();
    assert_eq!(s, PickerState::default());
}

#[test]
fn shown_rows_map_back_to_items_and_clicks_land_on_them() {
    let items = themes();
    let m = PickerMatches::rank(&items, "light", None);
    let picker = query_picker(&items, &m, "light", PickerState::new(0));
    assert_eq!(picker.shown_len(), m.len());
    let label = |shown| &*items[picker.item_index(shown).unwrap()].label;
    assert_eq!(label(0), "Lighthouse", "a prefix first");
    assert_eq!(label(1), "Shoreline light");
    assert_eq!(picker.item_index(99), None);
    let area = Rect::new(0, 0, 40, 6);
    assert_eq!(
        picker.row_at(area, 5, 0),
        None,
        "the query line is not a row"
    );
    assert_eq!(picker.row_at(area, 5, 1), Some(0));
    let plain = Picker::new(&items, PickerState::new(0));
    assert_eq!(plain.item_index(3), Some(3));
    assert_eq!(plain.row_at(area, 5, 3), Some(3));
}

#[test]
fn text_a_caller_passes_cannot_reorder_itself_in_the_picker() {
    let items = [PickerItem::new("rm -rf ~/\u{202e}txt.exe").detail("a\u{1b}[31mb")];
    let m = PickerMatches::rank(&items, "rm", None);
    assert_eq!(m.len(), 1, "matched against the safe text");
    assert_eq!(m.get(0).unwrap().positions, [0, 1]);
    let picker = query_picker(&items, &m, "rm\u{202e}", PickerState::new(0));
    for profile in Profile::ALL {
        let shown = text_of(40, 3, profile, |a, b, t| picker.paint(a, b, t));
        assert!(
            !shown.contains('\u{202e}') && !shown.contains('\u{1b}'),
            "{shown}"
        );
        assert!(shown.contains("rm -rf ~/txt.exe"), "{shown}");
    }
}

#[test]
fn a_shrinking_picker_keeps_a_visible_valid_selection_and_the_scrollbar_is_not_a_row() {
    let items = [PickerItem::new("one"), PickerItem::new("two")];
    let mut state = PickerState::new(usize::MAX);
    assert_eq!(state.visible_offset(2, 4), 0);
    assert_eq!(
        state.handle_key(key(KeyCode::Enter), 2, 4),
        PickerOutcome::Chose(1)
    );
    let t = Profile::DarkTrue.theme();
    let b = testing::render(20, 4, |a, b| {
        Picker::new(&items, PickerState::new(9)).paint(a, b, &t)
    });
    assert!(testing::text(&b).contains("one") && testing::text(&b).contains("two"));
    let picker = Picker::new(&items, PickerState::new(0));
    assert_eq!(picker.row_at(Rect::new(0, 0, 20, 1), 19, 0), None);
}

// ---------------------------------------------------------------- frames and snapshots

fn list_rows() -> Vec<Row> {
    let mut rs = rows(5);
    rs[3].disabled = true;
    rs
}

fn paint_list(area: Rect, buf: &mut Buffer, theme: &Theme) {
    List::new(&list_rows(), ListState::new(1)).paint(area, buf, theme);
}

fn paint_list_scrolling(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rs = rows(14);
    let mut state = ListState::new(9);
    state.scroll_into_view(rs.len(), area.height, |_| 1);
    List::new(&rs, state).paint(area, buf, theme);
}

fn paint_list_tall(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let mut rs = rows(5);
    for r in &mut rs {
        r.lines = 2;
    }
    let mut state = ListState::new(3);
    state.scroll_into_view(rs.len(), area.height, |i| rs[i].lines);
    List::new(&rs, state).paint(area, buf, theme);
}

fn paint_picker_query(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = themes();
    let matches = PickerMatches::rank(&items, "shore", None);
    query_picker(&items, &matches, "shore", PickerState::new(0)).paint(area, buf, theme);
}

fn paint_picker_query_cjk(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = [
        PickerItem::new("鲸鱼工作区").detail("默认的工作区"),
        PickerItem::new("鲸鱼记录").detail("会话记录"),
        PickerItem::new("Reef").detail("Warm dark ground"),
    ];
    let matches = PickerMatches::rank(&items, "鱼录", None);
    query_picker(&items, &matches, "鱼录", PickerState::new(0)).paint(area, buf, theme);
}

fn paint_picker_preview(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = themes();
    let labels = tab_labels();
    let of = tab_of();
    let tabs = PickerTabs::new(&labels, 1).assign(&of).all(0);
    let matches = PickerMatches::rank(&items, "", Some(&tabs));
    let preview = |area: Rect, buf: &mut Buffer, theme: &Theme, item: &PickerItem, _: usize| {
        let ink = theme
            .fg(codewhale_ratatui::Role::Foreground)
            .add_modifier(Modifier::BOLD);
        buf.set_stringn(area.x, area.y, &item.label, usize::from(area.width), ink);
        let detail = item.detail.as_deref().unwrap_or_default();
        buf.set_stringn(
            area.x,
            area.y + 1,
            detail,
            usize::from(area.width),
            theme.fg(codewhale_ratatui::Role::Muted),
        );
    };
    Picker::new(&items, PickerState::new(1))
        .query("", 0)
        .tabs(tabs)
        .matches(&matches)
        .preview(&preview)
        .paint(area, buf, theme);
}

fn paint_picker_no_match(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = themes();
    let matches = PickerMatches::rank(&items, "zebra", None);
    query_picker(&items, &matches, "zebra", PickerState::new(0)).paint(area, buf, theme);
}

fn paint_picker_plain(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let items = themes();
    Picker::new(&items, PickerState::new(2)).paint(area, buf, theme);
}

fn paint_empty(area: Rect, buf: &mut Buffer, theme: &Theme) {
    empty_state().paint(area, buf, theme);
}

fn paint_empty_list(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let none: Vec<Row> = Vec::new();
    List::new(&none, ListState::default())
        .empty(EmptyState::new("No saved sessions").action("Start one with a message"))
        .paint(area, buf, theme);
}

#[test]
fn every_frame_keeps_the_rules() {
    testing::assert_rules(5, paint_list);
    testing::assert_rules(5, paint_list_scrolling);
    testing::assert_rules(6, paint_list_tall);
    testing::assert_rules(6, paint_picker_query);
    // Caller text in another script is not ASCII, so the ASCII profile's
    // own check does not apply to it.
    let profiles: Vec<Profile> = Profile::ALL
        .into_iter()
        .filter(|p| *p != Profile::Ascii)
        .collect();
    testing::assert_frames_keep_the_rules(&testing::frames_for(
        "picker-query-cjk",
        &profiles,
        &testing::WIDTHS,
        5,
        paint_picker_query_cjk,
    ));
    testing::assert_rules(8, paint_picker_preview);
    testing::assert_rules(8, paint_picker_no_match);
    testing::assert_rules(8, paint_picker_plain);
    testing::assert_rules(14, paint_empty);
    testing::assert_rules(5, paint_empty_list);
}

#[test]
fn list_snapshot() {
    insta::assert_snapshot!("list", testing::snapshot(5, paint_list));
}

#[test]
fn list_scrolling_snapshot() {
    insta::assert_snapshot!("list-scrolling", testing::snapshot(5, paint_list_scrolling));
}

#[test]
fn list_tall_rows_snapshot() {
    insta::assert_snapshot!("list-tall-rows", testing::snapshot(6, paint_list_tall));
}

#[test]
fn picker_with_a_query_snapshot() {
    insta::assert_snapshot!("picker-query", testing::snapshot(6, paint_picker_query));
}

#[test]
fn picker_with_a_cjk_query_snapshot() {
    insta::assert_snapshot!(
        "picker-query-cjk",
        testing::snapshot(5, paint_picker_query_cjk)
    );
}

#[test]
fn picker_with_tabs_and_a_preview_snapshot() {
    insta::assert_snapshot!("picker-preview", testing::snapshot(8, paint_picker_preview));
}

#[test]
fn picker_with_no_match_snapshot() {
    insta::assert_snapshot!(
        "picker-no-match",
        testing::snapshot(8, paint_picker_no_match)
    );
}

#[test]
fn empty_state_snapshot() {
    insta::assert_snapshot!("empty-state", testing::snapshot(14, paint_empty));
}

#[test]
fn empty_list_snapshot() {
    insta::assert_snapshot!("empty-list", testing::snapshot(5, paint_empty_list));
}
