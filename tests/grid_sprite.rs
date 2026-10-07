//! The pixel sprites: the two grids as shipped, the parser, and the painter.
//!
//! The grids are byte copies of the design source, pinned by hash so a local
//! edit or a missed update fails here. The parser is checked against the real
//! files and against each rule the grammar says a loader must enforce. The
//! painter is checked for the rules every frame keeps (9 profiles), for what
//! it paints (snapshots at truecolor, no color and ASCII), and for staying
//! inside whatever rectangle a layout hands it.

use codewhale_ratatui::{
    GridSheet, GridSprite, GridToken, Paint, Role, TuiPalette,
    grid_sprite::{WHALE_GIRL_GRID, WHALE_GRID},
    testing::{self, Profile},
};
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    style::Color,
};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// The grids as shipped
// ---------------------------------------------------------------------------

/// SHA-256 of `pixel-system/sprites/whale.grid` (v3) and `whale-girl.grid`
/// (v2). To take a new drawing, copy the file again and update its hash.
const WHALE_SHA256: &str = "eb35ca8232c55af7426cb5177e476f50b77babda92abe8a844f06f39ae30f686";
const WHALE_GIRL_SHA256: &str = "8f3f00a9ea0df1e03f362f6d8249910c01bed1540def573c3fb2845be5c35069";

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn the_grids_are_byte_copies_of_the_design_source() {
    assert_eq!(sha256(WHALE_GRID), WHALE_SHA256, "whale.grid drifted");
    assert_eq!(
        sha256(WHALE_GIRL_GRID),
        WHALE_GIRL_SHA256,
        "whale-girl.grid drifted"
    );
}

/// Every sprite the parser holds is the block written in the file, and
/// nothing in the file was skipped: the rows survive the round trip.
fn assert_round_trip(source: &str, sheet: &GridSheet) {
    let lines: Vec<&str> = source.split('\n').collect();
    let mut declared = 0;
    for (at, line) in lines.iter().enumerate() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let ["sprite", name, size] = parts.as_slice() else {
            continue;
        };
        declared += 1;
        let (width, height) = size.split_once('x').expect("<w>x<h>");
        let (width, height): (usize, usize) = (width.parse().unwrap(), height.parse().unwrap());
        let rows = sheet.sprite_rows(name).expect("parsed sprite");
        assert_eq!(rows, &lines[at + 1..at + 1 + height], "{name}");
        assert!(rows.iter().all(|row| row.len() == width), "{name}");
    }
    assert_eq!(sheet.sprite_names().count(), declared);
    let declared_poses = lines.iter().filter(|l| l.starts_with("pose ")).count();
    assert_eq!(sheet.poses().len(), declared_poses);
}

#[test]
fn both_real_grids_parse_and_round_trip() {
    let whale = GridSheet::parse(WHALE_GRID).expect("whale.grid");
    assert_round_trip(WHALE_GRID, &whale);
    assert_eq!(whale.sprite_names().count(), 21);
    assert_eq!(whale.poses().len(), 13);

    let girl = GridSheet::parse(WHALE_GIRL_GRID).expect("whale-girl.grid");
    assert_round_trip(WHALE_GIRL_GRID, &girl);
    assert_eq!(girl.sprite_names().count(), 16);
    assert_eq!(girl.poses().len(), 16);

    // Stage sizes and the pose set, as the design records them.
    let sizes = |sheet: &GridSheet, body: &str| -> Vec<(String, u16, u16)> {
        sheet
            .poses()
            .iter()
            .filter(|p| p.body() == body)
            .map(|p| (p.name().to_string(), p.width(), p.height()))
            .collect()
    };
    let eight = [
        "rest",
        "think",
        "read",
        "write",
        "run",
        "needs-you",
        "done",
        "sleep",
    ];
    let named =
        |w, h| -> Vec<(String, u16, u16)> { eight.iter().map(|n| (n.to_string(), w, h)).collect() };
    assert_eq!(sizes(&whale, "mark"), [("rest".to_string(), 16, 14)]);
    assert_eq!(sizes(&whale, "favicon"), [("rest".to_string(), 16, 16)]);
    assert_eq!(sizes(&whale, "companion"), named(26, 22));
    assert_eq!(
        sizes(&whale, "hero"),
        ["rest", "done", "sleep"].map(|n| (n.to_string(), 42, 37))
    );
    assert_eq!(sizes(&girl, "small"), named(16, 22));
    assert_eq!(sizes(&girl, "hero"), named(39, 52));
    // 11 rows for the companion and the small girl; an odd stage rounds up.
    assert_eq!(whale.pose("companion", "rest").unwrap().rows(), 11);
    assert_eq!(whale.pose("hero", "rest").unwrap().rows(), 19);
    assert_eq!(girl.pose("small", "rest").unwrap().rows(), 11);
}

