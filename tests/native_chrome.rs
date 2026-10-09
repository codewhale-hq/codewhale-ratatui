use std::time::Duration;

use codewhale_ratatui::{
    NativeComposer, NativeComposerDensity, Paint, TuiPalette, WorkflowProgress, WorkflowRun,
    WorkflowRunState,
    testing::{Profile, render, text},
};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

#[test]
fn running_progress_matches_the_current_native_workbar_row() {
    let theme = Profile::DarkTrue.theme();
    let progress = WorkflowProgress::new(vec![
        WorkflowRun::new("Compare Cline with Codewhale", WorkflowRunState::Running)
            .outcomes(4, 1, 0, 10)
            .elapsed(Duration::from_secs(134))
            .tokens(1_234_567),
    ]);
    let buf = render(110, 1, |area, buf| progress.paint(area, buf, &theme));
    assert_eq!(
        text(&buf),
        " • Compare Cline with Codewhale  ████████××░░░░░░░░░░  4/10 done · 1 failed  2m 14s  ↓1.2M"
    );
    let narrow = render(40, 1, |area, buf| progress.paint(area, buf, &theme));
    let narrow = text(&narrow);
    assert!(
        narrow.contains("4/10 done") && narrow.contains("1 failed"),
        "{narrow}"
    );
    assert!(!narrow.contains(['█', '×', '░', '↓']), "{narrow}");
}

#[test]
fn failed_agents_never_fill_the_success_segment_or_round_fast_work_to_zero() {
    let theme = Profile::DarkTrue.theme();
    let progress = WorkflowProgress::new(vec![
        WorkflowRun::new("Release-readiness audit", WorkflowRunState::Failed)
            .outcomes(0, 2, 0, 2)
            .elapsed(Duration::from_millis(355))
            .reason("[auth] Authorization failed: sign in again. Nothing ran."),
    ]);
    let buf = render(140, 1, |area, buf| progress.paint(area, buf, &theme));
    let shown = text(&buf);
    assert_eq!(shown.matches('×').count(), 20, "{shown}");
    assert!(!shown.contains('█'));
    assert!(shown.contains("0/2 done · 2 failed"));
    assert!(shown.contains("355ms"));
    assert!(shown.contains("Authorization failed: sign in again"));
    assert!(!shown.contains("[auth]") && !shown.contains("Nothing ran"));
    assert!(
        buf.content
            .iter()
            .filter(|c| c.symbol() == "×")
            .all(|c| c.fg == theme.color(codewhale_ratatui::Role::Danger).unwrap())
    );
}

#[test]
fn queued_large_and_folded_run_facts_follow_native_thresholds() {
    let theme = Profile::DarkTrue.theme();
    let large = WorkflowProgress::new(vec![
        WorkflowRun::new("large", WorkflowRunState::Running)
            .outcomes(0, 1, 0, 25)
            .queued(2),
    ]);
    let buf = render(120, 1, |area, buf| large.paint(area, buf, &theme));
    let shown = text(&buf);
    assert!(shown.contains("Large workflow") && shown.contains("2 queued"));
    assert!(
        shown.contains('×'),
        "one failed agent of 25 still owns a visible cell"
    );
    let small = WorkflowProgress::new(vec![
        WorkflowRun::new("small", WorkflowRunState::Running).outcomes(0, 0, 0, 24),
    ]);
    let buf = render(120, 1, |area, buf| small.paint(area, buf, &theme));
    assert!(!text(&buf).contains("Large workflow"));
    assert!(!text(&buf).contains("queued"));
    let runs = (0..12)
        .map(|n| {
            WorkflowRun::new(format!("run {n}"), WorkflowRunState::Running).outcomes(0, 0, 0, 2)
        })
        .collect();
    let many = WorkflowProgress::new(runs);
    assert_eq!(many.desired_rows(), 7);
    let buf = render(100, 7, |area, buf| many.paint(area, buf, &theme));
    assert_eq!(text(&buf).lines().last(), Some(" +6 more · ↓ to manage"));
    let buf = render(100, 1, |area, buf| many.paint(area, buf, &theme));
    assert_eq!(text(&buf), " +12 more · ↓ to manage");
}

