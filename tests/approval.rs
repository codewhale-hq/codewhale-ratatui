//! Package Approval: the approval card, the review verdict and the aggregate.
//!
//! This is the surface a person reads before allowing an agent to act, so the
//! tests are about what must never go wrong: the command shown is the command
//! that runs, nothing is decided by a stray key, and a card that cannot show
//! everything says so.

use codewhale_ratatui::{
    ApprovalCard, ApprovalChoice, ApprovalEffect, ApprovalKey, ApprovalKind, ApprovalOutcome,
    ApprovalPaint, ApprovalScope, ApprovalState, ApprovalSubject, ApprovalWords, ChoiceId, KeyHint,
    KeyHints, Paint, ReviewAggregate, ReviewKind, ReviewVerdict, ReviewWords, Theme,
    testing::{self, Profile},
    visible_text,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

const ALLOW_ONCE: ChoiceId = ChoiceId(1);
const ALLOW_SESSION: ChoiceId = ChoiceId(2);
const DENY: ChoiceId = ChoiceId(3);
const DENY_WHY: ChoiceId = ChoiceId(4);

fn choices() -> ApprovalState {
    ApprovalState::new(vec![
        ApprovalChoice::new(ALLOW_ONCE, "Allow once", ApprovalEffect::Grants)
            .char_key('y')
            .char_key('1'),
        ApprovalChoice::new(
            ALLOW_SESSION,
            "Allow for this session",
            ApprovalEffect::Grants,
        )
        .char_key('a')
        .char_key('2'),
        ApprovalChoice::new(DENY, "Deny", ApprovalEffect::Refuses)
            .char_key('n')
            .char_key('d'),
        ApprovalChoice::new(DENY_WHY, "Deny and say why", ApprovalEffect::Other).char_key('e'),
    ])
    .reveal(ApprovalKey::char('o'))
}

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn card_buf(
    subject: &ApprovalSubject,
    width: u16,
    height: u16,
    theme: &Theme,
) -> (Buffer, ApprovalPaint) {
    let state = choices();
    let card = ApprovalCard::new(subject, &state);
    let mut report = ApprovalPaint::default();
    let buf = testing::render(width, height, |area, buf| {
        report = card.render(area, buf, theme);
    });
    (buf, report)
}

/// The text of the subject rows as painted: the cells of the rows that start
/// at the `$` gutter and continue under it, taken from the bold cells so row
/// padding never reads as a space the subject did not have. Stops once
/// `want` characters are collected or the continuation ends.
fn shown_subject(buf: &Buffer, want: usize) -> String {
    let (x0, width) = (2u16, buf.area.width);
    let blank = |x: u16, y: u16| {
        buf[(x, y)].symbol() == " " && !buf[(x, y)].modifier.contains(Modifier::BOLD)
    };
    let Some(first) = (0..buf.area.height)
        .find(|&y| width > x0 && matches!(buf[(x0, y)].symbol(), "$" | "›" | ">"))
    else {
        return String::new();
    };
    let mut out = String::new();
    for y in first..buf.area.height {
        if y > first && !(blank(x0, y) && blank(x0 + 1, y)) {
            break;
        }
        for x in (x0 + 2)..width.saturating_sub(2) {
            let cell = &buf[(x, y)];
            if cell.modifier.contains(Modifier::BOLD) {
                out.push_str(cell.symbol());
            }
        }
        if out.chars().count() >= want {
            break;
        }
    }
    out
}

fn all_text(buf: &Buffer) -> String {
    testing::text(buf)
}

// ---------------------------------------------------------------------------
// Verbatim
// ---------------------------------------------------------------------------

/// A command that is long, has spaces to wrap at and spaces that must stay.
fn long_command() -> String {
    let mut s = String::from("cargo run --release --example gallery --  --dump  out/ ");
    for i in 0..40 {
        s.push_str(&format!("--flag-{i}=value{i} "));
    }
    s.push_str("--end");
    s
}

#[test]
fn a_long_command_is_shown_verbatim_at_every_width() {
    let theme = Profile::DarkTrue.theme();
    let cmd = long_command();
    let want = visible_text(&cmd, false);
    for width in [40, 80, 120] {
        let subject = ApprovalSubject::new(ApprovalKind::Command, cmd.as_str())
            .cwd("codewhale")
            .scope(ApprovalScope::Inside);
        let state = choices();
        let card = ApprovalCard::new(&subject, &state);
        let height = card.height(width, &theme);
        let (buf, report) = card_buf(&subject, width, height, &theme);
        assert!(!report.clipped(), "{width}: {report:?}");
        let shown = shown_subject(&buf, want.chars().count());
        assert_eq!(shown, want, "{width}: the subject as painted");
        assert!(!all_text(&buf).contains("not shown"), "{width}");
    }
}

#[test]
fn a_word_with_no_spaces_wraps_at_the_cell_and_loses_nothing() {
    let theme = Profile::DarkTrue.theme();
    let cmd = "/very/long/path/".repeat(25) + "end";
    let want = visible_text(&cmd, false);
    for width in [40, 80, 120] {
        let subject = ApprovalSubject::new(ApprovalKind::FileChange, cmd.as_str());
        let state = choices();
        let height = ApprovalCard::new(&subject, &state).height(width, &theme);
        let (buf, report) = card_buf(&subject, width, height, &theme);
        assert!(!report.clipped(), "{width}");
        assert_eq!(shown_subject(&buf, want.len()), want, "{width}");
    }
}

#[test]
fn a_card_that_cannot_show_it_all_says_how_much_and_how_to_see_it() {
    let theme = Profile::DarkTrue.theme();
    let cmd = long_command();
    let total = cmd.chars().count();
    for (width, height) in [(40u16, 22u16), (80, 17), (120, 15)] {
        let subject = ApprovalSubject::new(ApprovalKind::Command, cmd.as_str())
            .cwd("codewhale")
            .scope(ApprovalScope::Inside);
        let (buf, report) = card_buf(&subject, width, height, &theme);
        assert!(report.clipped(), "{width}x{height}");
        assert!(report.subject.any(), "{width}x{height}: {report:?}");
        let shown = shown_subject(&buf, usize::MAX);
        // Shown text is a prefix of the subject, never a rewrite of it.
        assert!(cmd.starts_with(&shown), "{width}x{height}: {shown:?}");
        assert_eq!(
            report.subject.chars,
            total - shown.chars().count(),
            "{width}x{height}: hidden characters are exactly the ones not drawn"
        );
        let text = all_text(&buf);
        assert!(
            text.contains(&format!("{} characters not shown", report.subject.chars)),
            "{width}x{height}: {text}"
        );
        assert!(text.contains("o show all"), "{width}x{height}: {text}");
        // The choices are still there: a clipped card does not drop them.
        assert!(text.contains("Allow once"), "{width}x{height}: {text}");
        assert!(text.contains("Deny"), "{width}x{height}: {text}");
        assert_eq!(report.choices, 0);
    }
}

#[test]
fn clipping_is_never_silent_even_with_no_room_for_a_notice() {
    let theme = Profile::DarkTrue.theme();
    let subject =
        ApprovalSubject::new(ApprovalKind::Command, long_command()).scope(ApprovalScope::Inside);
    for height in 0..=9 {
        let (_, report) = card_buf(&subject, 60, height, &theme);
        assert!(report.clipped(), "60x{height}: {report:?}");
    }
}

#[test]
fn a_card_that_shows_anything_and_cut_something_says_so() {
    // At every height where the panel leaves a row of body, a cut is
    // announced: a card must never show its choices and hide what they are
    // for without saying so.
    let subject = ApprovalSubject::new(ApprovalKind::Command, long_command())
        .cwd("/work/project")
        .scope(ApprovalScope::Outside)
        .agent("builder", true)
        .risk_note("deletes files")
        .preview_line("+x");
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [40u16, 60, 80, 120] {
            for height in 7..=40 {
                let (buf, report) = card_buf(&subject, width, height, &theme);
                let text = all_text(&buf);
                if !report.clipped() {
                    continue;
                }
                assert!(
                    text.contains("not shown"),
                    "{} {width}x{height}: cut and silent\n{text}",
                    profile.name()
                );
            }
        }
    }
}

#[test]
fn the_natural_height_shows_everything_in_every_profile() {
    let subject = ApprovalSubject::new(ApprovalKind::FileChange, "src/main.rs")
        .cwd("/work/project")
        .scope(ApprovalScope::Outside)
        .agent("builder", true)
        .risk_note("writes outside the project")
        .preview_line("+fn main() {}");
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in 12..=130 {
            let state = choices();
            let card = ApprovalCard::new(&subject, &state);
            let height = card.height(width, &theme);
            let (_, report) = card_buf(&subject, width, height, &theme);
            assert!(
                !report.clipped(),
                "{} {width}x{height}: {report:?}",
                profile.name()
            );
        }
    }
}

