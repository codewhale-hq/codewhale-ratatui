//! Public terminal boundary checks; canonical performance receipts live with
//! the adapted native core in src/whale_motion/tests.rs.
use std::time::{Duration, Instant};

use codewhale_ratatui::{
    Whale, WhaleState,
    testing::Profile,
    whale_motion::{
        Activity, ColoredGrid, Context, Director, Grid, Inputs, Options, Presence, Stage, Tier,
        View, braille, colored_braille, rasterize, rasterize_colored, scene,
    },
};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

fn work(kind: &str) -> Inputs {
    Inputs {
        presence: Presence::Working,
        activity: Some(Activity {
            kind: Some(kind.into()),
            observed: true,
            ..Activity::default()
        }),
        context: Context {
            live: true,
            ..Context::default()
        },
    }
}

#[test]
fn invalid_public_time_dimensions_and_geometry_leave_a_safe_performance() {
    let mut director = Director::new(Options {
        ambient: false,
        ..Options::default()
    });
    let before = director.pose();
    for dt in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        director.step(dt);
        assert_eq!(director.pose(), before);
        assert_eq!(director.f, 0.);
    }
    let parts = scene(&director, View::hero(512., 1.));
    for (cols, rows) in [
        (0, 0),
        (0, 16),
        (32, 0),
        (161, 16),
        (32, 81),
        (usize::MAX, usize::MAX),
    ] {
        let grid = braille(&director, cols, rows);
        assert!(grid.cells.is_empty());
        assert!(grid.text().is_empty());
        assert!(grid.to_terminal().is_none());
        assert!(
            colored_braille(&director, cols, rows, true)
                .colors
                .is_empty()
        );
    }
    for aspect in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(rasterize(&parts, 32, 16, aspect).cells.is_empty());
    }
    for size in [0., -1., f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(scene(&director, View::hero(size, 1.)).shapes.is_empty());
    }
    for grid in [
        Grid {
            cols: 0,
            rows: 1,
            cells: vec![1],
        },
        Grid {
            cols: 32,
            rows: 16,
            cells: vec![1],
        },
        Grid {
            cols: usize::MAX,
            rows: 2,
            cells: Vec::new(),
        },
    ] {
        assert!(!grid.is_valid());
        assert!(grid.text().is_empty());
        assert!(grid.to_terminal().is_none());
    }
}

#[test]
fn hiding_stops_the_shared_clock_and_resuming_discards_old_events() {
    let mut stage = Stage::new();
    let start = Instant::now();
    stage.observe(Some("a"), work("executing"), false);
    stage.advance(start);
    stage.advance(start + Duration::from_millis(200));
    assert_eq!(
        stage.cadence(Tier::Terminal),
        Some(Duration::from_secs_f64(1. / 6.))
    );
    stage.set_visible(false);
    assert!(!stage.is_visible());
    let frame = stage.director().f;
    stage.observe(
        Some("a"),
        Inputs {
            presence: Presence::Done,
            activity: None,
            context: Context {
                live: true,
                turn_id: Some("completed-hidden".into()),
                status: Some("completed".into()),
                ..Context::default()
            },
        },
        false,
    );
    stage.advance(start + Duration::from_secs(600));
    assert_eq!(stage.director().f, frame);
    assert_eq!(stage.cadence(Tier::Terminal), None);
    stage.set_visible(true);
    assert!(stage.director().shots.is_empty());
    assert!(stage.director().particles.is_empty());
    stage.advance(start + Duration::from_secs(600));
    assert_eq!(
        stage.director().f,
        frame,
        "the resume only establishes a fresh clock"
    );
    stage.advance(start + Duration::from_secs(601));
    assert!(
        stage.director().emissions.is_empty(),
        "a hidden completion never replays its spout"
    );
    stage.observe(
        Some("b"),
        Inputs {
            presence: Presence::Idle,
            activity: None,
            context: Context::default(),
        },
        false,
    );
    assert_eq!(
        stage.cadence(Tier::Terminal),
        Some(Duration::from_secs_f64(1. / 2.))
    );
    stage.observe(Some("b"), work("reading"), true);
    assert_eq!(stage.cadence(Tier::Terminal), None);
}

