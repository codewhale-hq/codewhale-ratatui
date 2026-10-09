//! A rendering boundary for host-evaluated frames, not a Director port.

use codewhale_ratatui::{
    Paint, Role, Whale, WhaleState,
    testing::{Profile, text},
    whale::{self, Grid},
};
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    style::Modifier,
};

fn supplied(cols: u16, rows: u16, first: u8, last: u8) -> Grid {
    let mut cells = vec![0; usize::from(cols) * usize::from(rows)];
    cells[0] = first;
    *cells.last_mut().expect("a nonempty test frame") = last;
    Grid { cols, rows, cells }
}

#[test]
fn host_frames_change_the_drawing_without_changing_the_state_words() {
    let theme = Profile::DarkTrue.theme();
    let whale = Whale::new(WhaleState::Read);
    let bounds = Rect::new(7, 5, 16, 9);
    let mut first = Buffer::empty(bounds);
    let mut next = first.clone();
    whale.paint_frame(bounds, &mut first, &theme, &supplied(16, 8, 0x01, 0));
    whale.paint_frame(bounds, &mut next, &theme, &supplied(16, 8, 0xff, 0x80));
    assert_eq!(first[(7, 5)].symbol(), "⠁");
    assert_eq!(next[(7, 5)].symbol(), "⣿");
    assert_eq!(next[(22, 12)].symbol(), "⢀");
    assert_ne!(first, next);
    for x in bounds.left()..bounds.right() {
        assert_eq!(first[(x, 13)], next[(x, 13)]);
    }
    assert!(text(&next).contains("Reading"));
}

#[test]
fn callers_can_supply_a_detailed_frame_or_choose_the_existing_static_poster() {
    let whale = Whale::new(WhaleState::Connect);
    for profile in Profile::ALL {
        let theme = profile.theme();
        let bounds = Rect::new(3, 2, 36, 17);
        let base = theme
            .bg(Role::Surface)
            .patch(theme.fg(Role::Muted))
            .add_modifier(Modifier::ITALIC);
        let mut poster = Buffer::filled(bounds, Cell::new("·"));
        poster.set_style(bounds, base);
        let mut evaluated = poster.clone();
        whale.paint(bounds, &mut poster, &theme);
        let still = whale::frame(whale.state, 32, 16).expect("standard poster frame");
        whale.paint_frame(bounds, &mut evaluated, &theme, &still);
        assert_eq!(
            poster,
            evaluated,
            "static host choice in {}",
            profile.name()
        );

        let detailed_bounds = Rect::new(3, 2, 48, 25);
        let before = Buffer::filled(detailed_bounds, Cell::new("·"));
        let mut detailed = before.clone();
        detailed.set_style(detailed_bounds, base);
        let background = detailed[(3, 2)].bg;
        whale.paint_frame(
            detailed_bounds,
            &mut detailed,
            &theme,
            &supplied(48, 24, 0x01, 0x80),
        );
        if theme.ascii() {
            assert!(
                !text(&detailed)
                    .chars()
                    .any(|c| ('\u{2800}'..='\u{28ff}').contains(&c))
            );
        } else {
            assert_eq!(detailed[(3, 2)].symbol(), "⠁");
            assert_eq!(detailed[(50, 25)].symbol(), "⢀");
            assert_eq!(detailed[(3, 2)].bg, background, "art keeps the ground");
            assert!(detailed[(3, 2)].modifier.contains(Modifier::ITALIC));
            assert_eq!(detailed[(4, 2)].symbol(), "·", "zero bits are transparent");
        }
        assert!(text(&detailed).contains("Calling a connected app"));
    }
}

