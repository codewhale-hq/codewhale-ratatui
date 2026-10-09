//! Package Display: receipts, diff, workflow tree, progress and toasts.
//!
//! Each component is checked three ways: its behaviour (the formatting
//! helpers, the parser, the state machine), the rules every frame keeps
//! (`testing::assert_rules`, 9 profiles x 40/80/120 columns), and a snapshot
//! of what it paints (`testing::snapshot`). The snapshots reuse the gallery's
//! fixtures, so what the gallery shows is what is pinned here.

use std::time::{Duration, Instant};

use codewhale_ratatui::{
    Cost, CountBar, Diff, DiffGutter, DiffKind, DiffLine, DiffWrap, MotionMode, Paint, Receipt,
    ReceiptColumn, ReceiptDensity, ReceiptTable, ReceiptValue, Role, State, Theme, Toast, Toasts,
    TreeNode, TreeOutcome, TreeState, Ttl, WorkflowTree, diff_counts, format_bytes, format_cost,
    format_count, format_duration, format_tokens, gallery, parse_unified,
    testing::{self, Profile},
    tree_counts, unknown_value,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn render(component: &impl Paint, profile: Profile, width: u16, height: u16) -> Buffer {
    let theme = profile.theme();
    testing::render(width, height, |area, buf| {
        component.paint(area, buf, &theme)
    })
}

fn text_of(component: &impl Paint, profile: Profile, width: u16, height: u16) -> String {
    testing::text(&render(component, profile, width, height))
}

/// The row `y` of `buf` as cells (wide glyphs keep their first cell only).
fn row_cells(buf: &Buffer, y: u16) -> Vec<(u16, String)> {
    (0..buf.area.width)
        .map(|x| (x, buf[(x, y)].symbol().to_string()))
        .collect()
}

fn rightmost(buf: &Buffer, y: u16, symbol: &str) -> Option<u16> {
    row_cells(buf, y)
        .into_iter()
        .rev()
        .find(|(_, s)| s == symbol)
        .map(|(x, _)| x)
}

fn last_ink(buf: &Buffer, y: u16) -> Option<u16> {
    row_cells(buf, y)
        .into_iter()
        .rev()
        .find(|(_, s)| !s.trim().is_empty())
        .map(|(x, _)| x)
}

fn entry(name: &str) -> gallery::Entry {
    gallery::entries()
        .into_iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("gallery entry {name}"))
}

/// The rule check and the snapshot for a gallery entry, at its own height.
fn pinned(name: &str) {
    let e = entry(name);
    let paint = |area: Rect, buf: &mut Buffer, theme: &Theme| (e.draw)(area, buf, theme);
    testing::assert_rules(e.height, paint);
    insta::assert_snapshot!(name, testing::snapshot(e.height, paint));
}

/// One test per gallery entry: its frames keep the rules in every profile at
/// every width, and what it paints is pinned.
macro_rules! pinned_tests {
    ($($test:ident => $entry:literal),* $(,)?) => {
        $(
            #[test]
            fn $test() {
                pinned($entry);
            }
        )*
    };
}

pinned_tests! {
    receipt_row_frames => "receipt-row",
    receipt_table_frames => "receipt-table",
    receipt_table_compact_frames => "receipt-table-compact",
    receipt_table_minimal_frames => "receipt-table-minimal",
    receipt_table_clipped_frames => "receipt-table-clipped",
    diff_frames => "diff",
    diff_wrapped_frames => "diff-wrapped",
    diff_no_numbers_frames => "diff-no-numbers",
    diff_highlighted_frames => "diff-highlighted",
    workflow_tree_frames => "workflow-tree",
    workflow_tree_selected_frames => "workflow-tree-selected",
    workflow_tree_clipped_frames => "workflow-tree-clipped",
    workflow_tree_long_frames => "workflow-tree-long",
    count_bar_frames => "count-bars",
    toasts_stacked_frames => "toasts-stacked",
    toasts_fading_frames => "toasts-fading",
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

#[test]
fn durations_read_as_words_and_never_round_a_real_time_to_nothing() {
    assert_eq!(format_duration(Duration::ZERO), "0 ms");
    assert_eq!(format_duration(Duration::from_micros(400)), "<1 ms");
    assert_eq!(format_duration(ms(1)), "1 ms");
    assert_eq!(format_duration(ms(850)), "850 ms");
    assert_eq!(format_duration(ms(12_000)), "12 s");
    // Fractions are dropped, not rounded up into the next unit.
    assert_eq!(format_duration(ms(59_999)), "59 s");
    assert_eq!(format_duration(Duration::from_secs(60)), "1m 00s");
    assert_eq!(format_duration(Duration::from_secs(246)), "4m 06s");
    assert_eq!(format_duration(Duration::from_secs(3_599)), "59m 59s");
    assert_eq!(format_duration(Duration::from_secs(3_720)), "1h 02m");
    assert_eq!(format_duration(Duration::from_secs(90_000)), "1d 01h");
    // It agrees with the spinner's spelling where they overlap.
    for secs in [0, 7, 59, 61, 600, 3_700] {
        let d = Duration::from_secs(secs);
        if secs > 0 {
            assert_eq!(format_duration(d), codewhale_ratatui::duration(d));
        }
    }
    let _ = format_duration(Duration::MAX);
}

#[test]
fn token_counts_are_exact_below_a_thousand_and_compact_above() {
    assert_eq!(format_tokens(0), "0");
    assert_eq!(format_tokens(812), "812");
    assert_eq!(format_tokens(999), "999");
    assert_eq!(format_tokens(1_000), "1k");
    assert_eq!(format_tokens(12_345), "12.3k");
    assert_eq!(format_tokens(96_300), "96.3k");
    // Rounding must not show `1000k`: it is promoted.
    assert_eq!(format_tokens(999_949), "999.9k");
    assert_eq!(format_tokens(999_950), "1M");
    assert_eq!(format_tokens(1_000_000), "1M");
    assert_eq!(format_tokens(1_240_000), "1.2M");
    assert_eq!(format_tokens(1_999_999), "2M");
    assert_eq!(format_tokens(2_500_000_000), "2.5B");
    // Large values do not panic, and keep a unit.
    assert!(format_tokens(u64::MAX).ends_with('T'));
}

#[test]
fn counts_keep_every_digit_until_they_get_long() {
    assert_eq!(format_count(0), "0");
    assert_eq!(format_count(1_234), "1234", "no separators: locale-neutral");
    assert_eq!(format_count(9_999), "9999");
    assert_eq!(format_count(10_000), "10k");
}

#[test]
fn bytes_use_binary_units_and_promote_before_they_overflow_one() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1_023), "1023 B");
    assert_eq!(format_bytes(1_024), "1 KiB");
    assert_eq!(format_bytes(1_536), "1.5 KiB");
    assert_eq!(format_bytes(12 * 1_048_576), "12 MiB");
    // 1023.99 KiB would show as `1024 KiB`.
    assert_eq!(format_bytes(1_048_575), "1 MiB");
    assert!(format_bytes(u64::MAX).ends_with("EiB"));
}

#[test]
fn costs_keep_cheap_work_visible_and_zero_distinct() {
    assert_eq!(format_cost(0, "$"), "$0.00");
    assert_eq!(format_cost(49, "$"), "<$0.0001");
    assert_eq!(format_cost(100, "$"), "$0.0001");
    assert_eq!(format_cost(4_200, "$"), "$0.0042");
    assert_eq!(format_cost(9_949, "$"), "$0.0099");
    assert_eq!(format_cost(9_960, "$"), "$0.01");
    assert_eq!(format_cost(380_000, "$"), "$0.38");
    assert_eq!(format_cost(12_400_000, "€"), "€12.40");
    let _ = format_cost(u64::MAX, "$");
}

#[test]
fn unknown_is_a_mark_never_a_zero() {
    let none = [
        ReceiptValue::Duration(None),
        ReceiptValue::Tokens(None),
        ReceiptValue::Count(None),
        ReceiptValue::Bytes(None),
        ReceiptValue::Cost(None),
    ];
    for value in &none {
        assert!(value.is_unknown());
        assert_eq!(value.render(false), "\u{2014}");
        assert_eq!(value.render(true), "-");
    }
    assert_eq!(unknown_value(false), "\u{2014}");
    for zero in [
        ReceiptValue::Duration(Some(Duration::ZERO)),
        ReceiptValue::Tokens(Some(0)),
        ReceiptValue::Count(Some(0)),
        ReceiptValue::Bytes(Some(0)),
        ReceiptValue::Cost(Some(Cost::new(0, "$"))),
    ] {
        assert!(!zero.is_unknown());
        assert_ne!(zero.render(false), "\u{2014}", "a zero is a measurement");
        assert!(zero.render(false).contains('0'));
    }
    // An estimate says so, and a currency without an ASCII form drops its
    // symbol rather than printing a glyph ASCII cannot show.
    let estimate = ReceiptValue::Cost(Some(Cost::new(380_000, "€").estimate()));
    assert_eq!(estimate.render(false), "~€0.38");
    assert_eq!(estimate.render(true), "~0.38");
    let eur = ReceiptValue::Cost(Some(Cost::new(380_000, "€").ascii_symbol("EUR ")));
    assert_eq!(eur.render(true), "EUR 0.38");
}

