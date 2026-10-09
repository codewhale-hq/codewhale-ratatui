//! Every component paints inside the part of its area that the buffer holds,
//! and nowhere else.
//!
//! A host hands a component whatever rectangle its layout produced: it can sit
//! partly or wholly outside the buffer, have no width or height, or be a
//! single cell, and the buffer itself need not start at the origin. A
//! component must neither panic nor write outside the intersection of the
//! request and the buffer. This file paints every gallery entry (the gallery
//! holds a fixture for every component with a `Paint` impl, which
//! `every_paint_impl_has_a_gallery_entry` checks) into each of those shapes
//! and compares the buffer with a clone taken before painting.

use std::{
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
};

use codewhale_ratatui::{
    Depth, Dialog, Heading, HorizonRule, KeyHint, KeyHints, Paint, Panel, Segmented,
    SegmentedState, Sheet, SheetEdge, State, Tab, Tabs, TabsState, Theme, Toast, Toasts, Toggle,
    gallery, testing::Profile,
};
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
};

/// A buffer that starts away from the origin, so a component that confuses
/// buffer coordinates with area-relative ones writes somewhere visible.
const BUF: Rect = Rect::new(7, 5, 30, 12);

/// Named `(buffer area, requested area)` pairs: every shape a layout can hand
/// a component.
fn cases() -> Vec<(&'static str, Rect, Rect)> {
    let one = Rect::new(7, 5, 1, 1);
    vec![
        // (i) a buffer whose origin is not (0, 0), painted whole.
        ("nonzero origin", BUF, BUF),
        ("nonzero origin, inner", BUF, Rect::new(10, 7, 20, 6)),
        // (ii) partly outside, on each side and on all of them.
        ("partly right/bottom", BUF, Rect::new(17, 9, 40, 20)),
        ("partly left/top", BUF, Rect::new(2, 1, 20, 8)),
        ("partly left", BUF, Rect::new(0, 6, 20, 5)),
        ("partly top", BUF, Rect::new(9, 0, 12, 9)),
        ("covers and exceeds", BUF, Rect::new(0, 0, 100, 100)),
        (
            "starts inside, ends at the max",
            BUF,
            Rect::new(20, 8, u16::MAX, u16::MAX),
        ),
        // (iii) fully outside.
        ("outside left", BUF, Rect::new(0, 5, 6, 12)),
        ("outside above", BUF, Rect::new(7, 0, 30, 4)),
        ("outside right", BUF, Rect::new(37, 5, 30, 12)),
        ("outside below", BUF, Rect::new(7, 17, 30, 12)),
        (
            "outside, far corner",
            BUF,
            Rect::new(u16::MAX - 9, u16::MAX - 9, 9, 9),
        ),
        (
            "outside, far right",
            BUF,
            Rect::new(u16::MAX - 20, 6, 20, 5),
        ),
        (
            "outside, far below",
            BUF,
            Rect::new(8, u16::MAX - 20, 20, 20),
        ),
        // (iv) zero width.
        ("zero width", BUF, Rect::new(10, 7, 0, 6)),
        ("zero width, at the edge", BUF, Rect::new(37, 5, 0, 12)),
        // (v) zero height.
        ("zero height", BUF, Rect::new(10, 7, 20, 0)),
        ("zero height, at the edge", BUF, Rect::new(7, 17, 30, 0)),
        ("zero size", BUF, Rect::new(10, 7, 0, 0)),
        // (vi) a single cell, and a one-cell buffer.
        ("1x1, corner", BUF, Rect::new(7, 5, 1, 1)),
        ("1x1, middle", BUF, Rect::new(20, 10, 1, 1)),
        ("1x1, last cell", BUF, Rect::new(36, 16, 1, 1)),
        ("1x1, outside", BUF, Rect::new(2, 2, 1, 1)),
        ("1x1 buffer", one, one),
        ("1x1 buffer, larger request", one, Rect::new(0, 0, 40, 40)),
        (
            "1x1 buffer, request elsewhere",
            one,
            Rect::new(8, 5, 10, 10),
        ),
    ]
}

/// A buffer full of a symbol no component draws, so every write shows.
fn filled(area: Rect) -> Buffer {
    Buffer::filled(area, Cell::new("\u{00b7}"))
}

/// Where `before` and `after` differ, outside `allowed`.
fn stray_writes(before: &Buffer, after: &Buffer, allowed: Rect) -> Vec<(u16, u16)> {
    let area = before.area;
    let mut found = Vec::new();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let inside = x >= allowed.left()
                && x < allowed.right()
                && y >= allowed.top()
                && y < allowed.bottom();
            if !inside && before[(x, y)] != after[(x, y)] {
                found.push((x, y));
            }
        }
    }
    found
}

/// Paint every entry `keep` accepts into every shape in [`cases`], for a few
/// profiles, and list what went wrong.
fn failures_for(keep: impl Fn(&str) -> bool) -> Vec<String> {
    let entries: Vec<_> = gallery::entries()
        .into_iter()
        .filter(|e| keep(e.name))
        .collect();
    assert!(!entries.is_empty(), "the gallery has entries");
    let profiles = [
        Profile::DarkTrue,
        Profile::Ansi16,
        Profile::NoColor,
        Profile::Ascii,
    ];
    let mut failures = Vec::new();
    // A panic is reported in the failure list, not printed once per case.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    for entry in &entries {
        for profile in profiles {
            let theme = profile.theme();
            for (label, buf_area, request) in cases() {
                let before = filled(buf_area);
                let mut buf = before.clone();
                let painted = catch_unwind(AssertUnwindSafe(|| {
                    (entry.draw)(request, &mut buf, &theme);
                }));
                let case = format!(
                    "{} · {} · {label} ({request:?})",
                    entry.name,
                    profile.name()
                );
                if let Err(payload) = painted {
                    let message = payload
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| payload.downcast_ref::<&str>().copied())
                        .unwrap_or("(no message)");
                    failures.push(format!("{case}: panicked: {message}"));
                    continue;
                }
                let allowed = request.intersection(buf_area);
                let stray = stray_writes(&before, &buf, allowed);
                if !stray.is_empty() {
                    failures.push(format!(
                        "{case}: wrote {} cell(s) outside the intersection, first at {:?}",
                        stray.len(),
                        stray[0]
                    ));
                }
                if buf.area != buf_area {
                    failures.push(format!("{case}: the buffer area changed"));
                }
            }
        }
    }
    std::panic::set_hook(hook);
    failures
}