/// `height` is what the surface really needs: Panel(Overlay) chrome (edge,
/// padding, title row), the `Why: ` and `Risk: ` prefixes, wrapped hints. An
/// understated height clips the choices off the card, so it is checked
/// against the real render for every kind, profile and width, and for the
/// verdict as well as the card.
#[test]
fn height_never_understates_the_card_or_the_verdict() {
    let long = long_command();
    let subjects = [
        ApprovalSubject::new(ApprovalKind::Command, "ls"),
        ApprovalSubject::new(ApprovalKind::Command, long.as_str())
            .cwd("/work/project/with/a/long/path/name")
            .scope(ApprovalScope::Outside)
            .agent("builder", true)
            .risk_note("writes outside the project and reads the network at the same time"),
        ApprovalSubject::new(ApprovalKind::FileChange, "src/\u{9c8d}\u{8bb0}\u{5f55}.rs")
            .preview_line("+fn main() {}")
            .preview_line("-fn old() {}")
            .risk_note("overwrites a file"),
        ApprovalSubject::new(ApprovalKind::Network, "https://example.com/a/b/c")
            .scope(ApprovalScope::Inside),
        ApprovalSubject::new(ApprovalKind::ToolCall, "mcp__server__tool").agent("planner", false),
        ApprovalSubject::new(ApprovalKind::Elevation, "sudo make install")
            .scope(ApprovalScope::Outside),
    ];
    for profile in [Profile::DarkTrue, Profile::NoColor, Profile::Ascii] {
        let theme = profile.theme();
        for (n, subject) in subjects.iter().enumerate() {
            for width in 8..=130u16 {
                let state = choices();
                let card = ApprovalCard::new(subject, &state);
                let height = card.height(width, &theme);
                let (_, report) = card_buf(subject, width, height, &theme);
                assert!(
                    !report.clipped(),
                    "card {n} {} {width}x{height}: {report:?}",
                    profile.name()
                );
            }
        }
    }
    let hints = hints();
    for profile in [Profile::DarkTrue, Profile::NoColor, Profile::Ascii] {
        let theme = profile.theme();
        for kind in ReviewKind::ALL {
            let verdicts = [
                verdict_fixture(kind, &hints),
                verdict_fixture(kind, &hints)
                    .category("Deletes unseen files outside the project directory")
                    .id("ar_7K2M"),
                ReviewVerdict::new(
                    kind,
                    "Auto-Review",
                    "a reason long enough to wrap at narrow widths and then some more",
                    &hints,
                    None,
                )
                .unwrap()
                .subject(long.as_str()),
            ];
            for (n, verdict) in verdicts.iter().enumerate() {
                for width in 8..=130u16 {
                    let height = verdict.height(width, &theme);
                    let mut report = ApprovalPaint::default();
                    let _ = testing::render(width, height, |a, b| {
                        report = verdict.render(a, b, &theme);
                    });
                    assert!(
                        !report.clipped(),
                        "verdict {kind:?} {n} {} {width}x{height}: {report:?}",
                        profile.name()
                    );
                }
            }
        }
    }
}

/// A choice is shown whole (its key and its label, in the cells, wrapped if
/// need be) or the paint report says `choices > 0`. A choice cut at the right
/// edge or cut off its last rows is one the person is still offered but cannot
/// read.
#[test]
fn a_choice_is_either_shown_whole_or_reported_cut() {
    // Wrapping and the panel's side edges add whitespace and `|` between rows.
    let strip = |s: &str| -> String {
        s.chars()
            .filter(|c| !c.is_whitespace() && !matches!(c, '\u{2502}' | '|'))
            .collect()
    };
    for profile in [Profile::DarkTrue, Profile::NoColor, Profile::Ascii] {
        let theme = profile.theme();
        let state = choices();
        let wanted: Vec<String> = state
            .choices()
            .iter()
            .map(|c| {
                let key = match c.keys.first() {
                    Some(k) => k.label(codewhale_ratatui::keys::Platform::current(theme.ascii())),
                    None => String::new(),
                };
                strip(&format!("{key}{}", c.label))
            })
            .collect();
        let subject = ApprovalSubject::new(ApprovalKind::Command, "cargo test")
            .cwd("/work/project")
            .scope(ApprovalScope::Inside);
        for width in 4..=60u16 {
            for height in 1..=24u16 {
                let (buf, report) = card_buf(&subject, width, height, &theme);
                let text = strip(&all_text(&buf));
                let cut: Vec<usize> = (0..wanted.len())
                    .filter(|&i| !text.contains(wanted[i].as_str()))
                    .collect();
                assert!(
                    cut.is_empty() || report.choices > 0,
                    "{} {width}x{height}: choices {cut:?} are cut but the report says none: {report:?}\n{}",
                    profile.name(),
                    all_text(&buf)
                );
            }
        }
    }
}

