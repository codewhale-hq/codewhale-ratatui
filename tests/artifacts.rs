use std::borrow::Cow;

use codewhale_ratatui::{
    Artifact, ArtifactKind, ArtifactShelf, ArtifactWords, List, ListState, Paint, ReceiptValue,
    Role, State, Theme,
    testing::{self, Profile, render, text},
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

fn fixture() -> ArtifactShelf<'static> {
    ArtifactShelf::new(vec![
        Artifact::new("Release checklist.md", ArtifactKind::File, State::Done)
            .detail("docs/release-checklist.md")
            .action("Open")
            .receipt("Size", ReceiptValue::Bytes(Some(4280))),
        Artifact::new(
            "Conversation changes",
            ArtifactKind::Review,
            State::NeedsYou,
        )
        .detail("Composer and transcript")
        .action("Review")
        .receipt("Files", ReceiptValue::Count(Some(3))),
        Artifact::new("Local verification", ArtifactKind::Run, State::Done)
            .detail("Reported by the host")
            .action("Inspect")
            .receipt("Checks", ReceiptValue::Count(Some(15))),
    ])
    .selected(1)
    .focused(true)
}

fn shelf(area: Rect, buf: &mut Buffer, theme: &Theme) {
    fixture().paint(area, buf, theme);
}

#[test]
fn artifact_shelf_keeps_the_rules_in_every_profile_and_width() {
    testing::assert_rules(6, shelf);
    insta::assert_snapshot!("artifact__shelf", testing::snapshot(6, shelf));
}

#[test]
fn artifact_empty_is_an_honest_localizable_state() {
    let words = ArtifactWords {
        empty: Cow::Borrowed("Aucun resultat"),
        ..ArtifactWords::default()
    };
    let empty = ArtifactShelf::new(Vec::new()).words(&words);
    for profile in Profile::ALL {
        let theme = profile.theme();
        let buf = render(40, 2, |area, buf| empty.paint(area, buf, &theme));
        let shown = text(&buf);
        assert!(shown.contains("Aucun resultat"));
        assert!(!shown.contains("Done"));
    }
    insta::assert_snapshot!(
        "artifact__empty",
        testing::snapshot(2, |area, buf, theme| {
            ArtifactShelf::new(Vec::new()).paint(area, buf, theme);
        })
    );
}

#[test]
fn reported_states_actions_and_unknown_receipts_are_not_inferred() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        for state in State::ALL {
            let artifact = Artifact::new("Imported report", ArtifactKind::File, state)
                .detail("No check was reported")
                .action("Inspect")
                .receipt("Checks", ReceiptValue::Count(None));
            let buf = render(80, 2, |area, buf| artifact.paint(area, buf, &theme));
            let shown = text(&buf);
            assert!(shown.contains(state.word()), "{}: {shown}", profile.name());
            assert!(shown.contains("Inspect"));
            let unknown = ReceiptValue::Count(None).render(theme.ascii());
            assert!(shown.contains(&format!("Checks {unknown}")));
            assert!(!shown.contains("Checks 0"));
        }
    }
}

#[test]
fn the_selected_result_is_visible_and_pointer_rows_match_the_viewport() {
    let rows = (0..8)
        .map(|i| Artifact::new(format!("Result {i}"), ArtifactKind::File, State::Done))
        .collect();
    let shelf = ArtifactShelf::new(rows).selected(7).focused(true);
    let area = Rect::new(4, 3, 40, 5);
    let theme = Profile::DarkTrue.theme();
    let mut buf = Buffer::empty(area);
    shelf.paint(area, &mut buf, &theme);
    let shown = text(&buf);
    assert!(
        shown.contains("Result 6") && shown.contains("Result 7"),
        "{shown}"
    );
    assert!(!shown.contains("Result 0"));
    assert!(shown.contains("6 artifacts not fully shown"));
    assert_eq!(shelf.row_at(area, 10, 3), Some(6));
    assert_eq!(shelf.row_at(area, 10, 6), Some(7));
    assert_eq!(shelf.row_at(area, 10, 7), None); // omission rail
    assert_eq!(shelf.row_at(area, 3, 3), None);
    assert_eq!(shelf.row_at(area, 44, 3), None);
    assert!(buf[(6, 5)].modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(4, 5)].fg, theme.fg(Role::Primary).fg.unwrap());
}

