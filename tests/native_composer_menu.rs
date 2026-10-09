use codewhale_ratatui::{
    NativeComposerDensity, NativeComposerFrame, NativeComposerMenu, NativeComposerMenuItem,
    NativeComposerStyles,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn frame(menu: NativeComposerMenu) -> NativeComposerFrame<'static> {
    NativeComposerFrame {
        text: "/test".into(),
        cursor: 5,
        selection: None,
        placeholder: Line::default(),
        enclosed: false,
        density: NativeComposerDensity::Compact,
        history_search: false,
        focused: true,
        can_submit: false,
        ascii: false,
        top_title: None,
        top_right: None,
        hint: None,
        quiet_hint: None,
        styles: NativeComposerStyles {
            background: Style::default().bg(Color::Rgb(11, 23, 37)),
            text: Style::default().fg(Color::White),
            prompt: Style::default().fg(Color::Cyan),
            quiet_border: Style::default().fg(Color::Blue),
            ..Default::default()
        },
        menu,
    }
}
// Exact Engine ui_text::truncate_line_to_width contract for visible inputs.
fn legacy_columns(value: &str, width: usize) -> String {
    if value.width() <= width {
        return value.into();
    }
    let mut result = String::new();
    let mut used = 0;
    let budget = if width > 3 { width - 3 } else { width };
    for grapheme in value.graphemes(true) {
        if used + grapheme.width() > budget {
            break;
        }
        result.push_str(grapheme);
        used += grapheme.width();
    }
    if width > 3 {
        result.push_str("...");
    }
    result
}

#[test]
fn mounted_menu_column_truncation_retains_native_space_and_whole_buffer_styles() {
    let name_style = Style::default()
        .fg(Color::Rgb(41, 67, 103))
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default()
        .fg(Color::Rgb(109, 83, 59))
        .add_modifier(Modifier::ITALIC);
    let name = "/test (jiancha)";
    for description in [
        "Run checks for this project",
        "Run checks for 鲸鱼 cafe\u{0301}",
    ] {
        for width in [12, 20, 40, 80] {
            let area = Rect::new(7, 5, width, 3);
            let marker = Span::styled("›", name_style);
            let actual_frame = frame(NativeComposerMenu {
                items: vec![NativeComposerMenuItem::Columns {
                    name: name.into(),
                    description: description.into(),
                    prefix: Span::raw(""),
                    marker: marker.clone(),
                    name_style,
                    description_style: desc_style,
                }],
                selected: 0,
                reserved_rows: 1,
                pointer_rows: true,
            });
            let plan = actual_frame.plan(area);
            let inner_width = usize::from(plan.geometry.inner.width.max(1));
            let label_width = name.width().min(inner_width.saturating_sub(4)).max(8);
            let mut label = legacy_columns(name, label_width);
            while label.width() < label_width {
                label.push(' ');
            }
            let capacity = inner_width.saturating_sub(1 + 1 + label_width + 2);
            let desc = legacy_columns(description, capacity);
            if capacity == 18 {
                assert_eq!(desc, "Run checks for ...");
            }
            let expected_frame = frame(NativeComposerMenu {
                items: vec![NativeComposerMenuItem::Line(Line::from(vec![
                    Span::raw(" "),
                    marker,
                    Span::raw(""),
                    Span::styled(label, name_style),
                    Span::styled("  ", desc_style),
                    Span::styled(desc, desc_style),
                ]))],
                selected: 0,
                reserved_rows: 1,
                pointer_rows: true,
            });
            let mut actual = Buffer::empty(Rect::new(2, 3, width + 12, 11));
            for cell in &mut actual.content {
                cell.set_symbol("~").set_style(
                    Style::default()
                        .bg(Color::Green)
                        .add_modifier(Modifier::UNDERLINED),
                );
            }
            let mut expected = actual.clone();
            let actual_plan = actual_frame.render(area, &mut actual);
            let expected_plan = expected_frame.render(area, &mut expected);
            assert_eq!(
                actual, expected,
                "width={width} description={description:?}"
            );
            assert_eq!(actual_plan.menu_rects, expected_plan.menu_rects);
        }
    }
}
