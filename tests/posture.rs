use codewhale_ratatui::{
    MetricKind, MetricSegment, MetricsLine, Paint, PostureBar, PostureFact, Role,
    testing::{Profile, render, text},
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

fn working_posture() -> PostureBar<'static> {
    PostureBar::new("ask")
        .permission_key("Shift+Tab to change")
        .mode("work", Role::Primary)
        .mode_key("Tab")
        .turn_clock("working 1m 15s", Role::Live)
        .counts(vec![PostureFact::new("2 agents", Role::Live)])
        .session_clock("worked 41m 12s", Role::Live)
        .hint("Esc to interrupt", Role::Hint)
        .context_percent(61)
}

fn working_metrics() -> MetricsLine<'static> {
    MetricsLine::new(vec![
        MetricSegment::new(MetricKind::Model, "", "deepseek-v4").role(Role::Primary),
        MetricSegment::new(MetricKind::Context, "ctx", "61%").role(Role::Primary),
        MetricSegment::new(MetricKind::Cost, "", "$0.42"),
        MetricSegment::new(MetricKind::Ttft, "ttft", "400ms"),
        MetricSegment::new(MetricKind::Rate, "", "38 tok/s"),
        MetricSegment::new(MetricKind::OutputTokens, "↓", "1.2K"),
    ])
    .help_hint("/help")
}

fn row(component: &impl Paint, width: u16, profile: Profile) -> String {
    let theme = profile.theme();
    text(&render(width, 1, |area, buf| {
        component.paint(area, buf, &theme)
    }))
}

#[test]
fn posture_matches_the_current_native_golden_rows() {
    // Current TUI goldens/footer_{80x24,100x30,120x32,160x40}.txt.
    // Keep this literal native row grammar independent of kit composition.
    let full = " ● ask  Shift+Tab to change   work (Tab)   working 1m 15s   2 agents   worked 41m 12s   Esc to interrupt";
    for (width, expected) in [
        (
            80,
            " ● ask  Shift+Tab to change   work (Tab)   2 agents   Esc to interrupt",
        ),
        (
            100,
            " ● ask  Shift+Tab to change   work (Tab)   2 agents   worked 41m 12s   Esc to interrupt",
        ),
        (120, full),
        (160, full),
    ] {
        assert_eq!(
            row(&working_posture(), width, Profile::DarkTrue).trim_end(),
            expected
        );
    }
}

#[test]
fn metrics_match_the_current_native_golden_rows_and_pinned_help() {
    let prefix = "deepseek-v4   ctx 61%   $0.42   ttft 400ms   38 tok/s   ↓ 1.2K";
    for width in [80, 100, 120, 160] {
        let shown = row(&working_metrics(), width, Profile::DarkTrue);
        let expected = format!(
            "{prefix}{}{}",
            " ".repeat(usize::from(width) - codewhale_ratatui::text::width(prefix) - 5),
            "/help"
        );
        assert_eq!(shown.trim_end(), expected);
    }
}

#[test]
fn native_posture_shed_order_retains_permission_before_mode() {
    let fixture = working_posture();
    let first_width = |needle: &str| {
        (8..=160)
            .find(|width| row(&fixture, *width, Profile::DarkTrue).contains(needle))
            .unwrap()
    };
    let rungs = [
        "working 1m 15s",
        "worked 41m 12s",
        "Shift+Tab to change",
        "Esc to interrupt",
        "2 agents",
        "work (Tab)",
        "   work",
    ]
    .map(first_width);
    assert!(rungs.windows(2).all(|pair| pair[0] > pair[1]), "{rungs:?}");
    let permission = PostureBar::new("full access").mode("operate", Role::Primary);
    let mut permission_only = false;
    for width in 8..=120 {
        let shown = row(&permission, width, Profile::DarkTrue);
        assert!(
            !shown.contains("operate") || shown.contains("full access"),
            "{width}: {shown}"
        );
        permission_only |= shown.contains("full access") && !shown.contains("operate");
    }
    assert!(permission_only);
}

