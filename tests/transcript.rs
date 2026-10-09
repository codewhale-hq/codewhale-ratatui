use std::borrow::Cow;

use codewhale_ratatui::{
    CodeBlock, Paint, Transcript, TranscriptAction, TranscriptBlock, TranscriptSpan,
    TranscriptSpanRole,
    testing::{self, Profile, render},
};
use ratatui::layout::Rect;

fn fixture() -> Transcript<'static> {
    Transcript::new(vec![
        TranscriptBlock::heading(1, "Session closeout"),
        TranscriptBlock::paragraph(vec![
            TranscriptSpan::plain("The "),
            TranscriptSpan::strong("workbar"),
            TranscriptSpan::plain(" folds TODO, context and price; "),
            TranscriptSpan::emphasis("nothing"),
            TranscriptSpan::plain(" is opened from here."),
        ]),
        TranscriptBlock::quote("Ship the quiet version first."),
        TranscriptBlock::list(vec![
            TranscriptSpan::plain("Run the narrow checks"),
            TranscriptSpan::plain("Read the receipt"),
        ]),
        TranscriptBlock::table(
            vec![Cow::Borrowed("Check"), Cow::Borrowed("State")],
            vec![
                vec![Cow::Borrowed("narrow layout"), Cow::Borrowed("kept")],
                vec![Cow::Borrowed("all profiles"), Cow::Borrowed("kept")],
            ],
        ),
        TranscriptBlock::code(CodeBlock::new(
            "rust",
            "let theme = Theme::detect();\nframe.render_widget(view.themed(&theme), area);",
        )),
    ])
}

#[test]
fn transcript_keeps_the_rules_in_every_profile_and_width() {
    testing::assert_rules(16, |area, buf, theme| {
        fixture().paint(area, buf, theme);
    });
}

#[test]
fn code_copy_text_keeps_the_exact_original_newlines() {
    let source = "fn main() {\r\n\tprintln!(\"hi\");\r\n}\n";
    let block = CodeBlock::new("rust", source);
    assert_eq!(block.source(), source);
    assert_eq!(block.copy_text(), source);
    assert!(block.copy_text().contains("\r\n"));
    assert!(block.copy_text().ends_with('\n'));
    assert_eq!(block.copy_action(), Some(TranscriptAction::Copy));
    assert_eq!(block.actions(), [TranscriptAction::Copy]);

    let theme = Profile::DarkTrue.theme();
    let buf = render(40, 8, |area, buf| block.paint(area, buf, &theme));
    let shown = testing::text(&buf);
    assert!(shown.contains("rust"));
    assert!(shown.contains("Copy"));
    assert!(shown.contains("fn main() {"));
    assert!(!shown.contains('\r'));
}

#[test]
fn a_display_copy_never_changes_what_the_host_would_copy() {
    let block = CodeBlock::new("text", "exact\nsource\n").display("shown text");
    assert_eq!(block.copy_text(), "exact\nsource\n");
    let theme = Profile::DarkTrue.theme();
    let buf = render(40, 4, |area, buf| block.paint(area, buf, &theme));
    let shown = testing::text(&buf);
    assert!(shown.contains("shown text"));
    assert!(!shown.contains("source"));

    let bare = CodeBlock::new("text", "x")
        .copyable(false)
        .copy_label("Copy");
    assert_eq!(bare.copy_action(), None);
    assert!(bare.actions().is_empty());
    let buf = render(40, 2, |area, buf| bare.paint(area, buf, &theme));
    assert!(!testing::text(&buf).contains("Copy"));
}

#[test]
fn unsafe_display_is_sanitized_and_link_targets_stay_out_of_band() {
    let transcript = Transcript::new(vec![
        TranscriptBlock::paragraph(vec![TranscriptSpan::plain(
            "run rm -rf ~/\u{202E}txt.exe now",
        )]),
        TranscriptBlock::paragraph(vec![
            TranscriptSpan::plain("Read "),
            TranscriptSpan::link("the component guide", "https://example.invalid/guide"),
            TranscriptSpan::plain(" first."),
        ]),
    ]);
    let theme = Profile::DarkTrue.theme();
    let buf = render(60, 6, |area, buf| transcript.paint(area, buf, &theme));
    let shown = testing::text(&buf);
    assert!(shown.contains("rm -rf ~/txt.exe now"), "{shown}");
    assert!(!shown.contains('\u{202e}'));
    assert!(shown.contains("the component guide"));
    assert!(!shown.contains("example.invalid"));
    assert!(!shown.contains("https"));
    assert!(!shown.contains('\u{1b}'));
    let styled = testing::styled(&buf, &theme);
    assert!(styled.contains("underline"), "{styled}");

    let links = transcript.links();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].text, "the component guide");
    assert_eq!(links[0].target, "https://example.invalid/guide");
}

