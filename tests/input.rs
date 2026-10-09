//! Package Input: `LineBuffer`, `TextInput` and `Form`.
//!
//! Unit tests for every `LineBuffer` operation (CJK, combining marks, emoji
//! ZWJ sequences, paste sanitising, word boundaries, scroll-window
//! invariants), key-in/outcome-out tests for the two states, and the frame
//! rules plus a snapshot for each visual state at 40, 80 and 120 columns.

use std::borrow::Cow;

use codewhale_ratatui::{
    Form, FormField, FormFieldKind, FormOutcome, FormState, FormStatus, FormWords, LineBuffer,
    Paint, TextInput, TextInputOutcome, TextInputState, TextInputWords, Theme, gallery,
    line_window_of, testing, testing::Profile,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn alt(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::ALT)
}

fn press(c: char) -> KeyEvent {
    key(KeyCode::Char(c))
}

fn type_str(state: &mut TextInputState, text: &str) {
    for c in text.chars() {
        state.handle_key(press(c));
    }
}

fn form_type(form: &mut FormState, text: &str) {
    for c in text.chars() {
        form.handle_key(press(c));
    }
}

/// A buffer with `text` and the cursor after `at` graphemes.
fn buffer(text: &str, at: usize) -> LineBuffer {
    let mut b = LineBuffer::from_text(text);
    b.set_cursor(at);
    b
}

fn tuple(b: &LineBuffer) -> (String, usize) {
    (b.text().to_owned(), b.cursor())
}

// ---------------------------------------------------------------------------
// LineBuffer: inserting
// ---------------------------------------------------------------------------

#[test]
fn insert_puts_text_at_the_cursor_and_moves_past_it() {
    let mut b = LineBuffer::new();
    assert!(b.is_empty());
    assert!(b.insert_char('a'));
    assert!(b.insert_str("cd"));
    b.set_cursor(1);
    assert!(b.insert_char('b'));
    assert_eq!(tuple(&b), ("abcd".to_owned(), 2));
    assert_eq!(b.before_cursor(), "ab");
    assert_eq!(b.after_cursor(), "cd");
    assert_eq!(b.len(), 4);
    assert_eq!(b.cursor_byte(), 2);
}

#[test]
fn from_text_and_set_text_put_the_cursor_at_the_end() {
    let mut b = LineBuffer::from_text("鲸鱼");
    assert_eq!(b.cursor(), 2);
    b.set_text("x");
    assert_eq!(tuple(&b), ("x".to_owned(), 1));
    b.clear();
    assert_eq!(tuple(&b), (String::new(), 0));
}

#[test]
fn paste_keeps_one_line_and_drops_what_cannot_be_shown() {
    let mut b = LineBuffer::new();
    // Newlines are dropped, not turned into spaces.
    assert!(b.insert_str("line one\nline two\r\nthree"));
    assert_eq!(b.text(), "line oneline twothree");
    b.clear();
    // Tabs, escapes, DEL, a C1 control.
    assert!(b.insert_str("a\tb\u{1b}[31mc\u{7f}d\u{85}e"));
    assert_eq!(b.text(), "ab[31mcde");
    b.clear();
    // Bidi overrides (they can make text read as something else), line and
    // paragraph separators.
    assert!(b.insert_str("rm -rf ~/\u{202e}txt.exe\u{2028}\u{2029}\u{2066}x\u{2069}"));
    assert_eq!(b.text(), "rm -rf ~/txt.exex");
    b.clear();
    // Clusters that draw nothing: a zero-width space, a lone joiner, a BOM.
    assert!(b.insert_str("\u{200d}\u{200b}ad\u{200b}min\u{feff}"));
    assert_eq!(b.text(), "admin");
    // Nothing left to insert is not a change.
    assert!(!b.insert_str("\n\r\t\u{1b}"));
    assert!(!b.insert_str(""));
    assert_eq!(b.text(), "admin");
}

#[test]
fn paste_keeps_marks_and_sequences_that_belong_to_a_visible_character() {
    let mut b = LineBuffer::new();
    // A combining accent, a ZWJ sequence, a flag, a keycap.
    assert!(b.insert_str("cafe\u{301} 👩\u{200d}💻 🇺🇸 1\u{fe0f}\u{20e3}"));
    assert_eq!(b.text(), "cafe\u{301} 👩\u{200d}💻 🇺🇸 1\u{fe0f}\u{20e3}");
    assert_eq!(b.len(), 4 + 1 + 1 + 1 + 1 + 1 + 1);
}

#[test]
fn sanitize_is_what_insert_uses() {
    assert_eq!(LineBuffer::sanitize("a\nb\u{202e}c"), "abc");
    assert_eq!(LineBuffer::sanitize("鲸\u{200b}鱼"), "鲸鱼");
}

#[test]
fn the_limit_counts_graphemes_and_cuts_a_paste_at_it() {
    let mut b = LineBuffer::new().with_limit(5);
    assert!(b.insert_str("abcdefgh"));
    assert_eq!(b.text(), "abcde");
    assert!(!b.insert_char('x'), "full");
    assert!(!b.insert_str("yz"));
    // Deleting makes room again.
    assert!(b.backspace());
    assert!(b.insert_str("XYZ"));
    assert_eq!(b.text(), "abcdX");

    // A combining sequence is one toward the limit, not two.
    let mut b = LineBuffer::new().with_limit(3);
    assert!(b.insert_str("e\u{301}e\u{301}e\u{301}e\u{301}"));
    assert_eq!(b.len(), 3);
    assert_eq!(b.text(), "e\u{301}e\u{301}e\u{301}");

    // Lowering the limit cuts what is there.
    let b = LineBuffer::from_text("abcdef").with_limit(2);
    assert_eq!(tuple(&b), ("ab".to_owned(), 2));
    assert_eq!(LineBuffer::default().len(), 0);
}

#[test]
fn a_huge_paste_stops_at_the_default_limit() {
    let mut b = LineBuffer::new();
    b.insert_str(&"x".repeat(LineBuffer::DEFAULT_LIMIT + 1000));
    assert_eq!(b.len(), LineBuffer::DEFAULT_LIMIT);
}

#[test]
fn typed_text_that_joins_the_next_grapheme_leaves_the_cursor_between_graphemes() {
    // Regional indicators pair up: a flag in front of a flag re-pairs both.
    let mut b = LineBuffer::from_text("🇺🇸");
    b.set_cursor(0);
    assert!(b.insert_str("🇨"));
    let cursor = b.cursor_byte();
    let boundaries: Vec<usize> = b
        .text()
        .char_indices()
        .map(|(i, _)| i)
        .chain([b.text().len()])
        .collect();
    assert!(boundaries.contains(&cursor));
    // The cursor is on a grapheme boundary: moving left then right returns.
    let before = tuple(&b);
    b.move_left();
    b.move_right();
    assert_eq!(tuple(&b), before);
    assert_eq!(b.cursor(), 1, "after the pair 🇨🇺, not between its halves");
}

// ---------------------------------------------------------------------------
// LineBuffer: deleting
// ---------------------------------------------------------------------------

#[test]
fn backspace_and_delete_remove_one_grapheme_and_say_so() {
    let mut b = buffer("abc", 3);
    assert!(b.backspace());
    assert_eq!(tuple(&b), ("ab".to_owned(), 2));
    b.set_cursor(0);
    assert!(!b.backspace(), "nothing before the start");
    assert!(b.delete());
    assert_eq!(tuple(&b), ("b".to_owned(), 0));
    b.end();
    assert!(!b.delete(), "nothing after the end");
    assert!(b.backspace());
    assert!(!b.backspace(), "empty");
    assert!(!b.delete(), "empty");
}

#[test]
fn cjk_is_one_step_and_two_cells_each() {
    let mut b = buffer("鲸鱼日报", 4);
    assert_eq!(b.len(), 4);
    assert_eq!(b.width(), 8);
    assert_eq!(b.widths(false), [2, 2, 2, 2]);
    assert_eq!(b.widths(true), [1, 1, 1, 1]);
    assert!(b.move_left());
    assert_eq!(b.cursor(), 3);
    assert_eq!(b.cursor_byte(), 9, "three bytes per character");
    assert!(b.backspace());
    assert_eq!(tuple(&b), ("鲸鱼报".to_owned(), 2));
    assert!(b.delete());
    assert_eq!(tuple(&b), ("鲸鱼".to_owned(), 2));
}

