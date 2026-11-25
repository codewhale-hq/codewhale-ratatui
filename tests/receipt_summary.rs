use std::time::Duration;

use codewhale_ratatui::{
    Cost, Paint, TurnSummary, TurnSummaryWords,
    testing::{self, Profile},
};
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn turn_summary_formats_duration_steps_tokens_and_cost() {
    let summary = TurnSummary::new(Duration::from_secs(134), 6)
        .tokens(12_400)
        .cost(Cost::usd_cents(14));

    assert_eq!(
        summary.render_text(false),
        "Worked for 2m 14s \u{00b7} 6 steps \u{00b7} 12.4k tokens \u{00b7} $0.14"
    );
    assert_eq!(
        summary.render_text(true),
        "Worked for 2m 14s - 6 steps - 12.4k tokens - $0.14"
    );

    let single = TurnSummary::new(Duration::from_secs(5), 1);
    assert_eq!(single.render_text(false), "Worked for 5 s \u{00b7} 1 step");

    let custom = TurnSummary::new(Duration::from_secs(12), 2).words(TurnSummaryWords {
        worked_for: "Ran for".into(),
        step_one: "call".into(),
        step_many: "calls".into(),
        tokens: "tok".into(),
    });
    assert_eq!(custom.render_text(true), "Ran for 12 s - 2 calls");
}

#[test]
fn turn_summary_paints_safely_across_all_profiles_and_narrow_widths() {
    let summary = TurnSummary::new(Duration::from_secs(134), 6)
        .tokens(12_400)
        .cost(Cost::usd_cents(14));

    for profile in Profile::ALL {
        let theme = profile.theme();
        assert_eq!(summary.height(0, &theme), 0);
        assert_eq!(summary.height(40, &theme), 1);
        for width in [0u16, 1, 8, 16, 32, 60, 80] {
            let mut buf = Buffer::empty(Rect::new(0, 0, width, 1));
            summary.paint(buf.area, &mut buf, &theme);
            let shown = testing::text(&buf);
            if profile == Profile::Ascii {
                assert!(shown.is_ascii(), "non-ASCII at {width}: {shown:?}");
            }
            if width >= 60 {
                assert!(shown.contains("Worked for 2m 14s"));
                assert!(shown.contains("6 steps"));
            }
        }
    }
}