// ---------------------------------------------------------------------------
// Receipts
// ---------------------------------------------------------------------------

fn secs(s: u64) -> ReceiptValue {
    ReceiptValue::Duration(Some(Duration::from_secs(s)))
}

fn table() -> ReceiptTable {
    let columns = vec![
        ReceiptColumn::new("Time").essential(),
        ReceiptColumn::new("Tokens"),
        ReceiptColumn::new("Cost").essential(),
    ];
    let rows = vec![
        Receipt::new(State::Done, "Edited summary.md").values([
            secs(4),
            ReceiptValue::Tokens(Some(1_240)),
            ReceiptValue::Cost(Some(Cost::usd_cents(1))),
        ]),
        Receipt::new(State::Failed, "Ran cargo test").values([
            secs(72),
            ReceiptValue::Tokens(Some(12_400)),
            ReceiptValue::Cost(None),
        ]),
        Receipt::new(State::Working, "Writing CHANGELOG.md").values([
            secs(38),
            ReceiptValue::Tokens(None),
            ReceiptValue::Cost(Some(Cost::new(382_000, "$").estimate())),
        ]),
        Receipt::new(State::Done, "Read 14 files").values([
            ReceiptValue::Duration(Some(ms(850))),
            ReceiptValue::Tokens(Some(96_300)),
            ReceiptValue::Cost(Some(Cost::usd_cents(1_240))),
        ]),
    ];
    ReceiptTable::new(columns, rows).label_header("Action")
}

#[test]
fn density_follows_width() {
    assert_eq!(ReceiptDensity::for_width(40), ReceiptDensity::Minimal);
    assert_eq!(ReceiptDensity::for_width(43), ReceiptDensity::Minimal);
    assert_eq!(ReceiptDensity::for_width(44), ReceiptDensity::Compact);
    assert_eq!(ReceiptDensity::for_width(63), ReceiptDensity::Compact);
    assert_eq!(ReceiptDensity::for_width(64), ReceiptDensity::Full);
    assert_eq!(ReceiptDensity::for_width(120), ReceiptDensity::Full);
}

#[test]
fn a_full_table_has_a_header_every_column_and_the_state_word() {
    let text = text_of(&table(), Profile::NoColor, 80, 8);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].contains("Action") && lines[0].contains("Outcome"));
    assert!(lines[0].contains("Time") && lines[0].contains("Tokens") && lines[0].contains("Cost"));
    assert!(text.contains("Done") && text.contains("Failed") && text.contains("Working"));
    assert!(text.contains("1.2k") && text.contains("12.4k") && text.contains("96.3k"));
}

#[test]
fn numbers_line_up_on_their_unit_and_decimal_point() {
    let t = table();
    // `Time` is right-aligned: every duration ends where the header does.
    let one_column = ReceiptTable::new(
        vec![ReceiptColumn::new("Time").essential()],
        t.rows
            .iter()
            .map(|r| {
                let mut r = r.clone();
                r.values.truncate(1);
                r
            })
            .collect(),
    )
    .density(ReceiptDensity::Full);
    let buf = render(&one_column, Profile::NoColor, 60, 5);
    let end = last_ink(&buf, 0).expect("header");
    for y in 1..5 {
        assert_eq!(
            last_ink(&buf, y),
            Some(end),
            "row {y}\n{}",
            testing::text(&buf)
        );
    }

    // `Cost` lines up on the decimal point; the unknown row has none, and
    // its rightmost point is the Tokens column's, further left.
    let buf = render(&t, Profile::NoColor, 80, 8);
    let points: Vec<u16> = (1..5).filter_map(|y| rightmost(&buf, y, ".")).collect();
    let cost_point = *points.iter().max().expect("a point");
    let aligned = points.iter().filter(|x| **x == cost_point).count();
    assert_eq!(aligned, 3, "{points:?}\n{}", testing::text(&buf));
}

#[test]
fn compact_keeps_the_essential_columns_and_the_word() {
    let text = text_of(
        &table().density(ReceiptDensity::Compact),
        Profile::NoColor,
        56,
        8,
    );
    assert!(
        !text.contains("Tokens") && !text.contains("12.4k"),
        "{text}"
    );
    assert!(text.contains("Done") && text.contains("4 s") && text.contains("$0.01"));
    assert!(!text.contains("Outcome"), "no header below Full: {text}");
}

#[test]
fn minimal_is_a_mark_a_label_and_one_number() {
    let text = text_of(&table(), Profile::NoColor, 40, 8);
    assert!(
        !text.contains("Outcome") && !text.contains("Done"),
        "{text}"
    );
    assert!(
        !text.contains("$"),
        "only the first essential column: {text}"
    );
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            line.trim_end().ends_with('s'),
            "a time ends every row: {line}"
        );
    }
    assert!(text.contains("Edited summary.md") && text.contains("4 s"));
}

#[test]
fn unknown_cells_show_the_mark_and_the_legend_names_it() {
    for (profile, mark) in [(Profile::DarkTrue, "\u{2014}"), (Profile::Ascii, "-")] {
        let text = text_of(&table(), profile, 100, 9);
        assert!(text.contains("Ran cargo test"), "{text}");
        let failed = text.lines().find(|l| l.contains("Ran cargo test")).unwrap();
        assert!(failed.contains(mark), "{failed}");
        assert!(!failed.contains("$0"), "unknown is not zero: {failed}");
        assert!(text.contains(&format!("{mark} not reported")), "{text}");
        assert!(text.contains("~ estimated"), "{text}");
    }
}

#[test]
fn totals_are_honest_about_rows_that_did_not_report() {
    let totals = table().totals().column_totals();
    // Time: every row reported, so it is a plain sum (4 + 72 + 38 s + 850 ms).
    let (time, partial) = totals[0].clone().expect("a time total");
    assert!(!partial);
    assert_eq!(time.render(false), "1m 54s");
    // Tokens: one row did not report, so the sum is a floor.
    let (tokens, partial) = totals[1].clone().expect("a token total");
    assert!(partial);
    assert_eq!(tokens.render(false), "109.9k");
    // Cost: partial and an estimate somewhere in it.
    let (cost, partial) = totals[2].clone().expect("a cost total");
    assert!(partial);
    assert_eq!(cost.render(false), "~$12.79");

    let text = text_of(&table().totals(), Profile::DarkTrue, 100, 9);
    assert!(text.contains("Total"), "{text}");
    assert!(text.contains("\u{2265}"), "{text}");
    assert!(text.contains("some rows did not report"), "{text}");
    let ascii = text_of(&table().totals(), Profile::Ascii, 100, 9);
    assert!(ascii.contains(">="), "{ascii}");

    // A column nobody reported totals to the unknown mark, not to zero.
    let none = ReceiptTable::new(
        vec![ReceiptColumn::new("Tokens").essential()],
        vec![
            Receipt::new(State::Done, "a").value(ReceiptValue::Tokens(None)),
            Receipt::new(State::Done, "b").value(ReceiptValue::Tokens(None)),
        ],
    );
    let (total, partial) = none.column_totals()[0].clone().unwrap();
    assert!(total.is_unknown() && !partial);

    // Unlike things have no total.
    let mixed = ReceiptTable::new(
        vec![ReceiptColumn::new("Cost")],
        vec![
            Receipt::new(State::Done, "a")
                .value(ReceiptValue::Cost(Some(Cost::new(1_000_000, "$")))),
            Receipt::new(State::Done, "b")
                .value(ReceiptValue::Cost(Some(Cost::new(1_000_000, "€")))),
        ],
    );
    assert!(mixed.column_totals()[0].is_none());
}