/// The same for a verdict, whose keys are the hints it was given: a hint is
/// shown whole or `choices` counts rows that were not.
#[test]
fn a_verdict_hint_is_either_shown_whole_or_reported_cut() {
    let strip = |s: &str| -> String {
        s.chars()
            .filter(|c| !c.is_whitespace() && !matches!(c, '\u{2502}' | '|'))
            .collect()
    };
    let hints = hints();
    for profile in [Profile::DarkTrue, Profile::NoColor, Profile::Ascii] {
        let theme = profile.theme();
        for kind in [ReviewKind::Held, ReviewKind::Denied] {
            let verdict = verdict_fixture(kind, &hints);
            for width in 4..=60u16 {
                for height in 1..=14u16 {
                    let mut report = ApprovalPaint::default();
                    let buf = testing::render(width, height, |a, b| {
                        report = verdict.render(a, b, &theme);
                    });
                    let text = strip(&all_text(&buf));
                    let cut = hints
                        .items
                        .iter()
                        .any(|h| !text.contains(&strip(&format!("{}{}", h.keys, h.verb))));
                    assert!(
                        !cut || report.choices > 0,
                        "{kind:?} {} {width}x{height}: a hint is cut but the report says none: {report:?}\n{}",
                        profile.name(),
                        all_text(&buf)
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Spoofing: what is shown is what runs
// ---------------------------------------------------------------------------

/// Paint `target` in every non-ASCII profile at 60 columns and check each
/// frame shows `contains` and shows the subject verbatim.
fn shows(target: &str, contains: &[&str]) {
    let want = visible_text(target, false);
    for profile in Profile::ALL {
        if profile == Profile::Ascii {
            continue;
        }
        let theme = profile.theme();
        let subject =
            ApprovalSubject::new(ApprovalKind::Command, target).scope(ApprovalScope::Inside);
        let state = choices();
        let height = ApprovalCard::new(&subject, &state).height(60, &theme);
        let (buf, report) = card_buf(&subject, 60, height, &theme);
        assert!(!report.clipped(), "{}: {report:?}", profile.name());
        let text = all_text(&buf);
        for needle in contains {
            assert!(
                text.contains(needle),
                "{}: missing {needle:?}\n{text}",
                profile.name()
            );
        }
        assert_eq!(
            shown_subject(&buf, want.chars().count()),
            want,
            "{}",
            profile.name()
        );
        // Nothing the terminal could act on reaches a cell.
        for cell in buf.content() {
            for c in cell.symbol().chars() {
                assert!(
                    !c.is_control()
                        && !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200B}'..='\u{200F}'),
                    "{}: {c:?} reached a cell",
                    profile.name()
                );
            }
        }
    }
}

#[test]
fn an_escape_sequence_is_drawn_not_sent() {
    shows(
        "echo \u{1b}[31mred\u{1b}[0m",
        &["echo ‹ESC›[31mred‹ESC›[0m"],
    );
}

#[test]
fn a_carriage_return_cannot_overwrite_the_line() {
    shows("echo safe\rrm -rf ~", &["echo safe‹CR›rm -rf ~"]);
    shows("echo safe\r\nrm -rf ~", &["echo safe‹CR›‹LF›rm -rf ~"]);
}

#[test]
fn a_bidi_override_is_drawn_not_applied() {
    shows("rm -rf ~/\u{202e}txt.exe", &["rm -rf ~/‹RLO›txt.exe"]);
    shows(
        "a\u{2066}b\u{2067}c\u{2068}d\u{2069}e\u{202a}f\u{202b}g\u{202c}h\u{202d}i",
        &[
            "‹LRI›",
            "‹RLI›",
            "‹FSI›",
            "‹PDI›",
            "‹LRE›",
            "‹RLE›",
            "‹PDF›",
            "‹LRO›",
        ],
    );
}

#[test]
fn zero_width_and_invisible_characters_are_drawn() {
    shows("ls\u{200b}-la", &["ls‹ZWSP›-la"]);
    shows(
        "a\u{200d}b\u{200c}c\u{2060}d\u{feff}e",
        &["‹ZWJ›", "‹ZWNJ›", "‹WJ›", "‹BOM›"],
    );
    shows(
        "a\u{00a0}b\u{3000}c\u{2800}d",
        &["a‹NBSP›b‹U+3000›c‹U+2800›d"],
    );
    shows("a\u{fe0f}b\u{e0041}c", &["‹U+FE0F›", "‹U+E0041›"]);
    // A lone combining mark has no width: it is drawn, not hidden.
    shows("a b\u{0301}", &["a b\u{0301}"]);
    shows(" \u{0301}x", &["‹SP›‹U+0301›x"]);
}

#[test]
fn a_tab_is_drawn() {
    shows("cat\tfile", &["cat‹HT›file"]);
}

#[test]
fn leading_and_trailing_spaces_are_drawn() {
    shows("ls -la   ", &["ls -la‹SP×3›"]);
    shows("  ls", &["‹SP×2›ls"]);
    shows(" x ", &["‹SP›x‹SP›"]);
    shows("   ", &["‹SP×3›"]);
    // Spaces between words are ordinary and stay ordinary.
    shows("a  b", &["a  b"]);
}

#[test]
fn a_newline_cannot_start_a_second_command_unseen() {
    let theme = Profile::DarkTrue.theme();
    let target = "echo hi\nrm -rf ~";
    let subject = ApprovalSubject::new(ApprovalKind::Command, target);
    let (buf, _) = card_buf(&subject, 60, 14, &theme);
    let text = all_text(&buf);
    assert!(text.contains("echo hi‹LF›rm -rf ~"), "{text}");
    assert!(
        !text
            .lines()
            .any(|l| l.trim_start_matches(['│', ' ']).starts_with("rm -rf")),
        "the second command must not sit on a row of its own\n{text}"
    );
    shows(target, &["echo hi‹LF›rm -rf ~"]);
}

#[test]
fn text_that_looks_like_a_token_cannot_pass_for_one() {
    // The real escape and the characters "‹ESC›" must draw differently, or a
    // command could print the token and hide the character.
    let real = "echo \u{1b}";
    let fake = "echo ‹ESC›";
    assert_eq!(visible_text(real, false), "echo ‹ESC›");
    assert_eq!(visible_text(fake, false), "echo ‹U+2039›ESC›");
    assert_ne!(visible_text(real, false), visible_text(fake, false));
    // A lone `‹` that is not the start of a token stays itself.
    assert_eq!(visible_text("a ‹ b", false), "a ‹ b");
    assert_eq!(visible_text("‹SP×3›", false), "‹U+2039›SP×3›");
    // ASCII: the same, with `<` and `>`.
    assert_eq!(visible_text("echo \u{1b}", true), "echo <ESC>");
    assert_eq!(visible_text("echo <ESC>", true), "echo <U+003C>ESC>");
    assert_eq!(visible_text("a  \u{200d}  ", true), "a  <ZWJ><SPx2>");
    // Ordinary shell punctuation is untouched in ASCII.
    let shell = "cat < in > out; echo <<EOF; a<b>c; [ -f x ]";
    assert_eq!(visible_text(shell, true), shell);
    shows(fake, &["echo ‹U+2039›ESC›"]);
}

#[test]
fn every_character_class_decodes_back_to_one_subject() {
    // The encoding is one-to-one: decoding the tokens recovers the source.
    fn decode(s: &str) -> String {
        let chars: Vec<char> = s.chars().collect();
        let name_char =
            |c: char| c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, '+' | '×' | 'x');
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '‹' {
                let mut j = i + 1;
                while j < chars.len() && name_char(chars[j]) {
                    j += 1;
                }
                if j > i + 1 && j < chars.len() && chars[j] == '›' {
                    let name: String = chars[i + 1..j].iter().collect();
                    match name.as_str() {
                        "ESC" => out.push('\u{1b}'),
                        "HT" => out.push('\t'),
                        "LF" => out.push('\n'),
                        "CR" => out.push('\r'),
                        "RLO" => out.push('\u{202e}'),
                        "ZWJ" => out.push('\u{200d}'),
                        "ZWSP" => out.push('\u{200b}'),
                        "SP" => out.push(' '),
                        _ if name.starts_with("SP×") => {
                            out.push_str(&" ".repeat(name["SP×".len()..].parse().unwrap()));
                        }
                        _ if name.starts_with("U+") => out.push(
                            char::from_u32(u32::from_str_radix(&name[2..], 16).unwrap()).unwrap(),
                        ),
                        other => panic!("unknown token {other}"),
                    }
                    i = j + 1;
                    continue;
                }
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }
    for src in [
        "plain",
        "  lead",
        "trail   ",
        "a\u{1b}[0m\t\r\n\u{202e}\u{200d}b",
        "echo ‹ESC› and ‹ and › and ‹SP×3›",
        "‹",
        "‹A",
        "‹A›",
        "x\u{200b}y\u{0301}",
        "  ‹ESC›  ",
        "",
    ] {
        assert_eq!(decode(&visible_text(src, false)), src, "{src:?}");
    }
}

#[test]
fn homoglyph_text_is_flagged_and_counted() {
    // Cyrillic `а` (U+0430) looks like Latin `a`.
    let target = "g\u{0430}it push";
    for profile in Profile::ALL {
        if profile == Profile::Ascii {
            continue;
        }
        let theme = profile.theme();
        let subject = ApprovalSubject::new(ApprovalKind::Command, target);
        let state = choices();
        let height = ApprovalCard::new(&subject, &state).height(70, &theme);
        let (buf, report) = card_buf(&subject, 70, height, &theme);
        assert_eq!(report.non_ascii, 1, "{}", profile.name());
        let text = all_text(&buf);
        assert!(
            text.contains("Contains 1 non-ASCII character: check it"),
            "{}\n{text}",
            profile.name()
        );
        assert!(!report.clipped(), "{}", profile.name());
    }
    // A host whose people write in other scripts can turn the note off.
    let theme = Profile::DarkTrue.theme();
    let subject = ApprovalSubject::new(ApprovalKind::Command, target);
    let state = choices();
    let card = ApprovalCard::new(&subject, &state).flag_non_ascii(false);
    let buf = testing::render(70, 14, |a, b| card.paint(a, b, &theme));
    assert!(!all_text(&buf).contains("non-ASCII"));
    // The agent's name and the directory are untrusted too.
    let subject = ApprovalSubject::new(ApprovalKind::Command, "ls")
        .agent("buil\u{202e}der", true)
        .cwd("/tmp/\u{200b}x");
    let (buf, _) = card_buf(&subject, 70, 16, &theme);
    let text = all_text(&buf);
    assert!(text.contains("buil‹RLO›der"), "{text}");
    assert!(text.contains("/tmp/‹ZWSP›x"), "{text}");
}

#[test]
fn the_ascii_profile_stays_ascii_and_still_shows_every_token() {
    let theme = Profile::Ascii.theme();
    let target = "echo \u{1b}[31m\u{202e}x\r\n\ty  ";
    let subject = ApprovalSubject::new(ApprovalKind::Command, target)
        .cwd("/work")
        .scope(ApprovalScope::Outside)
        .agent("builder", true);
    let state = choices();
    let height = ApprovalCard::new(&subject, &state).height(60, &theme);
    let (buf, report) = card_buf(&subject, 60, height, &theme);
    let text = all_text(&buf);
    assert!(text.is_ascii(), "{text}");
    assert!(
        text.contains("echo <ESC>[31m<RLO>x<CR><LF><HT>y<SPx2>"),
        "{text}"
    );
    assert!(!report.clipped());
}

#[test]
fn tokens_are_told_from_text_without_color() {
    // Reversed and bold in every profile: shape and video, not hue.
    for profile in Profile::ALL {
        let theme = profile.theme();
        let subject = ApprovalSubject::new(ApprovalKind::Command, "a\u{1b}b");
        let (buf, _) = card_buf(&subject, 40, 14, &theme);
        let token_cells: Vec<_> = buf
            .content()
            .iter()
            .filter(|c| c.modifier.contains(Modifier::REVERSED))
            .map(|c| c.symbol().to_string())
            .collect();
        assert_eq!(
            token_cells.concat(),
            if profile == Profile::Ascii {
                "<ESC>"
            } else {
                "‹ESC›"
            },
            "{}",
            profile.name()
        );
    }
}

// ---------------------------------------------------------------------------
// Location, elevation, agent
// ---------------------------------------------------------------------------

#[test]
fn outside_the_project_and_elevation_carry_a_mark_and_words() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        let mark = if profile == Profile::Ascii {
            "!"
        } else {
            "⚠"
        };
        let outside = ApprovalSubject::new(ApprovalKind::Command, "rm -rf ../x")
            .cwd("/work/app")
            .scope(ApprovalScope::Outside);
        let (buf, _) = card_buf(&outside, 60, 16, &theme);
        let text = all_text(&buf);
        assert!(
            text.contains(&format!("{mark} Outside this project")),
            "{}\n{text}",
            profile.name()
        );
        assert!(text.contains("In /work/app"), "{}", profile.name());

        let inside = ApprovalSubject::new(ApprovalKind::Command, "ls").scope(ApprovalScope::Inside);
        let (buf, _) = card_buf(&inside, 60, 16, &theme);
        let text = all_text(&buf);
        assert!(text.contains("Inside this project"), "{}", profile.name());
        assert!(
            !text.contains(mark),
            "{}: inside has no warning mark",
            profile.name()
        );

        let unchecked = ApprovalSubject::new(ApprovalKind::Command, "ls");
        let (buf, _) = card_buf(&unchecked, 60, 16, &theme);
        assert!(
            all_text(&buf).contains("? Not checked against the project"),
            "{}: never assumed inside",
            profile.name()
        );

        let elevated = ApprovalSubject::new(ApprovalKind::Elevation, "npm i -g x")
            .scope(ApprovalScope::Inside);
        let (buf, _) = card_buf(&elevated, 60, 16, &theme);
        let text = all_text(&buf);
        assert!(
            text.contains(&format!("{mark} Runs with elevated access")),
            "{}\n{text}",
            profile.name()
        );
        assert!(text.contains("Raise access"), "{}", profile.name());
    }
}

#[test]
fn the_agent_is_named_and_a_sub_agent_says_so() {
    let theme = Profile::DarkTrue.theme();
    let sub = ApprovalSubject::new(ApprovalKind::Command, "ls").agent("reviewer-3", true);
    let (buf, _) = card_buf(&sub, 60, 16, &theme);
    assert!(all_text(&buf).contains("Requested by reviewer-3 (sub-agent)"));
    let root = ApprovalSubject::new(ApprovalKind::Command, "ls").agent("planner", false);
    let (buf, _) = card_buf(&root, 60, 16, &theme);
    let text = all_text(&buf);
    assert!(text.contains("Requested by planner"));
    assert!(!text.contains("sub-agent"));
}

#[test]
fn risk_notes_and_preview_lines_are_shown_and_counted_when_cut() {
    let theme = Profile::DarkTrue.theme();
    let subject = ApprovalSubject::new(ApprovalKind::FileChange, "a.rs")
        .scope(ApprovalScope::Inside)
        .risk_note("overwrites a.rs")
        .preview_line("-old")
        .preview_line("+new\u{1b}[2J")
        .preview_line("+more");
    let (buf, report) = card_buf(&subject, 60, 20, &theme);
    let text = all_text(&buf);
    assert!(text.contains("Risk: overwrites a.rs"), "{text}");
    assert!(text.contains("│ +new‹ESC›[2J"), "{text}");
    assert!(!report.clipped(), "{report:?}");
    // Short of rows, the preview goes first and says how many lines.
    let (buf, report) = card_buf(&subject, 60, 14, &theme);
    let text = all_text(&buf);
    assert!(report.preview.lines > 0, "{report:?}");
    assert!(report.clipped());
    assert!(
        text.contains(&format!("{} preview line", report.preview.lines))
            || text.contains(&format!("{} preview lines", report.preview.lines)),
        "{text}"
    );
    assert!(
        text.contains("Risk: overwrites a.rs"),
        "risk outranks the preview\n{text}"
    );
}

// ---------------------------------------------------------------------------
// Choices, focus and keys
// ---------------------------------------------------------------------------

#[test]
fn the_default_focus_is_the_safe_choice_never_an_allow() {
    let state = choices();
    assert_eq!(
        state.focused(),
        Some(DENY),
        "first choice is Allow once; focus is not"
    );
    // An allow listed first, a refusal last.
    let state = ApprovalState::new(vec![
        ApprovalChoice::new(ALLOW_ONCE, "Allow once", ApprovalEffect::Grants).char_key('y'),
        ApprovalChoice::new(DENY_WHY, "Say why", ApprovalEffect::Other).char_key('e'),
        ApprovalChoice::new(DENY, "Deny", ApprovalEffect::Refuses).char_key('n'),
    ]);
    assert_eq!(state.focused(), Some(DENY));
    // No refusal: the neutral choice.
    let state = ApprovalState::new(vec![
        ApprovalChoice::new(ALLOW_ONCE, "Allow once", ApprovalEffect::Grants),
        ApprovalChoice::new(DENY_WHY, "Open details", ApprovalEffect::Other),
    ]);
    assert_eq!(state.focused(), Some(DENY_WHY));
    // Only grants: no default at all, and Enter chooses nothing.
    let mut state = ApprovalState::new(vec![
        ApprovalChoice::new(ALLOW_ONCE, "Allow once", ApprovalEffect::Grants).char_key('y'),
    ]);
    assert_eq!(state.focused(), None);
    assert_eq!(
        state.handle_key(press(KeyCode::Enter)),
        ApprovalOutcome::Ignored
    );
    assert_eq!(
        state.handle_key(press(KeyCode::Char('y'))),
        ApprovalOutcome::Chose(ALLOW_ONCE)
    );
}

#[test]
fn the_focused_choice_is_marked_in_every_profile() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        let subject =
            ApprovalSubject::new(ApprovalKind::Command, "ls").scope(ApprovalScope::Inside);
        let (buf, _) = card_buf(&subject, 120, 12, &theme);
        let text = all_text(&buf);
        let marker = if profile == Profile::Ascii {
            ">"
        } else {
            "▸"
        };
        assert!(
            text.contains(&format!("{marker} n Deny")),
            "{}\n{text}",
            profile.name()
        );
        assert_eq!(
            text.matches(marker).count(),
            1,
            "{}: one focus",
            profile.name()
        );
        assert!(text.contains("y Allow once"), "{}", profile.name());
        assert!(
            !text.contains(&format!("{marker} y")),
            "{}: allow is not focused",
            profile.name()
        );
    }
}

#[test]
fn enter_chooses_the_focused_choice_and_nothing_else() {
    let mut state = choices();
    assert_eq!(
        state.handle_key(press(KeyCode::Enter)),
        ApprovalOutcome::Chose(DENY)
    );
    // Move to the neighbours; Enter follows focus.
    assert_eq!(
        state.handle_key(press(KeyCode::Right)),
        ApprovalOutcome::Moved
    );
    assert_eq!(state.focused(), Some(DENY_WHY));
    assert_eq!(
        state.handle_key(press(KeyCode::Enter)),
        ApprovalOutcome::Chose(DENY_WHY)
    );
    assert_eq!(
        state.handle_key(press(KeyCode::Tab)),
        ApprovalOutcome::Moved
    );
    assert_eq!(state.focused(), Some(ALLOW_ONCE), "focus wraps");
    assert_eq!(
        state.handle_key(press(KeyCode::Enter)),
        ApprovalOutcome::Chose(ALLOW_ONCE)
    );
    assert_eq!(
        state.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        ApprovalOutcome::Moved
    );
    assert_eq!(state.focused(), Some(DENY_WHY));
    assert_eq!(state.handle_key(press(KeyCode::Up)), ApprovalOutcome::Moved);
    assert_eq!(state.focused(), Some(DENY));
    // Enter with a modifier is not Enter.
    for mods in [
        KeyModifiers::CONTROL,
        KeyModifiers::ALT,
        KeyModifiers::SHIFT,
        KeyModifiers::SUPER,
    ] {
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, mods)),
            ApprovalOutcome::Ignored,
            "{mods:?}"
        );
    }
}

