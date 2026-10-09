use codewhale_ratatui::{
    Paint, SettingDetail, SettingRow, SettingWords,
    testing::{self, Profile},
};
use ratatui::{buffer::Buffer, layout::Rect};
use unicode_segmentation::UnicodeSegmentation;

fn styled_graphemes(lines: &[ratatui::text::Line<'_>]) -> Vec<(String, ratatui::style::Style)> {
    let mut result = Vec::new();
    for line in lines {
        for span in &line.spans {
            for grapheme in span.content.graphemes(true) {
                result.push((grapheme.to_string(), span.style));
            }
        }
    }
    result
}

#[test]
fn settings_preserve_value_source_and_apply_facts_when_narrow() {
    let theme = Profile::DarkTrue.theme();
    let row = SettingRow::new("Theme", "Shoreline")
        .source("this project")
        .apply("Applies next session")
        .changed(true)
        .modified(true)
        .selected(true);
    let baseline = row.lines(300, &theme);
    let unwrapped: String = baseline.iter().map(ToString::to_string).collect();
    for width in [4, 20, 40, 80] {
        let lines = row.lines(width, &theme);
        let all: String = lines.iter().map(ToString::to_string).collect();
        assert_eq!(all, unwrapped, "wrapping at {width} changes setting facts");
        assert_eq!(styled_graphemes(&lines), styled_graphemes(&baseline));
        for fact in [
            "Theme",
            "Shoreline",
            "this project",
            "Applies next session",
            "changed",
            "r reset to default",
        ] {
            assert!(all.contains(fact), "{width}: {all}");
        }
        assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
        assert_eq!(row.height(width, &theme) as usize, lines.len());
    }
}

#[test]
fn settings_description_wraps_whole_words_when_they_fit() {
    let theme = Profile::DarkTrue.theme();
    let description =
        "Sets the colors Codewhale paints. Shoreline follows your terminal's light or dark ground.";
    let detail = SettingDetail::new("Theme", description);
    for width in [20, 40, 80] {
        let lines = detail.lines(width, &theme);
        let all: String = lines.iter().map(ToString::to_string).collect();
        assert_eq!(all, format!("Theme{description}Applies now"));
        assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
        assert!(
            lines.iter().any(|line| line.to_string().contains("dark")),
            "dark was split at {width}: {lines:?}"
        );
        assert_eq!(detail.height(width, &theme) as usize, lines.len());
    }
}

#[test]
fn settings_long_words_wrap_at_whole_graphemes() {
    let theme = Profile::DarkTrue.theme();
    let description = "鲸鱼 cafe\u{301} extraordinarilylongword";
    let detail = SettingDetail::new("Theme", description);
    for width in [2, 4, 8] {
        let lines = detail.lines(width, &theme);
        let all: String = lines.iter().map(ToString::to_string).collect();
        assert_eq!(all, format!("Theme{description}Applies now"));
        assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
        assert!(
            lines
                .iter()
                .any(|line| line.to_string().contains("e\u{301}"))
        );
    }
    let lines = detail.lines(1, &theme);
    let all: String = lines.iter().map(ToString::to_string).collect();
    assert_eq!(
        all,
        "Theme?? cafe\u{301} extraordinarilylongwordApplies now"
    );
    assert!(lines.iter().all(|line| line.width() <= 1));
}

#[test]
fn locked_settings_explain_the_reason_without_promising_reset() {
    let theme = Profile::DarkTrue.theme();
    let row = SettingRow::new("Context", "200k tokens")
        .locked("Set by your admin")
        .modified(true);
    let all: String = row
        .lines(80, &theme)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(all.contains("Set by your admin"));
    assert!(!all.contains("reset"));
    let detail = SettingDetail::new("Context", "Limits the context available to each turn.")
        .locked("Set by your admin")
        .modified(true);
    let all: String = detail
        .lines(80, &theme)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(all.contains("Set by your admin"));
    assert!(!all.contains("reset"));
}

#[test]
fn localized_settings_and_untrusted_text_are_safe() {
    let words = SettingWords {
        applies_now: "Gilt sofort".into(),
        changed: "geändert".into(),
        reset: "zurücksetzen".into(),
        default: "Standard".into(),
    };
    let theme = Profile::DarkTrue.theme();
    let row = SettingRow::new("鲸鱼\u{202e}", "cafe\u{301}")
        .with_words(&words)
        .changed(true)
        .modified(true);
    let all: String = row
        .lines(20, &theme)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(all.contains("鲸鱼"));
    assert!(all.contains("cafe\u{301}"));
    assert!(!all.contains('\u{202e}'));
    assert!(all.contains("Gilt sofort"));
    assert!(all.contains("geändert"));
    assert!(all.contains("zurücksetzen"));
    let detail = SettingDetail::new("Theme", "Line one\nLine two")
        .with_words(&words)
        .default_value("System");
    let all: String = detail
        .lines(40, &theme)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(all.contains("Standard: System"));
}

#[test]
fn settings_follow_profiles_and_never_paint_outside_the_area() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        let row = SettingRow::new("Theme", "Shoreline")
            .selected(true)
            .source("this project");
        let detail = SettingDetail::new("Theme", "Sets the colors used by the terminal.")
            .default_value("System");
        for width in [0, 1, 4, 20] {
            for component in [&row as &dyn Paint, &detail as &dyn Paint] {
                let mut buf = Buffer::empty(Rect::new(0, 0, 28, 18));
                let area = Rect::new(3, 4, width, 8);
                component.paint(area, &mut buf, &theme);
                for y in 0..18 {
                    for x in 0..28 {
                        if !area.contains((x, y).into()) {
                            assert_eq!(buf[(x, y)].symbol(), " ");
                        }
                    }
                }
            }
        }
    }
    testing::assert_rules(4, |area, buf, theme| {
        SettingRow::new("Theme", "Shoreline")
            .selected(true)
            .modified(true)
            .paint(area, buf, theme)
    });
}

#[test]
fn settings_clip_paint_to_buffers_with_nonzero_origins() {
    let theme = Profile::DarkTrue.theme();
    let row = SettingRow::new("Theme", "Shoreline").selected(true);
    let detail = SettingDetail::new("Theme", "Colors for the terminal.");
    let buffer_area = Rect::new(8, 6, 12, 8);
    for requested in [
        Rect::new(0, 0, 11, 8),
        Rect::new(18, 12, 8, 8),
        Rect::new(24, 24, 4, 4),
    ] {
        for component in [&row as &dyn Paint, &detail as &dyn Paint] {
            let mut buf = Buffer::empty(buffer_area);
            let untouched = buf.clone();
            component.paint(requested, &mut buf, &theme);
            let painted = requested.intersection(buffer_area);
            for y in buffer_area.y..buffer_area.bottom() {
                for x in buffer_area.x..buffer_area.right() {
                    if !painted.contains((x, y).into()) {
                        assert_eq!(buf[(x, y)], untouched[(x, y)]);
                    }
                }
            }
        }
    }
}