#[test]
fn inks_carry_their_token_and_their_one_color_level() {
    let level = |sheet: &GridSheet, fill: bool| -> String {
        sheet
            .inks()
            .iter()
            .filter(|(_, ink)| ink.fill == fill)
            .map(|(key, _)| *key)
            .collect()
    };
    let (whale, girl) = (GridSheet::whale(), GridSheet::whale_girl());
    assert_eq!(level(whale, true), "BLDKSTA");
    assert_eq!(level(whale, false), "WPEO");
    assert_eq!(level(girl, true), "NBDGKA");
    assert_eq!(level(girl, false), "TSsC");
    // Two inks may share a token: the whale's eye and its outline.
    let token = |key| {
        whale
            .inks()
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap()
            .1
            .token
    };
    assert_eq!(token('E'), GridToken::Line);
    assert_eq!(token('K'), GridToken::Line);
    assert_eq!(token('B'), GridToken::Whale);
    for token in GridToken::ALL {
        assert_eq!(GridToken::from_name(token.name()), Some(token));
    }
}

#[test]
fn a_pose_composes_its_layers_at_their_slots_and_keeps_the_still_frame() {
    let whale = GridSheet::whale();
    let rest = whale.pose("companion", "rest").unwrap();
    let think = whale.pose("companion", "think").unwrap();
    // `bubbles` is frames 0..=2 at slot prop (18, 0); the still is the last.
    let still = whale.sprite_rows("bubbles.2").unwrap();
    for (row, line) in still.iter().enumerate() {
        for (column, key) in line.chars().enumerate() {
            let (x, y) = (18 + column as u16, row as u16);
            let expected = whale
                .inks()
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, i)| *i);
            assert_eq!(think.cell(x, y), expected.or(rest.cell(x, y)));
        }
    }
    assert_ne!(
        whale.sprite_rows("bubbles.0").unwrap(),
        still,
        "the frames differ, so the choice of still is observable"
    );
    // Outside the prop the body is untouched.
    assert_eq!(think.cell(0, 12), rest.cell(0, 12));
    // A later layer wins: sleep closes the eye that rest leaves open.
    let sleep = whale.pose("companion", "sleep").unwrap();
    assert_ne!(sleep.cell(12, 14), rest.cell(12, 14));
    // Off the stage is empty, never a panic.
    assert_eq!(rest.cell(26, 0), None);
    assert_eq!(rest.cell(0, 22), None);
    assert_eq!(rest.cell(u16::MAX, u16::MAX), None);
}

