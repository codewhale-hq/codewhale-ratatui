//! The two pixel characters as built-in avatar packs: the shipped files, the
//! painter's cell-for-cell rule, and what a terminal shows.
//!
//! The pages are the art; `terminal.rgba` is one pixel per art cell, checked
//! here against the page and the hash manifest. The painted buffer is then
//! compared with the pose as written in the hand-drawn grid, kept as a small
//! fixture so the test needs nothing outside this repository.

use codewhale_ratatui::{
    Paint,
    avatar_builtin::{self, Builtin},
    testing::{self, Profile},
};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

struct Shipped {
    id: &'static str,
    manifest: &'static str,
    pack: &'static [u8],
    page: &'static [u8],
}
const SHIPPED: [Shipped; 2] = [
    Shipped {
        id: "pixel-whale",
        manifest: include_str!("../assets/pixel-whale/manifest.json"),
        pack: include_bytes!("../assets/pixel-whale/avatar.json"),
        page: include_bytes!("../assets/pixel-whale/page-00.png"),
    },
    Shipped {
        id: "pixel-whale-girl",
        manifest: include_str!("../assets/pixel-whale-girl/manifest.json"),
        pack: include_bytes!("../assets/pixel-whale-girl/avatar.json"),
        page: include_bytes!("../assets/pixel-whale-girl/page-00.png"),
    },
];

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn builtin(id: &str) -> &'static Builtin {
    avatar_builtin::find(id).expect("a built-in character")
}

#[test]
fn both_packs_validate_and_match_their_manifest() {
    for shipped in &SHIPPED {
        let builtin = builtin(shipped.id);
        let pack = builtin.pack();
        pack.validate().unwrap();
        pack.validate_png(shipped.page).unwrap();
        assert_eq!(pack.atlases, ["page-00.png"]);
        let manifest: serde_json::Value = serde_json::from_str(shipped.manifest).unwrap();
        assert_eq!(manifest["packSha256"], sha256(shipped.pack));
        assert_eq!(manifest["pages"][0]["sha256"], sha256(shipped.page));
        assert_eq!(manifest["terminalSha256"], sha256(builtin.pixels));
        assert_eq!(manifest["terminalTile"][0], builtin.width);
        assert_eq!(manifest["terminalTile"][1], builtin.height);
        assert_eq!(
            manifest["cell"],
            usize::from(pack.tile_width) / builtin.width
        );
    }
}

/// `terminal.rgba` is the page with each flat art cell read once. Every page
/// pixel is compared, so a page that is not pixel art on that grid fails too.
#[test]
fn terminal_tiles_are_the_page_one_pixel_per_art_cell() {
    for shipped in &SHIPPED {
        let builtin = builtin(shipped.id);
        let pack = builtin.pack();
        let page = image::load_from_memory_with_format(shipped.page, image::ImageFormat::Png)
            .unwrap()
            .to_rgba8();
        let cell = u32::from(pack.tile_width) / builtin.width as u32;
        for (x, y, pixel) in page.enumerate_pixels() {
            let frame = (y / u32::from(pack.tile_height)) * u32::from(pack.columns)
                + x / u32::from(pack.tile_width);
            let (tx, ty) = (
                x % u32::from(pack.tile_width) / cell,
                y % u32::from(pack.tile_height) / cell,
            );
            let at =
                ((frame as usize * builtin.height + ty as usize) * builtin.width + tx as usize) * 4;
            assert_eq!(
                pixel.0,
                builtin.pixels[at..at + 4],
                "{} page pixel {x},{y}",
                shipped.id
            );
        }
    }
}

