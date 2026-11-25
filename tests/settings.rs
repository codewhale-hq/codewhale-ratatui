use codewhale_ratatui::{
    Paint, SettingDetail, SettingRow, SettingWords, SettingsRail, SettingsSection,
    section_match_counts,
    testing::{self, Profile},
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};
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

#[test]
fn setting_row_show_default_apply_suppresses_default_fact_on_unmodified_rows() {
    let theme = Profile::DarkTrue.theme();
    let default_row = SettingRow::new("Theme", "Shoreline");
    let default_text: String = default_row
        .lines(80, &theme)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(default_text.contains("Applies now"), "{default_text}");

    let quiet = SettingRow::new("Theme", "Shoreline").show_default_apply(false);
    let quiet_text: String = quiet
        .lines(80, &theme)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(!quiet_text.contains("Applies now"), "{quiet_text}");

    let modified = SettingRow::new("Theme", "Shoreline")
        .modified(true)
        .show_default_apply(false);
    let modified_text: String = modified
        .lines(80, &theme)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(modified_text.contains("Applies now"), "{modified_text}");

    let explicit = SettingRow::new("Theme", "Shoreline")
        .apply("Applies next session")
        .show_default_apply(false);
    let explicit_text: String = explicit
        .lines(80, &theme)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        explicit_text.contains("Applies next session"),
        "{explicit_text}"
    );
}

#[test]
fn settings_rail_counts_dimming_and_clear_cue_work_across_profiles() {
    assert_eq!(section_match_counts(4, &[0, 2, 0, 99, 2]), vec![2, 0, 2, 0]);
    let sections = [
        SettingsSection::new("General", 3),
        SettingsSection::new("Providers", 0),
        SettingsSection::new("Models", 2),
    ];
    let rail = SettingsRail::new(&sections, 0).searching(true);
    for profile in Profile::ALL {
        let theme = profile.theme();
        assert_eq!(rail.height(24, &theme), 4);
        let buf = testing::render(24, 5, |area, buf| rail.paint(area, buf, &theme));
        let shown = testing::text(&buf);
        assert!(shown.contains("General"), "{}: {shown}", profile.name());
        assert!(shown.contains('3'), "{}: {shown}", profile.name());
        assert!(shown.contains("Providers"), "{}: {shown}", profile.name());
        assert!(shown.contains("Esc clear"), "{}: {shown}", profile.name());
        let providers_line = shown.lines().nth(1).unwrap_or_default();
        let col = providers_line.find("Providers").expect("Providers painted");
        assert!(
            buf[(u16::try_from(col).unwrap_or(0), 1)]
                .modifier
                .contains(Modifier::DIM),
            "{}: zero-match section must dim while searching",
            profile.name()
        );
    }
    testing::assert_rules(5, |area, buf, theme| rail.paint(area, buf, theme));
}

#[test]
fn settings_rail_handles_narrow_widths_and_clipping() {
    let sections = [
        SettingsSection::new("Appearance\u{202e}", 4),
        SettingsSection::new("Keyboard", 0),
    ];
    let rail = SettingsRail::new(&sections, 1).searching(true);
    let theme = Profile::Ascii.theme();
    for width in [0, 1, 4, 10, 24] {
        let mut buf = Buffer::empty(Rect::new(0, 0, 28, 8));
        let area = Rect::new(2, 2, width, 4);
        rail.paint(area, &mut buf, &theme);
        for y in 0..8 {
            for x in 0..28 {
                if !area.contains((x, y).into()) {
                    assert_eq!(buf[(x, y)].symbol(), " ");
                }
            }
        }
    }
    let buf = testing::render(24, 4, |area, buf| rail.paint(area, buf, &theme));
    let shown = testing::text(&buf);
    assert!(!shown.contains('\u{202e}'));
    assert!(shown.contains("Appearance"));
}