#[test]
fn composer_density_enclosure_and_send_geometry_match_the_mounted_policy() {
    for (density, quiet, enclosed, cap) in [
        (NativeComposerDensity::Compact, 2, 3, 7),
        (NativeComposerDensity::Comfortable, 3, 4, 9),
        (NativeComposerDensity::Spacious, 4, 5, 12),
    ] {
        let composer = NativeComposer::new("").density(density);
        assert_eq!(
            composer.clone().enclosed(false).desired_height(80, 20),
            quiet
        );
        assert_eq!(composer.desired_height(80, 20), enclosed);
        assert_eq!(
            NativeComposer::new("x\n".repeat(40))
                .density(density)
                .desired_height(80, 60),
            cap
        );
        assert_eq!(composer.desired_height(80, 1), 1);
    }
    let composer = NativeComposer::new("ship it")
        .focused(true)
        .can_submit(true);
    for width in 1..12 {
        let area = Rect::new(10, 20, width, 4);
        assert!(!composer.has_panel(area));
        assert!(composer.geometry(area).submit.is_none());
    }
    let area = Rect::new(10, 20, 40, 4);
    let geometry = composer.geometry(area);
    assert_eq!(geometry.submit, Some(Rect::new(45, 22, 3, 1)));
    assert_eq!(geometry.inner, Rect::new(11, 21, 33, 2));
    assert_eq!(geometry.text, Rect::new(13, 21, 31, 2));
    assert!(geometry.text.right() < geometry.submit.unwrap().x);
    let mut buf = Buffer::empty(area);
    composer.paint(area, &mut buf, &Profile::DarkTrue.theme());
    assert_eq!(buf[(10, 20)].symbol(), "╭");
    assert_eq!(buf[(49, 23)].symbol(), "╯");
    assert_eq!(buf[(45, 22)].symbol(), "[");
    assert_eq!(buf[(46, 22)].symbol(), "↵");
    assert_eq!(buf[(47, 22)].symbol(), "]");
}

#[test]
fn composer_uses_the_native_background_info_soft_and_agent_target_slots() {
    let source: serde_json::Value =
        serde_json::from_str(include_str!("../assets/tui-palettes.json")).unwrap();
    let source = source.as_array().unwrap();
    let slot = |record: &serde_json::Value, key: &str| {
        let value = &record["slots"][key];
        if let Some(rgb) = value.as_array() {
            Color::Rgb(
                rgb[0].as_u64().unwrap().try_into().unwrap(),
                rgb[1].as_u64().unwrap().try_into().unwrap(),
                rgb[2].as_u64().unwrap().try_into().unwrap(),
            )
        } else {
            match value.as_str().unwrap() {
                "Color::Reset" => Color::Reset,
                "Color::Cyan" => Color::Cyan,
                "Color::Yellow" => Color::Yellow,
                color => panic!("unexpected native slot {key}: {color}"),
            }
        }
    };
    for palette in TuiPalette::ALL {
        let record = source
            .iter()
            .find(|record| record["name"] == palette.name())
            .unwrap();
        let theme = Profile::DarkTrue.theme().tui_palette(palette);
        let composer = NativeComposer::new("")
            .focused(true)
            .placeholder("Write a task")
            .can_submit(true)
            .target("Builder");
        let area = Rect::new(0, 0, 40, 4);
        let geometry = composer.geometry(area);
        let buf = render(area.width, area.height, |area, buf| {
            composer.paint(area, buf, &theme)
        });
        let expected_background = slot(record, "composer_bg");
        assert!(
            buf.content
                .iter()
                .all(|cell| cell.bg == expected_background),
            "{} composer ground must include its complete chrome",
            palette.name()
        );
        let submit = geometry.submit.unwrap();
        assert_eq!(
            buf[(submit.x + 1, submit.y)].fg,
            slot(record, "info"),
            "{} ready submit ink",
            palette.name()
        );
        assert_eq!(
            buf[(geometry.text.x, geometry.text.y)].fg,
            slot(record, "text_soft"),
            "{} idle prompt ink",
            palette.name()
        );
        let target = buf
            .content
            .iter()
            .find(|cell| cell.symbol() == "B")
            .unwrap();
        assert_eq!(
            target.fg,
            slot(record, "accent_action"),
            "{} agent target",
            palette.name()
        );
        if palette == TuiPalette::ShorelineLight {
            assert_ne!(expected_background, slot(record, "panel_bg"));
            assert_ne!(expected_background, slot(record, "elevated_bg"));
        }
    }
}