#[test]
fn invalid_nonfitting_and_tiny_frames_degrade_to_the_same_words() {
    let whale = Whale::new(WhaleState::NeedsYou).words("请审阅 café\u{301}\u{202e}\u{1b}");
    let frames = [
        Grid {
            cols: 0,
            rows: 8,
            cells: Vec::new(),
        },
        Grid {
            cols: 16,
            rows: 0,
            cells: Vec::new(),
        },
        Grid {
            cols: 16,
            rows: 8,
            cells: vec![1; 127],
        },
        Grid {
            cols: 16,
            rows: 8,
            cells: vec![1; 129],
        },
        Grid {
            cols: u16::MAX,
            rows: u16::MAX,
            cells: Vec::new(),
        },
        supplied(33, 16, 1, 1),
        supplied(16, 17, 1, 1),
        supplied(15, 8, 1, 1),
        supplied(16, 7, 1, 1),
    ];
    let bounds = Rect::new(u16::MAX - 32, u16::MAX - 17, 32, 17);
    for profile in Profile::ALL {
        let theme = profile.theme();
        let before = Buffer::filled(bounds, Cell::new("·"));
        let mut expected = before.clone();
        whale.paint(
            Rect {
                height: 1,
                ..bounds
            },
            &mut expected,
            &theme,
        );
        for grid in &frames {
            let mut actual = before.clone();
            whale.paint_frame(bounds, &mut actual, &theme, grid);
            assert_eq!(actual, expected, "{} rejected {grid:?}", profile.name());
        }
        assert!(!text(&expected).contains(['\u{202e}', '\u{1b}']));
        for (width, height) in [(15, 9), (16, 8), (1, 1)] {
            let request = Rect::new(bounds.x, bounds.y, width, height);
            let mut actual = before.clone();
            let mut expected = before.clone();
            whale.paint(
                Rect {
                    height: 1,
                    ..request
                },
                &mut expected,
                &theme,
            );
            whale.paint_frame(request, &mut actual, &theme, &supplied(16, 8, 1, 1));
            assert_eq!(actual, expected, "a complete frame plus label must fit");
        }
    }
}

#[test]
fn frames_obey_offset_offbuffer_and_coordinate_limit_boundaries() {
    let whale = Whale::new(WhaleState::Connect);
    let grid = supplied(16, 8, 0xff, 0x80);
    for bounds in [
        Rect::new(0, 0, 18, 12),
        Rect::new(7, 5, 18, 12),
        Rect::new(u16::MAX - 18, u16::MAX - 12, 18, 12),
    ] {
        let requests = [
            bounds,
            Rect::new(bounds.x, bounds.y, 16, 9),
            Rect::new(
                bounds.x.saturating_sub(2),
                bounds.y.saturating_sub(2),
                18,
                11,
            ),
            Rect::new(
                bounds.x.saturating_add(4),
                bounds.y.saturating_add(3),
                20,
                10,
            ),
            Rect::new(bounds.right(), bounds.y, 16, 9),
            Rect::new(bounds.x, bounds.bottom(), 16, 9),
            Rect::new(bounds.x, bounds.y, 0, 9),
            Rect::new(bounds.x, bounds.y, 16, 0),
        ];
        for profile in Profile::ALL {
            let theme = profile.theme();
            let mut before = Buffer::filled(bounds, Cell::new("·"));
            before.set_style(
                bounds,
                theme.bg(Role::Surface).add_modifier(Modifier::ITALIC),
            );
            for request in requests {
                let mut actual = before.clone();
                whale.paint_frame(request, &mut actual, &theme, &grid);
                let allowed = request.intersection(bounds);
                for y in bounds.top()..bounds.bottom() {
                    for x in bounds.left()..bounds.right() {
                        if !allowed.contains((x, y).into()) {
                            assert_eq!(actual[(x, y)], before[(x, y)], "frame stray write");
                        }
                    }
                }
                if allowed.is_empty() {
                    assert_eq!(actual, before);
                } else {
                    let local = Rect::new(0, 0, allowed.width, allowed.height);
                    let mut expected = Buffer::filled(local, Cell::new("·"));
                    expected.set_style(
                        local,
                        theme.bg(Role::Surface).add_modifier(Modifier::ITALIC),
                    );
                    whale.paint_frame(local, &mut expected, &theme, &grid);
                    for y in 0..allowed.height {
                        for x in 0..allowed.width {
                            assert_eq!(
                                actual[(allowed.x + x, allowed.y + y)],
                                expected[(x, y)],
                                "{} preserves translated frame ink and label",
                                profile.name()
                            );
                        }
                    }
                }
            }
        }
    }
    for bounds in [Rect::new(7, 5, 0, 12), Rect::new(7, 5, 18, 0)] {
        let mut buf = Buffer::empty(bounds);
        let before = buf.clone();
        whale.paint_frame(Rect::MAX, &mut buf, &Profile::DarkTrue.theme(), &grid);
        assert_eq!(buf, before);
    }
}