#[test]
fn a_combining_sequence_moves_and_deletes_as_one() {
    let text = "cafe\u{301}!";
    let mut b = buffer(text, 5);
    assert_eq!(b.len(), 5, "c a f é !");
    assert_eq!(b.width(), 5);
    assert!(b.move_left());
    assert!(b.move_left());
    assert_eq!(b.cursor(), 3, "before the é");
    assert_eq!(b.before_cursor(), "caf");
    assert!(b.delete(), "takes the e and its accent together");
    assert_eq!(b.text(), "caf!");
    let mut b = buffer(text, 4);
    assert!(b.backspace());
    assert_eq!(b.text(), "caf!");
    assert_eq!(b.cursor(), 3);
}

#[test]
fn an_emoji_sequence_is_one_grapheme_two_cells_and_one_delete() {
    let family = "👨\u{200d}👩\u{200d}👧\u{200d}👦";
    let text = format!("a{family}🇺🇸b");
    let mut b = LineBuffer::from_text(&text);
    assert_eq!(b.len(), 4, "a, the family, the flag, b");
    assert_eq!(b.widths(false), [1, 2, 2, 1]);
    b.set_cursor(2);
    assert_eq!(b.before_cursor(), format!("a{family}"));
    assert!(b.backspace());
    assert_eq!(b.text(), "a🇺🇸b");
    assert!(b.delete());
    assert_eq!(tuple(&b), ("ab".to_owned(), 1));
}

// ---------------------------------------------------------------------------
// LineBuffer: words, lines and moves
// ---------------------------------------------------------------------------

#[test]
fn delete_word_back_takes_trailing_space_and_the_word_before_it() {
    let mut b = buffer("foo bar  baz", 12);
    assert!(b.delete_word_back());
    assert_eq!(tuple(&b), ("foo bar  ".to_owned(), 9));
    assert!(b.delete_word_back(), "spaces first, then the word");
    assert_eq!(tuple(&b), ("foo ".to_owned(), 4));
    assert!(b.delete_word_back());
    assert_eq!(tuple(&b), (String::new(), 0));
    assert!(!b.delete_word_back());
}

#[test]
fn a_word_is_a_run_of_non_whitespace() {
    // As the engine's composer has it: a path is one word.
    let mut b = buffer("cd /usr/local/bin", 17);
    assert!(b.delete_word_back());
    assert_eq!(b.text(), "cd ");
    // Unicode spaces separate too: U+3000 is a space.
    let mut b = buffer("鲸鱼\u{3000}日报", 5);
    assert!(b.delete_word_back());
    assert_eq!(b.text(), "鲸鱼\u{3000}");
    assert!(b.delete_word_back());
    assert_eq!(b.text(), "");
}

#[test]
fn delete_word_back_from_the_middle_of_a_word_keeps_the_rest() {
    let mut b = buffer("hello world", 8);
    assert!(b.delete_word_back());
    assert_eq!(tuple(&b), ("hello rld".to_owned(), 6));
}

#[test]
fn delete_word_forward_takes_leading_space_and_the_next_word() {
    let mut b = buffer("foo bar  baz", 0);
    assert!(b.delete_word_forward());
    assert_eq!(tuple(&b), (" bar  baz".to_owned(), 0));
    assert!(b.delete_word_forward());
    assert_eq!(tuple(&b), ("  baz".to_owned(), 0));
    assert!(b.delete_word_forward());
    assert_eq!(b.text(), "");
    assert!(!b.delete_word_forward());
}

#[test]
fn word_moves_land_on_word_starts() {
    let mut b = buffer("foo bar  baz", 12);
    assert!(b.word_left());
    assert_eq!(b.cursor(), 9);
    assert!(b.word_left());
    assert_eq!(b.cursor(), 4);
    assert!(b.word_left());
    assert_eq!(b.cursor(), 0);
    assert!(!b.word_left());
    assert!(b.word_right());
    assert_eq!(b.cursor(), 4);
    assert!(b.word_right());
    assert_eq!(b.cursor(), 9);
    assert!(b.word_right());
    assert_eq!(b.cursor(), 12, "the last word runs to the end of the line");
    assert!(!b.word_right());
}

#[test]
fn word_moves_step_over_wide_and_combining_text() {
    let mut b = buffer("鲸鱼 cafe\u{301} 👩\u{200d}💻", 0);
    assert!(b.word_right());
    assert_eq!(b.cursor(), 3);
    assert!(b.word_right());
    assert_eq!(b.cursor(), 8, "past café and its space, before the emoji");
    assert!(b.word_right());
    assert_eq!(b.cursor(), 9);
    assert!(b.word_left());
    assert_eq!(b.cursor(), 8);
    assert!(b.word_left());
    assert_eq!(b.cursor(), 3);
}

#[test]
fn kill_to_start_and_end_split_at_the_cursor() {
    let mut b = buffer("hello world", 5);
    assert!(b.kill_to_end());
    assert_eq!(tuple(&b), ("hello".to_owned(), 5));
    assert!(!b.kill_to_end());
    let mut b = buffer("hello world", 6);
    assert!(b.kill_to_start());
    assert_eq!(tuple(&b), ("world".to_owned(), 0));
    assert!(!b.kill_to_start());
}

/// Ctrl+U is "delete to the start of the line", not "clear": the suffix after
/// the cursor stays, and the cut falls between whole graphemes.
#[test]
fn ctrl_u_deletes_only_from_the_start_to_the_cursor() {
    // Combining mark, ZWJ family, regional-indicator flag, CJK, then ASCII.
    let text = "e\u{301}\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}\u{1f1fa}\u{1f1f8}\u{9c8d}tail";
    for (at, kept) in [
        (0, text),
        (
            1,
            "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}\u{1f1fa}\u{1f1f8}\u{9c8d}tail",
        ),
        (2, "\u{1f1fa}\u{1f1f8}\u{9c8d}tail"),
        (3, "\u{9c8d}tail"),
        (4, "tail"),
        (6, "il"),
    ] {
        let mut state = TextInputState::with_text(text);
        state.buffer_mut().set_cursor(at);
        let outcome = state.handle_key(ctrl('u'));
        assert_eq!(state.text(), kept, "cursor at grapheme {at}");
        assert_eq!(state.buffer().cursor(), 0, "cursor at grapheme {at}");
        assert_eq!(
            outcome,
            if at == 0 {
                TextInputOutcome::Ignored
            } else {
                TextInputOutcome::Changed
            },
            "cursor at grapheme {at}"
        );
    }
    // At the end it clears the line, because everything is before the cursor.
    let mut state = TextInputState::with_text("hello");
    state.handle_key(ctrl('u'));
    assert_eq!(state.text(), "");
}

#[test]
fn moves_report_whether_the_cursor_moved() {
    let mut b = buffer("ab", 1);
    assert!(b.move_left());
    assert!(!b.move_left());
    assert!(b.move_right());
    assert!(b.move_right());
    assert!(!b.move_right());
    assert!(b.home());
    assert!(!b.home());
    assert!(b.end());
    assert!(!b.end());
    b.set_cursor(99);
    assert_eq!(b.cursor(), 2, "clamped to the end");
}

#[test]
fn graphemes_iterate_what_the_cursor_steps_over() {
    let b = LineBuffer::from_text("e\u{301}鲸👩\u{200d}💻x");
    let all: Vec<&str> = b.graphemes().collect();
    assert_eq!(all, ["e\u{301}", "鲸", "👩\u{200d}💻", "x"]);
}

// ---------------------------------------------------------------------------
// the scroll window
// ---------------------------------------------------------------------------

fn span(widths: &[usize], a: usize, b: usize) -> usize {
    widths[a..b].iter().sum()
}