#[test]
fn rows_that_do_not_fit_say_how_many_with_the_real_number() {
    let t = table().totals();
    let text = text_of(&t, Profile::NoColor, 100, 5);
    // Header, one row, `+3 more`, totals and the legend: 4 rows hidden - 1.
    assert!(text.contains("+3 more"), "{text}");
    assert!(text.contains("Total"), "the totals row is kept: {text}");
    let all = text_of(&t, Profile::NoColor, 100, 9);
    assert!(!all.contains("more"), "{all}");
    assert_eq!(t.height(100, &Profile::NoColor.theme()), 7);
}

#[test]
fn a_missing_cell_is_unknown_not_blank() {
    let t = ReceiptTable::new(
        vec![ReceiptColumn::new("Time"), ReceiptColumn::new("Cost")],
        vec![Receipt::new(State::Done, "short").value(secs(3))],
    );
    let text = text_of(&t, Profile::NoColor, 80, 3);
    assert!(text.contains("3 s"));
    let row = text.lines().find(|l| l.contains("short")).unwrap();
    assert!(row.contains('\u{2014}'), "{row}");
}

#[test]
fn caller_text_in_a_receipt_cannot_reorder_itself() {
    let r = Receipt::new(State::Done, "rm -rf ~/\u{202E}txt.exe").word("Do\u{202E}ne");
    let text = text_of(&r, Profile::DarkTrue, 60, 1);
    assert!(
        text.contains("rm -rf ~/txt.exe") && text.contains("Done"),
        "{text}"
    );
    assert!(!text.contains('\u{202E}'));
}

#[test]
fn receipt_row_paints_in_the_state_hue() {
    let theme = Profile::DarkTrue.theme();
    let buf = render(
        &Receipt::new(State::Failed, "Ran cargo test").value(secs(72)),
        Profile::DarkTrue,
        40,
        1,
    );
    assert_eq!(buf[(0, 0)].symbol(), "\u{2715}");
    assert_eq!(buf[(0, 0)].fg, theme.fg(Role::Danger).fg.unwrap());
}

// ---------------------------------------------------------------------------
// Parsing a unified diff
// ---------------------------------------------------------------------------

const SAMPLE: &str = "\
diff --git a/src/old_name.rs b/src/new_name.rs
similarity index 88%
rename from src/old_name.rs
rename to src/new_name.rs
index 3f2a1c9..8b7d4e0 100644
--- a/src/old_name.rs
+++ b/src/new_name.rs
@@ -1,3 +1,3 @@ fn first() {
 fn first() {
-    one();
+    uno();
 }
--- note: a removed line that starts like a header
\\ No newline at end of file
diff --git a/img.png b/img.png
Binary files a/img.png and b/img.png differ
diff --git a/new.txt b/new.txt
new file mode 100644
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+hello
+world
\\ No newline at end of file
";

#[test]
fn parse_reads_renames_hunks_numbers_and_notes() {
    let lines = parse_unified(SAMPLE);
    let kinds: Vec<DiffKind> = lines.iter().map(|l| l.kind).collect();
    use DiffKind::*;
    assert_eq!(
        kinds,
        [
            FileHeader, FileHeader, FileHeader, FileHeader, FileHeader, FileHeader, FileHeader,
            HunkHeader, Context, Removed, Added, Context,    // the hunk
            FileHeader, // `--- note` after the hunk's counts ran out
            Note, FileHeader, FileHeader, FileHeader, FileHeader, FileHeader, FileHeader,
            HunkHeader, Added, Added, Note,
        ]
    );
    assert_eq!(lines[2].text, "rename from src/old_name.rs");
    // Numbers: context carries both, a removal the old, an addition the new.
    assert_eq!((lines[8].old_no, lines[8].new_no), (Some(1), Some(1)));
    assert_eq!((lines[9].old_no, lines[9].new_no), (Some(2), None));
    assert_eq!((lines[10].old_no, lines[10].new_no), (None, Some(2)));
    assert_eq!((lines[11].old_no, lines[11].new_no), (Some(3), Some(3)));
    assert_eq!(lines[9].text, "    one();");
    // A new file's lines count from 1 on the new side.
    let hello = lines.iter().find(|l| l.text == "hello").unwrap();
    assert_eq!((hello.old_no, hello.new_no), (None, Some(1)));
    assert_eq!(diff_counts(&lines), (3, 1));
}

#[test]
fn a_removed_line_that_reads_like_a_file_header_stays_a_removal() {
    // `--- note` inside a hunk is the removed line `-- note`.
    let diff = "@@ -1,2 +1,1 @@\n--- note\n keep\n";
    let lines = parse_unified(diff);
    assert_eq!(lines[1].kind, DiffKind::Removed);
    assert_eq!(lines[1].text, "-- note");
    assert_eq!(lines[2].kind, DiffKind::Context);
    // And an added `++ x` the same: `+++ x`.
    let lines = parse_unified("@@ -1,1 +1,2 @@\n keep\n+++ x\n");
    assert_eq!(lines[2].kind, DiffKind::Added);
    assert_eq!(lines[2].text, "++ x");
}

#[test]
fn no_newline_at_end_of_file_is_a_note_and_does_not_move_the_numbers() {
    let diff =
        "@@ -1,2 +1,2 @@\n-a\n\\ No newline at end of file\n+b\n\\ No newline at end of file\n c\n";
    let lines = parse_unified(diff);
    let kinds: Vec<_> = lines.iter().map(|l| l.kind).collect();
    use DiffKind::*;
    assert_eq!(kinds, [HunkHeader, Removed, Note, Added, Note, Context]);
    assert_eq!(lines[5].old_no, Some(2));
    assert_eq!(lines[5].new_no, Some(2));
}

#[test]
fn crlf_endings_are_dropped_and_tabs_are_kept_for_painting() {
    let diff = "@@ -1,2 +1,2 @@\r\n-\told\r\n+\tnew\r\n keep\r\n";
    let lines = parse_unified(diff);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].text, "@@ -1,2 +1,2 @@");
    assert_eq!(lines[1].text, "\told");
    assert_eq!(lines[2].text, "\tnew");
    assert_eq!(lines[3].text, "keep");
    assert!(lines.iter().all(|l| !l.text.contains('\r')));
}

#[test]
fn an_empty_context_line_that_an_editor_stripped_is_still_context() {
    let lines = parse_unified("@@ -1,3 +1,3 @@\n a\n\n c\n");
    assert_eq!(lines[2].kind, DiffKind::Context);
    assert_eq!(lines[2].text, "");
    assert_eq!(lines[3].old_no, Some(3));
}

#[test]
fn garbage_and_empty_input_never_panic() {
    assert!(parse_unified("").is_empty());
    for diff in [
        "@@ nonsense @@\n+x\n-y\n z\n",
        "@@ -a,b +c,d @@\n+x\n",
        "@@ -99999999999,1 +1,1 @@\n+x\n",
        "+++\n---\n\n\n\\\n",
        "@@ -1 +1 @@\n x\n",
    ] {
        let lines = parse_unified(diff);
        let _ = Diff::new(lines).height(40, &Profile::DarkTrue.theme());
    }
}

// ---------------------------------------------------------------------------
// Painting a diff
// ---------------------------------------------------------------------------

fn small_diff() -> Vec<DiffLine<'static>> {
    vec![
        DiffLine::file("src/lib.rs"),
        DiffLine::hunk("@@ -10,3 +10,3 @@ fn main() {"),
        DiffLine::context(10, 10, "fn main() {"),
        DiffLine::removed(11, "    let depth = 1;"),
        DiffLine::added(11, "    let depth = 2;"),
        DiffLine::context(12, 12, "}"),
    ]
}

#[test]
fn every_changed_line_carries_its_sign_without_color() {
    for (profile, minus) in [
        (Profile::NoColor, "\u{2212}"),
        (Profile::Ansi16, "\u{2212}"),
        (Profile::Ascii, "-"),
    ] {
        let diff = Diff::new(small_diff());
        let buf = render(&diff, profile, 60, 6);
        let text = testing::text(&buf);
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[3].contains(&format!("{minus} ")),
            "{profile:?}: {}",
            lines[3]
        );
        assert!(lines[4].contains("+ "), "{profile:?}: {}", lines[4]);
        // The context line has neither.
        assert!(
            !lines[2].contains('+') && !lines[2].contains(minus),
            "{}",
            lines[2]
        );
        // No ground: the sign is all there is, and it is enough.
        assert!(
            buf.content()
                .iter()
                .all(|c| c.bg == ratatui::style::Color::Reset)
        );
    }
    let ascii = text_of(&Diff::new(small_diff()), Profile::Ascii, 60, 6);
    assert!(ascii.is_ascii(), "{ascii}");
}