/// A pose as written in the grid: ink colours, then one character per cell.
struct Drawn {
    inks: BTreeMap<char, Color>,
    rows: Vec<Vec<char>>,
}
fn drawn(fixture: &str) -> Drawn {
    let mut inks = BTreeMap::new();
    let mut rows = Vec::new();
    for line in fixture.lines().filter(|line| !line.starts_with('#')) {
        if let Some(ink) = line.strip_prefix("ink ") {
            let (key, hex) = ink.split_once(" #").expect("ink <key> #rrggbb");
            let channel = |at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap();
            inks.insert(
                key.chars().next().unwrap(),
                Color::Rgb(channel(0), channel(2), channel(4)),
            );
        } else {
            rows.push(line.chars().collect());
        }
    }
    Drawn { inks, rows }
}

/// What a half-block cell must be for the two grid cells it stands for.
fn expected(drawn: &Drawn, x: usize, row: usize) -> (&'static str, Color, Color) {
    let ink = |y: usize| {
        drawn
            .rows
            .get(y)
            .map(|cells| cells[x])
            .filter(|&c| c != '.')
            .map(|c| drawn.inks[&c])
    };
    match (ink(row * 2), ink(row * 2 + 1)) {
        (Some(top), Some(bottom)) => ("▀", top, bottom),
        (Some(top), None) => ("▀", top, Color::Reset),
        (None, Some(bottom)) => ("▄", bottom, Color::Reset),
        (None, None) => (" ", Color::Reset, Color::Reset),
    }
}

fn assert_painted_as_drawn(id: &str, state: &str, fixture: &str, area: Rect, offset: (u16, u16)) {
    let drawn = drawn(fixture);
    let (width, height) = (drawn.rows[0].len(), drawn.rows.len());
    let builtin = builtin(id);
    let frame = builtin.pack().sample(state, 0., true, None, None).index;
    let sprite = builtin.sprite(frame).unwrap();
    assert_eq!(sprite.reduction(area), 1, "{id} gets one cell per pixel");
    // No ground is painted first, so a cell's colours are the pack's own.
    let theme = Profile::DarkTrue.theme();
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 40));
    sprite.paint(area, &mut buf, &theme);
    let mut painted = 0;
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            let inside = x >= offset.0
                && y >= offset.1
                && usize::from(x - offset.0) < width
                && usize::from(y - offset.1) < height.div_ceil(2);
            let want = if inside {
                expected(&drawn, usize::from(x - offset.0), usize::from(y - offset.1))
            } else {
                (" ", Color::Reset, Color::Reset)
            };
            assert_eq!(
                (cell.symbol(), cell.fg, cell.bg),
                want,
                "{id} {state} at {x},{y}"
            );
            painted += usize::from(cell.symbol() != " ");
        }
    }
    assert!(painted > width, "{id} {state} painted");
}

#[test]
fn the_painted_cells_equal_the_hand_drawn_grid_cell_for_cell() {
    // An exact area, then roomy ones: the art stays 1:1 and is centred.
    let whale = include_str!("fixtures/pixel-avatar/pixel-whale-rest.grid");
    assert_painted_as_drawn(
        "pixel-whale",
        "rest",
        whale,
        Rect::new(0, 0, 26, 11),
        (0, 0),
    );
    assert_painted_as_drawn(
        "pixel-whale",
        "rest",
        whale,
        Rect::new(4, 2, 64, 32),
        (23, 12),
    );
    let girl = include_str!("fixtures/pixel-avatar/pixel-whale-girl-needs.grid");
    assert_painted_as_drawn(
        "pixel-whale-girl",
        "needs",
        girl,
        Rect::new(0, 0, 39, 26),
        (0, 0),
    );
    assert_painted_as_drawn(
        "pixel-whale-girl",
        "needs",
        girl,
        Rect::new(1, 3, 60, 30),
        (11, 5),
    );
}