#[test]
fn text_that_fits_is_not_clipped() {
    let b = LineBuffer::from_text("hello");
    let w = b.window(10, 0, false);
    assert_eq!((w.start, w.end), (0, 5));
    assert!(!w.left_clipped && !w.right_clipped);
    assert_eq!(w.cursor_col, Some(5), "past the last grapheme");
    // Exactly full still shows the cursor cell: 5 + 1.
    let w = b.window(6, 0, false);
    assert_eq!((w.start, w.end, w.cursor_col), (0, 5, Some(5)));
    assert!(!w.left_clipped && !w.right_clipped);
}

#[test]
fn the_cursor_at_the_end_of_long_text_scrolls_the_head_off() {
    let b = LineBuffer::from_text("hello world foo");
    let w = b.window(10, 0, false);
    assert!(w.left_clipped && w.left_mark);
    assert!(!w.right_clipped);
    assert_eq!((w.start, w.end), (7, 15));
    assert_eq!(w.cursor_col, Some(9), "the cell after the last grapheme");
}

#[test]
fn the_cursor_in_the_middle_clips_both_sides() {
    let b = buffer("0123456789abcdefghij", 10);
    let w = b.window(8, 0, false);
    assert!(w.right_clipped && w.right_mark);
    assert!(w.start <= 10 && 10 < w.end);
    assert!(w.cursor_col.is_some());
}

#[test]
fn scrolling_is_sticky_until_the_cursor_leaves_the_window() {
    let mut b = LineBuffer::from_text("0123456789abcdefghij");
    let mut start = b.window(10, 0, false).start;
    let first = start;
    // Moving left inside the window does not move the text.
    for _ in 0..4 {
        b.move_left();
        start = b.window(10, start, false).start;
        assert_eq!(start, first);
    }
    // Moving past the left edge scrolls just enough to keep the cursor.
    while b.cursor() > first {
        b.move_left();
        start = b.window(10, start, false).start;
    }
    b.move_left();
    let next = b.window(10, start, false).start;
    assert_eq!(next, first - 1);
    assert_eq!(
        b.window(10, next, false).cursor_col,
        Some(1),
        "just right of the left mark"
    );
}

#[test]
fn deleting_uses_spare_room_before_leaving_a_gap() {
    let mut b = LineBuffer::from_text("0123456789abcdefghij");
    let start = b.window(10, 0, false).start;
    assert!(start > 0);
    // Delete down to something that fits: no gap on the right, no clip left.
    for _ in 0..12 {
        b.backspace();
    }
    let w = b.window(10, start, false);
    assert_eq!((w.start, w.end), (0, 8));
    assert!(!w.left_clipped && !w.right_clipped);
}

#[test]
fn wide_characters_are_never_split_at_the_edges() {
    let b = LineBuffer::from_text("鲸鱼日报夜间版本");
    for avail in 4..=20 {
        let w = b.window(avail, 0, false);
        let widths = b.widths(false);
        let drawn =
            span(&widths, w.start, w.end) + usize::from(w.left_mark) + usize::from(w.right_mark);
        assert!(drawn <= avail, "avail {avail}: drew {drawn}");
        assert!(w.cursor_col.is_some(), "avail {avail}");
    }
}

#[test]
fn a_masked_window_counts_one_cell_per_grapheme() {
    let b = LineBuffer::from_text("鲸鱼日报夜间");
    let w = b.window(8, 0, true);
    assert_eq!((w.start, w.end), (0, 6));
    assert!(!w.left_clipped && !w.right_clipped);
    let w = b.window(8, 0, false);
    assert!(w.left_clipped, "unmasked it is 12 cells wide");
}

#[test]
fn an_unfocused_field_shows_the_head_of_the_line() {
    let b = LineBuffer::from_text("0123456789abcdefghij");
    let w = b.head_window(8, false);
    assert_eq!(w.start, 0);
    assert!(w.right_clipped && !w.left_clipped);
    assert_eq!(w.cursor_col, None);
    assert_eq!(w.end, 7, "seven graphemes and the right mark");
}

#[test]
fn a_field_too_narrow_for_marks_still_shows_the_cursor() {
    let b = LineBuffer::from_text("abcdef");
    for avail in 1..4 {
        let w = b.window(avail, 0, false);
        assert!(!w.left_mark && !w.right_mark, "avail {avail}");
        assert!(
            w.cursor_col.is_some_and(|c| c < avail),
            "avail {avail}: {w:?}"
        );
    }
    let w = b.window(0, 0, false);
    assert_eq!(w.end, w.start);
    assert_eq!(w.cursor_col, None);
    let empty = LineBuffer::new().window(5, 0, false);
    assert_eq!((empty.start, empty.end, empty.cursor_col), (0, 0, Some(0)));
}