#[test]
fn the_loader_rejects_what_it_cannot_draw() {
    const BASE: &str = "ink B whale fill\nsprite dot 2x1\nBB\nstage s 4x2\nslot s at 2 1\n";
    let rejects = |source: &str, why: &str| {
        let error = GridSheet::parse(source).err();
        assert!(
            error.as_deref().is_some_and(|e| e.contains(why)),
            "{source:?} should fail with {why:?}, got {error:?}"
        );
    };
    assert!(GridSheet::parse(&format!("{BASE}pose s rest dot dot@at\n")).is_ok());
    rejects("ink B whale fill\nsprite a 2x2\nBB\nB\n", "ragged row");
    rejects("ink B whale fill\nsprite a 2x1\nBX\n", "no ink for 'X'");
    rejects("ink B plaid fill\n", "unknown token plaid");
    rejects("ink B whale\n", "unknown line");
    rejects("ink B whale solid\n", "fill or open");
    rejects("ink BB whale fill\n", "one character");
    rejects("ink B whale fill\nink B cream open\n", "already taken");
    rejects("ink B whale fill\nsprite a 2x3\nBB\n", "cut short");
    rejects("sprite a 0x1\n", "expected <w>x<h>");
    rejects("sprite a 2\n", "expected <w>x<h>");
    rejects("stage s 257x1\n", "expected <w>x<h>");
    rejects(&format!("{BASE}sprite dot 1x1\nB\n"), "defined twice");
    rejects(&format!("{BASE}pose nowhere rest dot\n"), "no stage");
    rejects(&format!("{BASE}pose s rest ghost\n"), "no sprite ghost");
    rejects(
        &format!("{BASE}pose s rest dot@missing\n"),
        "no slot missing",
    );
    rejects(
        &format!("{BASE}slot s edge 3 1\npose s rest dot@edge\n"),
        "leaves the 4x2 stage",
    );
    rejects(&format!("{BASE}pose s rest\n"), "unknown line");
    rejects(
        &format!("{BASE}pose s rest dot\npose s rest dot\n"),
        "defined twice",
    );
    rejects("draw a whale\n", "unknown line");
    // Every frame of an animated layer must fit, even though one is drawn.
    rejects(
        "ink B whale fill\nsprite f.0 3x1\nBBB\nsprite f.1 1x1\nB\nstage s 2x1\npose s rest f\n",
        "leaves the 2x1 stage",
    );
}

// ---------------------------------------------------------------------------
// The painter
// ---------------------------------------------------------------------------

fn companion(pose: &str) -> GridSprite<'static> {
    GridSprite::whale("companion", pose).expect("companion pose")
}

fn small(pose: &str) -> GridSprite<'static> {
    GridSprite::whale_girl("small", pose).expect("small pose")
}

/// One frame per profile at the sprite's own size.
fn frames(sprite: GridSprite<'static>, profiles: &[Profile]) -> Vec<testing::Frame> {
    testing::frames_for(
        "",
        profiles,
        &[sprite.columns()],
        sprite.rows(),
        |area, buf, theme| sprite.paint(area, buf, theme),
    )
}

/// Truecolor on both grounds with the ink of each run, then the one-ink map
/// as glyphs and its ASCII form.
fn snapshot(sprite: GridSprite<'static>) -> String {
    let mut out = String::new();
    for frame in frames(sprite, &[Profile::DarkTrue, Profile::LightTrue]) {
        out += &format!("== {}\n{}\n", frame.label(), frame.styled());
    }
    for frame in frames(sprite, &[Profile::NoColor, Profile::Ascii]) {
        out += &format!("== {}\n{}\n\n", frame.label(), frame.text());
    }
    out
}

#[test]
fn whale_companion_needs_you() {
    let sprite = companion("needs-you");
    testing::assert_rules(sprite.rows(), |area, buf, theme| {
        sprite.paint(area, buf, theme)
    });
    insta::assert_snapshot!("whale-companion-needs-you", snapshot(sprite));
}

#[test]
fn whale_girl_small_rest() {
    let sprite = small("rest");
    testing::assert_rules(sprite.rows(), |area, buf, theme| {
        sprite.paint(area, buf, theme)
    });
    insta::assert_snapshot!("whale-girl-small-rest", snapshot(sprite));
}