#[test]
fn caller_unicode_lines_and_caret_survive_while_every_field_loses_controls() {
    let theme = Profile::DarkTrue.theme();
    let composer = NativeComposer::new("cafe\u{301} 鲸鱼\u{202e}\n\nlast\u{1b}")
        .focused(true)
        .can_submit(true)
        .cursor(0)
        .submit_hint("Enter\u{202e} send")
        .target("reviewer\u{1b}");
    let area = Rect::new(0, 0, 60, 7);
    let buf = render(area.width, area.height, |area, buf| {
        composer.paint(area, buf, &theme)
    });
    let shown = text(&buf);
    assert!(shown.contains("cafe\u{301} 鲸鱼"), "{shown}");
    assert!(shown.contains("last"));
    assert!(!shown.contains(['\u{202e}', '\u{1b}']));
    let caret = composer.cursor_position(area).unwrap();
    assert_eq!(caret.x, composer.geometry(area).text.x);
    assert_eq!(
        buf[(composer.geometry(area).prompt_x.unwrap(), caret.y)].symbol(),
        "❯"
    );
    assert!(
        shown
            .lines()
            .any(|row| row.trim_matches(['│', ' ']).is_empty()),
        "logical blank row retained: {shown}"
    );
    let progress = WorkflowProgress::new(vec![
        WorkflowRun::new("A\u{202e}\nB", WorkflowRunState::Degraded)
            .outcomes(1, 1, 0, 2)
            .reason("reason\u{1b}\u{2066} text"),
    ]);
    let buf = render(140, 1, |area, buf| progress.paint(area, buf, &theme));
    assert!(!text(&buf).contains(['\u{202e}', '\u{1b}', '\u{2066}']));
}

#[test]
fn native_chrome_is_bounded_in_tiny_offset_and_maximum_origin_buffers() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [0, 1, 4, 12, 40] {
            for height in [0, 1, 2, 4] {
                let area = Rect::new(4, 3, width, height);
                let surfaces: Vec<Box<dyn Paint>> = vec![
                    Box::new(
                        NativeComposer::new("鲸鱼 cafe\u{301}\nlast")
                            .focused(true)
                            .can_submit(true)
                            .target("worker")
                            .submit_hint("Enter send"),
                    ),
                    Box::new(WorkflowProgress::new(vec![
                        WorkflowRun::new("鲸鱼 cafe\u{301}", WorkflowRunState::Failed)
                            .outcomes(usize::MAX, usize::MAX, usize::MAX, usize::MAX)
                            .reason("retry later"),
                    ])),
                ];
                for surface in surfaces {
                    let mut buf = Buffer::empty(Rect::new(2, 2, 50, 8));
                    for cell in &mut buf.content {
                        cell.set_symbol("z");
                    }
                    let before = buf.clone();
                    surface.paint(area, &mut buf, &theme);
                    let clipped = area.intersection(buf.area);
                    for y in buf.area.top()..buf.area.bottom() {
                        for x in buf.area.left()..buf.area.right() {
                            if !clipped.contains((x, y).into()) {
                                assert_eq!(buf[(x, y)], before[(x, y)]);
                            }
                        }
                    }
                    let mut edge = Buffer::empty(Rect::new(u16::MAX - 50, u16::MAX - 4, 50, 4));
                    surface.paint(edge.area, &mut edge, &theme);
                }
            }
        }
        let ascii = NativeComposer::new("send this")
            .focused(true)
            .can_submit(true);
        let buf = render(40, 4, |area, buf| ascii.paint(area, buf, &theme));
        if profile == Profile::Ascii {
            assert!(buf.content.iter().all(|cell| cell.symbol().is_ascii()));
        }
    }
}