/// The invariants of `line_window_of`, over text of every shape, every width,
/// every cursor and every previous scroll.
#[test]
fn window_invariants_hold_for_every_width_cursor_and_previous_scroll() {
    let samples: [&str; 8] = [
        "",
        "a",
        "hello world",
        "0123456789abcdefghij",
        "鲸鱼日报夜间版本",
        "cafe\u{301} au lait e\u{301}e\u{301}",
        "👩\u{200d}💻 and 🇺🇸 flags 👨\u{200d}👩\u{200d}👧\u{200d}👦",
        "mixed 鲸 and ascii 鱼 text",
    ];
    for text in samples {
        let b = LineBuffer::from_text(text);
        let n = b.len();
        for masked in [false, true] {
            let widths = b.widths(masked);
            let widest = widths.iter().copied().max().unwrap_or(1);
            for avail in 0..=24usize {
                for cursor in 0..=n {
                    for prev in 0..=n + 1 {
                        let w = line_window_of(&widths, Some(cursor), prev, avail);
                        let at = format!(
                            "{text:?} masked={masked} avail={avail} cursor={cursor} prev={prev}: {w:?}"
                        );
                        assert!(w.start <= w.end && w.end <= n, "{at}");
                        assert_eq!(w.left_clipped, w.start > 0, "{at}");
                        assert_eq!(w.right_clipped, w.end < n, "{at}");
                        assert!(!w.left_mark || w.left_clipped, "{at}");
                        assert!(!w.right_mark || w.right_clipped, "{at}");
                        if avail >= 4 {
                            assert_eq!(w.left_mark, w.left_clipped, "{at}");
                            assert_eq!(w.right_mark, w.right_clipped, "{at}");
                        } else {
                            assert!(!w.left_mark && !w.right_mark, "{at}");
                        }
                        // What is drawn fits.
                        let drawn = span(&widths, w.start, w.end)
                            + usize::from(w.left_mark)
                            + usize::from(w.right_mark);
                        assert!(drawn <= avail, "{at}: drew {drawn}");
                        // The cursor, when shown, is inside the field and
                        // where the text says.
                        if let Some(col) = w.cursor_col {
                            assert!(col < avail, "{at}");
                            if cursor < w.end {
                                assert!(cursor >= w.start, "{at}");
                                assert_eq!(
                                    col,
                                    usize::from(w.left_mark) + span(&widths, w.start, cursor),
                                    "{at}"
                                );
                            } else {
                                assert_eq!(cursor, n, "{at}");
                                assert_eq!(w.end, n, "{at}");
                            }
                        }
                        // Wide enough for the cursor and both marks: it shows.
                        let cursor_w = widths.get(cursor).copied().unwrap_or(1);
                        if avail >= cursor_w + 2 {
                            assert!(w.cursor_col.is_some(), "{at}");
                        }
                        assert!(widest > 0);
                        // Asking again from where it landed changes nothing.
                        let again = line_window_of(&widths, Some(cursor), w.start, avail);
                        assert_eq!(again, w, "{at}: not stable");
                    }
                }
                // No cursor: the head of the line.
                let head = line_window_of(&widths, None, 5, avail);
                assert_eq!(head.start, 0);
                assert_eq!(head.cursor_col, None);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TextInputState: keys in, outcomes out
// ---------------------------------------------------------------------------

#[test]
fn typing_edits_and_reports_changed() {
    let mut s = TextInputState::new();
    assert_eq!(s.handle_key(press('h')), TextInputOutcome::Changed);
    assert_eq!(s.handle_key(press('i')), TextInputOutcome::Changed);
    assert_eq!(s.text(), "hi");
    assert_eq!(s.cursor(), 2);
    assert_eq!(
        s.handle_key(key(KeyCode::Backspace)),
        TextInputOutcome::Changed
    );
    assert_eq!(s.text(), "h");
    // Shift+letter is an upper-case letter, still typed.
    let shifted = KeyEvent::new(KeyCode::Char('H'), KeyModifiers::SHIFT);
    assert_eq!(s.handle_key(shifted), TextInputOutcome::Changed);
    assert_eq!(s.text(), "hH");
}

#[test]
fn enter_submits_and_escape_cancels_without_touching_the_text() {
    let mut s = TextInputState::with_text("keep");
    assert_eq!(
        s.handle_key(key(KeyCode::Enter)),
        TextInputOutcome::Submitted
    );
    assert_eq!(s.handle_key(key(KeyCode::Esc)), TextInputOutcome::Cancelled);
    assert_eq!(s.text(), "keep");
}

#[test]
fn keys_a_field_has_no_use_for_are_ignored_so_the_host_can_use_them() {
    let mut s = TextInputState::with_text("abc");
    for ignored in [
        key(KeyCode::Tab),
        key(KeyCode::BackTab),
        key(KeyCode::Up),
        key(KeyCode::Down),
        key(KeyCode::PageUp),
        key(KeyCode::F(2)),
        ctrl('c'),
        ctrl('z'),
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::SUPER),
        alt(KeyCode::Char('x')),
    ] {
        assert_eq!(
            s.handle_key(ignored),
            TextInputOutcome::Ignored,
            "{ignored:?}"
        );
    }
    assert_eq!(s.text(), "abc");
    let mut release = press('x');
    release.kind = KeyEventKind::Release;
    assert_eq!(s.handle_key(release), TextInputOutcome::Ignored);
    let mut repeat = press('x');
    repeat.kind = KeyEventKind::Repeat;
    assert_eq!(
        s.handle_key(repeat),
        TextInputOutcome::Changed,
        "held keys repeat"
    );
    assert_eq!(s.text(), "abcx");
}

#[test]
fn a_no_op_at_a_boundary_is_ignored() {
    let mut s = TextInputState::new();
    for code in [
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Home,
        KeyCode::End,
    ] {
        assert_eq!(
            s.handle_key(key(code)),
            TextInputOutcome::Ignored,
            "{code:?}"
        );
    }
    let mut s = TextInputState::with_text("a");
    assert_eq!(
        s.handle_key(key(KeyCode::Right)),
        TextInputOutcome::Ignored,
        "already at the end"
    );
    assert_eq!(s.handle_key(key(KeyCode::Left)), TextInputOutcome::Changed);
}

#[test]
fn the_edit_keys_do_what_the_table_says() {
    let mut s = TextInputState::with_text("foo bar baz");
    // Ctrl+W, Alt+Backspace, Ctrl+Backspace
    assert_eq!(s.handle_key(ctrl('w')), TextInputOutcome::Changed);
    assert_eq!(s.text(), "foo bar ");
    assert_eq!(
        s.handle_key(alt(KeyCode::Backspace)),
        TextInputOutcome::Changed
    );
    assert_eq!(s.text(), "foo ");
    let ctrl_backspace = KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL);
    assert_eq!(s.handle_key(ctrl_backspace), TextInputOutcome::Changed);
    assert_eq!(s.text(), "");

    // Ctrl+U / Ctrl+K around the cursor
    let mut s = TextInputState::with_text("hello world");
    s.buffer_mut().set_cursor(5);
    assert_eq!(s.handle_key(ctrl('k')), TextInputOutcome::Changed);
    assert_eq!(s.text(), "hello");
    s.buffer_mut().set_cursor(2);
    assert_eq!(s.handle_key(ctrl('u')), TextInputOutcome::Changed);
    assert_eq!(s.text(), "llo");

    // Delete, Alt+D, Alt+Delete, Ctrl+Delete
    let mut s = TextInputState::with_text("ab cd ef gh");
    s.buffer_mut().set_cursor(0);
    assert_eq!(
        s.handle_key(key(KeyCode::Delete)),
        TextInputOutcome::Changed
    );
    assert_eq!(s.text(), "b cd ef gh");
    assert_eq!(
        s.handle_key(alt(KeyCode::Char('d'))),
        TextInputOutcome::Changed
    );
    assert_eq!(s.text(), " cd ef gh");
    assert_eq!(
        s.handle_key(alt(KeyCode::Delete)),
        TextInputOutcome::Changed
    );
    assert_eq!(s.text(), " ef gh");
    let ctrl_delete = KeyEvent::new(KeyCode::Delete, KeyModifiers::CONTROL);
    assert_eq!(s.handle_key(ctrl_delete), TextInputOutcome::Changed);
    assert_eq!(s.text(), " gh");

    // Ctrl+H is the backspace many terminals send.
    let mut s = TextInputState::with_text("ab");
    assert_eq!(s.handle_key(ctrl('h')), TextInputOutcome::Changed);
    assert_eq!(s.text(), "a");
}

#[test]
fn the_move_keys_do_what_the_table_says() {
    let mut s = TextInputState::with_text("one two three");
    s.handle_key(key(KeyCode::Home));
    assert_eq!(s.cursor(), 0);
    s.handle_key(key(KeyCode::End));
    assert_eq!(s.cursor(), 13);
    s.handle_key(ctrl('a'));
    assert_eq!(s.cursor(), 0);
    s.handle_key(ctrl('e'));
    assert_eq!(s.cursor(), 13);
    s.handle_key(ctrl('b'));
    s.handle_key(key(KeyCode::Left));
    assert_eq!(s.cursor(), 11);
    s.handle_key(ctrl('f'));
    s.handle_key(key(KeyCode::Right));
    assert_eq!(s.cursor(), 13);
    // Words: Alt+arrows, Ctrl+arrows, Alt+B / Alt+F.
    s.handle_key(alt(KeyCode::Left));
    assert_eq!(s.cursor(), 8);
    s.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
    assert_eq!(s.cursor(), 4);
    s.handle_key(alt(KeyCode::Char('b')));
    assert_eq!(s.cursor(), 0);
    s.handle_key(alt(KeyCode::Right));
    assert_eq!(s.cursor(), 4);
    s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL));
    assert_eq!(s.cursor(), 8);
    s.handle_key(alt(KeyCode::Char('f')));
    assert_eq!(s.cursor(), 13);
    // Typing goes in at the cursor.
    s.handle_key(alt(KeyCode::Left));
    s.handle_key(press('X'));
    assert_eq!(s.text(), "one two Xthree");
}

#[test]
fn alt_with_a_non_ascii_character_types_it_as_macos_option_does() {
    let mut s = TextInputState::new();
    assert_eq!(
        s.handle_key(alt(KeyCode::Char('å'))),
        TextInputOutcome::Changed
    );
    assert_eq!(s.text(), "å");
}

#[test]
fn paste_goes_in_clean() {
    let mut s = TextInputState::new();
    assert_eq!(s.paste("one\ntwo\u{1b}"), TextInputOutcome::Changed);
    assert_eq!(s.text(), "onetwo");
    assert_eq!(s.paste("\n\n"), TextInputOutcome::Ignored);
    assert_eq!(s.cursor(), 6);
}

#[test]
fn a_limit_stops_typing() {
    let mut s = TextInputState::new().with_limit(3);
    type_str(&mut s, "abcdef");
    assert_eq!(s.text(), "abc");
    assert_eq!(s.handle_key(press('x')), TextInputOutcome::Ignored);
    assert_eq!(
        s.handle_key(key(KeyCode::Backspace)),
        TextInputOutcome::Changed
    );
}