#[test]
fn partial_rows_and_host_omissions_are_counted_once() {
    let shelf = fixture().selected(0).omitted(3);
    let theme = Profile::DarkTrue.theme();
    let buf = render(40, 4, |area, buf| shelf.paint(area, buf, &theme));
    let shown = text(&buf);
    // One complete row and the next row's name fit. The partial row and
    // absent third row count once each, alongside three host omissions.
    assert!(shown.contains("Conversation changes"));
    assert!(shown.contains("5 artifacts not fully shown"), "{shown}");
    let buf = render(40, 1, |area, buf| shelf.paint(area, buf, &theme));
    assert!(text(&buf).contains("6 artifacts not fully shown"));
}

#[test]
fn a_nonzero_offset_reports_rows_above_the_view_even_with_spare_height() {
    // With no selection, a host's explicit scroll position is preserved.
    // A selected row instead uses ListState's keep-selection-visible rule.
    let shelf = ArtifactShelf::new(fixture().artifacts).offset(2);
    let theme = Profile::DarkTrue.theme();
    let buf = render(80, 12, |area, buf| shelf.paint(area, buf, &theme));
    let shown = text(&buf);
    assert!(shown.contains("Local verification"));
    assert!(shown.contains("2 artifacts not fully shown"));
    assert!(!shown.contains("Release checklist.md"));
}

#[test]
fn a_stale_offset_that_settles_to_a_full_view_does_not_claim_zero_omissions() {
    let shelf = fixture().selected(0).offset(usize::MAX);
    let theme = Profile::DarkTrue.theme();
    let buf = render(80, 8, |area, buf| shelf.paint(area, buf, &theme));
    let shown = text(&buf);
    assert!(shown.contains("Release checklist.md"));
    assert!(shown.contains("Local verification"));
    assert!(!shown.contains("artifacts not fully shown"));
}

#[test]
fn receipt_facts_keep_priority_over_long_paths_in_a_narrow_terminal() {
    let artifact = Artifact::new("Checklist", ArtifactKind::File, State::Done)
        .detail("docs/a-very-long-relative-path/checklist.md")
        .receipt("Size", ReceiptValue::Bytes(Some(4280)));
    for profile in Profile::ALL {
        let theme = profile.theme();
        let buf = render(40, 2, |area, buf| artifact.paint(area, buf, &theme));
        let shown = text(&buf);
        assert!(shown.contains(&format!(
            "Size {}",
            ReceiptValue::Bytes(Some(4280)).render(theme.ascii())
        )));
        assert!(shown.contains("Done"));
    }
}

#[test]
fn all_caller_fields_are_sanitized_and_ascii_fallbacks_keep_clusters_whole() {
    let words = ArtifactWords {
        file: Cow::Borrowed("Arch\u{202e}ivo"),
        omitted: Cow::Borrowed("oc\u{2066}ultos"),
        empty: Cow::Borrowed("Na\u{1b}da"),
        ..ArtifactWords::default()
    };
    let artifact = Artifact::new(
        "cafe\u{301} 鲸鱼\u{202e}.md",
        ArtifactKind::File,
        State::NeedsYou,
    )
    .status_word("Re\u{2067}visar")
    .detail("docs/\u{200e}report.md")
    .action("Abr\u{1b}ir")
    .receipt("Med\u{061c}ida", ReceiptValue::text("val\u{202d}or"))
    .words(&words);
    for profile in Profile::ALL {
        let theme = profile.theme();
        let buf = render(80, 2, |area, buf| artifact.paint(area, buf, &theme));
        let shown = text(&buf);
        assert!(shown.contains("Archivo"));
        assert!(shown.contains("Revisar"));
        assert!(shown.contains("Abrir"));
        assert!(shown.contains("docs/report.md"));
        assert!(shown.contains("Medida valor"));
        assert!(!shown.contains([
            '\u{202e}', '\u{2067}', '\u{200e}', '\u{061c}', '\u{202d}', '\u{1b}'
        ]));
        if profile == Profile::Ascii {
            assert!(shown.is_ascii());
            assert!(shown.contains("caf? ??.md"));
        } else {
            assert!(shown.contains("cafe\u{301} 鲸鱼.md"));
        }
        for cell in &buf.content {
            assert!(!cell.symbol().starts_with('\u{301}'));
        }
    }
    let shelf = ArtifactShelf::new(vec![artifact.clone()])
        .words(&words)
        .omitted(2);
    let theme = Profile::DarkTrue.theme();
    let buf = render(80, 3, |area, buf| shelf.paint(area, buf, &theme));
    assert!(text(&buf).contains("2 ocultos"));
    let empty = ArtifactShelf::new(Vec::new()).words(&words);
    let buf = render(40, 1, |area, buf| empty.paint(area, buf, &theme));
    assert!(text(&buf).contains("Nada"));
}