#[test]
fn cap_warning_and_native_compact_policy_outrank_live_clocks() {
    let fixture = working_posture().context_percent(83);
    let mut warning_only = false;
    for width in 8..=160 {
        let shown = row(&fixture, width, Profile::DarkTrue);
        assert!(
            !shown.contains("worked 41m 12s") || shown.contains("surface soon"),
            "{width}: {shown}"
        );
        assert!(!shown.contains("Esc to interrupt"));
        assert!(!shown.contains("83%"));
        warning_only |= shown.contains("surface soon") && !shown.contains("worked 41m 12s");
    }
    assert!(warning_only);
    let compact = row(
        &fixture.compact(true).right("/rc connected", Role::Primary),
        160,
        Profile::DarkTrue,
    );
    for kept in [" ● ask", "   work (Tab)", "surface soon", "/rc connected"] {
        assert!(compact.contains(kept), "{compact}");
    }
    for gone in [
        "working 1m 15s",
        "worked 41m 12s",
        "Shift+Tab to change",
        "2 agents",
    ] {
        assert!(!compact.contains(gone), "{compact}");
    }
}

#[test]
fn a_turn_only_clock_keeps_the_native_session_clock_priority() {
    let mut turn_only = working_posture();
    turn_only.session_clock = None;
    let both = working_posture();
    let mut checked = false;
    for width in 8..=160 {
        let shown_both = row(&both, width, Profile::DarkTrue);
        if shown_both.contains("worked 41m 12s") && !shown_both.contains("working 1m 15s") {
            assert!(row(&turn_only, width, Profile::DarkTrue).contains("working 1m 15s"));
            checked = true;
        }
    }
    assert!(checked);
}

#[test]
fn metrics_shed_secondary_counts_then_help_and_keep_model_and_context() {
    let fixture = working_metrics();
    let theme = Profile::DarkTrue.theme();
    for width in 23..=160 {
        let hits = fixture.hitboxes(Rect::new(0, 0, width, 1), &theme);
        assert!(hits.iter().any(|hit| hit.kind == MetricKind::Model));
        assert!(hits.iter().any(|hit| hit.kind == MetricKind::Context));
        let shown = row(&fixture, width, Profile::DarkTrue);
        assert!(
            shown.contains("deepseek-v4") && shown.contains("ctx 61%"),
            "{width}: {shown}"
        );
    }
    let mut saw_help_without_tokens = false;
    for width in 23..=160 {
        let shown = row(&fixture, width, Profile::DarkTrue);
        saw_help_without_tokens |= shown.contains("/help") && !shown.contains("1.2K");
    }
    assert!(saw_help_without_tokens);
    let compact = row(&fixture.compact(true), 160, Profile::DarkTrue);
    assert!(
        compact.contains("400ms") && compact.contains("38 tok/s"),
        "{compact}"
    );
    assert!(
        !compact.contains("/help") && !compact.contains("1.2K"),
        "{compact}"
    );
}

#[test]
fn counts_are_a_comma_group_and_hitboxes_match_native_inset_cells() {
    let theme = Profile::DarkTrue.theme();
    let component = PostureBar::new("ask")
        .mode("work", Role::Primary)
        .counts(vec![
            PostureFact::new("2 agents", Role::Live),
            PostureFact::new("1 task", Role::Primary),
        ]);
    let area = Rect::new(4, 7, 80, 2);
    let mut buf = Buffer::empty(area);
    component.paint(area, &mut buf, &theme);
    assert!(text(&buf).contains(" ● ask   work   2 agents, 1 task"));
    let hits = component.count_hitboxes(area, &theme);
    assert_eq!(
        hits,
        vec![(0, Rect::new(20, 7, 8, 1)), (1, Rect::new(30, 7, 6, 1))]
    );
    for (_, hit) in hits {
        let words: String = (hit.x..hit.right())
            .map(|x| buf[(x, hit.y)].symbol())
            .collect();
        assert!(words == "2 agents" || words == "1 task", "{words}");
    }
}