#[test]
fn a_chord_chooses_directly_and_esc_cancels() {
    let mut state = choices();
    for (key, id) in [
        ('y', ALLOW_ONCE),
        ('1', ALLOW_ONCE),
        ('a', ALLOW_SESSION),
        ('n', DENY),
        ('d', DENY),
        ('e', DENY_WHY),
    ] {
        assert_eq!(
            state.handle_key(press(KeyCode::Char(key))),
            ApprovalOutcome::Chose(id),
            "{key}"
        );
    }
    // `3` is not declared in this fixture.
    assert_eq!(
        state.handle_key(press(KeyCode::Char('3'))),
        ApprovalOutcome::Ignored
    );
    assert_eq!(
        state.handle_key(press(KeyCode::Esc)),
        ApprovalOutcome::Cancelled
    );
    assert_eq!(
        state.handle_key(press(KeyCode::Char('o'))),
        ApprovalOutcome::Reveal
    );
    // Choosing by chord does not move focus.
    assert_eq!(state.focused(), Some(DENY));
}

#[test]
fn no_key_outside_the_declared_chords_produces_a_choice() {
    let mut state = choices();
    let declared: Vec<ApprovalKey> = state
        .choices()
        .iter()
        .flat_map(|c| c.keys.clone())
        .collect();
    let mut codes: Vec<KeyCode> = (0x20u8..0x7f)
        .map(|b| KeyCode::Char(char::from(b)))
        .collect();
    codes.extend(['é', '鲸', 'Y', 'N', '\u{202e}'].map(KeyCode::Char));
    codes.extend((1..=12).map(KeyCode::F));
    codes.extend([
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Insert,
        KeyCode::Null,
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::CapsLock,
        KeyCode::Menu,
    ]);
    let mods = [
        KeyModifiers::NONE,
        KeyModifiers::SHIFT,
        KeyModifiers::CONTROL,
        KeyModifiers::ALT,
        KeyModifiers::SUPER,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        KeyModifiers::ALT | KeyModifiers::SHIFT,
    ];
    let mut chose = 0;
    for code in &codes {
        for m in mods {
            for kind in [
                KeyEventKind::Press,
                KeyEventKind::Repeat,
                KeyEventKind::Release,
            ] {
                let mut key = KeyEvent::new(*code, m);
                key.kind = kind;
                let out = state.handle_key(key);
                if let ApprovalOutcome::Chose(id) = out {
                    chose += 1;
                    assert_eq!(
                        kind,
                        KeyEventKind::Press,
                        "{code:?} {m:?}: only a press chooses"
                    );
                    let by_chord = declared.contains(&ApprovalKey::new(*code, m));
                    let by_enter = *code == KeyCode::Enter && m.is_empty();
                    assert!(
                        by_chord || by_enter,
                        "{code:?} {m:?} chose {id:?} with no chord"
                    );
                    if by_enter {
                        assert_eq!(id, DENY, "Enter chooses the focused (safe) choice");
                    }
                }
            }
        }
    }
    // 10 declared chords + Enter, each once as a press.
    assert_eq!(chose, declared.len() + 1);
    // Case matters: `Y` is not `y`.
    assert_eq!(
        state.handle_key(KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT)),
        ApprovalOutcome::Ignored
    );
}