#[test]
fn translated_templates_keep_count_order_and_fit_before_clipping() {
    use codewhale_ratatui::WorkflowProgressWords;
    let progress = WorkflowProgress::new(vec![
        WorkflowRun::new("Review", WorkflowRunState::Running)
            .outcomes(2, 1, 1, 4)
            .queued(3),
    ])
    .words(WorkflowProgressWords {
        done: "完了 {done}/{total}".into(),
        failed: "失敗 {count}".into(),
        cancelled: "中止 {count}".into(),
        queued: "待機 {count}".into(),
        more: "ほか {count} 件".into(),
        ..WorkflowProgressWords::default()
    });
    let theme = Profile::DarkTrue.theme();
    let wide = render(120, 1, |area, buf| progress.paint(area, buf, &theme));
    let shown = text(&wide);
    for fact in ["完了 2/4", "失敗 1", "中止 1", "待機 3"] {
        assert!(shown.contains(fact), "{shown}");
    }
    let mut failed = progress.clone();
    failed.runs[0].state = WorkflowRunState::Failed;
    let failure = render(120, 1, |area, buf| failed.paint(area, buf, &theme));
    assert!(!text(&failure).contains("{count}"));
    let narrow = render(50, 1, |area, buf| progress.paint(area, buf, &theme));
    let shown = text(&narrow);
    assert!(
        shown.contains("完了 2/4") && shown.contains("失敗 1"),
        "{shown}"
    );
    let folded = render(120, 0, |area, buf| progress.paint(area, buf, &theme));
    assert!(text(&folded).is_empty());
    let mut many = progress.clone();
    many.runs = (0..8)
        .map(|_| WorkflowRun::new("Review", WorkflowRunState::Running))
        .collect();
    let folded = render(50, 1, |area, buf| many.paint(area, buf, &theme));
    assert!(text(&folded).starts_with(" ほか 8 件"));
    assert_eq!(WorkflowProgress::desired_rows_for(0), 0);
    assert_eq!(WorkflowProgress::desired_rows_for(8), 7);
}