#[test]
fn the_two_tints_paint_behind_whole_rows_only_where_grounds_paint() {
    let rows = |profile: Profile| render(&Diff::new(small_diff()), profile, 60, 6);
    for profile in [Profile::DarkTrue, Profile::Dark256, Profile::LightTrue] {
        let theme = profile.theme();
        let buf = rows(profile);
        let added = theme.bg(Role::DiffAddedTint).bg.unwrap();
        let removed = theme.bg(Role::DiffRemovedTint).bg.unwrap();
        assert_ne!(added, removed);
        for x in 0..60 {
            assert_eq!(buf[(x, 4)].bg, added, "{profile:?} added row, col {x}");
            assert_eq!(buf[(x, 3)].bg, removed, "{profile:?} removed row, col {x}");
            assert_ne!(buf[(x, 2)].bg, added, "context is not tinted");
        }
    }
    for profile in [Profile::Ansi16, Profile::NoColor, Profile::UnknownGround] {
        let buf = rows(profile);
        assert!(
            buf.content()
                .iter()
                .all(|c| c.bg == ratatui::style::Color::Reset),
            "{profile:?} paints no tint"
        );
    }
}

#[test]
fn line_numbers_are_right_aligned_and_blank_on_the_side_a_line_lacks() {
    let diff = Diff::new(vec![
        DiffLine::context(9, 9, "a"),
        DiffLine::removed(10, "b"),
        DiffLine::added(10, "c"),
        DiffLine::context(100, 100, "d"),
    ])
    .gutter(DiffGutter::Both);
    let text = text_of(&diff, Profile::NoColor, 40, 4);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "  9   9   a");
    assert_eq!(lines[1], " 10     \u{2212} b");
    assert_eq!(lines[2], "     10 + c");
    assert_eq!(lines[3], "100 100   d");
    let single = text_of(
        &diff.clone().gutter(DiffGutter::Single),
        Profile::NoColor,
        40,
        4,
    );
    assert_eq!(single.lines().nth(1), Some(" 10 \u{2212} b"));
    assert_eq!(single.lines().nth(2), Some(" 10 + c"));
    let off = text_of(&diff.gutter(DiffGutter::Off), Profile::NoColor, 40, 4);
    assert_eq!(off.lines().nth(2), Some("+ c"));
}

#[test]
fn a_long_line_is_cut_with_a_mark_or_wrapped_whole() {
    let long = "abcdefghij".repeat(6);
    let line = DiffLine::added(1, &long);
    let cut = Diff::new(vec![line]).gutter(DiffGutter::Off);
    let unicode = text_of(&cut, Profile::DarkTrue, 20, 3);
    assert_eq!(unicode.lines().next(), Some("+ abcdefghijabcdefg\u{2026}"));
    assert_eq!(
        unicode.lines().filter(|l| !l.is_empty()).count(),
        1,
        "a cut line stays one row"
    );
    let ascii = text_of(&cut, Profile::Ascii, 20, 3);
    assert_eq!(ascii.lines().next(), Some("+ abcdefghijabcde..."));

    let wrapped = cut.clone().wrap(DiffWrap::Wrap);
    let theme = Profile::NoColor.theme();
    assert_eq!(cut.height(20, &theme), 1);
    assert_eq!(wrapped.height(20, &theme), 4, "60 cells in 18-cell rows");
    let text = text_of(&wrapped, Profile::NoColor, 20, 4);
    let joined: String = text.lines().map(|l| &l[2..]).collect();
    assert_eq!(joined, long, "wrapping loses nothing");
    // Every row of a changed line still carries its sign.
    assert!(text.lines().all(|l| l.starts_with("+ ")), "{text}");
}

#[test]
fn wide_text_is_never_split_across_rows_or_the_cut() {
    let cjk = "鲸鱼".repeat(10);
    let line = DiffLine::added(1, &cjk);
    let diff = Diff::new(vec![line]).gutter(DiffGutter::Off);
    for width in [10, 11, 15, 21] {
        let text = text_of(&diff, Profile::NoColor, width, 2);
        assert!(!text.contains('\u{fffd}'));
        let wrapped = text_of(
            &diff.clone().wrap(DiffWrap::Wrap),
            Profile::NoColor,
            width,
            12,
        );
        let joined: String = wrapped
            .lines()
            .map(|l| l.trim_start_matches("+ "))
            .collect();
        assert_eq!(joined, cjk, "width {width}");
    }
}

#[test]
fn tabs_expand_to_the_next_stop() {
    let diff = Diff::new(vec![DiffLine::added(1, "\tx"), DiffLine::added(2, "ab\tx")])
        .gutter(DiffGutter::Off)
        .tab_width(4);
    let text = text_of(&diff, Profile::NoColor, 20, 2);
    assert_eq!(text.lines().next(), Some("+     x"));
    assert_eq!(text.lines().nth(1), Some("+ ab  x"));
    let wide = text_of(&diff.tab_width(8), Profile::NoColor, 20, 2);
    assert_eq!(wide.lines().next(), Some("+         x"));
}

#[test]
fn hostile_text_is_neutralised_before_it_is_measured_or_painted() {
    let hostile = "let s = \"\u{1b}[31mred\u{1b}[0m\u{202E}evil\u{7}\"; \u{9b}2J";
    let line = DiffLine::removed(1, hostile);
    let diff = Diff::new(vec![DiffLine::file("a\u{1b}]0;title\u{7}b"), line]);
    for profile in Profile::ALL {
        let buf = render(&diff, profile, 60, 2);
        for cell in buf.content() {
            assert!(
                cell.symbol()
                    .chars()
                    .all(|c| !c.is_control() && c != '\u{202E}'),
                "{profile:?}: {:?}",
                cell.symbol()
            );
        }
    }
    let text = text_of(&diff, Profile::NoColor, 60, 2);
    // The escape is gone; what was inside it is now harmless visible text.
    assert!(text.contains("[31mred[0mevil"), "{text}");
    assert!(text.contains("ab") || text.contains("a]0;titleb"), "{text}");
}