#[test]
fn a_secret_edits_as_one_block() {
    let mut s = TextInputState::secret();
    assert!(s.is_masked());
    type_str(&mut s, "pass word two");
    // Word keys act on the whole line: where the spaces are does not leak.
    s.buffer_mut().set_cursor(5);
    s.handle_key(alt(KeyCode::Left));
    assert_eq!(s.cursor(), 0);
    s.handle_key(alt(KeyCode::Right));
    assert_eq!(s.cursor(), 13);
    s.buffer_mut().set_cursor(4);
    s.handle_key(ctrl('w'));
    assert_eq!(s.text(), " word two");
    s.buffer_mut().set_cursor(1);
    s.handle_key(alt(KeyCode::Char('d')));
    assert_eq!(s.text(), " ");
}

#[test]
fn debug_never_prints_a_secret() {
    let mut s = TextInputState::secret();
    type_str(&mut s, "correct-horse");
    let shown = format!("{s:?}");
    assert!(
        !shown.contains("correct") && !shown.contains("horse"),
        "{shown}"
    );
    assert!(shown.contains("masked: true"));
    // A plain field does show its text; that is what Debug is for there.
    let plain = format!("{:?}", TextInputState::with_text("visible"));
    assert!(plain.contains("visible"));
    // And a form holding a secret does not leak it either.
    let mut form = FormState::new(vec![FormField::secret("Key")]);
    form_type(&mut form, "correct-horse");
    assert!(!format!("{form:?}").contains("correct"));
}

#[test]
fn state_is_cloneable_and_shareable() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TextInputState>();
    assert_send_sync::<FormState>();
    let a = TextInputState::with_text("abc");
    let mut b = a.clone();
    b.handle_key(press('d'));
    assert_eq!((a.text(), b.text()), ("abc", "abcd"));
}

// ---------------------------------------------------------------------------
// painting: states
// ---------------------------------------------------------------------------

/// A gallery entry's painter, by name.
fn entry(name: &str) -> (u16, fn(Rect, &mut Buffer, &Theme)) {
    let found = gallery::entries().into_iter().find(|e| e.name == name);
    let e = found.unwrap_or_else(|| panic!("no gallery entry {name}"));
    (e.height, e.draw)
}

#[test]
fn every_state_keeps_the_frame_rules_and_matches_its_snapshot() {
    macro_rules! state {
        ($name:literal) => {{
            let (height, draw) = entry($name);
            testing::assert_rules(height, draw);
            insta::assert_snapshot!($name, testing::snapshot(height, draw));
        }};
    }
    state!("text-input-empty");
    state!("text-input-typed");
    state!("text-input-unfocused");
    state!("text-input-invalid");
    state!("text-input-disabled");
    state!("text-input-secret");
    state!("text-input-long");
    state!("text-input-wide-text");
    state!("form");
}

/// A form straight after it is built: no error yet, focus on the first field.
fn fresh_form() -> FormState {
    FormState::new(vec![
        FormField::text("Name")
            .placeholder("e.g. nightly-report")
            .validate(non_empty),
        FormField::secret("API key").help("Stored in your keychain."),
        FormField::read_only("Schedule", "every day at 7"),
        FormField::check("Notify me", false),
    ])
}

#[test]
fn a_fresh_form_snapshot() {
    let paint = |area: Rect, buf: &mut Buffer, theme: &Theme| {
        Form::new(&fresh_form()).paint(area, buf, theme);
    };
    testing::assert_rules(12, paint);
    insta::assert_snapshot!("form-fresh", testing::snapshot(12, paint));
}

#[test]
fn a_form_taller_than_its_area_scrolls_to_the_focused_field() {
    let mut state = fresh_form();
    state.set_focus(3);
    let paint = |area: Rect, buf: &mut Buffer, theme: &Theme| {
        Form::new(&state).paint(area, buf, theme);
    };
    testing::assert_rules(5, paint);
    insta::assert_snapshot!("form-scrolled", testing::snapshot(5, paint));
    let theme = Profile::NoColor.theme();
    let buf = testing::render(40, 5, |area, buf| paint(area, buf, &theme));
    let shown = testing::text(&buf);
    assert!(
        shown.contains("Notify me") && shown.contains("Off"),
        "{shown}"
    );
    assert!(!shown.contains("Name"), "{shown}");
}

#[test]
fn a_long_error_wraps_under_the_field_and_is_whole() {
    let message = "Enter a port from 1 to 65535, and not one that another service on this machine already uses.";
    let state = TextInputState::with_text("70000");
    let input = TextInput::new(&state)
        .label("Port")
        .focused(true)
        .error(message);
    let theme = Profile::NoColor.theme();
    let rows = input.height(40, &theme);
    assert!(rows >= 4, "label, input and a wrapped error: {rows}");
    let buf = testing::render(40, rows, |area, buf| input.paint(area, buf, &theme));
    let shown = testing::text(&buf);
    let words: Vec<&str> = message.split_whitespace().collect();
    for word in &words {
        assert!(shown.contains(word), "lost {word:?}:\n{shown}");
    }
    assert!(shown.contains("✕ Enter a port"), "{shown}");
    for line in shown.lines() {
        assert!(codewhale_ratatui::text::width(line) <= 40, "{line:?}");
    }
    let paint = |area: Rect, buf: &mut Buffer, theme: &Theme| input.paint(area, buf, theme);
    testing::assert_rules(rows, paint);
    insta::assert_snapshot!("text-input-long-error", testing::snapshot(rows, paint));
}

#[test]
fn a_note_that_does_not_fit_in_three_rows_is_cut_with_an_ellipsis() {
    let message = "word ".repeat(60);
    let state = TextInputState::new();
    let input = TextInput::new(&state).error(message);
    for profile in [Profile::DarkTrue, Profile::Ascii] {
        let theme = profile.theme();
        let rows = input.height(30, &theme);
        assert_eq!(rows, 4, "an input row and three note rows");
        let buf = testing::render(30, rows, |area, buf| input.paint(area, buf, &theme));
        let shown = testing::text(&buf);
        let last = shown.lines().last().unwrap_or_default();
        let ellipsis = if profile == Profile::Ascii {
            "..."
        } else {
            "…"
        };
        assert!(last.ends_with(ellipsis), "{profile:?}: {last:?}");
    }
}

#[test]
fn a_short_area_drops_the_label_before_the_error() {
    let state = TextInputState::with_text("70000");
    let input = TextInput::new(&state)
        .label("Port")
        .focused(true)
        .error("Enter a port.");
    let theme = Profile::NoColor.theme();
    let render =
        |rows: u16| testing::text(&testing::render(30, rows, |a, b| input.paint(a, b, &theme)));
    assert_eq!(render(3), "Port\n› 70000\n✕ Enter a port.");
    assert_eq!(render(2), "› 70000\n✕ Enter a port.");
    assert_eq!(render(1), "› 70000");
}