/// The design's reference renderer (`render.py`, `halfblocks` with no theme)
/// wrote this for the mark. A second implementation agreeing cell for cell is
/// the check that the fill/open map is the documented one.
#[test]
fn the_one_color_map_matches_the_reference_renderer() {
    const REFERENCE: &str = "   ▄▄███▀▀▀
 ▄███▀▀▀▀███
▄██▀
███      ▄▄▄▄███
████▄████▀█▀  ▄▀
 ███████▀▀   ▄▀
  ▀████▄▄▄▄▀▀";
    let mark = GridSprite::whale("mark", "rest").unwrap();
    for profile in [Profile::NoColor, Profile::Ansi16, Profile::UnknownGround] {
        assert_eq!(testing::text(&frames(mark, &[profile])[0].buf), REFERENCE);
    }
    // Sixteen colors and an unmeasured ground get the map in the one ink the
    // terminal can be asked for; NO_COLOR gets no color at all.
    let ink = |profile| frames(mark, &[profile])[0].buf[(5, 0)].fg;
    assert_eq!(ink(Profile::Ansi16), Color::Blue);
    assert_eq!(ink(Profile::UnknownGround), Color::Blue);
    assert_eq!(ink(Profile::NoColor), Color::Reset);
}

#[test]
fn every_pose_of_both_characters_keeps_the_rules_in_every_profile() {
    for sheet in [GridSheet::whale(), GridSheet::whale_girl()] {
        for pose in sheet.poses() {
            let sprite = GridSprite::new(pose);
            let frames = testing::frames_for(
                pose.name(),
                &Profile::ALL,
                &[pose.width()],
                pose.rows(),
                |area, buf, theme| sprite.paint(area, buf, theme),
            );
            testing::assert_frames_keep_the_rules(&frames);
            for frame in &frames {
                assert!(
                    !frame.text().trim().is_empty(),
                    "{} {} paints in {}",
                    pose.body(),
                    pose.name(),
                    frame.label()
                );
            }
        }
    }
}

#[test]
fn colored_inks_come_from_the_theme() {
    for profile in [Profile::DarkTrue, Profile::DarkGraphite, Profile::LightTrue] {
        let theme = profile.theme();
        let frame = &frames(companion("needs-you"), &[profile])[0];
        // Blue means the whale is present: the body is exactly Primary.
        let body = &frame.buf[(2, 4)];
        assert_eq!((body.symbol(), body.fg), ("█", theme.token(Role::Primary)));
        // The `?` is the theme's attention ink over the panel's paper.
        let mark = &frame.buf[(20, 1)];
        assert_eq!(
            (mark.symbol(), mark.fg, mark.bg),
            (
                "▀",
                theme.token(Role::Attention),
                theme.token(Role::Surface)
            )
        );
    }
    // A palette that leaves its grounds to the terminal never paints an ink
    // with the terminal's own color, in any native palette.
    for palette in TuiPalette::ALL {
        for profile in [Profile::DarkTrue, Profile::LightTrue, Profile::Dark256] {
            let theme = profile.theme().tui_palette(palette);
            for sprite in [
                companion("needs-you"),
                GridSprite::whale_girl("hero", "done").unwrap(),
            ] {
                let buf = testing::render(sprite.columns(), sprite.rows(), |area, buf| {
                    sprite.paint(area, buf, &theme)
                });
                assert!(
                    buf.content()
                        .iter()
                        .all(|cell| cell.symbol() == " " || cell.fg != Color::Reset),
                    "{palette:?} {profile:?}"
                );
            }
        }
    }
}

#[test]
fn a_sprite_keeps_the_ground_and_clears_the_modifiers_under_it() {
    use ratatui::style::{Modifier, Style};
    let theme = Profile::DarkTrue.theme();
    let ground = Color::Rgb(1, 2, 3);
    let sprite = companion("rest");
    let buf = testing::render(26, 11, |area, buf| {
        buf.set_style(
            area,
            Style::default().bg(ground).add_modifier(Modifier::REVERSED),
        );
        sprite.paint(area, buf, &theme);
    });
    // (5, 0) is a lower-half cell: ink below, the caller's ground above.
    let half = &buf[(5, 0)];
    assert_eq!((half.symbol(), half.bg), ("▄", ground));
    assert!(half.modifier.is_empty());
    // An empty cell is not touched at all.
    let empty = &buf[(0, 0)];
    assert_eq!((empty.symbol(), empty.bg), (" ", ground));
    assert!(empty.modifier.contains(Modifier::REVERSED));
}