/// When the area is too small the art is reduced by a whole number: every
/// painted pixel is still one of the drawing's own cells, never a blend.
#[test]
fn a_small_area_reduces_by_a_whole_number_and_never_blends() {
    let theme = Profile::DarkTrue.theme();
    for id in ["pixel-whale", "pixel-whale-girl"] {
        let builtin = builtin(id);
        let own: std::collections::HashSet<Color> = builtin
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] == 255)
            .map(|p| Color::Rgb(p[0], p[1], p[2]))
            .collect();
        let frame = builtin.pack().sample("rest", 0., true, None, None).index;
        let sprite = builtin.sprite(frame).unwrap();
        for (width, height) in [(20, 20), (13, 6), (7, 3), (1, 1), (200, 1)] {
            let area = Rect::new(2, 1, width, height);
            let step = sprite.reduction(area);
            assert!(step >= 2, "{id} does not fit {width}x{height} whole");
            let mut buf = Buffer::empty(Rect::new(0, 0, 210, 24));
            sprite.paint(area, &mut buf, &theme);
            let mut painted = 0;
            for y in 0..24 {
                for x in 0..210 {
                    let cell = &buf[(x, y)];
                    if cell.symbol() == " " {
                        continue;
                    }
                    painted += 1;
                    assert!(area.contains((x, y).into()), "{id} left its area");
                    for color in [cell.fg, cell.bg] {
                        assert!(
                            color == Color::Reset || own.contains(&color),
                            "{id} {color:?}"
                        );
                    }
                }
            }
            assert!(painted > 0, "{id} painted at {width}x{height}");
        }
        assert_eq!(sprite.reduction(Rect::new(0, 0, 0, 5)), 0);
    }
}

#[test]
fn pixel_characters_obey_clip_bounds_and_terminal_color_caps() {
    for builtin in avatar_builtin::all().iter().filter(|b| b.pixel_art) {
        let sprite = builtin.sprite(2).unwrap();
        for profile in Profile::ALL {
            let theme = profile.theme();
            for area in [
                Rect::new(0, 0, 0, 0),
                Rect::new(0, 0, 1, 1),
                Rect::new(1, 1, 30, 15),
                Rect::new(20, 10, 60, 30),
                Rect::new(65500, 65500, 10, 10),
            ] {
                let mut buffer = Buffer::empty(Rect::new(0, 0, 34, 20));
                sprite.paint(area, &mut buffer, &theme);
                let clipped = area.intersection(buffer.area);
                for y in 0..20 {
                    for x in 0..34 {
                        let c = &buffer[(x, y)];
                        if !clipped.contains((x, y).into()) {
                            assert_eq!(c.symbol(), " ");
                            assert_eq!(c.fg, Color::Reset);
                        }
                        assert!(testing::color_allowed(profile, c.fg), "{profile:?}");
                        assert!(testing::color_allowed(profile, c.bg), "{profile:?}");
                        if profile == Profile::Ascii {
                            assert!(c.symbol().is_ascii());
                        }
                    }
                }
            }
        }
    }
}

/// Glyphs and the colour of every run at truecolor (the pack's own hex, or
/// the role it happens to equal), then the one-ink rendering a terminal
/// without colour gets.
fn dump(id: &str, state: &str) -> String {
    let builtin = builtin(id);
    let frame = builtin.pack().sample(state, 0., true, None, None).index;
    let sprite = builtin.sprite(frame).unwrap();
    let art = builtin.art.unwrap();
    let (width, height) = (art.width, art.height.div_ceil(2));
    let mut out = String::new();
    for profile in [Profile::DarkTrue, Profile::NoColor] {
        let theme = profile.theme();
        let buf = testing::render(width, height, |area, buf| sprite.paint(area, buf, &theme));
        out.push_str(&format!("== {}\n", profile.name()));
        out.push_str(&if profile == Profile::NoColor {
            testing::text(&buf)
        } else {
            testing::styled(&buf, &theme)
        });
        out.push('\n');
    }
    out
}

#[test]
fn pixel_whale_rest_and_pixel_whale_girl_needs_snapshots() {
    insta::assert_snapshot!("pixel-whale-rest", dump("pixel-whale", "rest"));
    insta::assert_snapshot!("pixel-whale-girl-needs", dump("pixel-whale-girl", "needs"));
}