#[test]
fn every_height_and_width_paints_inside_its_area() {
    // A small area must never write outside itself (a buffer bigger than the
    // area would show it) or panic.
    let secret = TextInputState::secret();
    let long =
        TextInputState::with_text("鲸鱼日报 cafe\u{301} 👩\u{200d}💻 and a very long line indeed");
    let empty = TextInputState::new();
    for state in [&secret, &long, &empty] {
        let input = TextInput::new(state)
            .label("Label")
            .placeholder("placeholder")
            .focused(true)
            .error("An error that wraps around the narrow field.");
        for width in 0..=12u16 {
            for height in 0..=5u16 {
                let theme = Profile::DarkTrue.theme();
                let inner = Rect::new(2, 1, width, height);
                let mut buf = Buffer::empty(Rect::new(0, 0, width + 4, height + 3));
                input.paint(inner, &mut buf, &theme);
                for y in 0..buf.area.height {
                    for x in 0..buf.area.width {
                        let inside = x >= 2 && x < 2 + width && y >= 1 && y < 1 + height;
                        if !inside {
                            assert_eq!(buf[(x, y)].symbol(), " ", "({x},{y}) in {width}x{height}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_secret_never_shows_its_text_or_its_last_character() {
    let secret = "zqxjvw!7";
    let mut state = TextInputState::secret();
    type_str(&mut state, secret);
    let input = TextInput::new(&state)
        .label("Key")
        .placeholder("none")
        .focused(true);
    for frame in testing::frames(2, |a, b, t| input.paint(a, b, t)) {
        let shown = frame.text();
        for c in secret.chars() {
            assert!(
                !shown.contains(c),
                "{}: leaked {c:?}:\n{shown}",
                frame.label()
            );
        }
        let mask = if frame.profile == Profile::Ascii {
            '*'
        } else {
            '•'
        };
        assert_eq!(
            shown.chars().filter(|c| *c == mask).count(),
            secret.chars().count(),
            "{}: one mask per character, the last included:\n{shown}",
            frame.label()
        );
    }
}

#[test]
fn a_secret_hides_wide_characters_behind_one_cell_each() {
    let mut state = TextInputState::secret();
    type_str(&mut state, "鲸鱼🙂");
    let input = TextInput::new(&state).focused(true);
    let theme = Profile::NoColor.theme();
    let shown = testing::text(&testing::render(12, 1, |a, b| input.paint(a, b, &theme)));
    assert_eq!(shown, "› •••");
}

#[test]
fn the_painted_cursor_is_where_cursor_position_says() {
    let reversed = |buf: &Buffer| -> Vec<(u16, u16)> {
        let mut found = Vec::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if buf[(x, y)].modifier.contains(Modifier::REVERSED) {
                    found.push((x, y));
                }
            }
        }
        found
    };
    let theme = Profile::DarkTrue.theme();
    let area = Rect::new(3, 1, 20, 4);
    for text in [
        "",
        "abc",
        "鲸鱼日报",
        "a long line of text that passes the end of the field",
    ] {
        for at in [0, 1, 5, 99] {
            let mut state = TextInputState::with_text(text);
            state.buffer_mut().set_cursor(at);
            let input = TextInput::new(&state)
                .label("L")
                .placeholder("p")
                .focused(true);
            let mut buf = Buffer::empty(Rect::new(0, 0, 30, 8));
            input.paint(area, &mut buf, &theme);
            let cells = reversed(&buf);
            let pos = input
                .cursor_position(area)
                .expect("a focused field has a cursor");
            assert_eq!(cells.first(), Some(&(pos.x, pos.y)), "{text:?} at {at}");
            // A wide character is reversed across both of its cells.
            assert!(cells.len() <= 2, "{cells:?}");
            // An unfocused field paints and offers none.
            let quiet = TextInput::new(&state).label("L");
            assert_eq!(quiet.cursor_position(area), None);
            let mut buf = Buffer::empty(Rect::new(0, 0, 30, 8));
            quiet.paint(area, &mut buf, &theme);
            assert!(reversed(&buf).is_empty());
        }
    }
}

#[test]
fn the_terminal_cursor_waits_where_the_text_ends() {
    let state = TextInputState::with_text("abc");
    let input = TextInput::new(&state).label("L").focused(true);
    let pos = input
        .cursor_position(Rect::new(4, 2, 20, 2))
        .expect("cursor");
    // Label row, then the input row; two cells of prompt, then three letters.
    assert_eq!((pos.x, pos.y), (4 + 2 + 3, 3));
}

#[test]
fn a_disabled_field_says_why_in_words_and_cannot_be_focused() {
    let state = TextInputState::with_text("shannon-labs");
    let theme = Profile::NoColor.theme();
    let render = |input: TextInput<'_>| {
        testing::text(&testing::render(40, 3, |a, b| input.paint(a, b, &theme)))
    };
    let shown = render(
        TextInput::new(&state)
            .label("Org")
            .focused(true)
            .disabled("Set by your administrator"),
    );
    assert_eq!(shown, "Org\n  shannon-labs\n· Set by your administrator");
    // Without a reason it still says it, in the host's words.
    let shown = render(TextInput::new(&state).label("Org").disabled(""));
    assert!(shown.ends_with("· Disabled"), "{shown}");
    let german = TextInputWords {
        disabled: "Deaktiviert".into(),
    };
    let shown = render(
        TextInput::new(&state)
            .label("Org")
            .disabled("")
            .words(&german),
    );
    assert!(shown.ends_with("· Deaktiviert"), "{shown}");
    assert_eq!(
        TextInput::new(&state)
            .focused(true)
            .disabled("x")
            .cursor_position(Rect::new(0, 0, 20, 3)),
        None
    );
}

#[test]
fn an_error_wins_over_help_and_a_reason() {
    let state = TextInputState::new();
    let theme = Profile::NoColor.theme();
    let render = |input: TextInput<'_>| {
        testing::text(&testing::render(40, 2, |a, b| input.paint(a, b, &theme)))
    };
    let helped = render(TextInput::new(&state).help("Digits only."));
    assert!(helped.contains("Digits only."), "{helped}");
    let errored = render(
        TextInput::new(&state)
            .help("Digits only.")
            .error("Not a number."),
    );
    assert!(
        errored.contains("✕ Not a number.") && !errored.contains("Digits only."),
        "{errored}"
    );
}

#[test]
fn a_masked_state_set_through_buffer_mut_keeps_following_the_cursor() {
    // A host edit through `buffer_mut` scrolls on the next paint.
    let mut state = TextInputState::new();
    state.buffer_mut().insert_str(&"abcdefghij".repeat(4));
    let input = TextInput::new(&state).focused(true);
    let theme = Profile::NoColor.theme();
    let shown = testing::text(&testing::render(14, 1, |a, b| input.paint(a, b, &theme)));
    assert!(shown.starts_with("› …"), "{shown}");
}

#[test]
fn scroll_is_remembered_between_paints_and_keys() {
    // Typing in a narrow field, then moving left inside the window, does not
    // shift the text under the person's eyes.
    let mut state = TextInputState::with_text(&"0123456789".repeat(3));
    state.scroll_into_view(10);
    let theme = Profile::NoColor.theme();
    let paint = |state: &TextInputState| {
        let input = TextInput::new(state).focused(true);
        testing::text(&testing::render(12, 1, |a, b| input.paint(a, b, &theme)))
    };
    let at_end = paint(&state);
    state.handle_key(key(KeyCode::Left));
    state.handle_key(key(KeyCode::Left));
    let moved = paint(&state);
    assert_eq!(
        at_end, moved,
        "the cursor moved inside the window; the text did not"
    );
    // Far enough left, it scrolls.
    for _ in 0..12 {
        state.handle_key(key(KeyCode::Left));
    }
    assert_ne!(paint(&state), at_end);
}

#[test]
fn text_with_bidi_controls_is_painted_without_them() {
    let mut state = TextInputState::new();
    state.buffer_mut().insert_str("rm -rf ~/\u{202e}txt.exe");
    let input = TextInput::new(&state)
        .label("Spoof\u{202e}ed")
        .focused(true);
    let theme = Profile::NoColor.theme();
    let shown = testing::text(&testing::render(40, 2, |a, b| input.paint(a, b, &theme)));
    assert!(!shown.contains('\u{202e}'), "{shown:?}");
    assert!(shown.contains("rm -rf ~/txt.exe"));
}

// ---------------------------------------------------------------------------
// Form
// ---------------------------------------------------------------------------

fn non_empty(text: &str) -> Result<(), Cow<'static, str>> {
    if text.trim().is_empty() {
        Err("Give it a name.".into())
    } else {
        Ok(())
    }
}

fn port(text: &str) -> Result<(), Cow<'static, str>> {
    match text.parse::<u32>() {
        Ok(1..=65535) => Ok(()),
        Ok(_) => Err("Enter a port from 1 to 65535.".into()),
        Err(_) => Err("A port is a number.".into()),
    }
}

fn two_fields() -> FormState {
    FormState::new(vec![
        FormField::text("Name").validate(non_empty),
        FormField::text("Port").validate(port),
    ])
}

fn shown(form: &FormState, width: u16) -> String {
    let theme = Profile::NoColor.theme();
    let f = Form::new(form);
    testing::text(&testing::render(width, f.height(width, &theme), |a, b| {
        f.paint(a, b, &theme);
    }))
}

#[test]
fn focus_starts_on_the_first_field_that_takes_it() {
    let form = FormState::new(vec![
        FormField::read_only("Schedule", "daily"),
        FormField::text("Name"),
    ]);
    assert_eq!(form.focus(), 1);
    assert_eq!(FormState::new(vec![]).focus(), 0);
    assert!(FormState::new(vec![]).is_empty());
}

#[test]
fn tab_and_down_move_forward_and_wrap_shift_tab_and_up_move_back() {
    let mut form = FormState::new(vec![
        FormField::text("A"),
        FormField::read_only("Skipped", "x"),
        FormField::check("B", false),
        FormField::text("C"),
    ]);
    assert_eq!(form.len(), 4);
    assert_eq!(form.handle_key(key(KeyCode::Tab)), FormOutcome::Changed);
    assert_eq!(form.focus(), 2, "the read-only row is skipped");
    form.handle_key(key(KeyCode::Down));
    assert_eq!(form.focus(), 3);
    form.handle_key(key(KeyCode::Tab));
    assert_eq!(form.focus(), 0, "wraps");
    form.handle_key(key(KeyCode::BackTab));
    assert_eq!(form.focus(), 3, "wraps back");
    form.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
    assert_eq!(form.focus(), 2);
    form.handle_key(key(KeyCode::Up));
    assert_eq!(form.focus(), 0, "skips the read-only row going up too");
}

#[test]
fn tab_with_nowhere_to_go_is_left_to_the_host() {
    let mut one = FormState::new(vec![FormField::text("A"), FormField::read_only("B", "x")]);
    assert_eq!(one.handle_key(key(KeyCode::Tab)), FormOutcome::Ignored);
    assert_eq!(one.handle_key(key(KeyCode::Up)), FormOutcome::Ignored);
    let mut none = FormState::new(vec![]);
    for code in [
        KeyCode::Tab,
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Char('a'),
    ] {
        assert_eq!(none.handle_key(key(code)), FormOutcome::Ignored);
    }
    assert_eq!(none.paste("x"), FormOutcome::Ignored);
    assert_eq!(
        none.submit(),
        FormOutcome::Submitted,
        "an empty form has nothing invalid"
    );
}

#[test]
fn keys_edit_the_focused_field_only() {
    let mut form = two_fields();
    form_type(&mut form, "nightly");
    form.handle_key(key(KeyCode::Tab));
    form_type(&mut form, "80");
    assert_eq!(form.text(0), Some("nightly"));
    assert_eq!(form.text(1), Some("80"));
    assert_eq!(form.handle_key(key(KeyCode::Left)), FormOutcome::Changed);
    assert_eq!(form.handle_key(key(KeyCode::Home)), FormOutcome::Changed);
    assert_eq!(
        form.handle_key(key(KeyCode::Home)),
        FormOutcome::Ignored,
        "no move"
    );
    assert_eq!(form.handle_key(ctrl('c')), FormOutcome::Ignored);
    let mut release = press('x');
    release.kind = KeyEventKind::Release;
    assert_eq!(form.handle_key(release), FormOutcome::Ignored);
    assert_eq!(form.text(5), None);
    assert_eq!(form.checked(0), None);
}

#[test]
fn an_error_shows_when_focus_leaves_the_field_not_while_typing() {
    let mut form = two_fields();
    form_type(&mut form, "   ");
    assert_eq!(
        form.status(0),
        Some(&FormStatus::Untouched),
        "still typing: say nothing"
    );
    assert!(!shown(&form, 40).contains('✕'));
    form.handle_key(key(KeyCode::Tab));
    assert_eq!(
        form.status(0),
        Some(&FormStatus::Invalid("Give it a name.".into()))
    );
    let text = shown(&form, 40);
    assert!(text.contains("✕ Give it a name."), "{text}");
}

#[test]
fn an_error_clears_the_moment_the_text_is_valid() {
    let mut form = FormState::new(vec![
        FormField::text("Name").validate(non_empty),
        FormField::text("Note"),
    ]);
    form.handle_key(key(KeyCode::Tab)); // blur Name while empty
    form.handle_key(key(KeyCode::BackTab));
    assert!(matches!(form.status(0), Some(FormStatus::Invalid(_))));
    form_type(&mut form, "n");
    assert_eq!(
        form.status(0),
        Some(&FormStatus::Valid),
        "cleared on the keystroke that fixed it"
    );
    assert!(!shown(&form, 40).contains('✕'));
    // Editing a valid field into an invalid one does not shout mid-edit.
    form.handle_key(key(KeyCode::Backspace));
    assert_eq!(form.status(0), Some(&FormStatus::Untouched));
}

#[test]
fn a_shown_error_follows_the_text_while_it_stays_wrong() {
    fn password(text: &str) -> Result<(), Cow<'static, str>> {
        if text.chars().count() < 8 {
            Err("Use at least 8 characters.".into())
        } else if !text.chars().any(|c| c.is_ascii_digit()) {
            Err("Add a digit.".into())
        } else {
            Ok(())
        }
    }
    let mut form = FormState::new(vec![
        FormField::secret("Password").validate(password),
        FormField::text("Note"),
    ]);
    form_type(&mut form, "abc");
    assert_eq!(form.status(0), Some(&FormStatus::Untouched));
    form.handle_key(key(KeyCode::Tab));
    form.handle_key(key(KeyCode::Tab));
    let short = FormStatus::Invalid("Use at least 8 characters.".into());
    assert_eq!(form.status(0), Some(&short), "shown once focus left it");
    form_type(&mut form, "defgh");
    assert_eq!(
        form.status(0),
        Some(&FormStatus::Invalid("Add a digit.".into())),
        "same field, a new reason"
    );
    form_type(&mut form, "1");
    assert_eq!(form.status(0), Some(&FormStatus::Valid));
}