#[test]
fn a_held_key_cannot_answer_the_card() {
    let mut state = choices();
    let mut key = press(KeyCode::Char('y'));
    key.kind = KeyEventKind::Repeat;
    assert_eq!(state.handle_key(key), ApprovalOutcome::Ignored);
    let mut enter = press(KeyCode::Enter);
    enter.kind = KeyEventKind::Repeat;
    assert_eq!(state.handle_key(enter), ApprovalOutcome::Ignored);
    let mut esc = press(KeyCode::Esc);
    esc.kind = KeyEventKind::Repeat;
    assert_eq!(state.handle_key(esc), ApprovalOutcome::Ignored);
    // Moving focus may repeat.
    let mut right = press(KeyCode::Right);
    right.kind = KeyEventKind::Repeat;
    assert_eq!(state.handle_key(right), ApprovalOutcome::Moved);
}

#[test]
fn keys_typed_before_the_card_was_drawn_are_ignored() {
    let mut state = choices().unarmed();
    for key in [
        KeyCode::Char('y'),
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Right,
        KeyCode::Char('o'),
    ] {
        assert_eq!(
            state.handle_key(press(key)),
            ApprovalOutcome::Ignored,
            "{key:?}"
        );
    }
    state.arm();
    assert_eq!(
        state.handle_key(press(KeyCode::Char('y'))),
        ApprovalOutcome::Chose(ALLOW_ONCE)
    );
}

#[test]
fn chords_the_card_owns_are_never_advertised_or_taken() {
    // A host that binds Enter, an arrow or the reveal key to a choice must
    // not get "Enter allows": those keys keep their one meaning.
    let state = ApprovalState::new(vec![
        ApprovalChoice::new(ALLOW_ONCE, "Allow once", ApprovalEffect::Grants)
            .key(ApprovalKey::new(KeyCode::Enter, KeyModifiers::NONE))
            .key(ApprovalKey::new(KeyCode::Right, KeyModifiers::NONE))
            .char_key('o')
            .char_key('y'),
        ApprovalChoice::new(DENY, "Deny", ApprovalEffect::Refuses).char_key('n'),
    ])
    .reveal(ApprovalKey::char('o'));
    assert_eq!(state.choices()[0].keys, vec![ApprovalKey::char('y')]);
    let mut state = state;
    assert_eq!(
        state.handle_key(press(KeyCode::Enter)),
        ApprovalOutcome::Chose(DENY)
    );
    assert_eq!(
        state.handle_key(press(KeyCode::Char('o'))),
        ApprovalOutcome::Reveal
    );
    // The reveal key cannot itself be a reserved key.
    let state = choices().reveal(ApprovalKey::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(state.reveal_key(), None);
}

#[test]
fn a_chord_belongs_to_one_choice_and_a_host_can_ask_what_grants() {
    let mut state = ApprovalState::new(vec![
        ApprovalChoice::new(ALLOW_ONCE, "Allow once", ApprovalEffect::Grants).char_key('y'),
        // The same `y` on a later choice would advertise a key that allows.
        ApprovalChoice::new(DENY, "Deny", ApprovalEffect::Refuses)
            .char_key('y')
            .char_key('n'),
    ]);
    assert_eq!(state.choices()[1].keys, vec![ApprovalKey::char('n')]);
    assert_eq!(
        state.handle_key(press(KeyCode::Char('y'))),
        ApprovalOutcome::Chose(ALLOW_ONCE)
    );
    assert!(state.grants(ALLOW_ONCE));
    assert!(!state.grants(DENY));
    assert!(!state.grants(ChoiceId(99)));
    // A host that refuses a blind approval composes the two.
    let theme = Profile::DarkTrue.theme();
    let subject = ApprovalSubject::new(ApprovalKind::Command, long_command());
    let card = ApprovalCard::new(&subject, &state);
    let area = Rect::new(0, 0, 40, 10);
    let report = card.render(area, &mut Buffer::empty(area), &theme);
    let refuse = |id| report.clipped() && state.grants(id);
    assert!(refuse(ALLOW_ONCE));
    assert!(!refuse(DENY), "a denial is never blind");
}

#[test]
fn the_painted_chords_are_the_handled_chords() {
    let theme = Profile::DarkTrue.theme();
    let subject = ApprovalSubject::new(ApprovalKind::Command, "ls").scope(ApprovalScope::Inside);
    let (buf, _) = card_buf(&subject, 120, 12, &theme);
    let text = all_text(&buf);
    let mut state = choices();
    for choice in state.clone().choices() {
        let key = choice.keys.first().expect("fixture has keys");
        assert!(
            text.contains(&format!(
                "{} {}",
                key.label(codewhale_ratatui::keys::Platform::current(false)),
                choice.label
            )),
            "{text}"
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(key.code, key.modifiers)),
            ApprovalOutcome::Chose(choice.id)
        );
    }
    let ctrl = ApprovalState::new(vec![
        ApprovalChoice::new(DENY, "Deny", ApprovalEffect::Refuses).key(ApprovalKey::ctrl('d')),
    ]);
    let card = ApprovalCard::new(&subject, &ctrl);
    let buf = testing::render(60, 10, |a, b| card.paint(a, b, &theme));
    assert!(all_text(&buf).contains("Ctrl+D Deny"), "{}", all_text(&buf));
}

#[test]
fn a_card_with_more_choices_than_rows_counts_the_hidden_ones() {
    let theme = Profile::DarkTrue.theme();
    let subject = ApprovalSubject::new(ApprovalKind::Command, "ls");
    // Height 3: the panel's own rows leave none for the choices.
    let (_, report) = card_buf(&subject, 40, 3, &theme);
    assert!(report.clipped());
    assert!(report.choices > 0, "{report:?}");
}

// ---------------------------------------------------------------------------
// Words
// ---------------------------------------------------------------------------

