use std::time::{Duration, Instant};

use codewhale_ratatui::{
    KeyHint, Paint, State, StatusMark, TOAST_ERROR_SECONDS, Toast, Toasts, Ttl,
    testing::{Profile, render, text},
};

#[test]
fn toast_presets_set_expected_states_and_ttls() {
    assert_eq!(TOAST_ERROR_SECONDS, 8);

    let ok = Toast::success("Saved settings");
    assert_eq!(ok.state, State::Done);
    assert_eq!(ok.ttl, Ttl::Seconds(5));
    assert_eq!(ok.action, None);

    let attn = Toast::attention("Approval waiting");
    assert_eq!(attn.state, State::NeedsYou);
    assert_eq!(attn.ttl, Ttl::UntilSeen);

    let err = Toast::error("Disk full");
    assert_eq!(err.state, State::Failed);
    assert_eq!(err.ttl, Ttl::UntilSeen);

    let timed = Toast::timed_error("Temporary network blip");
    assert_eq!(timed.state, State::Failed);
    assert_eq!(timed.ttl, Ttl::Seconds(TOAST_ERROR_SECONDS));
    let born = Instant::now();
    let timed = timed.born(born);
    assert!(timed.is_live(born + Duration::from_secs(7)));
    assert!(!timed.is_live(born + Duration::from_secs(8)));

    let info = Toast::info("Connected to local engine");
    assert_eq!(info.state, State::Ready);
    assert_eq!(info.ttl, Ttl::Seconds(5));
}

#[test]
fn toast_action_renders_and_preserves_status_mark_across_all_profiles_and_narrow_widths() {
    let items = vec![
        Toast::success("Theme set to Shoreline").action(KeyHint::new("u", "undo")),
        Toast::attention("Command waiting for approval")
            .action(KeyHint::new("Enter", "review"))
            .opens(),
        Toast::timed_error("Connection lost while streaming").action(KeyHint::new("r", "retry")),
        Toast::info("Workspace indexed").action(KeyHint::new("g", "open").disabled()),
    ];

    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in 10..=80 {
            let stack = Toasts::new(items.clone()).max_visible(4);
            let buf = render(width, 4, |area, buf| stack.paint(area, buf, &theme));
            let rendered = text(&buf);
            for (row, toast) in items.iter().enumerate() {
                let line = rendered.lines().nth(row).unwrap_or("");
                let mark = StatusMark::new(toast.state).glyph(&theme);
                assert!(
                    line.contains(mark),
                    "{} width={width} row={row} dropped status mark {mark:?}: {line:?}",
                    profile.name()
                );
            }
        }

        let wide = render(80, 4, |area, buf| {
            Toasts::new(items.clone())
                .max_visible(4)
                .paint(area, buf, &theme);
        });
        let wide_text = text(&wide);
        assert!(
            wide_text.contains("u undo"),
            "{}: {wide_text}",
            profile.name()
        );
        assert!(
            wide_text.contains("Enter review"),
            "{}: {wide_text}",
            profile.name()
        );
        assert!(
            wide_text.contains("r retry"),
            "{}: {wide_text}",
            profile.name()
        );
        assert!(
            wide_text.contains("g open"),
            "{}: {wide_text}",
            profile.name()
        );
    }
}