fn assert_no_failures(failures: &[String]) {
    assert!(
        failures.is_empty(),
        "{} paint-bounds failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn every_gallery_entry_paints_only_inside_the_buffer_and_the_request() {
    assert_no_failures(&failures_for(|name| !name.starts_with("whale")));
}

/// The whale shares the same clipping contract, including a nonzero origin
/// and requests extending outside the buffer.
#[test]
fn the_whale_paints_only_inside_the_buffer_and_the_request() {
    assert_no_failures(&failures_for(|name| name.starts_with("whale")));
}

/// A component lays itself out in the part of its area the buffer holds:
/// painting a request that runs past the buffer is the same as painting the
/// request clipped to it. Anchored, centered and right-aligned parts (the
/// toasts' bottom-right corner, a dialog's centre, a sheet's edge) would
/// otherwise land outside the buffer, where nobody sees them.
#[test]
fn a_request_past_the_buffer_paints_what_its_visible_part_would() {
    let hints = KeyHints::new(vec![
        KeyHint::new("Enter", "select a thing"),
        KeyHint::new("Esc", "cancel and go back"),
        KeyHint::new("?", "more"),
    ]);
    let tabs = [
        Tab::new("General"),
        Tab::new("Appearance"),
        Tab::new("Agents"),
    ];
    let toasts = Toasts::new(vec![
        Toast::new(State::Done, "Saved summary.md"),
        Toast::new(State::NeedsYou, "A command is waiting for your approval"),
        Toast::new(State::Failed, "Could not save: the file is read-only"),
    ]);
    let dialog = Dialog::new()
        .title("Stop the running workflow?")
        .body_rows(2)
        .hints(&hints);
    let sheet_bottom = Sheet::new().title("Settings").aside("3 changed").size(6);
    let sheet_right = Sheet::new().edge(SheetEdge::Right).title("Agents").size(14);
    let horizon = HorizonRule::new().label("Ask").aside("12% of context used");
    let heading = Heading::new("Settings").meta("saved to this project");
    let toggle = Toggle::new("Reduced motion", true).focused(true);
    let segmented = Segmented::new(["Full", "Reduced", "Still"], SegmentedState::new(1));
    let tabs = Tabs::new(&tabs, TabsState::new(1)).focused(true);
    let panel = Panel::new(Depth::Overlay).title("Mode").hints(&hints);
    let components: Vec<(&str, &dyn Paint)> = vec![
        ("toasts", &toasts),
        ("dialog", &dialog),
        ("sheet (bottom)", &sheet_bottom),
        ("sheet (right)", &sheet_right),
        ("horizon rule", &horizon),
        ("heading", &heading),
        ("toggle", &toggle),
        ("segmented", &segmented),
        ("tabs", &tabs),
        ("panel", &panel),
        ("key hints", &hints),
    ];
    let requests = [
        Rect::new(7, 5, 60, 40),
        Rect::new(0, 0, 100, 100),
        Rect::new(2, 1, 20, 8),
        Rect::new(9, 0, 12, 9),
        Rect::new(20, 8, u16::MAX, u16::MAX),
    ];
    let theme: Theme = Profile::DarkTrue.theme();
    for (name, component) in components {
        for request in requests {
            let clipped = request.intersection(BUF);
            let paint = |area: Rect| {
                let mut buf = filled(BUF);
                component.paint(area, &mut buf, &theme);
                buf
            };
            assert_eq!(
                paint(request),
                paint(clipped),
                "{name}: {request:?} must paint what {clipped:?} paints"
            );
        }
    }
}

/// The gallery draws a fixture for every component, so the check above covers
/// every `Paint` impl. A component added without a gallery entry would slip
/// past it: fail instead.
#[test]
fn every_paint_impl_has_a_gallery_entry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let read = |dir: &str| -> Vec<(String, String)> {
        std::fs::read_dir(root.join(dir))
            .expect("source directory")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .map(|p| {
                let source = std::fs::read_to_string(&p).expect("source file");
                (p.display().to_string(), source)
            })
            .collect()
    };
    let mut painted: BTreeSet<String> = BTreeSet::new();
    for (_, source) in read("components") {
        for line in source.lines() {
            if let Some((_, rest)) = line.split_once("Paint for ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    painted.insert(name);
                }
            }
        }
    }
    assert!(painted.len() > 20, "found the Paint impls: {painted:?}");
    let gallery_source: String = read("gallery").into_iter().map(|(_, s)| s).collect();
    let missing: Vec<_> = painted
        .iter()
        .filter(|name| !contains_word(&gallery_source, name))
        .collect();
    assert!(
        missing.is_empty(),
        "no gallery entry draws {missing:?}; add one so tests/paint_bounds.rs reaches it"
    );
}

fn contains_word(haystack: &str, word: &str) -> bool {
    haystack.match_indices(word).any(|(at, _)| {
        let before = haystack[..at].chars().next_back();
        let after = haystack[at + word.len()..].chars().next();
        let part = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        !part(before) && !part(after)
    })
}