#[test]
fn caller_semantic_roles_are_painted_as_given() {
    let theme = Profile::DarkTrue.theme();
    let spans: Vec<TranscriptSpan<'static>> = TranscriptSpanRole::ALL
        .iter()
        .map(|role| TranscriptSpan::new("x", *role))
        .collect();
    let block = TranscriptBlock::paragraph(spans);
    let buf = render(40, 2, |area, buf| block.paint(area, buf, &theme));
    let styled = testing::styled(&buf, &theme);
    assert!(styled.contains("underline"), "{styled}");
    assert!(styled.contains("bold"), "{styled}");
    assert!(styled.contains("italic"), "{styled}");
}

#[test]
fn a_requested_offset_and_height_clip_the_painted_rows() {
    let transcript = Transcript::new(vec![
        TranscriptBlock::heading(1, "First"),
        TranscriptBlock::heading(2, "Second"),
    ])
    .offset(1);
    let theme = Profile::DarkTrue.theme();
    let buf = render(40, 1, |area, buf| transcript.paint(area, buf, &theme));
    let shown = testing::text(&buf);
    assert!(shown.contains("Second"), "{shown}");
    assert!(!shown.contains("First"));
    assert_eq!(
        transcript.lines(40, &theme).len(),
        usize::from(transcript.height(40, &theme))
    );
}

#[test]
fn narrow_unicode_code_and_tables_stay_inside_the_requested_width() {
    let transcript = Transcript::new(vec![
        TranscriptBlock::code(CodeBlock::new("rust", "鲸".repeat(12))),
        TranscriptBlock::table(
            vec![Cow::Borrowed("文件"), Cow::Borrowed("state")],
            vec![vec![Cow::Borrowed("提交记录"), Cow::Borrowed("queued")]],
        ),
        TranscriptBlock::list(vec![TranscriptSpan::plain("鲸鱼".repeat(20))]),
    ]);
    let theme = Profile::DarkTrue.theme();
    for width in [12u16, 20, 40] {
        let buf = render(width, 24, |area, buf| transcript.paint(area, buf, &theme));
        for line in testing::text(&buf).lines() {
            assert!(
                codewhale_ratatui::text::width(line) <= usize::from(width),
                "{width}: {line}"
            );
        }
    }
}

#[test]
fn painting_outside_the_buffer_area_writes_nothing() {
    let transcript = fixture();
    let theme = Profile::DarkTrue.theme();
    let buf = render(20, 4, |_, buf| {
        transcript.paint(Rect::new(30, 10, 24, 6), buf, &theme);
    });
    assert!(testing::text(&buf).trim().is_empty());
}

#[test]
fn structured_blocks_render_their_own_marks() {
    let theme = Profile::DarkTrue.theme();
    let buf = render(60, 16, |area, buf| fixture().paint(area, buf, &theme));
    let shown = testing::text(&buf);
    assert!(shown.contains("Session closeout"));
    assert!(shown.contains("Ship the quiet version first."));
    assert!(shown.contains("• Run the narrow checks"));
    assert!(shown.contains("Check"));
    assert!(shown.contains("narrow layout"));
    assert!(shown.contains("rust"));
    assert!(shown.contains("Copy"));

    let ordered = TranscriptBlock::ordered(vec![TranscriptSpan::plain("one")]);
    let buf = render(40, 1, |area, buf| ordered.paint(area, buf, &theme));
    assert!(testing::text(&buf).contains("1. one"));
}