#[test]
fn metrics_geometry_hover_and_warning_ink_follow_the_native_component() {
    let theme = Profile::DarkTrue.theme();
    let area = Rect::new(3, 5, 80, 1);
    let metrics = working_metrics().hovered(MetricKind::Context);
    let mut buf = Buffer::empty(area);
    metrics.paint(area, &mut buf, &theme);
    let context = metrics.context_hitbox(area, &theme).unwrap();
    assert_eq!(context, Rect::new(17, 5, 7, 1));
    assert!(!buf[(17, 5)].modifier.contains(Modifier::UNDERLINED));
    assert!(
        buf[(21, 5)]
            .modifier
            .contains(Modifier::BOLD | Modifier::UNDERLINED)
    );
    let cost = working_metrics().hovered(MetricKind::Cost);
    let mut cost_buf = Buffer::empty(area);
    cost.paint(area, &mut cost_buf, &theme);
    let hit = cost
        .hitboxes(area, &theme)
        .into_iter()
        .find(|hit| hit.kind == MetricKind::Cost)
        .unwrap();
    assert!(
        !cost_buf[(hit.area.x, hit.area.y)]
            .modifier
            .contains(Modifier::UNDERLINED)
    );
    let warning = MetricsLine::new(vec![
        MetricSegment::new(MetricKind::Context, "ctx", "91%").role(Role::Danger),
    ]);
    let warning_buf = render(20, 1, |area, buf| warning.paint(area, buf, &theme));
    assert_eq!(warning_buf[(0, 0)].fg, theme.fg(Role::Danger).fg.unwrap());
    assert_eq!(warning_buf[(4, 0)].fg, theme.fg(Role::Danger).fg.unwrap());
}

#[test]
fn native_posture_truncation_uses_three_dots_and_protects_permission() {
    let component = PostureBar::new("full access")
        .mode("operate", Role::Primary)
        .right("Auto-denied exec_shell", Role::Attention);
    let shown = row(&component, 30, Profile::DarkTrue);
    assert!(shown.contains("full access"), "{shown}");
    assert!(shown.trim_end().ends_with("..."), "{shown}");
    assert!(!shown.contains('…'));
    for width in 8..=160 {
        let shown = row(&component, width, Profile::DarkTrue);
        if shown.contains("operate") {
            assert!(shown.contains("full access"));
        }
    }
}

#[test]
fn caller_text_is_safe_and_authored_marks_follow_ascii_policy() {
    let posture = PostureBar::new("ask\u{202e}")
        .mode("work\u{1b}", Role::Primary)
        .context_percent(90);
    let metrics = MetricsLine::new(vec![
        MetricSegment::new(MetricKind::Model, "", "cafe\u{301} 鲸鱼\u{2066}").role(Role::Primary),
        MetricSegment::new(MetricKind::OutputTokens, "↓", "1.2K\u{1b}"),
    ]);
    let rich = row(&metrics, 80, Profile::DarkTrue);
    assert!(rich.contains("cafe\u{301} 鲸鱼"), "{rich}");
    assert!(!rich.contains(['\u{202e}', '\u{2066}', '\u{1b}']));
    let ascii_posture = row(&posture, 80, Profile::Ascii);
    assert!(
        ascii_posture.starts_with(" . ask") && ascii_posture.contains("^ surface soon - /compact"),
        "{ascii_posture}"
    );
    assert!(ascii_posture.is_ascii());
    let ascii_metrics = row(&working_metrics(), 112, Profile::Ascii);
    assert!(
        ascii_metrics.contains("v 1.2K") && ascii_metrics.is_ascii(),
        "{ascii_metrics}"
    );
}

#[test]
fn every_profile_and_clipped_buffer_is_bounded_and_only_one_row_paints() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [0, 1, 3, 7, 8, 12, 40, u16::MAX] {
            for height in [0, 1, 2, 6] {
                let area = Rect::new(4, 3, width, height);
                let surfaces: [&dyn Paint; 2] = [&working_posture(), &working_metrics()];
                for surface in surfaces {
                    let mut buf = Buffer::empty(Rect::new(2, 2, 30, 6));
                    for cell in &mut buf.content {
                        cell.set_symbol("z");
                    }
                    let before = buf.clone();
                    surface.paint(area, &mut buf, &theme);
                    let clip = area.intersection(buf.area);
                    for y in buf.area.y..buf.area.bottom() {
                        for x in buf.area.x..buf.area.right() {
                            if !clip.contains((x, y).into()) || y != clip.y {
                                assert_eq!(
                                    buf[(x, y)],
                                    before[(x, y)],
                                    "{} {width}x{height} ({x},{y})",
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
