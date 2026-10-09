//! Whale painting obeys the same buffer/request boundary as every component.

use codewhale_ratatui::{Paint, Role, Whale, WhaleState, gallery, testing::Profile};
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    style::Modifier,
};

#[test]
fn every_state_and_profile_paints_only_inside_the_request_and_buffer() {
    for (x, y) in [(0, 0), (7, 5), (u16::MAX - 40, u16::MAX - 24)] {
        let bounds = Rect::new(x, y, 36, 20);
        let requests = [
            bounds,
            Rect::new(x + 3, y + 2, 20, 12),
            Rect::new(x.saturating_sub(4), y.saturating_sub(3), 24, 14),
            Rect::new(x + 18, y + 8, 40, 30),
            Rect::new(0, 0, u16::MAX, u16::MAX),
            Rect::new(bounds.right(), y, 20, 20),
            Rect::new(x, bounds.bottom(), 36, 10),
            Rect::new(x.saturating_sub(5), y, 2, 20),
            Rect::new(x, y.saturating_sub(4), 36, 2),
            Rect::new(u16::MAX, u16::MAX, 0, 0),
            Rect::new(x + 2, y + 2, 0, 12),
            Rect::new(x + 2, y + 2, 20, 0),
            Rect::new(x, y, 1, 1),
            Rect::new(bounds.right() - 1, bounds.bottom() - 1, 1, 1),
        ];
        for state in WhaleState::ALL {
            for profile in Profile::ALL {
                let theme = profile.theme();
                for request in requests {
                    let before = Buffer::filled(bounds, Cell::new("·"));
                    let mut after = before.clone();
                    Whale::new(state).paint(request, &mut after, &theme);
                    assert_eq!(after.area, bounds);
                    let allowed = request.intersection(bounds);
                    for row in bounds.top()..bounds.bottom() {
                        for col in bounds.left()..bounds.right() {
                            if col < allowed.left()
                                || col >= allowed.right()
                                || row < allowed.top()
                                || row >= allowed.bottom()
                            {
                                assert_eq!(
                                    after[(col, row)],
                                    before[(col, row)],
                                    "{state:?}, {}, request {request:?}: stray write at ({col}, {row})",
                                    profile.name()
                                );
                            }
                        }
                    }
                    if allowed.is_empty() {
                        assert_eq!(after, before, "disjoint or empty requests paint nothing");
                    }
                    if request == bounds {
                        assert_ne!(
                            after,
                            before,
                            "{state:?} paints art or its state words in {}",
                            profile.name()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn every_state_and_profile_accepts_an_empty_buffer_at_a_nonzero_origin() {
    for bounds in [
        Rect::new(7, 5, 0, 20),
        Rect::new(7, 5, 36, 0),
        Rect::new(7, 5, 0, 0),
    ] {
        for state in WhaleState::ALL {
            for profile in Profile::ALL {
                let mut buffer = Buffer::empty(bounds);
                let before = buffer.clone();
                Whale::new(state).paint(Rect::new(0, 0, 100, 100), &mut buffer, &profile.theme());
                assert_eq!(buffer, before);
            }
        }
    }
}

#[test]
fn the_action_sheet_accepts_a_buffer_origin_near_the_coordinate_limit() {
    let entry = gallery::entries()
        .into_iter()
        .find(|entry| entry.name == "whale-actions")
        .expect("the gallery includes every whale action");
    let area = Rect::new(u16::MAX - 35, u16::MAX - 20, 35, 20);
    for profile in Profile::ALL {
        let theme = profile.theme();
        let base = theme
            .bg(Role::Surface)
            .patch(theme.fg(Role::Muted))
            .add_modifier(Modifier::ITALIC);
        let mut before = Buffer::filled(area, Cell::new("·"));
        before.set_style(area, base);
        let mut after = before.clone();
        (entry.draw)(area, &mut after, &theme);
        assert_eq!(after.area, area);
        assert_ne!(
            after,
            before,
            "the visible tile still paints in {}",
            profile.name()
        );
        let local = Rect::new(0, 0, area.width, area.height);
        let mut expected = Buffer::filled(local, Cell::new("·"));
        expected.set_style(local, base);
        (entry.draw)(local, &mut expected, &theme);
        assert_eq!(
            after.content(),
            expected.content(),
            "coordinate translation preserves ink, clipping and existing cell styles"
        );
    }
}