#[test]
fn wrapping_retains_all_words_and_code_keeps_logical_lines() {
    let theme = Profile::DarkTrue.theme();
    let block = TranscriptBlock::prose("alpha beta gamma delta epsilon zeta");
    let lines = block.lines(12, &theme);
    assert!(lines.len() >= 3);
    let visible: String = lines
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    for word in ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"] {
        assert!(visible.contains(word), "lost {word}: {visible}");
    }
    let code = CodeBlock::new("rust", "first();\n\tsecond();\nthird();");
    let shown = code.lines(40, &theme);
    assert_eq!(shown.len(), 4, "header plus three numbered source lines");
    assert!(shown[1].to_string().contains("1 │"));
    assert!(shown[2].to_string().contains("2 │"));
    assert!(shown[2].to_string().contains("    second();"));
    assert!(shown[3].to_string().contains("3 │"));
    let wide = TranscriptBlock::prose("鲸鱼".repeat(10));
    let lines = wide.lines(8, &theme);
    assert_eq!(
        lines.iter().map(|l| l.to_string()).collect::<String>(),
        "鲸鱼".repeat(10)
    );
    assert!(lines.iter().all(|line| line.width() <= 8));
}

#[test]
fn narrow_code_elides_chrome_before_hiding_source_and_tables_keep_separation() {
    let theme = Profile::DarkTrue.theme();
    for width in [2, 4, 6, 8] {
        let lines = CodeBlock::new("", "x鲸")
            .copyable(false)
            .lines(width, &theme);
        let shown = lines
            .iter()
            .map(|line| line.to_string())
            .collect::<String>();
        assert!(
            shown.contains('x'),
            "source hidden by gutter at {width}: {shown}"
        );
        assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
    }
    let table = TranscriptBlock::table(vec!["value".into()], vec![vec!["first\nsecond".into()]]);
    assert!(
        table
            .lines(40, &theme)
            .iter()
            .any(|line| line.to_string().contains("first second"))
    );
}