#[test]
fn every_word_is_a_parameter_with_an_english_default() {
    let theme = Profile::DarkTrue.theme();
    let subject = ApprovalSubject::new(ApprovalKind::Command, "ls")
        .cwd("/w")
        .scope(ApprovalScope::Outside)
        .agent("a", true)
        .risk_note("x");
    let state = choices();
    let english = testing::render(70, 18, |a, b| {
        ApprovalCard::new(&subject, &state).paint(a, b, &theme)
    });
    let words = ApprovalWords {
        needs_you: "Braucht dich".into(),
        command: "Befehl ausführen".into(),
        outside: "Außerhalb des Projekts".into(),
        requested_by: "Angefragt von".into(),
        sub_agent: "Unter-Agent".into(),
        risk: "Risiko".into(),
        in_dir: "In".into(),
        ..ApprovalWords::default()
    };
    let german = testing::render(70, 18, |a, b| {
        ApprovalCard::new(&subject, &state)
            .words(&words)
            .paint(a, b, &theme)
    });
    let (e, g) = (all_text(&english), all_text(&german));
    assert!(e.contains("Needs you · Run a command"), "{e}");
    assert!(e.contains("Requested by a (sub-agent)"), "{e}");
    assert!(g.contains("Braucht dich · Befehl ausführen"), "{g}");
    assert!(g.contains("Außerhalb des Projekts"), "{g}");
    assert!(g.contains("Angefragt von a (Unter-Agent)"), "{g}");
    assert!(g.contains("Risiko: x"), "{g}");
    assert!(!g.contains("Needs you") && !g.contains("Outside"), "{g}");
    assert_eq!(ApprovalWords::default(), ApprovalWords::default());
}

#[test]
fn counts_use_the_singular_and_the_plural() {
    let w = ApprovalWords::default();
    assert_eq!(w.clip.hidden_chars.render(1), "1 character not shown");
    assert_eq!(w.clip.hidden_chars.render(37), "37 characters not shown");
    assert_eq!(
        w.non_ascii.render(2),
        "Contains 2 non-ASCII characters: check them"
    );
}

// ---------------------------------------------------------------------------
// Tiny and degenerate sizes
// ---------------------------------------------------------------------------

#[test]
fn nothing_panics_at_tiny_sizes() {
    let subject = ApprovalSubject::new(ApprovalKind::Command, "rm -rf \u{1b}[2J/\u{202e}x\n鲸鱼  ")
        .cwd("/tmp/鲸鱼")
        .scope(ApprovalScope::Outside)
        .agent("a\u{200b}gent", true)
        .risk_note("danger")
        .preview_line("+x");
    let hints = KeyHints::new(vec![KeyHint::new("y", "allow once")]);
    let verdict = ReviewVerdict::new(ReviewKind::Held, "Auto", "because\u{1b}", &hints, None)
        .unwrap()
        .subject("git push\n")
        .category("cat")
        .id("ar_1");
    let aggregate = ReviewAggregate {
        allowed: 1,
        denied: 1,
        held: 1,
        undecided: 1,
        reviewing: 1,
        ..ReviewAggregate::default()
    };
    let state = choices();
    for profile in Profile::ALL {
        let theme = profile.theme();
        for (w, h) in [
            (0, 0),
            (0, 5),
            (5, 0),
            (1, 1),
            (2, 2),
            (3, 3),
            (4, 1),
            (7, 7),
            (9, 3),
            (12, 4),
            (1, 20),
        ] {
            let buf = testing::render(w, h, |a, b| {
                let report = ApprovalCard::new(&subject, &state).render(a, b, &theme);
                if w * h <= 4 {
                    assert!(report.clipped(), "{w}x{h}");
                }
                verdict.render(a, b, &theme);
                aggregate.render(a, b, &theme);
            });
            assert_eq!(buf.area, Rect::new(0, 0, w, h));
        }
        // Painting into an area bigger than the buffer is clipped, not a panic.
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 4));
        ApprovalCard::new(&subject, &state).paint(Rect::new(0, 0, 50, 50), &mut buf, &theme);
        for width in 0..=3 {
            let _ = ApprovalCard::new(&subject, &state).height(width, &theme);
            let _ = verdict.height(width, &theme);
            let _ = aggregate.height(width, &theme);
        }
    }
}

// ---------------------------------------------------------------------------
// Review verdicts and the aggregate
// ---------------------------------------------------------------------------

fn hints() -> KeyHints {
    KeyHints::new(vec![
        KeyHint::new("y", "allow once"),
        KeyHint::new("n", "deny"),
    ])
}

fn verdict_fixture(kind: ReviewKind, hints: &KeyHints) -> ReviewVerdict<'_> {
    let reason = match kind {
        ReviewKind::Allowed => "reads files inside this project",
        ReviewKind::Denied => "deletes files outside this project",
        ReviewKind::Held => "pushes to a protected branch",
        ReviewKind::Undecided => "the review timed out",
    };
    let subject = match kind {
        ReviewKind::Allowed => "cat src/lib.rs",
        ReviewKind::Denied => "rm -rf ../build",
        _ => "git push origin main",
    };
    ReviewVerdict::new(kind, "Auto-Review", reason, hints, None)
        .unwrap()
        .subject(subject)
}

#[test]
fn every_verdict_has_its_own_mark_and_word_at_every_depth() {
    let hints = hints();
    let words = ReviewWords::default();
    for profile in [
        Profile::Ansi16,
        Profile::NoColor,
        Profile::Ascii,
        Profile::DarkTrue,
    ] {
        let theme = profile.theme();
        let mut firsts = Vec::new();
        for kind in ReviewKind::ALL {
            let v = verdict_fixture(kind, &hints);
            let buf = testing::render(60, v.height(60, &theme), |a, b| v.paint(a, b, &theme));
            let text = all_text(&buf);
            let first = text.lines().next().unwrap().to_string();
            let word = match kind {
                ReviewKind::Allowed => &words.allowed,
                ReviewKind::Denied => &words.denied,
                ReviewKind::Held => &words.held,
                ReviewKind::Undecided => &words.undecided,
            };
            assert!(first.contains(&**word), "{}: {first}", profile.name());
            assert!(first.contains("Auto-Review"), "{}", profile.name());
            firsts.push(first.chars().next().unwrap());
        }
        for (i, a) in firsts.iter().enumerate() {
            for b in &firsts[i + 1..] {
                assert_ne!(
                    a,
                    b,
                    "{}: two verdicts share a mark: {firsts:?}",
                    profile.name()
                );
            }
        }
    }
    // Spelled out: the ASCII marks.
    let theme = Profile::Ascii.theme();
    let marks: Vec<char> = ReviewKind::ALL
        .iter()
        .map(|k| {
            let v = verdict_fixture(*k, &hints);
            all_text(&testing::render(60, 6, |a, b| v.paint(a, b, &theme)))
                .chars()
                .next()
                .unwrap()
        })
        .collect();
    assert_eq!(marks, ['Y', 'X', '*', '?']);
}

#[test]
fn a_verdict_that_could_not_decide_says_it_is_not_a_verdict() {
    let hints = hints();
    let theme = Profile::NoColor.theme();
    let v = verdict_fixture(ReviewKind::Undecided, &hints);
    let text = all_text(&testing::render(70, 8, |a, b| v.paint(a, b, &theme)));
    assert!(text.contains("? Could not decide"), "{text}");
    assert!(
        text.contains("No verdict. This is not a safety decision: your call."),
        "{text}"
    );
    assert!(!text.contains("Allowed"));
    let v = verdict_fixture(ReviewKind::Held, &hints);
    let text = all_text(&testing::render(70, 8, |a, b| v.paint(a, b, &theme)));
    assert!(text.contains("◆ Held for you"), "{text}");
    assert!(text.contains("Your call."), "{text}");
}

#[test]
fn a_denied_held_or_undecided_verdict_must_offer_a_next_step() {
    let none = KeyHints::default();
    let disabled = KeyHints::new(vec![KeyHint::new("y", "allow once").disabled()]);
    for kind in [ReviewKind::Denied, ReviewKind::Held, ReviewKind::Undecided] {
        assert!(
            ReviewVerdict::new(kind, "Auto", "why", &none, None).is_err(),
            "{kind:?}"
        );
        assert!(
            ReviewVerdict::new(kind, "Auto", "why", &disabled, None).is_err(),
            "{kind:?}"
        );
        assert!(
            ReviewVerdict::new(kind, "Auto", "why", &none, Some("  ".into())).is_err(),
            "{kind:?}"
        );
        let waiting =
            ReviewVerdict::new(kind, "Auto", "why", &none, Some("a file listing".into())).unwrap();
        let theme = Profile::NoColor.theme();
        let text = all_text(&testing::render(60, 6, |a, b| waiting.paint(a, b, &theme)));
        assert!(text.contains("Waiting for: a file listing"), "{text}");
        assert!(ReviewVerdict::new(kind, "Auto", "why", &hints(), None).is_ok());
    }
    // An allowed verdict asks nothing of anyone.
    assert!(ReviewVerdict::new(ReviewKind::Allowed, "Auto", "why", &none, None).is_ok());
}

#[test]
fn a_key_that_would_do_nothing_is_not_shown() {
    let hints = KeyHints::new(vec![
        KeyHint::new("y", "allow once").disabled(),
        KeyHint::new("n", "deny"),
    ]);
    let v = ReviewVerdict::new(ReviewKind::Denied, "Auto", "why", &hints, None).unwrap();
    let theme = Profile::NoColor.theme();
    let text = all_text(&testing::render(60, 4, |a, b| v.paint(a, b, &theme)));
    assert!(
        text.contains("n deny") && !text.contains("allow once"),
        "{text}"
    );
}