fn rich_composer_fixture(
    theme: &codewhale_ratatui::Theme,
) -> codewhale_ratatui::NativeComposerFrame<'static> {
    use codewhale_ratatui::{NativeComposerFrame, NativeComposerMenu, NativeComposerStyles};
    use ratatui::{style::Modifier, text::Line};
    let background = theme.tui_ground(codewhale_ratatui::TuiGround::Composer);
    let plain = theme.fg(codewhale_ratatui::Role::Foreground);
    let primary = theme.fg(codewhale_ratatui::Role::Primary);
    NativeComposerFrame {
        text: "a\t\u{202e}中b".into(),
        cursor: 4,
        selection: Some((3, 4)),
        placeholder: Line::styled("Write a task or use /.", plain),
        enclosed: true,
        density: NativeComposerDensity::Comfortable,
        history_search: false,
        focused: true,
        can_submit: true,
        ascii: theme.ascii(),
        top_title: None,
        top_right: None,
        hint: None,
        quiet_hint: None,
        menu: NativeComposerMenu::default(),
        styles: NativeComposerStyles {
            background,
            border: primary,
            quiet_border: theme.fg(codewhale_ratatui::Role::Border),
            text: plain,
            selection: plain.patch(theme.bg(codewhale_ratatui::Role::Selected)),
            prompt: primary,
            submit: theme
                .tui_ink(codewhale_ratatui::TuiInk::Info)
                .add_modifier(Modifier::BOLD),
        },
    }
}
#[test]
fn mounted_source_scalars_survive_hidden_controls_and_exact_wrap_boundaries() {
    use codewhale_ratatui::{
        native_composer_source_at, native_composer_source_cursor, native_composer_source_plan,
        native_composer_source_rows,
    };
    let input = "a\t\u{202e}中b";
    let rows = native_composer_source_rows(input, 4);
    assert_eq!(rows, vec![(0, input.to_owned()), (5, String::new())]);
    for (cursor, expected) in [
        (1, (0, 1)),
        (2, (0, 1)),
        (3, (0, 1)),
        (4, (0, 3)),
        (5, (1, 0)),
    ] {
        assert_eq!(native_composer_source_cursor(&rows, cursor), expected);
    }
    assert_eq!(native_composer_source_at(input, 4, 1, 0, 0, 0), 3);
    assert_eq!(
        native_composer_source_at(input, 4, 2, 0, 0, 0),
        3,
        "second cell of wide glyph keeps its source start"
    );
    assert_eq!(native_composer_source_at(input, 4, 3, 0, 0, 0), 4);
    let plan = native_composer_source_plan(input, 5, 4, 1);
    assert_eq!(plan.scroll_offset, 1);
    assert_eq!(plan.visible, vec![(5, String::new())]);
    assert_eq!((plan.cursor_row, plan.cursor_col), (0, 0));
    assert_eq!(
        native_composer_source_rows("a\n\nb", 8),
        vec![(0, "a".into()), (2, "".into()), (3, "b".into())]
    );
    // Key motion keeps a display column; the existing wheel keeps its scalar column.
    // Both use the painter's exact source row boundaries.
    for (column, expected) in [
        (codewhale_ratatui::NativeComposerRowColumn::DisplayCells, 7),
        (codewhale_ratatui::NativeComposerRowColumn::SourceScalars, 6),
    ] {
        assert_eq!(
            codewhale_ratatui::native_composer_step_row("中ab\ncdef", 2, 8, 1, column),
            Some(expected)
        );
    }
    assert!(
        codewhale_ratatui::native_composer_step_row(
            input,
            0,
            4,
            -1,
            codewhale_ratatui::NativeComposerRowColumn::DisplayCells
        )
        .is_none()
    );
    for input in [
        "cafe\u{0301} 中 a long URL https://example.com/path",
        "1\u{fe0f}\u{20e3} 👩\u{200d}💻 more words",
    ] {
        for width in [1, 2, 5, 11, 40] {
            assert_eq!(
                codewhale_ratatui::native_composer_wrap_text(input, width).join(""),
                input
            );
        }
    }
}
#[test]
fn mounted_selection_caret_and_ime_empty_row_share_the_plan() {
    let theme = Profile::DarkTrue.theme();
    let mut composer = rich_composer_fixture(&theme);
    let original = composer.text.clone();
    let area = Rect::new(7, 5, 40, 5);
    let mut buf = Buffer::empty(area);
    let plan = composer.render(area, &mut buf);
    assert_eq!(
        composer.text, original,
        "render cannot edit the source draft"
    );
    assert!(!text(&buf).contains(['\t', '\u{202e}']));
    assert_eq!(
        buf[(plan.geometry.text.x + 1, plan.cursor.unwrap().y)].symbol(),
        "中"
    );
    assert_eq!(
        buf[(plan.geometry.text.x + 1, plan.cursor.unwrap().y)].bg,
        theme.bg(codewhale_ratatui::Role::Selected).bg.unwrap()
    );
    composer.text = "".into();
    composer.cursor = 0;
    composer.selection = None;
    for width in [12, 14, 40, 80] {
        let area = Rect::new(7, 5, width, 5);
        let plan = composer.plan(area);
        assert_eq!(
            plan.cursor.unwrap().y,
            plan.geometry.text.y + 1,
            "wrapped hint cannot consume the IME caret row"
        );
        assert_eq!(plan.top_padding, 1);
    }
}
#[test]
fn mounted_menu_pointer_rows_follow_real_wrapping_and_clipping() {
    use codewhale_ratatui::{NativeComposerMenuItem, testing};
    use ratatui::text::Line;
    let theme = Profile::DarkTrue.theme();
    let mut composer = rich_composer_fixture(&theme);
    composer.text = "x".into();
    composer.cursor = 1;
    composer.selection = None;
    composer.menu.reserved_rows = 2;
    composer.menu.pointer_rows = true;
    composer.menu.items = vec![
        NativeComposerMenuItem::Line(Line::from("first long menu label wraps over several rows")),
        NativeComposerMenuItem::Line(Line::from("second row")),
    ];
    let area = Rect::new(7, 5, 20, 12);
    let mut buf = Buffer::empty(area);
    let plan = composer.render(area, &mut buf);
    assert!(
        plan.menu_rects[0].1.height > 1,
        "the last wrapped row belongs to the same option"
    );
    for (_, rect) in &plan.menu_rects {
        assert!(rect.y >= plan.geometry.inner.y);
        assert!(rect.bottom() <= plan.geometry.inner.bottom());
    }
    let canvas = Rect::new(9, 6, 15, 6);
    let mut clipped = Buffer::empty(canvas);
    let partial = composer.render(area, &mut clipped);
    assert_eq!(partial.area, area.intersection(canvas));
    for (_, rect) in partial.menu_rects {
        assert_eq!(rect.intersection(canvas), rect);
    }
    for name in [
        "native-composer-rich-selection",
        "native-composer-rich-search",
    ] {
        let entry = codewhale_ratatui::gallery::entries()
            .into_iter()
            .find(|entry| entry.name == name)
            .unwrap();
        testing::assert_rules(entry.height, |area, buf, theme| {
            (entry.draw)(area, buf, theme)
        });
    }
}
#[test]
fn mounted_bounds_and_empty_frames_never_publish_stale_targets() {
    use ratatui::buffer::Cell;
    let theme = Profile::DarkTrue.theme();
    let composer = rich_composer_fixture(&theme);
    let canvas = Rect::new(7, 5, 40, 10);
    for requested in [
        Rect::new(9, 6, 80, 12),
        Rect::new(0, 0, 100, 100),
        Rect::new(7, 5, 0, 10),
        Rect::new(7, 5, 40, 0),
    ] {
        let mut buf = Buffer::filled(canvas, Cell::new("~"));
        let before = buf.clone();
        let plan = composer.render(requested, &mut buf);
        let visible = requested.intersection(canvas);
        for y in canvas.y..canvas.bottom() {
            for x in canvas.x..canvas.right() {
                if !visible.contains((x, y).into()) {
                    assert_eq!(buf[(x, y)], before[(x, y)]);
                }
            }
        }
        if visible.is_empty() {
            assert!(plan.cursor.is_none());
            assert!(plan.menu_rects.is_empty());
            assert!(plan.geometry.submit.is_none());
        }
    }
}