#[test]
fn a_pose_that_does_not_fit_is_left_out_rather_than_cropped() {
    let sprite = companion("rest");
    assert_eq!((sprite.columns(), sprite.rows()), (26, 11));
    assert_eq!(sprite.height(80, &Profile::NoColor.theme()), 11);
    for profile in Profile::ALL {
        let theme = profile.theme();
        for (width, height) in [(0, 0), (1, 1), (26, 0), (0, 11), (25, 11), (26, 10), (3, 2)] {
            let area = Rect::new(0, 0, width, height);
            assert!(!sprite.fits(area));
            let buf = testing::render(40, 20, |_, buf| sprite.paint(area, buf, &theme));
            assert_eq!(buf, Buffer::empty(Rect::new(0, 0, 40, 20)), "{area:?}");
        }
        // Room to spare centres it, at one column per cell.
        let buf = testing::render(40, 20, |area, buf| sprite.paint(area, buf, &theme));
        let exact = testing::render(26, 11, |area, buf| sprite.paint(area, buf, &theme));
        for y in 0..11 {
            for x in 0..26 {
                assert_eq!(buf[(x + 7, y + 4)], exact[(x, y)]);
            }
        }
    }
}

/// The request and buffer shapes of `tests/paint_bounds.rs`: a buffer away
/// from the origin, and areas partly or wholly outside it, empty, or one cell.
#[test]
fn painting_stays_inside_the_request_and_the_buffer() {
    let bounds = Rect::new(7, 5, 30, 12);
    let requests = [
        bounds,
        Rect::new(10, 5, 26, 11),
        Rect::new(17, 9, 40, 20),
        Rect::new(2, 1, 30, 12),
        Rect::new(0, 6, 26, 11),
        Rect::new(9, 0, 26, 11),
        Rect::new(0, 0, 100, 100),
        Rect::new(20, 8, u16::MAX, u16::MAX),
        Rect::new(0, 0, u16::MAX, u16::MAX),
        Rect::new(0, 5, 6, 12),
        Rect::new(37, 5, 30, 12),
        Rect::new(7, 17, 30, 12),
        Rect::new(u16::MAX - 9, u16::MAX - 9, 9, 9),
        Rect::new(u16::MAX, u16::MAX, 0, 0),
        Rect::new(9, 7, 0, 11),
        Rect::new(9, 7, 26, 0),
        Rect::new(7, 5, 1, 1),
        Rect::new(36, 16, 1, 1),
    ];
    let sprites = [
        GridSprite::whale("mark", "rest").unwrap(),
        companion("done"),
        GridSprite::whale("hero", "sleep").unwrap(),
        small("needs-you"),
        GridSprite::whale_girl("hero", "rest").unwrap(),
    ];
    let mut clipped = 0;
    for profile in Profile::ALL {
        let theme = profile.theme();
        for sprite in sprites {
            for request in requests {
                let before = Buffer::filled(bounds, Cell::new("·"));
                let mut after = before.clone();
                sprite.paint(request, &mut after, &theme);
                assert_eq!(after.area, bounds);
                let allowed = request.intersection(bounds);
                let mut changed = 0;
                for y in bounds.top()..bounds.bottom() {
                    for x in bounds.left()..bounds.right() {
                        if after[(x, y)] != before[(x, y)] {
                            changed += 1;
                            assert!(
                                allowed.contains((x, y).into()),
                                "{profile:?} {request:?} wrote ({x}, {y})"
                            );
                        }
                    }
                }
                if changed > 0 && !sprite.fits(allowed) {
                    clipped += 1;
                }
            }
        }
    }
    // Some requests placed a sprite across the buffer's edge, so the clip
    // itself ran rather than only the early return.
    assert!(clipped > 0);
}