#[test]
fn a_verdict_shows_its_action_verbatim_and_escapes_its_text() {
    let hints = hints();
    let theme = Profile::DarkTrue.theme();
    let v = ReviewVerdict::new(
        ReviewKind::Denied,
        "Auto\u{202e}Review",
        "because\u{1b}[2J",
        &hints,
        None,
    )
    .unwrap()
    .subject("rm -rf ~/\u{200b}x  ")
    .category("Deletes unseen files")
    .id("ar_7K2M");
    let h = v.height(80, &theme);
    let text = all_text(&testing::render(80, h, |a, b| v.paint(a, b, &theme)));
    assert!(text.contains("Auto‹RLO›Review"), "{text}");
    assert!(text.contains("Deletes unseen files"), "{text}");
    assert!(text.contains("$ rm -rf ~/‹ZWSP›x‹SP×2›"), "{text}");
    assert!(text.contains("Why: because‹ESC›[2J"), "{text}");
    assert!(
        text.lines().next().unwrap().trim_end().ends_with("ar_7K2M"),
        "{text}"
    );
    let mut report = ApprovalPaint::default();
    let _ = testing::render(80, h, |a, b| report = v.render(a, b, &theme));
    assert!(!report.clipped(), "{report:?}");
    // A verdict that cannot show its whole action says so.
    let long = ReviewVerdict::new(ReviewKind::Held, "Auto", "why", &hints, None)
        .unwrap()
        .subject("x".repeat(300));
    let mut report = ApprovalPaint::default();
    let buf = testing::render(40, 7, |a, b| report = long.render(a, b, &theme));
    assert!(report.subject.chars > 0, "{report:?}");
    assert!(
        all_text(&buf).contains(&format!("{} characters not shown", report.subject.chars)),
        "{}",
        all_text(&buf)
    );
}

#[test]
fn the_aggregate_shows_real_counts() {
    let words = codewhale_ratatui::ReviewAggregateWords::default();
    let agg = ReviewAggregate::from_kinds(
        std::iter::repeat_n(ReviewKind::Allowed, 42)
            .chain(std::iter::repeat_n(ReviewKind::Denied, 3))
            .chain([ReviewKind::Held, ReviewKind::Undecided]),
    )
    .reviewing(2);
    assert_eq!(
        (
            agg.allowed,
            agg.denied,
            agg.held,
            agg.undecided,
            agg.reviewing
        ),
        (42, 3, 1, 1, 2)
    );
    let theme = Profile::DarkTrue.theme();
    let text = all_text(&testing::render(120, 1, |a, b| agg.paint(a, b, &theme)));
    assert_eq!(
        text,
        "Auto-Review · ✓ 42 allowed · ✕ 3 denied · ◆ 1 held for you · ? 1 could not decide · ● reviewing 2"
    );
    let theme = Profile::Ascii.theme();
    let text = all_text(&testing::render(120, 1, |a, b| agg.paint(a, b, &theme)));
    assert_eq!(
        text,
        "Auto-Review  Y 42 allowed  X 3 denied  * 1 held for you  ? 1 could not decide  . reviewing 2"
    );
    // Zeros are left out; nothing checked says so.
    let none = ReviewAggregate::default();
    let text = all_text(&testing::render(60, 1, |a, b| none.paint(a, b, &theme)));
    assert_eq!(text, format!("{}  {}", words.title, words.none_yet));
    // Narrow: items wrap whole, and the count of items not shown is reported.
    let theme = Profile::DarkTrue.theme();
    assert!(agg.height(30, &theme) > 1);
    let mut hidden = 0;
    let buf = testing::render(30, 2, |a, b| hidden = agg.render(a, b, &theme));
    assert!(hidden > 0, "{}", all_text(&buf));
    let mut hidden = 0;
    let _ = testing::render(30, agg.height(30, &theme), |a, b| {
        hidden = agg.render(a, b, &theme)
    });
    assert_eq!(hidden, 0);
}

// ---------------------------------------------------------------------------
// Rules and snapshots
// ---------------------------------------------------------------------------

fn paint_card(subject: &ApprovalSubject, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let state = choices();
    ApprovalCard::new(subject, &state).paint(area, buf, theme);
}

macro_rules! card_case {
    ($name:ident, $snap:literal, $height:expr, $subject:expr) => {
        #[test]
        fn $name() {
            let subject = $subject;
            let paint = |a: Rect, b: &mut Buffer, t: &Theme| paint_card(&subject, a, b, t);
            testing::assert_rules($height, paint);
            insta::assert_snapshot!($snap, testing::snapshot($height, paint));
        }
    };
}

card_case!(
    command_inside_the_project,
    "approval-command-inside",
    17,
    ApprovalSubject::new(ApprovalKind::Command, "cargo publish --dry-run")
        .cwd("codewhale/crates/tui")
        .scope(ApprovalScope::Inside)
        .agent("builder", false)
        .risk_note("checks the package before release; publishes nothing")
);

card_case!(
    command_outside_the_project,
    "approval-command-outside",
    18,
    ApprovalSubject::new(ApprovalKind::Command, "rm -rf ../build /tmp/cache")
        .cwd("/Users/me/work/codewhale")
        .scope(ApprovalScope::Outside)
        .agent("reviewer-3", true)
        .risk_note("deletes files outside this project")
);

card_case!(
    file_patch_with_a_preview,
    "approval-patch",
    20,
    ApprovalSubject::new(ApprovalKind::FileChange, "src/lib.rs, src/theme.rs")
        .cwd("codewhale-ratatui")
        .scope(ApprovalScope::Inside)
        .agent("builder", false)
        .preview_line("@@ -12,3 +12,4 @@")
        .preview_line("-pub const OCEAN_TINT: f64 = 0.5;")
        .preview_line("+pub const OCEAN_TINT: f64 = 0.55;")
        .preview_line(" pub enum Ground {")
);

card_case!(
    elevation,
    "approval-elevation",
    20,
    ApprovalSubject::new(ApprovalKind::Elevation, "npm install --global typescript")
        .cwd("codewhale-app")
        .scope(ApprovalScope::Inside)
        .agent("builder", false)
        .risk_note("the sandbox blocked a write to /usr/local; this runs without it")
);

card_case!(
    clipped_subject,
    "approval-clipped",
    11,
    ApprovalSubject::new(
        ApprovalKind::Command,
        "curl -fsSL https://example.com/install.sh | sh -s -- --prefix /opt/tools \
         --channel stable --no-modify-path --verbose --accept-license",
    )
    .cwd("codewhale")
    .scope(ApprovalScope::Inside)
);

card_case!(
    spoofed_subject,
    "approval-spoofed",
    20,
    ApprovalSubject::new(
        ApprovalKind::Command,
        "echo ok\u{1b}[2K\u{200b}\u{202e}txt.exe\tx\nrm -rf ~  ",
    )
    .cwd("codewhale")
    .scope(ApprovalScope::Inside)
    .agent("builder", false)
);

#[test]
fn each_verdict_keeps_the_rules_and_its_snapshot() {
    let hints = hints();
    for kind in ReviewKind::ALL {
        let v = verdict_fixture(kind, &hints);
        let paint = |a: Rect, b: &mut Buffer, t: &Theme| v.paint(a, b, t);
        testing::assert_rules(8, paint);
        let name = match kind {
            ReviewKind::Allowed => "review-allowed",
            ReviewKind::Denied => "review-denied",
            ReviewKind::Held => "review-held",
            ReviewKind::Undecided => "review-undecided",
        };
        insta::assert_snapshot!(name, testing::snapshot(8, paint));
    }
}

#[test]
fn a_verdict_with_its_category_and_id_keeps_the_rules_and_its_snapshot() {
    let hints = hints();
    let v = verdict_fixture(ReviewKind::Denied, &hints)
        .category("Deletes unseen files")
        .id("ar_7K2M");
    let paint = |a: Rect, b: &mut Buffer, t: &Theme| v.paint(a, b, t);
    testing::assert_rules(8, paint);
    insta::assert_snapshot!("review-denied-detailed", testing::snapshot(8, paint));
}

#[test]
fn the_aggregate_keeps_the_rules_and_its_snapshot() {
    let agg = ReviewAggregate {
        allowed: 42,
        denied: 3,
        held: 1,
        undecided: 1,
        reviewing: 2,
        ..ReviewAggregate::default()
    };
    let paint = |a: Rect, b: &mut Buffer, t: &Theme| agg.paint(a, b, t);
    testing::assert_rules(3, paint);
    insta::assert_snapshot!("review-aggregate", testing::snapshot(3, paint));
}