fn bold_cells(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .filter(|x| buf[(*x, y)].modifier.contains(Modifier::BOLD))
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

#[test]
fn emphasis_ranges_keep_pointing_at_the_right_text_after_cleanup() {
    // The ESC is removed and the tab widened; the range still covers `cd`.
    let text = "a\u{1b}b\tcd";
    let at = text.find("cd").unwrap();
    let emphasis: Vec<_> = std::iter::once(at..at + 2).collect();
    let diff =
        Diff::new(vec![DiffLine::added(1, text).emphasis(&emphasis)]).gutter(DiffGutter::Off);
    for profile in [
        Profile::DarkTrue,
        Profile::Ansi16,
        Profile::NoColor,
        Profile::Ascii,
    ] {
        let buf = render(&diff, profile, 30, 1);
        // The sign is bold everywhere; the emphasis adds `cd`.
        assert_eq!(bold_cells(&buf, 0), "+cd", "{profile:?}");
    }
    // Without a ground the emphasis is also underlined.
    let buf = render(&diff, Profile::NoColor, 30, 1);
    let underlined: String = (0..30)
        .filter(|x| buf[(*x, 0)].modifier.contains(Modifier::UNDERLINED))
        .map(|x| buf[(x, 0)].symbol().to_string())
        .collect();
    assert_eq!(underlined, "cd");
    let buf = render(&diff, Profile::DarkTrue, 30, 1);
    assert!((0..30).all(|x| !buf[(x, 0)].modifier.contains(Modifier::UNDERLINED)));
}

#[test]
fn syntax_highlight_is_a_caller_hook_and_respects_the_tint_audit() {
    fn hook(line: &str) -> Vec<(std::ops::Range<usize>, Role)> {
        vec![(0..line.len().min(2), Role::Primary)]
    }
    let diff = Diff::new(vec![
        DiffLine::context(1, 1, "fn keep() {}"),
        DiffLine::added(2, "fn added() {}"),
    ])
    .gutter(DiffGutter::Off)
    .highlight(hook);
    let theme = Profile::DarkTrue.theme();
    let buf = render(&diff, Profile::DarkTrue, 30, 2);
    let primary = theme.fg(Role::Primary).fg.unwrap();
    let foreground = theme.fg(Role::Foreground).fg.unwrap();
    assert_eq!(
        buf[(2, 0)].fg,
        primary,
        "context lines take the hook's role"
    );
    assert_eq!(
        buf[(2, 1)].fg,
        foreground,
        "a tinted line is audited for Foreground only"
    );
    // Without a tint (no grounds) the hook's role is honoured on changed lines.
    let nocolor = render(&diff, Profile::Ansi16, 30, 2);
    assert_eq!(
        nocolor[(2, 1)].fg,
        Profile::Ansi16.theme().fg(Role::Primary).fg.unwrap()
    );
}

#[test]
fn a_clipped_diff_counts_the_lines_it_left_out() {
    let lines: Vec<DiffLine> = (1..=10).map(|n| DiffLine::added(n, "line")).collect();
    let diff = Diff::new(lines);
    let text = text_of(&diff, Profile::NoColor, 40, 4);
    assert_eq!(text.lines().last(), Some("+7 more"), "{text}");
    assert_eq!(text.lines().filter(|l| l.contains("line")).count(), 3);
    let scrolled = text_of(&diff.clone().scroll(5), Profile::NoColor, 40, 4);
    assert_eq!(scrolled.lines().last(), Some("+2 more"), "{scrolled}");
    assert!(
        text_of(&diff, Profile::NoColor, 40, 10)
            .lines()
            .all(|l| !l.contains("more"))
    );
    // One row shows the first line, not a lone count.
    assert_eq!(text_of(&diff, Profile::NoColor, 40, 1), " 1 + line");
}

/// A changed line says what it is at any width: number columns are shed before
/// the sign. The number gutter used to take a fixed five cells ahead of it, so
/// in 1 to 4 columns a NO_COLOR diff showed blanks or digits and no `+`/`-`.
#[test]
fn a_very_narrow_diff_keeps_the_sign_and_sheds_the_numbers() {
    let lines = vec![
        DiffLine::context(12345, 12345, "keep"),
        DiffLine::removed(12346, "gone"),
        DiffLine::added(12346, "new"),
    ];
    for (profile, minus) in [(Profile::NoColor, "\u{2212}"), (Profile::Ascii, "-")] {
        for gutter in [None, Some(DiffGutter::Both), Some(DiffGutter::Single)] {
            let mut diff = Diff::new(lines.clone()).wrap(DiffWrap::Truncate);
            diff.gutter = gutter;
            for width in 1..=4u16 {
                let text = text_of(&diff, profile, width, 3);
                let rows: Vec<&str> = text.lines().collect();
                let case = format!("{profile:?} {gutter:?} width {width}: {rows:?}");
                assert!(rows[1].starts_with(minus), "{case}");
                assert!(rows[2].starts_with('+'), "{case}");
                assert!(
                    !rows[0].contains('+') && !rows[0].contains(minus),
                    "context carries no sign: {case}"
                );
                assert!(
                    rows.iter().all(|r| !r.chars().any(|c| c.is_ascii_digit())),
                    "no number fits: {case}"
                );
                if width >= 3 {
                    assert!(rows[2].starts_with("+ n"), "{case}");
                }
            }
        }
    }
    // The widths the finding names, exactly.
    let diff = Diff::new(lines);
    assert_eq!(text_of(&diff, Profile::NoColor, 1, 3), "\n\u{2212}\n+");
    assert_eq!(
        text_of(&diff, Profile::NoColor, 3, 3).lines().nth(2),
        Some("+ n")
    );
    assert_eq!(
        text_of(&diff, Profile::NoColor, 4, 3).lines().nth(2),
        Some("+ n\u{2026}")
    );
    // A width that holds the numbers keeps them.
    assert_eq!(
        text_of(&diff, Profile::NoColor, 12, 3).lines().nth(2),
        Some("12346 + new")
    );
    // A note under a changed line shares the prefix and never outgrows it.
    let note = Diff::new(vec![DiffLine::new(DiffKind::Note, "\\ No newline")]);
    for width in 1..=4 {
        let buf = render(&note, Profile::NoColor, width, 1);
        assert_eq!(buf.area.width, width);
    }
}

/// `+N more` counts logical lines that are not fully on screen, so a wrapped
/// line cut off part-way is counted: it used to count every line with any row
/// shown and report `+0 more` for three hidden rows.
#[test]
fn a_partly_shown_wrapped_line_is_counted_as_left_out() {
    let long = "x".repeat(40);
    let diff = Diff::new(vec![DiffLine::added(1, &long)])
        .wrap(DiffWrap::Wrap)
        .gutter(DiffGutter::Off);
    // Eleven cells of text: 4 rows. Height 2: one row, then the count.
    let theme = Profile::NoColor.theme();
    assert_eq!(diff.height(13, &theme), 4);
    let text = text_of(&diff, Profile::NoColor, 13, 2);
    assert_eq!(text.lines().count(), 2, "{text}");
    assert_eq!(text.lines().last(), Some("+1 more"), "{text}");

    // The partly shown line and the lines after it, each counted once.
    let diff = Diff::new(vec![
        DiffLine::added(1, "first"),
        DiffLine::added(2, &long),
        DiffLine::added(3, "third"),
    ])
    .wrap(DiffWrap::Wrap)
    .gutter(DiffGutter::Off);
    let text = text_of(&diff, Profile::NoColor, 13, 3);
    assert!(text.contains("first"), "{text}");
    assert_eq!(text.lines().last(), Some("+2 more"), "{text}");
    // Exactly enough height: nothing is left out and no count is drawn.
    let rows = diff.height(13, &theme);
    let text = text_of(&diff, Profile::NoColor, 13, rows);
    assert!(!text.contains("more"), "{text}");
    // One row short: the count replaces the last row, so the long line loses
    // its final row and "third" goes too: two lines are not fully shown
    // (before, the long line counted as shown and this read "+1 more").
    let text = text_of(&diff, Profile::NoColor, 13, rows - 1);
    assert_eq!(text.lines().last(), Some("+2 more"), "{text}");
}

/// A rectangle that runs past the buffer is clipped to it before anything is
/// laid out, so the `+N more` row lands on the buffer's last row instead of
/// below it, and nothing is written outside the visible part.
#[test]
fn a_diff_is_clipped_to_the_buffer_before_it_is_laid_out() {
    let lines: Vec<DiffLine> = (1..=10).map(|n| DiffLine::added(n, "line")).collect();
    let diff = Diff::new(lines);
    let theme = Profile::NoColor.theme();
    // A buffer that does not start at the origin, four rows tall.
    let area = Rect::new(5, 3, 30, 4);
    let cases = [
        ("taller than the buffer", Rect::new(5, 3, 30, 10)),
        ("starting above and left of it", Rect::new(0, 0, 40, 12)),
        ("wider and taller", Rect::new(5, 3, 100, 100)),
    ];
    for (label, request) in cases {
        let before = Buffer::filled(area, ratatui::buffer::Cell::new("\u{b7}"));
        let mut buf = before.clone();
        diff.paint(request, &mut buf, &theme);
        let visible = request.intersection(area);
        let last = buf.area.bottom() - 1;
        let row = |y: u16| -> String {
            (area.left()..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        };
        assert!(
            row(last).trim_end().starts_with("+7 more"),
            "{label}: {}",
            row(last)
        );
        assert!(row(area.y).contains("line"), "{label}: {}", row(area.y));
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let inside = x >= visible.left()
                    && x < visible.right()
                    && y >= visible.top()
                    && y < visible.bottom();
                if !inside {
                    assert_eq!(
                        buf[(x, y)],
                        before[(x, y)],
                        "{label}: ({x}, {y}) was written"
                    );
                }
            }
        }
    }
    // Entirely outside, empty, and far away: nothing changes, nothing panics.
    for request in [
        Rect::new(40, 3, 10, 4),
        Rect::new(5, 7, 30, 4),
        Rect::new(0, 0, 4, 2),
        Rect::new(5, 3, 0, 4),
        Rect::new(5, 3, 30, 0),
        Rect::new(u16::MAX - 5, u16::MAX - 5, 5, 5),
    ] {
        let before = Buffer::filled(area, ratatui::buffer::Cell::new("\u{b7}"));
        let mut buf = before.clone();
        diff.paint(request, &mut buf, &theme);
        assert_eq!(buf, before, "{request:?}");
    }
}

// ---------------------------------------------------------------------------
// Tree
// ---------------------------------------------------------------------------

fn run() -> Vec<TreeNode> {
    vec![
        TreeNode::new(State::Working, "Release notes")
            .detail("4m 06s")
            .children([
                TreeNode::new(State::Done, "Gather")
                    .collapsed(true)
                    .children([
                        TreeNode::new(State::Done, "reader-1"),
                        TreeNode::new(State::Done, "reader-2")
                            .child(TreeNode::new(State::Failed, "retry")),
                    ]),
                TreeNode::new(State::Working, "Draft").children([
                    TreeNode::new(State::Working, "writer-1"),
                    TreeNode::new(State::Failed, "writer-2")
                        .detail("tests did not pass")
                        .opens(),
                    TreeNode::new(State::Ready, "writer-3"),
                ]),
                TreeNode::new(State::Ready, "Review"),
            ]),
    ]
}