#[test]
fn colored_frames_keep_native_pointer_ink_and_the_exact_monochrome_geometry() {
    let mut director = Director::new(Options {
        reduced: true,
        ambient: false,
        ..Options::default()
    });
    let inputs = work("computer");
    director.set(inputs.presence, inputs.activity, inputs.context);
    let mono = braille(&director, 48, 24);
    let dark = colored_braille(&director, 48, 24, true);
    let light = colored_braille(&director, 48, 24, false);
    assert_eq!(dark.grid, mono);
    assert_eq!(light.grid, mono);
    assert!(
        dark.colors.contains(&Some(0x66d6de)),
        "the computer pointer keeps its native cyan"
    );
    assert!(
        light.colors.contains(&Some(0x147888)),
        "the paper pointer keeps its native teal"
    );
    assert_ne!(dark.colors, light.colors);
    let body_colors: std::collections::HashSet<_> = dark.colors.iter().flatten().copied().collect();
    assert!(
        body_colors.len() > 8,
        "the body contains a resolved ombre rather than one flat ink"
    );
    for (bits, color) in dark.grid.cells.iter().zip(&dark.colors) {
        assert_eq!(*bits == 0, color.is_none());
    }
}

#[test]
fn a_mixed_cell_uses_majority_ink_and_later_paint_order_only_for_ties() {
    use codewhale_ratatui::whale_motion::{Parts, Role, Shape, rig::parse};
    let shape = |role, path| Shape {
        id: "sample".into(),
        role,
        opacity: 1.,
        path: parse(path),
    };
    let body = shape(
        Role::Body,
        "M-62 -124 C62 -124 62 -124 62 -124 C62 124 62 124 62 124 C-62 124 -62 124 -62 124 C-62 -124 -62 -124 -62 -124 Z",
    );
    let accent = shape(
        Role::Accent,
        "M-62 -124 C0 -124 0 -124 0 -124 C0 124 0 124 0 124 C-62 124 -62 124 -62 124 C-62 -124 -62 -124 -62 -124 Z",
    );
    let pointer = shape(
        Role::Pointer,
        "M0 -124 C62 -124 62 -124 62 -124 C62 0 62 0 62 0 C0 0 0 0 0 0 C0 -124 0 -124 0 -124 Z",
    );
    let mut parts = Parts {
        shapes: vec![body, accent],
        ..Parts::default()
    };
    let tie = rasterize_colored(&parts, 1, 1, 0.5, true);
    assert_eq!(tie.grid.cells, vec![255]);
    assert_eq!(
        tie.colors,
        vec![Some(0xd2a34e)],
        "four gold dots win their tie with earlier body ink"
    );
    parts.shapes.push(pointer);
    let majority = rasterize_colored(&parts, 1, 1, 0.5, true);
    assert_eq!(majority.grid.cells, vec![255]);
    assert_eq!(
        majority.colors,
        vec![Some(0xd2a34e)],
        "two later pointer dots cannot override four gold dots"
    );
    assert!(parse("M-").is_empty());
    assert!(parse("M0 0 C1").is_empty());
    assert!(codewhale_ratatui::whale_motion::holes_for("魚bc", &[], true).is_empty());
}

