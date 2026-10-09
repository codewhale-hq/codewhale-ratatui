//! Every gallery component in every terminal profile, snapshotted with
//! insta, plus the rules every frame must keep.
//!
//! Snapshots record glyphs and the role each run was painted with, not hex
//! values, so a token change does not rewrite them but painting the wrong
//! role does. Review changes with `cargo insta review`.

use codewhale_ratatui::{
    Paint, State, Toast, Toasts, gallery,
    testing::{self, Frame, Profile},
};

fn dump(entry: &gallery::Entry) -> String {
    let mut out = String::new();
    for profile in Profile::ALL {
        let theme = gallery::theme_for(entry, &profile.theme());
        let buf = gallery::render(entry, &theme);
        out.push_str(&format!("== {}\n", profile.name()));
        out.push_str(&testing::styled(&buf, &theme));
        out.push('\n');
    }
    out
}

/// Entries whose every cell is pinned elsewhere. `whale-actions` is the 17
/// whales `tests/whale.rs` checks dot for dot against the kit's stills; a
/// 4,000-line copy here would bury real diffs. The rule checks below still
/// run over it in every profile.
const PINNED_ELSEWHERE: &[&str] = &["whale-actions"];

#[test]
fn gallery_snapshots() {
    for entry in gallery::entries() {
        if PINNED_ELSEWHERE.contains(&entry.name) {
            continue;
        }
        insta::assert_snapshot!(entry.name, dump(&entry));
    }
}

/// The rules every frame keeps live in `testing::rule_violations`, so a
/// component's own test file can check its frames the same way.
#[test]
fn every_frame_keeps_the_rules() {
    let frames: Vec<Frame> = Profile::ALL
        .into_iter()
        .flat_map(|profile| {
            let theme = profile.theme();
            gallery::entries()
                .into_iter()
                .map(move |entry| Frame::new(entry.name, profile, gallery::render(&entry, &theme)))
                .collect::<Vec<_>>()
        })
        .collect();
    testing::assert_frames_keep_the_rules(&frames);
}

#[test]
fn caller_text_cannot_reorder_itself() {
    let theme = Profile::DarkTrue.theme();
    let toasts = Toasts::new(vec![Toast::new(
        State::NeedsYou,
        "Approve rm -rf ~/\u{202E}txt.exe",
    )]);
    let buf = testing::render(40, 1, |area, buf| toasts.paint(area, buf, &theme));
    let text = testing::text(&buf);
    assert!(text.contains("rm -rf ~/txt.exe"), "{text}");
    assert!(!text.contains('\u{202E}'));
}