fn press(state: &mut TreeState, nodes: &[TreeNode], code: KeyCode, rows: u16) -> TreeOutcome {
    state.handle_key(key(code), nodes, rows)
}

#[test]
fn keys_move_fold_and_open() {
    let nodes = run();
    let mut s = TreeState::new();
    // Visible: Release, Gather (folded), Draft, w1, w2, w3, Review = 7 rows.
    assert_eq!(s.visible_len(&nodes), 7);
    assert_eq!(press(&mut s, &nodes, KeyCode::Down, 10), TreeOutcome::Moved);
    assert_eq!(s.selected_path(&nodes), Some(vec![0, 0]));
    // Gather is folded: Right unfolds it, and the tree grows by its children.
    assert_eq!(
        press(&mut s, &nodes, KeyCode::Right, 10),
        TreeOutcome::Expanded
    );
    assert_eq!(s.visible_len(&nodes), 10);
    // Unfolded: Right steps into the first child.
    assert_eq!(
        press(&mut s, &nodes, KeyCode::Right, 10),
        TreeOutcome::Moved
    );
    assert_eq!(s.selected_path(&nodes), Some(vec![0, 0, 0]));
    // A leaf: Left steps out to the parent; Left again folds it.
    assert_eq!(press(&mut s, &nodes, KeyCode::Left, 10), TreeOutcome::Moved);
    assert_eq!(s.selected_path(&nodes), Some(vec![0, 0]));
    assert_eq!(
        press(&mut s, &nodes, KeyCode::Left, 10),
        TreeOutcome::Collapsed
    );
    assert_eq!(s.visible_len(&nodes), 7);
    // Space toggles, Enter opens, Esc cancels, a leaf ignores Right.
    assert_eq!(
        press(&mut s, &nodes, KeyCode::Char(' '), 10),
        TreeOutcome::Expanded
    );
    assert_eq!(
        press(&mut s, &nodes, KeyCode::Char(' '), 10),
        TreeOutcome::Collapsed
    );
    assert_eq!(
        press(&mut s, &nodes, KeyCode::Enter, 10),
        TreeOutcome::Opened(vec![0, 0])
    );
    assert_eq!(
        press(&mut s, &nodes, KeyCode::Esc, 10),
        TreeOutcome::Cancelled
    );
    assert_eq!(
        press(&mut s, &nodes, KeyCode::Char('q'), 10),
        TreeOutcome::Ignored
    );
    s.selected = 4; // writer-2, a leaf
    assert_eq!(
        press(&mut s, &nodes, KeyCode::Right, 10),
        TreeOutcome::Ignored
    );
    // Releases do nothing; Home and End go to the ends and stop there.
    let mut release = key(KeyCode::Down);
    release.kind = KeyEventKind::Release;
    assert_eq!(s.handle_key(release, &nodes, 10), TreeOutcome::Ignored);
    assert_eq!(press(&mut s, &nodes, KeyCode::End, 10), TreeOutcome::Moved);
    assert_eq!(
        press(&mut s, &nodes, KeyCode::Down, 10),
        TreeOutcome::Ignored
    );
    assert_eq!(press(&mut s, &nodes, KeyCode::Home, 10), TreeOutcome::Moved);
    assert_eq!(press(&mut s, &nodes, KeyCode::Up, 10), TreeOutcome::Ignored);
    // An empty tree ignores everything.
    assert_eq!(
        press(&mut TreeState::new(), &[], KeyCode::Down, 5),
        TreeOutcome::Ignored
    );
}

#[test]
fn the_viewport_always_holds_the_selection_and_the_counts_stay_exact() {
    let nodes: Vec<TreeNode> = (0..40)
        .map(|i| {
            TreeNode::new(State::Working, format!("job-{i}")).children([
                TreeNode::new(State::Done, format!("step-{i}-a")),
                TreeNode::new(State::Ready, format!("step-{i}-b")),
            ])
        })
        .collect();
    let keys = [
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::PageDown,
        KeyCode::Right,
        KeyCode::Down,
        KeyCode::PageDown,
        KeyCode::Char(' '),
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::Up,
        KeyCode::Left,
        KeyCode::Home,
        KeyCode::PageDown,
        KeyCode::Left,
        KeyCode::Right,
    ];
    for rows in [1u16, 2, 3, 4, 7, 200] {
        let mut s = TreeState::new();
        let mut seed = 7u32;
        for _ in 0..400 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let code = keys[(seed >> 16) as usize % keys.len()];
            s.handle_key(key(code), &nodes, rows);
            let len = s.visible_len(&nodes);
            assert!(
                s.selected < len,
                "rows {rows}: selected {} of {len}",
                s.selected
            );
            let (offset, shown) = s.window(len, usize::from(rows));
            assert!(
                offset <= s.selected && s.selected < offset + shown,
                "rows {rows}: {offset}+{shown} vs {}",
                s.selected
            );
            assert!(offset + shown <= len);
            assert_eq!(s.offset, offset, "the stored offset is the painted one");
            // The rows below the window are exactly the rows not shown.
            let below = len - offset - shown;
            if len > usize::from(rows) && shown < usize::from(rows) {
                assert!(below > 0, "a row spent on `+n more` means rows are hidden");
            } else if len <= usize::from(rows) {
                assert_eq!((offset, below), (0, 0), "everything fits");
            }
        }
    }
}

#[test]
fn a_window_that_ends_the_tree_gives_the_last_row_back() {
    let s = TreeState::new();
    assert_eq!(s.window(10, 10), (0, 10));
    assert_eq!(s.window(5, 10), (0, 5));
    assert_eq!(s.window(10, 4), (0, 3), "three rows and `+7 more`");
    let mut end = TreeState::new();
    end.selected = 9;
    assert_eq!(
        end.window(10, 4),
        (6, 4),
        "at the end all four rows are rows"
    );
    assert_eq!(s.window(10, 1), (0, 1));
    assert_eq!(s.window(0, 4), (0, 0));
}

#[test]
fn a_folded_parent_reports_every_node_it_hides_and_how_they_stand() {
    let nodes = run();
    let text = text_of(&WorkflowTree::new(&nodes), Profile::NoColor, 100, 8);
    let gather = text.lines().find(|l| l.contains("Gather")).unwrap();
    // reader-1, reader-2 and retry sit below Gather: 3 nodes, 2 done, 1 failed.
    assert!(gather.contains("+3 hidden"), "{gather}");
    assert!(
        gather.contains("1 failed") && gather.contains("2 done"),
        "{gather}"
    );
    assert!(!text.contains("reader-1"), "folded children are not drawn");
    // The folded breakdown always adds up to the hidden total.
    assert_eq!(1 + 2, nodes[0].children[0].descendants());
    // Unfolded by the person, they are drawn and the suffix goes.
    let mut s = TreeState::new();
    s.selected = 1;
    s.handle_key(key(KeyCode::Right), &nodes, 8);
    let text = text_of(
        &WorkflowTree::new(&nodes).state(&s),
        Profile::NoColor,
        100,
        9,
    );
    assert!(
        text.contains("reader-1") && !text.contains("hidden"),
        "{text}"
    );
}

#[test]
fn the_summary_counts_every_node_not_just_the_rows_that_fit() {
    let nodes = run();
    let counts = tree_counts(&nodes);
    assert_eq!(counts.iter().map(|(_, n)| n).sum::<usize>(), 10);
    assert_eq!(counts[0], (State::Failed, 2), "failures first");
    let text = text_of(
        &WorkflowTree::new(&nodes).summary(),
        Profile::NoColor,
        100,
        4,
    );
    let last = text.lines().last().unwrap();
    assert_eq!(
        last,
        "2 failed \u{b7} 3 working \u{b7} 2 ready \u{b7} 3 done"
    );
    assert!(text.lines().any(|l| l.contains("more")), "{text}");
}

#[test]
fn a_clipped_tree_says_how_many_rows_are_below_with_the_real_number() {
    let nodes: Vec<TreeNode> = (0..12)
        .map(|i| TreeNode::new(State::Ready, format!("n{i}")))
        .collect();
    let text = text_of(&WorkflowTree::new(&nodes), Profile::NoColor, 40, 5);
    assert!(text.lines().last().unwrap().contains("+8 more"), "{text}");
    assert_eq!(text.lines().filter(|l| l.contains(" n")).count(), 4);
    // Scrolled to the end, there is nothing below and every row is a row.
    let mut s = TreeState::new();
    s.selected = 11;
    s.scroll_into_view(12, 5);
    let text = text_of(
        &WorkflowTree::new(&nodes).state(&s),
        Profile::NoColor,
        40,
        5,
    );
    assert!(!text.contains("more"), "{text}");
    assert!(text.contains("n11"), "{text}");
}