#[test]
fn a_non_ascii_subject_keeps_the_rules_outside_ascii() {
    let subject = ApprovalSubject::new(ApprovalKind::Command, "python 脚本.py g\u{0430}it")
        .cwd("/tmp/鲸鱼")
        .scope(ApprovalScope::Inside);
    let profiles: Vec<Profile> = Profile::ALL
        .into_iter()
        .filter(|p| *p != Profile::Ascii)
        .collect();
    let frames = testing::frames_for("", &profiles, &testing::WIDTHS, 14, |a, b, t| {
        paint_card(&subject, a, b, t);
    });
    testing::assert_frames_keep_the_rules(&frames);
}

fn native_decision_fixture() -> codewhale_ratatui::DecisionBand {
    use codewhale_ratatui::{DecisionBand, DecisionBandAction, DecisionBandSave};
    use ratatui::{
        style::{Color, Style},
        text::{Line, Span},
    };
    let plain = Style::default().fg(Color::Rgb(181, 192, 203));
    DecisionBand {
        body: (0..20)
            .map(|i| Line::styled(format!("  request line {i}: 你好 cafe\u{0301}"), plain))
            .collect(),
        saves: vec![DecisionBandSave {
            summary: "always allow".into(),
            entries: vec!["change src/你好.rs".into()],
            omitted: 2,
            label: "Save:   ".into(),
            separator: " · ".into(),
            compact_more: " +{count} more".into(),
            full_more: "... {count} more".into(),
            label_style: plain,
            summary_style: plain,
            entries_style: plain,
            more_style: plain,
        }],
        question: Line::styled("  Continue?", plain),
        actions: vec![
            DecisionBandAction {
                line: Line::styled("  [1/y] Allow exactly this call", plain),
                persistent: false,
            },
            DecisionBandAction {
                line: Line::styled("  [p] Save this exact project rule", plain),
                persistent: true,
            },
            DecisionBandAction {
                line: Line::styled("> [3/n] Deny", plain.add_modifier(Modifier::BOLD)),
                persistent: false,
            },
        ],
        footer: Line::styled("  Enter choose; Alt+V details", plain),
        save_hint: Some(Span::styled(" / s save ask rule", plain)),
        background: Style::default().bg(Color::Rgb(21, 32, 43)),
        rule: Span::styled("─", plain),
        truncation_hint: Span::styled("  truncated: Alt+V", plain),
        collapsed: None,
    }
}

#[test]
fn native_band_gallery_entries_keep_every_profile_and_bounds() {
    for name in ["approval-native-band", "approval-native-band-collapsed"] {
        let entry = codewhale_ratatui::gallery::entries()
            .into_iter()
            .find(|entry| entry.name == name)
            .expect("actual native gallery fixture");
        testing::assert_rules(entry.height, |area, buf, theme| {
            (entry.draw)(area, buf, theme)
        });
    }
}

#[test]
fn native_band_save_coverage_actions_and_wrapped_hitboxes_are_one_plan() {
    let band = native_decision_fixture();
    for width in [20, 40, 80] {
        for height in [1, 6, 9, 12, 20, 40] {
            let area = Rect::new(7, 5, width, height);
            let plan = band.plan(area);
            assert_eq!(plan.region.bottom(), area.bottom());
            assert_eq!(plan.action_rects.len(), 3);
            assert_eq!(plan.action_rects[1].height > 0, plan.save_shown);
            if plan.save_shown {
                assert!(plan.save_rect.height > 0);
                assert!(plan.save_rect.bottom() <= plan.control_rect.y);
            }
            for rect in &plan.action_rects {
                if !rect.is_empty() {
                    assert_eq!(rect.width, width);
                    assert!(rect.y >= plan.control_rect.y);
                    assert!(rect.bottom() <= area.bottom());
                }
            }
            let mut buf = Buffer::empty(area);
            let painted = band.render(area, &mut buf);
            assert_eq!(painted.region, plan.region);
            assert_eq!(painted.action_rects, plan.action_rects);
            let text = all_text(&buf);
            if !plan.save_shown {
                assert!(!text.contains("[p]"));
                assert!(!text.contains("s save"));
            }
        }
    }
    let plan = band.plan(Rect::new(7, 5, 20, 40));
    assert!(
        plan.action_rects[0].height > 1,
        "an option owns all of its wrapped rows"
    );
}

#[test]
fn native_band_clips_before_planning_and_preserves_guard_cells() {
    use ratatui::buffer::Cell;
    let band = native_decision_fixture();
    let canvas = Rect::new(7, 5, 30, 12);
    for requested in [
        Rect::new(10, 7, 40, 20),
        Rect::new(0, 0, 100, 100),
        Rect::new(50, 50, 20, 20),
        Rect::new(7, 5, 0, 10),
    ] {
        let before = Buffer::filled(canvas, Cell::new("~"));
        let mut actual = before.clone();
        let plan = band.render(requested, &mut actual);
        let visible = requested.intersection(canvas);
        let mut expected = before.clone();
        band.render(visible, &mut expected);
        assert_eq!(actual, expected);
        for y in canvas.y..canvas.bottom() {
            for x in canvas.x..canvas.right() {
                if !visible.contains((x, y).into()) {
                    assert_eq!(actual[(x, y)], before[(x, y)]);
                }
            }
        }
        if visible.is_empty() {
            assert!(!plan.save_shown);
            assert!(plan.action_rects.iter().all(|rect| rect.is_empty()));
        }
    }
}

#[test]
fn native_band_collapsed_and_empty_frames_withdraw_all_interactive_geometry() {
    use ratatui::text::Line;
    let mut band = native_decision_fixture();
    band.collapsed = Some(Line::from("  Waiting [Tab expand]"));
    let area = Rect::new(7, 5, 40, 20);
    let plan = band.plan(area);
    assert_eq!(plan.region, Rect::new(7, 24, 40, 1));
    assert!(!plan.save_shown);
    assert_eq!(plan.action_rects, vec![Rect::default(); 3]);
    band.collapsed = None;
    let plan = band.plan(Rect::new(7, 5, 0, 20));
    assert!(!plan.save_shown);
    assert_eq!(plan.action_rects, vec![Rect::default(); 3]);
}

#[test]
fn native_band_sanitizes_every_styled_field_before_fit_and_paint() {
    fn hostile(value: &str) -> String {
        format!("\u{202e}\n\t\u{1b}{value}\u{2066}\u{0007}")
    }
    fn hostile_line(line: &mut ratatui::text::Line<'static>) {
        for span in &mut line.spans {
            span.content = hostile(&span.content).into();
        }
    }
    let clean = native_decision_fixture();
    let mut dirty = clean.clone();
    for line in &mut dirty.body {
        hostile_line(line);
    }
    hostile_line(&mut dirty.question);
    for action in &mut dirty.actions {
        hostile_line(&mut action.line);
    }
    hostile_line(&mut dirty.footer);
    if let Some(span) = &mut dirty.save_hint {
        span.content = hostile(&span.content).into();
    }
    dirty.rule.content = hostile(&dirty.rule.content).into();
    dirty.truncation_hint.content = hostile(&dirty.truncation_hint.content).into();
    for save in &mut dirty.saves {
        save.summary = hostile(&save.summary);
        for entry in &mut save.entries {
            *entry = hostile(entry);
        }
        save.label = hostile(&save.label);
        save.separator = hostile(&save.separator);
        save.compact_more = hostile(&save.compact_more);
        save.full_more = hostile(&save.full_more);
    }
    for width in [20, 40, 80] {
        for height in [1, 6, 12, 40] {
            let area = Rect::new(7, 5, width, height);
            let mut expected = Buffer::empty(area);
            let expected_plan = clean.render(area, &mut expected);
            let mut actual = Buffer::empty(area);
            let actual_plan = dirty.render(area, &mut actual);
            assert_eq!(actual, expected, "all content and exact styles, {area:?}");
            assert_eq!(actual_plan.region, expected_plan.region);
            assert_eq!(actual_plan.save_shown, expected_plan.save_shown);
            assert_eq!(actual_plan.action_rects, expected_plan.action_rects);
        }
    }
    let mut clean = clean;
    clean.collapsed = Some(ratatui::text::Line::styled(
        "  Waiting [Tab expand]",
        clean.rule.style,
    ));
    dirty.collapsed = clean.collapsed.clone();
    hostile_line(dirty.collapsed.as_mut().unwrap());
    let area = Rect::new(7, 5, 40, 20);
    let mut expected = Buffer::empty(area);
    let mut actual = expected.clone();
    clean.render(area, &mut expected);
    let plan = dirty.render(area, &mut actual);
    assert_eq!(actual, expected);
    assert_eq!(plan.action_rects, vec![Rect::default(); 3]);
    assert!(!plan.save_shown);
}