#[test]
fn mounted_viewport_pinned_offset_repaint_and_style_facts_are_exact() {
    use codewhale_ratatui::TranscriptViewport;
    use ratatui::{
        buffer::{Buffer, Cell},
        style::{Color, Modifier, Style},
        text::{Line, Span},
    };
    let styles = [
        Style::default().fg(Color::Rgb(31, 43, 59)),
        Style::default()
            .bg(Color::Rgb(71, 83, 97))
            .add_modifier(Modifier::BOLD),
    ];
    let rows = vec![
        Line::styled("Pinned source", styles[0]),
        Line::from("old row"),
        Line::from(vec![
            Span::styled("中 cafe\u{0301} ", styles[0]),
            Span::styled("selected", styles[1]),
        ]),
    ];
    let raw = rows.clone();
    let area = Rect::new(7, 5, 40, 6);
    let mut buf = Buffer::filled(area, Cell::new("~"));
    let mut viewport = TranscriptViewport::new(&rows);
    viewport.pinned_rows = 1;
    viewport.offset = 1;
    viewport.style = Style::default().bg(Color::Rgb(11, 23, 37));
    let plan = viewport.render(area, &mut buf);
    assert_eq!(rows, raw);
    assert_eq!(plan.body_area, Rect::new(7, 6, 40, 5));
    assert!(testing::text(&buf).contains("Pinned source"));
    assert!(!testing::text(&buf).contains("old row"));
    // Paragraph paints source rows and preserves unused symbols in this
    // non-filling projection; the host owns the surrounding frame substrate.
    assert_eq!(buf[(7, 10)].symbol(), "~");
    assert_eq!(buf[(7, 6)].fg, Color::Rgb(31, 43, 59));
    let selected = &buf[(15, 6)];
    assert_eq!(selected.symbol(), "s");
    assert_eq!(selected.bg, Color::Rgb(71, 83, 97));
    assert!(selected.modifier.contains(Modifier::BOLD));
    viewport.offset = usize::MAX;
    assert_eq!(viewport.height(40, &Profile::DarkTrue.theme()), 1);
    assert_eq!(viewport.height(0, &Profile::DarkTrue.theme()), 0);
    let wrapped = [Line::from("alpha beta gamma")];
    let mut viewport = TranscriptViewport::new(&wrapped);
    viewport.wrap = true;
    assert_eq!(viewport.height(6, &Profile::DarkTrue.theme()), 3);
}
#[test]
fn mounted_viewport_link_bounds_exclude_scroll_rail_and_opaque_jump_cells() {
    use codewhale_ratatui::{TranscriptScrollFacts, TranscriptViewport};
    use ratatui::text::Line;
    let rows = vec![Line::from("visible guide"); 8];
    let mut viewport = TranscriptViewport::new(&rows);
    viewport.pinned_rows = 1;
    viewport.scrollbar = Some(TranscriptScrollFacts {
        top: 3,
        visible: 7,
        total: 20,
    });
    viewport.jump_to_latest = true;
    let plan = viewport.plan(Rect::new(7, 5, 20, 8));
    let button = plan.jump.unwrap();
    assert_eq!(plan.link_area.width, 19);
    assert!(plan.link_rects(8, 0, 2).is_empty());
    assert!(plan.link_rects(0, usize::MAX, usize::MAX).is_empty());
    assert!(plan.link_rects(0, 4, 2).is_empty());
    for row in 0..8 {
        for rect in plan.link_rects(row, 0, usize::MAX) {
            assert_eq!(rect.intersection(plan.link_area), rect);
            assert!(rect.intersection(button).is_empty());
        }
    }
    assert_eq!(plan.link_rects(0, 2, 40), vec![Rect::new(9, 5, 17, 1)]);
}
#[test]
fn mounted_viewport_guards_every_styled_field_without_changing_copy_source() {
    use codewhale_ratatui::{TranscriptViewport, transcript_selected_spans_measured};
    use ratatui::{
        style::{Modifier, Style},
        text::{Line, Span},
    };
    let source = "unsafe\u{202e} text\x1b after";
    let raw = Line::from(vec![
        Span::styled(source, Style::default().add_modifier(Modifier::ITALIC)),
        Span::raw(" 1\u{20e3}① 中 e\u{0301} 👩\u{200d}💻"),
    ]);
    let selected = transcript_selected_spans_measured(
        &raw,
        1,
        6,
        Style::default().add_modifier(Modifier::REVERSED),
        |g| {
            if g.contains('\u{20e3}') || g == "①" {
                2
            } else {
                codewhale_ratatui::text::width(g)
            }
        },
    );
    let extreme =
        transcript_selected_spans_measured(&raw, 0, usize::MAX, Style::default(), |_| usize::MAX);
    assert_eq!(extreme.len(), raw.spans.len());
    let line = Line::from(selected);
    let theme = Profile::DarkTrue.theme();
    let buf = render(60, 3, |area, buf| {
        TranscriptViewport::new(std::slice::from_ref(&line)).paint(area, buf, &theme)
    });
    let shown = testing::text(&buf);
    assert!(!shown.contains(['\u{202e}', '\x1b']));
    assert!(shown.contains("unsafe text after"));
    assert_eq!(raw.spans[0].content, source);
    assert!(buf.content.iter().any(|cell| {
        cell.modifier
            .contains(Modifier::REVERSED | Modifier::ITALIC)
    }));
}
#[test]
fn mounted_viewport_offset_buffers_empty_resize_and_real_gallery_stay_bounded() {
    use codewhale_ratatui::{TranscriptScrollFacts, TranscriptViewport};
    use ratatui::{
        buffer::{Buffer, Cell},
        text::Line,
    };
    let rows = vec![Line::from("tail 中 and cafe\u{0301}"); 8];
    let mut viewport = TranscriptViewport::new(&rows);
    viewport.scrollbar = Some(TranscriptScrollFacts {
        top: 4,
        visible: 3,
        total: 12,
    });
    viewport.jump_to_latest = true;
    let canvas = Rect::new(7, 5, 20, 8);
    for requested in [
        Rect::new(9, 6, 40, 12),
        Rect::new(0, 0, 100, 100),
        Rect::new(7, 5, 0, 8),
        Rect::new(7, 5, 20, 0),
    ] {
        let mut buf = Buffer::filled(canvas, Cell::new("~"));
        let before = buf.clone();
        let plan = viewport.render(requested, &mut buf);
        let visible = requested.intersection(canvas);
        assert_eq!(plan.area, visible);
        for y in canvas.y..canvas.bottom() {
            for x in canvas.x..canvas.right() {
                if !visible.contains((x, y).into()) {
                    assert_eq!(buf[(x, y)], before[(x, y)]);
                }
            }
        }
        if visible.is_empty() {
            assert!(plan.jump.is_none());
            assert!(plan.link_rects(0, 0, 9).is_empty());
        }
    }
    // Public staged chrome painting also clips independently supplied buffers.
    let plan = viewport.plan(Rect::new(0, 0, 100, 100));
    plan.paint_chrome(&mut Buffer::empty(canvas));
    for name in ["transcript-mounted", "transcript-mounted-focus"] {
        let entry = codewhale_ratatui::gallery::entries()
            .into_iter()
            .find(|e| e.name == name)
            .unwrap();
        testing::assert_rules(entry.height, |area, buf, theme| {
            (entry.draw)(area, buf, theme)
        });
    }
}