#[test]
fn mounted_prompt_separator_keeps_raw_ink_for_placeholder_input_and_selection() {
    use ratatui::{
        buffer::Cell,
        style::{Color, Modifier, Style},
        text::{Line, Span},
    };
    let theme = Profile::DarkTrue.theme();
    let inherited = Color::Rgb(182, 192, 212);
    let override_ink = Color::Rgb(210, 230, 250);
    for enclosed in [false, true] {
        for source in ["", "x", "xy"] {
            let mut composer = rich_composer_fixture(&theme);
            composer.enclosed = enclosed;
            composer.text = source.to_string().into();
            composer.cursor = 0;
            composer.selection = (source == "xy").then_some((0, 1));
            composer.placeholder = Line::from(vec![
                Span::raw("hint"),
                Span::styled(
                    "!",
                    Style::default()
                        .fg(override_ink)
                        .add_modifier(Modifier::ITALIC),
                ),
            ])
            .style(Style::default().fg(inherited).add_modifier(Modifier::BOLD));
            composer.styles.text = Style::default().fg(inherited);
            composer.styles.selection = composer.styles.selection.fg(inherited);
            let area = Rect::new(7, 5, 20, 4);
            let guard = Rect::new(2, 3, 32, 11);
            let mut buf = Buffer::filled(guard, Cell::new("~"));
            let original = buf.clone();
            let plan = composer.render(area, &mut buf);
            let cursor = plan.cursor.unwrap();
            let separator = &buf[(plan.geometry.inner.x + 1, cursor.y)];
            assert_eq!(separator.symbol(), " ");
            assert_eq!(separator.fg, Color::Reset, "separator {enclosed}/{source}");
            assert_eq!(separator.modifier, Modifier::empty());
            let body = &buf[(plan.geometry.text.x, cursor.y)];
            assert_eq!(body.fg, inherited);
            if source.is_empty() {
                assert_eq!(body.symbol(), "h");
                assert!(body.modifier.contains(Modifier::BOLD));
                let own = &buf[(plan.geometry.text.x + 4, cursor.y)];
                assert_eq!(own.fg, override_ink);
                assert!(own.modifier.contains(Modifier::ITALIC | Modifier::BOLD));
            } else {
                assert_eq!(body.symbol(), "x");
            }
            if source == "xy" {
                assert_eq!(body.bg, composer.styles.selection.bg.unwrap());
            }
            for y in guard.y..guard.bottom() {
                for x in guard.x..guard.right() {
                    if !area.contains((x, y).into()) {
                        assert_eq!(buf[(x, y)], original[(x, y)]);
                    }
                }
            }
        }
    }
}
