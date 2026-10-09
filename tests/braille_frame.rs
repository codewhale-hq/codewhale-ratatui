use codewhale_ratatui::BrailleFrame;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Paragraph, Widget, Wrap},
};
use unicode_width::UnicodeWidthStr;

fn legacy(area: Rect, buf: &mut Buffer, grid: &[u8], label: &str, style: Style) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if label.width() > usize::from(area.width) || area.height < 4 {
        Paragraph::new(label)
            .style(style)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .render(area, buf);
        return;
    }
    for y in 0..area.height - 1 {
        for x in 0..area.width {
            let bits = grid
                .get(usize::from(y) * usize::from(area.width) + usize::from(x))
                .copied()
                .unwrap_or(0);
            if bits != 0
                && let Some(cell) = buf.cell_mut((area.x + x, area.y + y))
            {
                let glyph = char::from_u32(0x2800 + u32::from(bits)).expect("braille");
                cell.set_symbol(&glyph.to_string()).set_style(style);
            }
        }
    }
    let x = area.x + (area.width - label.width() as u16) / 2;
    buf.set_stringn(x, area.bottom() - 1, label, usize::from(area.width), style);
}

fn seeded(area: Rect) -> Buffer {
    let mut b = Buffer::empty(area);
    for c in &mut b.content {
        c.set_symbol("~").set_style(
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::Blue)
                .add_modifier(Modifier::ITALIC),
        );
    }
    b
}
#[test]
fn packed_frame_exact_native_buffers_include_cameo_transparency_and_wrapped_cues() {
    for width in 0..=24 {
        for height in 0..=8 {
            for label in ["m · 6", "鲸 · cafe\u{0301}", "resting with uncertain gait"] {
                let area = Rect::new(4, 3, width, height);
                let style = Style::default()
                    .fg(Color::Rgb(27, 81, 135))
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
                let cells = (0..90)
                    .map(|i| if i % 5 == 0 { 0 } else { (i + 1) as u8 })
                    .collect::<Vec<_>>();
                let mut actual = seeded(Rect::new(1, 1, 32, 14));
                let mut expected = actual.clone();
                legacy(area, &mut expected, &cells, label, style);
                BrailleFrame {
                    cells: &cells,
                    caption: label,
                    style,
                }
                .render(area, &mut actual);
                assert_eq!(actual, expected, "area={area:?} caption={label}");
            }
        }
    }
}
#[test]
fn clipped_grid_uses_original_coordinates_and_clipped_caption_origin() {
    let area = Rect::new(3, 2, 18, 6);
    let clipped = Rect::new(13, 2, 6, 6);
    let cells = (0..90)
        .map(|i| if i % 3 == 0 { 0 } else { (i + 1) as u8 })
        .collect::<Vec<_>>();
    let frame = BrailleFrame {
        cells: &cells,
        caption: "m · 6",
        style: Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    };
    let mut full = seeded(Rect::new(0, 0, 25, 10));
    frame.render(area, &mut full);
    let mut actual = seeded(clipped);
    frame.render(area, &mut actual);
    for y in clipped.y..clipped.bottom() {
        for x in clipped.x..clipped.right() {
            assert_eq!(actual[(x, y)], full[(x, y)], "cell=({x},{y})");
        }
    }
}