#[test]
fn guides_have_an_ascii_form() {
    let nodes = run();
    let unicode = text_of(&WorkflowTree::new(&nodes), Profile::DarkTrue, 80, 9);
    assert!(
        unicode.contains("\u{251c} ") && unicode.contains("\u{2514} "),
        "{unicode}"
    );
    let ascii = text_of(&WorkflowTree::new(&nodes), Profile::Ascii, 80, 9);
    assert!(ascii.is_ascii(), "{ascii}");
    assert!(
        ascii.contains("+ ") && ascii.contains("\\ ") && ascii.contains("| "),
        "{ascii}"
    );
}

#[test]
fn every_row_has_its_mark_and_its_word_and_failures_carry_their_reason() {
    let nodes = run();
    let text = text_of(&WorkflowTree::new(&nodes), Profile::NoColor, 100, 9);
    let w2 = text.lines().find(|l| l.contains("writer-2")).unwrap();
    assert!(
        w2.contains('\u{2715}') && w2.contains("failed") && w2.contains("tests did not pass"),
        "{w2}"
    );
    assert!(w2.ends_with('\u{2192}'), "{w2}");
    let w1 = text.lines().find(|l| l.contains("writer-1")).unwrap();
    assert!(w1.contains('\u{25cf}') && w1.contains("working"), "{w1}");
}

#[test]
fn the_selected_row_is_marked_bold_and_grounded() {
    let nodes = run();
    let mut s = TreeState::new();
    s.selected = 2;
    let theme = Profile::DarkTrue.theme();
    let buf = render(
        &WorkflowTree::new(&nodes).state(&s),
        Profile::DarkTrue,
        80,
        8,
    );
    assert_eq!(buf[(0, 2)].symbol(), "\u{25b8}");
    assert_eq!(buf[(40, 2)].bg, theme.bg(Role::Selected).bg.unwrap());
    assert_eq!(buf[(0, 1)].symbol(), " ");
    // Without grounds the marker alone says which row.
    let text = text_of(
        &WorkflowTree::new(&nodes).state(&s),
        Profile::NoColor,
        80,
        8,
    );
    assert!(text.lines().nth(2).unwrap().starts_with('\u{25b8}'));
    let ascii = text_of(&WorkflowTree::new(&nodes).state(&s), Profile::Ascii, 80, 8);
    assert!(ascii.lines().nth(2).unwrap().starts_with('>'));
}

#[test]
fn deep_and_long_trees_never_overflow_their_area() {
    let mut node = TreeNode::new(State::Working, "leaf");
    for depth in 0..30 {
        node = TreeNode::new(
            State::Working,
            format!("level-{depth} with a long, long label"),
        )
        .child(node);
    }
    let nodes = vec![node];
    for width in [1u16, 5, 12, 40] {
        let text = text_of(
            &WorkflowTree::new(&nodes).summary(),
            Profile::DarkTrue,
            width,
            12,
        );
        for line in text.lines() {
            assert!(unicode_width_ok(line, width), "{width}: {line:?}");
        }
    }
}

fn unicode_width_ok(line: &str, width: u16) -> bool {
    line.chars().count() <= usize::from(width) * 2
}

#[test]
fn caller_text_in_a_tree_cannot_reorder_itself() {
    let nodes =
        vec![TreeNode::new(State::NeedsYou, "rm -rf ~/\u{202E}txt.exe").detail("a\u{1b}[31mb")];
    let text = text_of(&WorkflowTree::new(&nodes), Profile::DarkTrue, 80, 1);
    assert!(text.contains("rm -rf ~/txt.exe"), "{text}");
    assert!(!text.contains('\u{202E}') && !text.contains('\u{1b}'));
}

// ---------------------------------------------------------------------------
// Progress
// ---------------------------------------------------------------------------

#[test]
fn a_count_bar_says_the_count_in_words() {
    let bar = CountBar::new(3, 5).label("phases");
    let text = text_of(&bar, Profile::NoColor, 60, 1);
    assert!(text.contains("3 of 5 phases"), "{text}");
    assert!(text.contains("Working"));
    assert!(
        text.contains('[') && text.contains('\u{2588}') && text.contains('\u{2591}'),
        "{text}"
    );
    let ascii = text_of(&bar, Profile::Ascii, 60, 1);
    assert!(
        ascii.is_ascii() && ascii.contains('#') && ascii.contains(':'),
        "{ascii}"
    );
    // Narrow: the numbers outlast the unit.
    let narrow = text_of(&bar, Profile::NoColor, 14, 1);
    assert!(narrow.contains("3 of 5"), "{narrow}");
}

#[test]
fn an_unknown_total_gets_no_bar_and_no_percentage() {
    let bar = CountBar::unknown(12).label("files");
    for profile in Profile::ALL {
        let text = text_of(&bar, profile, 60, 1);
        assert!(
            text.contains("12 files so far, total unknown"),
            "{profile:?}: {text}"
        );
        assert!(
            !text.contains('%') && !text.contains('[') && !text.contains('/'),
            "{text}"
        );
        assert!(text.contains("Unknown"), "the state word: {text}");
    }
    assert_eq!(bar.filled(20), 0);
    // Narrow, the state word goes before the admission that the total is
    // unknown does.
    let forty = text_of(&bar, Profile::NoColor, 40, 1);
    assert!(forty.contains("12 files so far, total unknown"), "{forty}");
    let narrow = text_of(&bar, Profile::NoColor, 12, 1);
    assert!(narrow.contains("12"), "{narrow}");
}

#[test]
fn the_bar_never_shows_progress_that_has_not_happened() {
    for total in 1..=24u64 {
        for cells in 1..=30u16 {
            let mut last = 0;
            for done in 0..=total {
                let bar = CountBar::new(done, total);
                let filled = bar.filled(cells);
                assert!(filled >= last, "monotone");
                last = filled;
                if done < total {
                    assert!(
                        filled < cells,
                        "{done}/{total} in {cells}: full before it is"
                    );
                } else {
                    assert_eq!(filled, cells);
                }
                if done == 0 {
                    assert_eq!(filled, 0);
                }
            }
        }
    }
    // Past complete: the words keep the real numbers, the fill is clamped.
    let over = CountBar::new(7, 5).label("checks").state(State::NeedsYou);
    assert_eq!(over.filled(10), 10);
    assert!(text_of(&over, Profile::NoColor, 60, 1).contains("7 of 5 checks"));
    // A zero total paints no completed ink and is not "done".
    let zero = CountBar::new(0, 0).label("tasks").state(State::Ready);
    assert_eq!(zero.filled(10), 0);
    assert!(text_of(&zero, Profile::NoColor, 60, 1).contains("0 of 0 tasks"));
    // The state is the host's word: 5 of 5 is not announced as done by itself.
    assert!(text_of(&CountBar::new(5, 5), Profile::NoColor, 60, 1).contains("Working"));
}

