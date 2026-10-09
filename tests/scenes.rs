use codewhale_ratatui::{
    gallery,
    testing::{self, Profile, text},
};

#[test]
fn project_scene_has_real_readouts_and_a_usable_composer() {
    let entries = gallery::entries();
    let project = entries
        .iter()
        .find(|entry| entry.name == "workspace-scene")
        .unwrap();
    for profile in Profile::ALL {
        let shown = text(&gallery::render(project, &profile.theme()));
        for expected in ["Show me the review", "Files & review", "Needs you"] {
            assert!(
                shown.contains(expected),
                "{} is missing {expected}:\n{shown}",
                profile.name()
            );
        }
    }
}

#[test]
fn narrow_project_keeps_composer_and_reports_where_the_dock_went() {
    let entries = gallery::entries();
    let project = entries
        .iter()
        .find(|entry| entry.name == "workspace-scene-narrow")
        .unwrap();
    for profile in Profile::ALL {
        let shown = text(&gallery::render(project, &profile.theme()));
        assert!(shown.contains("Show me the review"), "{shown}");
        assert!(
            shown.contains("going."),
            "the entire draft must remain readable: {shown}"
        );
        assert!(shown.contains("Tab opens files"), "{shown}");
        assert!(!shown.contains("Files & review"));
    }
}

#[test]
fn review_scene_contains_exact_decision_and_unknown_evidence() {
    let entries = gallery::entries();
    let review = entries
        .iter()
        .find(|entry| entry.name == "review-scene")
        .unwrap();
    let shown = text(&gallery::render(review, &Profile::DarkTrue.theme()));
    assert!(shown.contains("git diff --check"), "{shown}");
    assert!(
        shown.contains("Allow once") && shown.contains("Deny"),
        "{shown}"
    );
    assert!(shown.contains("Run results"), "{shown}");
    assert!(
        shown.contains('—'),
        "unreported measurements must remain unknown"
    );
}

#[test]
fn every_new_fixture_obeys_terminal_profiles_and_responsive_widths() {
    for entry in gallery::entries() {
        if entry.name.contains("scene")
            || entry.name.starts_with("workbar")
            || entry.name.starts_with("context-ribbon")
            || entry.name == "workbench-frame"
            || entry.name == "pane-header"
            || entry.name.starts_with("artifact")
            || entry.name.starts_with("attention")
            || entry.name == "fish-school"
            || entry.name == "jellyfish"
            || entry.name == "bubble-field"
            || entry.name.starts_with("habitat")
        {
            testing::assert_rules(entry.height, entry.draw);
        }
    }
}