#[test]
fn fields_with_no_rule_never_show_a_verdict() {
    let mut form = FormState::new(vec![FormField::text("Note"), FormField::text("Other")]);
    form_type(&mut form, "anything");
    form.handle_key(key(KeyCode::Tab));
    assert_eq!(form.status(0), Some(&FormStatus::Untouched));
    assert_eq!(form.submit(), FormOutcome::Submitted);
}

#[test]
fn submit_is_blocked_while_invalid_and_focus_goes_to_the_first_invalid_field() {
    let mut form = FormState::new(vec![
        FormField::text("Name").validate(non_empty),
        FormField::text("Port").validate(port),
        FormField::text("Backup port").validate(port),
    ]);
    form_type(&mut form, "nightly");
    form.handle_key(key(KeyCode::Tab));
    form.handle_key(key(KeyCode::Tab));
    form.handle_key(key(KeyCode::Tab));
    assert_eq!(form.focus(), 0);
    form.handle_key(key(KeyCode::Down));
    form.handle_key(key(KeyCode::Down));
    assert_eq!(form.focus(), 2);
    form_type(&mut form, "9");
    // Port is empty, Backup port is "9": blocked on Port, the first.
    assert_eq!(
        form.handle_key(key(KeyCode::Enter)),
        FormOutcome::Blocked(1)
    );
    assert_eq!(form.focus(), 1);
    assert_eq!(form.text(0), Some("nightly"), "nothing was cleared");
    assert_eq!(form.text(2), Some("9"));
    assert!(matches!(form.status(1), Some(FormStatus::Invalid(_))));
    assert_eq!(form.status(0), Some(&FormStatus::Valid));
    assert_eq!(form.status(2), Some(&FormStatus::Valid));
    let text = shown(&form, 40);
    assert!(text.contains("✕ A port is a number."), "{text}");
    assert!(!form.is_valid());
    // Fix it and submit: through.
    form_type(&mut form, "8080");
    assert!(form.is_valid());
    assert_eq!(form.handle_key(key(KeyCode::Enter)), FormOutcome::Submitted);
    assert_eq!(form.focus(), 1, "a good submit does not move focus");
}

#[test]
fn enter_inside_a_text_field_submits_the_form_not_the_field() {
    let mut form = two_fields();
    form_type(&mut form, "n");
    form.handle_key(key(KeyCode::Tab));
    form_type(&mut form, "80");
    assert_eq!(form.handle_key(key(KeyCode::Enter)), FormOutcome::Submitted);
    assert_eq!(form.handle_key(key(KeyCode::Esc)), FormOutcome::Cancelled);
}