#[test]
fn no_count_bar_ever_prints_a_percentage() {
    for bar in [
        CountBar::new(1, 3),
        CountBar::new(3, 3),
        CountBar::new(9, 3),
        CountBar::unknown(1),
    ] {
        for profile in Profile::ALL {
            for width in [8u16, 20, 40, 120] {
                assert!(!text_of(&bar, profile, width, 1).contains('%'));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Toasts
// ---------------------------------------------------------------------------

#[test]
fn failures_wait_to_be_seen_and_the_rest_time_out() {
    assert_eq!(Toast::new(State::Failed, "x").ttl, Ttl::UntilSeen);
    assert_eq!(Toast::new(State::NeedsYou, "x").ttl, Ttl::UntilSeen);
    assert_eq!(Toast::new(State::Done, "x").ttl, Ttl::Seconds(5));
}

#[test]
fn toasts_expire_when_the_host_says_so() {
    let t0 = Instant::now();
    let mut toasts = Toasts::new(vec![
        Toast::new(State::Done, "short")
            .born(t0)
            .ttl(Ttl::Seconds(2)),
        Toast::new(State::Done, "default").born(t0),
        Toast::new(State::Done, "unborn"),
        Toast::new(State::Failed, "failure").born(t0),
    ]);
    assert_eq!(toasts.retain_live(t0 + ms(1_999)), 0);
    assert_eq!(
        toasts.retain_live(t0 + ms(2_000)),
        1,
        "`short` goes at its ttl"
    );
    assert_eq!(toasts.retain_live(t0 + ms(4_999)), 0);
    assert_eq!(
        toasts.retain_live(t0 + ms(5_000)),
        1,
        "`default` goes at five seconds"
    );
    // The unborn toast never times out, and an unseen failure stays however
    // long it has been.
    assert_eq!(toasts.retain_live(t0 + Duration::from_secs(3_600)), 0);
    assert_eq!(toasts.items.len(), 2);
}

#[test]
fn a_failure_goes_a_moment_after_the_person_has_seen_it() {
    let t0 = Instant::now();
    let mut toasts = Toasts::new(vec![Toast::new(State::Failed, "failure").born(t0)]);
    // Painted into too few rows to show: not seen.
    let seen_at = t0 + Duration::from_secs(10);
    toasts.mark_seen(seen_at, 0);
    assert_eq!(toasts.retain_live(t0 + Duration::from_secs(60)), 0);
    toasts.mark_seen(seen_at, 3);
    assert_eq!(toasts.retain_live(seen_at + ms(1_999)), 0);
    assert_eq!(toasts.retain_live(seen_at + ms(2_000)), 1);
}

#[test]
fn only_the_toasts_that_were_on_screen_count_as_seen() {
    let t0 = Instant::now();
    let mut toasts = Toasts::new(
        (0..5)
            .map(|i| Toast::new(State::Failed, format!("f{i}")).born(t0))
            .collect(),
    );
    toasts.mark_seen(t0, 3); // window(3) = two toasts and `+3 more`
    let seen: Vec<bool> = toasts.items.iter().map(|t| t.seen.is_some()).collect();
    assert_eq!(seen, [false, false, false, true, true]);
}

fn stack(n: usize) -> Toasts {
    Toasts::new(
        (0..n)
            .map(|i| Toast::new(State::Done, format!("toast {i}")))
            .collect(),
    )
}

#[test]
fn a_stack_shows_the_newest_and_counts_the_rest() {
    let s = stack(6);
    assert_eq!(s.window(5), (3, 3, true));
    assert_eq!(s.window(3), (2, 4, true));
    assert_eq!(s.window(2), (1, 5, true));
    assert_eq!(
        s.window(1),
        (1, 5, false),
        "one row: the newest, count unspoken"
    );
    assert_eq!(s.window(0), (0, 0, false));
    assert_eq!(stack(3).window(3), (3, 0, false));
    assert_eq!(stack(0).window(3), (0, 0, false));
    let wide = stack(6).max_visible(10);
    assert_eq!(wide.window(10), (6, 0, false));
    assert_eq!(wide.window(4), (3, 3, true));
    assert_eq!(stack(2).max_visible(0).window(4), (0, 2, true));

    let text = text_of(&s, Profile::NoColor, 40, 5);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 5);
    assert!(lines[1].trim() == "+3 more", "{text}");
    assert!(
        lines[2].contains("toast 3")
            && lines[3].contains("toast 4")
            && lines[4].contains("toast 5")
    );
    assert!(!text.contains("toast 2"));
    assert_eq!(s.height(40, &Profile::NoColor.theme()), 4);
    assert_eq!(stack(2).height(40, &Profile::NoColor.theme()), 2);
}

#[test]
fn every_toast_is_anchored_to_the_bottom_right() {
    let buf = render(&stack(2), Profile::NoColor, 40, 4);
    for y in [2, 3] {
        assert_eq!(buf[(39, y)].symbol(), " ", "a trailing space of padding");
        assert!(last_ink(&buf, y).unwrap() >= 36);
    }
    assert!(buf[(0, 2)].symbol() == " ");
}

#[test]
fn ink_steps_in_and_out_only_under_full_motion() {
    let t0 = Instant::now();
    let toast = Toast::new(State::Done, "x").born(t0).ttl(Ttl::Seconds(1));
    let ink = |at: u64, motion| toast.ink(t0 + ms(at), motion);
    // Arriving.
    assert_eq!(ink(0, MotionMode::Full), Role::Dim);
    assert_eq!(ink(59, MotionMode::Full), Role::Dim);
    assert_eq!(ink(60, MotionMode::Full), Role::Hint);
    assert_eq!(ink(120, MotionMode::Full), Role::Muted);
    assert_eq!(ink(180, MotionMode::Full), Role::Foreground);
    assert_eq!(ink(500, MotionMode::Full), Role::Foreground);
    // Leaving (it expires at 1000 ms).
    assert_eq!(ink(700, MotionMode::Full), Role::Foreground);
    assert_eq!(ink(800, MotionMode::Full), Role::Muted);
    assert_eq!(ink(900, MotionMode::Full), Role::Hint);
    assert_eq!(ink(950, MotionMode::Full), Role::Dim);
    // Reduced and still motion show it settled, always.
    for at in [0, 70, 130, 900, 950] {
        assert_eq!(ink(at, MotionMode::Reduced), Role::Foreground);
        assert_eq!(ink(at, MotionMode::Still), Role::Foreground);
    }
    // No `born`, no fade.
    assert_eq!(
        Toast::new(State::Done, "x").ink(t0, MotionMode::Full),
        Role::Foreground
    );
    // A clock before `born` does not underflow.
    assert_eq!(
        toast.ink(t0.checked_sub(ms(10)).unwrap_or(t0), MotionMode::Full),
        Role::Dim
    );
}

#[test]
fn the_fade_reaches_the_painted_cells_through_roles() {
    let t0 = Instant::now();
    let theme = Profile::DarkTrue.theme();
    let toasts = |motion| {
        Toasts::new(vec![Toast::new(State::Done, "Saved").born(t0)])
            .at(t0 + ms(10))
            .motion(motion)
    };
    let sentence = |motion| {
        let buf = render(&toasts(motion), Profile::DarkTrue, 20, 1);
        // The `S` of `Saved`, found by symbol.
        let x = (0..20).find(|x| buf[(*x, 0)].symbol() == "S").unwrap();
        (buf[(x, 0)].fg, buf[(x - 2, 0)].fg)
    };
    let (word, mark) = sentence(MotionMode::Full);
    assert_eq!(word, theme.fg(Role::Dim).fg.unwrap());
    assert_eq!(
        mark,
        theme.fg(Role::Live).fg.unwrap(),
        "the mark keeps its hue"
    );
    let (word, _) = sentence(MotionMode::Reduced);
    assert_eq!(word, theme.fg(Role::Foreground).fg.unwrap());
    // Without `at` the host gave no clock, so nothing fades.
    let plain = Toasts::new(vec![Toast::new(State::Done, "Saved").born(t0)]);
    let buf = render(&plain, Profile::DarkTrue, 20, 1);
    let x = (0..20).find(|x| buf[(*x, 0)].symbol() == "S").unwrap();
    assert_eq!(buf[(x, 0)].fg, theme.fg(Role::Foreground).fg.unwrap());
}

#[test]
fn the_host_is_told_when_anything_next_changes() {
    let t0 = Instant::now();
    let one = |motion| {
        Toasts::new(vec![Toast::new(State::Done, "x").born(t0)])
            .motion(motion)
            .next_change_in(t0 + ms(10))
    };
    assert_eq!(one(MotionMode::Full), Some(ms(50)), "the next step of ink");
    assert_eq!(one(MotionMode::Reduced), Some(ms(4_990)), "only the expiry");
    assert_eq!(one(MotionMode::Still), Some(ms(4_990)));
    let idle = Toasts::new(vec![
        Toast::new(State::Done, "x"),
        Toast::new(State::Failed, "y").born(t0),
    ]);
    assert_eq!(
        idle.motion(MotionMode::Reduced).next_change_in(t0),
        None,
        "nothing will change"
    );
    assert_eq!(Toasts::default().next_change_in(t0), None);
}

#[test]
fn toasts_keep_their_old_api() {
    // The three-toast stack the gallery always showed still fits whole.
    let toasts = Toasts::new(vec![
        Toast::new(State::Done, "Theme set to Shoreline"),
        Toast::new(State::NeedsYou, "waiting").opens(),
        Toast::new(State::Failed, "Could not save").opens(),
    ]);
    assert_eq!(toasts.items.len(), 3);
    assert_eq!(toasts.height(60, &Profile::DarkTrue.theme()), 3);
    let text = text_of(&toasts, Profile::NoColor, 60, 3);
    assert_eq!(text.lines().count(), 3);
    assert!(!text.contains("more"));
}
