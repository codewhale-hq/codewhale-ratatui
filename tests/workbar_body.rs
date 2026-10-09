use codewhale_ratatui::{WorkbarLayout, WorkbarScrollbar};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

#[test]
fn shared_body_layout_preserves_fold_overflow_offset_and_tiny_viewport_contract() {
    for width in 0..=80 {
        for height in 0..=12 {
            for (goal, progress) in [(false, false), (true, false), (false, true), (true, true)] {
                for count in [0, 1, 2, 9, 128] {
                    let body = Rect::new(7, 5, width, height);
                    let layout = WorkbarLayout::for_body(body, count, usize::MAX, goal, progress);
                    let goal_rows = u16::from(goal && height >= 2);
                    let progress_rows = u16::from(
                        progress
                            && !(goal_rows > 0 && width >= 72)
                            && height.saturating_sub(goal_rows) >= 2,
                    );
                    let list_height = height.saturating_sub(goal_rows + progress_rows);
                    let overflow = count > usize::from(list_height);
                    let more = overflow && list_height >= 2;
                    let visible = usize::from(list_height) - usize::from(more);
                    assert_eq!(layout.goal_height, goal_rows);
                    assert_eq!(layout.progress_height, progress_rows);
                    assert_eq!(layout.content.height, list_height);
                    assert_eq!(layout.visible_rows, visible);
                    assert_eq!(layout.offset, count.saturating_sub(visible.max(1)));
                    assert_eq!(layout.more_row, more);
                    assert_eq!(layout.overflow, overflow);
                    assert_eq!(layout.content.x, body.x + u16::from(width >= 16));
                }
            }
        }
    }
}

#[test]
fn shared_body_scrollbar_whole_buffers_preserve_both_native_and_ascii_ink() {
    for height in 0..=16 {
        for total in [0, 1, 3, 31] {
            for visible in [0, 1, 3, 9] {
                for offset in [0, 1, 7, usize::MAX] {
                    for ascii in [false, true] {
                        let area = Rect::new(9, 5, 1, height);
                        let thumb = if ascii { "|" } else { "┃" };
                        let track = if ascii { "|" } else { "│" };
                        let thumb_style =
                            Style::default().fg(Color::Rgb(29, 71, 113)).bg(Color::Blue);
                        let track_style =
                            Style::default().fg(Color::Rgb(43, 83, 127)).bg(Color::Blue);
                        let mut actual = Buffer::empty(Rect::new(2, 3, 20, 20));
                        for cell in &mut actual.content {
                            cell.set_symbol("~").set_style(
                                Style::default()
                                    .bg(Color::Green)
                                    .add_modifier(Modifier::ITALIC | Modifier::REVERSED),
                            );
                        }
                        let mut expected = actual.clone();
                        if height > 0 && total > 0 {
                            let h = usize::from(height);
                            let size = (h.saturating_mul(visible) / total).max(1).min(h);
                            let start = offset.saturating_mul(h.saturating_sub(size))
                                / total.saturating_sub(visible).max(1);
                            for row in 0..h {
                                let active = row >= start && row < start.saturating_add(size);
                                expected[(9, 5 + row as u16)]
                                    .set_symbol(if active { thumb } else { track })
                                    .set_style(if active { thumb_style } else { track_style });
                            }
                        }
                        WorkbarScrollbar {
                            offset,
                            visible,
                            total,
                            thumb,
                            track,
                            thumb_style,
                            track_style,
                        }
                        .paint(area, &mut actual);
                        assert_eq!(
                            actual, expected,
                            "height={height} total={total} visible={visible} offset={offset} ascii={ascii}"
                        );
                    }
                }
            }
        }
    }
}