#[test]
fn a_checkbox_toggles_with_space_and_can_be_required() {
    let mut form = FormState::new(vec![
        FormField::text("Name"),
        FormField::check("I understand", false).require("Tick this to continue."),
        FormField::check("Notify me", true),
    ]);
    assert_eq!(form.checked(1), Some(false));
    form.handle_key(key(KeyCode::Tab));
    assert_eq!(
        form.handle_key(press('x')),
        FormOutcome::Ignored,
        "letters mean nothing here"
    );
    assert_eq!(form.handle_key(ctrl(' ')), FormOutcome::Ignored);
    assert_eq!(
        form.handle_key(key(KeyCode::Enter)),
        FormOutcome::Blocked(1)
    );
    let text = shown(&form, 40);
    assert!(text.contains("✕ Tick this to continue."), "{text}");
    assert!(text.contains("Off") && text.contains("On"), "{text}");
    assert_eq!(form.handle_key(press(' ')), FormOutcome::Changed);
    assert_eq!(form.checked(1), Some(true));
    assert_eq!(
        form.status(1),
        Some(&FormStatus::Valid),
        "cleared on the toggle"
    );
    assert_eq!(form.handle_key(key(KeyCode::Enter)), FormOutcome::Submitted);
    // Moving on from an unchecked required box shows its error.
    form.handle_key(press(' '));
    assert_eq!(
        form.status(1),
        Some(&FormStatus::Untouched),
        "unchecking does not shout"
    );
    form.handle_key(key(KeyCode::Tab));
    assert!(matches!(form.status(1), Some(FormStatus::Invalid(_))));
}

#[test]
fn a_read_only_row_shows_its_value_and_what_it_is() {
    let form = FormState::new(vec![FormField::read_only("Schedule", "every day at 7")]);
    let text = shown(&form, 40);
    assert_eq!(text, "Schedule\n  every day at 7\n· Read only");
}

#[test]
fn dirty_means_something_would_be_lost() {
    let mut form = FormState::new(vec![
        FormField::text("Name").value("nightly"),
        FormField::check("Notify", true),
    ]);
    assert!(!form.is_dirty());
    form_type(&mut form, "x");
    assert!(form.is_dirty());
    form.handle_key(key(KeyCode::Backspace));
    assert!(!form.is_dirty(), "typed and undone");
    form.set_focus(1);
    form.handle_key(press(' '));
    assert!(form.is_dirty());
}

#[test]
fn the_host_can_set_a_status_and_move_focus() {
    let mut form = two_fields();
    form.set_status(1, FormStatus::Invalid("Port 80 is in use.".into()));
    assert!(shown(&form, 40).contains("✕ Port 80 is in use."));
    form.set_status(9, FormStatus::Valid);
    form.set_focus(1);
    assert_eq!(form.focus(), 1);
    assert!(
        matches!(form.status(0), Some(FormStatus::Invalid(_))),
        "leaving Name checked it"
    );
    form.set_focus(9);
    assert_eq!(form.focus(), 1);
    assert_eq!(form.field(1).map(FormField::label), Some("Port"));
    assert!(matches!(
        form.field(1).map(FormField::kind),
        Some(FormFieldKind::Text(_))
    ));
}

#[test]
fn paste_lands_in_the_focused_text_field_only() {
    let mut form = FormState::new(vec![FormField::check("A", false), FormField::text("B")]);
    assert_eq!(
        form.paste("x"),
        FormOutcome::Ignored,
        "focus is on a checkbox"
    );
    form.handle_key(key(KeyCode::Tab));
    assert_eq!(form.paste("one\ntwo"), FormOutcome::Changed);
    assert_eq!(form.text(1), Some("onetwo"));
    assert_eq!(form.paste("\n"), FormOutcome::Ignored);
}

#[test]
fn a_secret_field_in_a_form_never_leaks_into_what_is_painted() {
    let secret = "zqxjvw!7";
    let mut form = FormState::new(vec![FormField::secret("Key").validate(non_empty)]);
    form_type(&mut form, secret);
    assert_eq!(form.text(0), Some(secret), "the host gets the real value");
    for frame in testing::frames(4, |a, b, t| Form::new(&form).paint(a, b, t)) {
        let text = frame.text();
        for c in secret.chars() {
            assert!(!text.contains(c), "{}: leaked {c:?}\n{text}", frame.label());
        }
    }
    assert_eq!(form.handle_key(key(KeyCode::Enter)), FormOutcome::Submitted);
}

#[test]
fn the_form_cursor_is_where_the_painted_cursor_is() {
    let theme = Profile::DarkTrue.theme();
    let mut form = two_fields();
    form_type(&mut form, "abc");
    form.handle_key(key(KeyCode::Tab));
    form_type(&mut form, "80");
    let area = Rect::new(2, 1, 30, 8);
    let f = Form::new(&form);
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
    f.paint(area, &mut buf, &theme);
    let pos = f.cursor_position(area).expect("focused text field");
    assert!(buf[(pos.x, pos.y)].modifier.contains(Modifier::REVERSED));
    // Port's text row is the fourth row (Name label, Name input, Port label, Port input).
    assert_eq!((pos.x, pos.y), (2 + 2 + 2, 1 + 3));
    assert_eq!(Form::new(&form).focused(false).cursor_position(area), None);
    // On a checkbox the terminal cursor has nowhere to be.
    let boxed = FormState::new(vec![FormField::check("A", false)]);
    assert_eq!(Form::new(&boxed).cursor_position(area), None);
}

#[test]
fn an_unfocused_form_paints_no_prompt_and_no_cursor() {
    let form = two_fields();
    let theme = Profile::NoColor.theme();
    let f = Form::new(&form).focused(false);
    let text = testing::text(&testing::render(30, f.height(30, &theme), |a, b| {
        f.paint(a, b, &theme)
    }));
    assert!(!text.contains('›'), "{text}");
}

#[test]
fn the_forms_own_words_are_parameters() {
    let form = FormState::new(vec![
        FormField::check("Benachrichtigen", true),
        FormField::read_only("Zeitplan", "täglich"),
    ]);
    let words = FormWords {
        on: "An".into(),
        off: "Aus".into(),
        read_only: "Nur lesbar".into(),
    };
    let theme = Profile::NoColor.theme();
    let f = Form::new(&form).words(&words);
    let text = testing::text(&testing::render(40, f.height(40, &theme), |a, b| {
        f.paint(a, b, &theme)
    }));
    assert!(
        text.contains("● An") && text.contains("· Nur lesbar"),
        "{text}"
    );
    assert!(
        !text.contains("On") && !text.contains("Read only"),
        "{text}"
    );
    assert_eq!(FormWords::default().on, "On");
    assert_eq!(TextInputWords::default().disabled, "Disabled");
}

#[test]
fn form_heights_add_up() {
    let theme = Profile::NoColor.theme();
    let form = fresh_form();
    // Name (label, input), API key (label, input, help), Schedule (label,
    // input, note), Notify me (label, input).
    assert_eq!(Form::new(&form).height(40, &theme), 2 + 3 + 3 + 2);
    let mut blocked = fresh_form();
    blocked.handle_key(key(KeyCode::Enter));
    assert_eq!(Form::new(&blocked).height(40, &theme), 2 + 1 + 3 + 3 + 2);
}

#[test]
fn a_form_with_nothing_in_it_paints_nothing() {
    let form = FormState::new(vec![]);
    testing::assert_rules(3, |a, b, t| Form::new(&form).paint(a, b, t));
    let theme = Profile::NoColor.theme();
    assert_eq!(Form::new(&form).height(40, &theme), 0);
}

#[test]
fn every_form_state_keeps_the_frame_rules_at_every_height() {
    let mut blocked = fresh_form();
    blocked.handle_key(key(KeyCode::Enter));
    for form in [fresh_form(), blocked] {
        for height in [1, 2, 3, 5, 8, 12, 16] {
            testing::assert_rules(height, |a, b, t| Form::new(&form).paint(a, b, t));
        }
    }
}