#[test]
fn decorative_contrast_adjustment_preserves_geometry_ground_and_unknown_fallback() {
    let director = Director::new(Options {
        reduced: true,
        ambient: false,
        ..Options::default()
    });
    let grid = colored_braille(&director, 32, 16, true);
    let original = grid.clone();
    let area = Rect::new(0, 0, 40, 20);
    let whale = Whale::new(WhaleState::Rest);
    for profile in Profile::ALL {
        let theme = profile.theme();
        let mut plain = Buffer::empty(area);
        for cell in &mut plain.content {
            cell.set_bg(
                theme
                    .color(codewhale_ratatui::Role::Background)
                    .unwrap_or(Color::Reset),
            );
        }
        let mut adjusted = plain.clone();
        grid.paint(&whale, area, &mut plain, &theme);
        grid.paint_with_contrast(&whale, area, &mut adjusted, &theme, 3.0);
        for (a, b) in plain.content.iter().zip(&adjusted.content) {
            assert_eq!(a.symbol(), b.symbol());
            assert_eq!(a.bg, b.bg);
            if b.symbol()
                .chars()
                .any(|c| ('\u{2800}'..='\u{28ff}').contains(&c))
                && let Some(ratio) = codewhale_ratatui::color::contrast_ratio(b.fg, b.bg)
            {
                assert!(ratio >= 3.0, "{}: {ratio}", profile.name());
            }
        }
        if !theme.paints_grounds() {
            assert_eq!(plain, adjusted);
        }
    }
    assert_eq!(grid, original);
}

#[test]
fn colored_terminal_paint_preserves_words_grounds_and_buffer_bounds_in_all_profiles() {
    let mut director = Director::new(Options {
        reduced: true,
        ambient: false,
        ..Options::default()
    });
    let inputs = work("computer");
    director.set(inputs.presence, inputs.activity, inputs.context);
    let grid = colored_braille(&director, 32, 16, true);
    let whale = Whale::new(WhaleState::Computer).words("操作中 · café · 🐋\u{1b}[31m");
    let cases = [
        (Rect::new(0, 0, 40, 20), Rect::new(2, 1, 36, 18)),
        (Rect::new(100, 200, 40, 20), Rect::new(95, 195, 50, 30)),
        (
            Rect::new(65_495, 65_515, 40, 20),
            Rect::new(65_495, 65_515, 40, 20),
        ),
        (Rect::new(20, 30, 20, 8), Rect::new(0, 0, 5, 5)),
        (Rect::new(20, 30, 20, 8), Rect::new(25, 32, 5, 3)),
        (Rect::new(0, 0, 0, 0), Rect::new(0, 0, 0, 0)),
    ];
    for profile in Profile::ALL {
        let theme = profile.theme();
        for (buffer_area, request) in cases {
            let mut buffer = Buffer::empty(buffer_area);
            for cell in &mut buffer.content {
                cell.set_symbol("·").set_bg(Color::Rgb(12, 34, 56));
            }
            let original = buffer.clone();
            grid.paint(&whale, request, &mut buffer, &theme);
            let visible = request.intersection(buffer_area);
            for y in buffer_area.y..buffer_area.bottom() {
                for x in buffer_area.x..buffer_area.right() {
                    assert_eq!(buffer[(x, y)].bg, original[(x, y)].bg);
                    if !visible.contains((x, y).into()) {
                        assert_eq!(
                            buffer[(x, y)],
                            original[(x, y)],
                            "outside {request:?}, {}",
                            profile.name()
                        );
                    }
                }
            }
            assert!(
                buffer
                    .content
                    .iter()
                    .all(|cell| !cell.symbol().contains('\u{1b}'))
            );
            if profile == Profile::Ascii {
                assert!(!buffer.content.iter().any(|cell| {
                    cell.symbol()
                        .chars()
                        .any(|ch| ('\u{2800}'..='\u{28ff}').contains(&ch))
                }));
            }
        }
    }
    // A malformed color plane follows the painter's words-only fallback.
    let invalid = ColoredGrid {
        grid: grid.grid,
        colors: Vec::new(),
    };
    let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 20));
    invalid.paint(
        &Whale::new(WhaleState::Computer).words("Caller words"),
        buffer.area,
        &mut buffer,
        &Profile::DarkTrue.theme(),
    );
    let text: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(text.contains("Caller words"));
    assert!(
        !text
            .chars()
            .any(|ch| ('\u{2800}'..='\u{28ff}').contains(&ch))
    );
}