#[test]
fn unicode_cuts_preserve_the_name_and_action_hierarchy_at_useful_widths() {
    let artifact = Artifact::new(
        "cafe\u{301} 鲸鱼 terminal review with a long name.md",
        ArtifactKind::Review,
        State::NeedsYou,
    )
    .detail("A long review summary with cafe\u{301} and 鲸鱼")
    .action("Review");
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [1, 2, 5, 12, 40, 80] {
            let buf = render(width, 2, |area, buf| artifact.paint(area, buf, &theme));
            let shown = text(&buf);
            assert!(
                shown
                    .lines()
                    .all(|line| codewhale_ratatui::text::width(line) <= usize::from(width))
            );
            for cell in &buf.content {
                assert!(!cell.symbol().starts_with('\u{301}'));
            }
            if width >= 40 {
                assert!(shown.lines().next().unwrap().contains("Review"));
                assert!(shown.contains("Needs you"));
                if profile != Profile::Ascii {
                    assert!(shown.contains("cafe\u{301} 鲸鱼"));
                }
            }
        }
    }
}

#[test]
fn artifacts_reuse_existing_selectable_lists_without_another_key_framework() {
    let rows = vec![
        Artifact::new("Notes", ArtifactKind::Link, State::Ready).action("Open"),
        Artifact::new("Diff", ArtifactKind::Review, State::NeedsYou).action("Review"),
    ];
    let list = List::new(&rows, ListState::new(1));
    let theme = Profile::NoColor.theme();
    let buf = render(40, 4, |area, buf| list.paint(area, buf, &theme));
    assert!(text(&buf).contains("Diff"));
    assert!(buf[(2, 2)].modifier.contains(Modifier::BOLD));
    assert_eq!(list.row_at(Rect::new(0, 0, 40, 4), 5, 3), Some(1));
}

#[test]
fn every_size_and_off_buffer_request_paints_only_the_visible_intersection() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [0, 1, 2, 5, 40, 80] {
            for height in [0, 1, 2, 3, 6] {
                for area in [
                    Rect::new(4, 3, width, height),
                    Rect::new(0, 0, width, height),
                    Rect::new(40, 40, width, height),
                ] {
                    let surfaces: Vec<Box<dyn Paint>> = vec![
                        Box::new(fixture().selected(2).omitted(2)),
                        Box::new(
                            Artifact::new("cafe\u{301} 鲸鱼", ArtifactKind::File, State::Unknown)
                                .action("Open"),
                        ),
                        Box::new(ArtifactShelf::new(Vec::new())),
                    ];
                    for surface in surfaces {
                        let mut buf = Buffer::empty(Rect::new(2, 2, 10, 6));
                        for cell in &mut buf.content {
                            cell.set_symbol("z");
                        }
                        let before = buf.clone();
                        let clipped = area.intersection(buf.area);
                        let mut expected = before.clone();
                        surface.paint(area, &mut buf, &theme);
                        surface.paint(clipped, &mut expected, &theme);
                        assert_eq!(buf, expected, "{}: {area:?}", profile.name());
                        for y in buf.area.y..buf.area.bottom() {
                            for x in buf.area.x..buf.area.right() {
                                if !clipped.contains((x, y).into()) {
                                    assert_eq!(
                                        buf[(x, y)],
                                        before[(x, y)],
                                        "{}: {area:?}",
                                        profile.name()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn the_last_representable_terminal_edges_keep_unicode_and_actions_in_bounds() {
    let buffer_area = Rect::new(u16::MAX - 10, u16::MAX - 6, 10, 6);
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [1, 2, 9, 10, 40, 80] {
            for height in [1, 2, 6, 20] {
                let area = Rect::new(buffer_area.x, buffer_area.y, width, height);
                let surfaces: Vec<Box<dyn Paint>> = vec![
                    Box::new(fixture().selected(2).omitted(1)),
                    Box::new(
                        Artifact::new("cafe\u{301} 鲸鱼.md", ArtifactKind::File, State::NeedsYou)
                            .action("Review"),
                    ),
                ];
                for surface in surfaces {
                    let mut buf = Buffer::empty(buffer_area);
                    for cell in &mut buf.content {
                        cell.set_symbol("z");
                    }
                    let clipped = area.intersection(buf.area);
                    let mut expected = buf.clone();
                    surface.paint(area, &mut buf, &theme);
                    surface.paint(clipped, &mut expected, &theme);
                    assert_eq!(buf, expected, "{}: {area:?}", profile.name());
                }
            }
        }
    }
}
