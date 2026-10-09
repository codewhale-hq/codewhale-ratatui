//! The atmosphere finishing pass: `src/ombre.rs` and its gallery entries.
//!
//! The checks here are properties, not constants: a ground keeps the
//! luminance the tokens audited it with, every cell keeps its glyph and its
//! ink, an ink is never asked to survive less than its floor, the fallback
//! profiles keep the audited theme byte for byte, and the wash stays anchored
//! to the requested area through clipping and `u16::MAX`.

use codewhale_ratatui::{
    Ground, Ombre, OmbreDirection, Paint, Role, Theme, WaterPalette,
    color::{contrast_ratio, relative_luminance},
    gallery,
    testing::{Profile, assert_frames_keep_the_rules, frames_for, render, text},
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

/// The structural grounds the pass is allowed to repaint.
const GROUNDS: [Role; 5] = [
    Role::Sidebar,
    Role::Background,
    Role::Surface,
    Role::Hover,
    Role::Selected,
];

/// The darks that carry a wash, and the audit floors the theme holds them to.
fn audited_inks() -> Vec<(Role, f32)> {
    vec![
        (Role::Foreground, 4.5),
        (Role::Muted, 4.5),
        (Role::Primary, 4.5),
        (Role::Live, 4.5),
        (Role::Attention, 4.5),
        (Role::Danger, 4.5),
        (Role::Hint, 4.5),
        (Role::Dim, 3.0),
        (Role::BorderStrong, 3.0),
    ]
}

/// Rows each ground band gets at `height`.
fn band_rows(height: u16) -> u16 {
    (height / u16::try_from(GROUNDS.len()).unwrap_or(5)).max(1)
}

/// The first row of `index`'s band.
fn band_top(area: Rect, index: usize) -> u16 {
    area.y.saturating_add(
        u16::try_from(index)
            .unwrap_or(0)
            .saturating_mul(band_rows(area.height)),
    )
}

/// A scene of five horizontal bands, one structural ground each, so every
/// ramp sample of every ground is on screen.
fn grounds(width: u16, height: u16, theme: &Theme) -> Buffer {
    render(width, height, |area, buf| {
        for (index, role) in GROUNDS.iter().enumerate() {
            let top = band_top(area, index);
            let band = Rect {
                x: area.x,
                y: top,
                width: area.width,
                height: band_rows(area.height).min(area.height.saturating_sub(top - area.y)),
            };
            buf.set_style(band, theme.bg(*role));
        }
    })
}

/// Every cell's glyph and style, for before/after comparisons.
fn snapshot(buf: &Buffer) -> Vec<(String, Option<Color>, Option<Color>, Modifier)> {
    buf.content()
        .iter()
        .map(|cell| {
            let style = cell.style();
            (
                cell.symbol().to_string(),
                style.fg,
                style.bg,
                style.add_modifier,
            )
        })
        .collect()
}

/// Each ground band's current luminance, in [`GROUNDS`] order.
fn ground_luminance(buf: &Buffer, area: Rect) -> Vec<f32> {
    (0..GROUNDS.len())
        .map(|index| {
            relative_luminance(buf[(area.left(), band_top(area, index))].bg).unwrap_or(0.0)
        })
        .collect()
}

/// The order of the grounds by luminance, darkest first.
fn elevation(buf: &Buffer, area: Rect) -> Vec<usize> {
    let mut indexed: Vec<(usize, f32)> = ground_luminance(buf, area)
        .into_iter()
        .enumerate()
        .collect();
    indexed.sort_by(|a, b| a.1.total_cmp(&b.1));
    indexed.into_iter().map(|(index, _)| index).collect()
}

/// A palette wash never moves a ground's audited luminance, never drops a
/// token ink below its floor, and keeps the elevation order of the grounds.
#[test]
fn every_palette_keeps_luminance_floors_and_elevation() {
    for profile in [Profile::DarkTrue, Profile::DarkGraphite, Profile::LightTrue] {
        let theme = profile.theme();
        let before = grounds(120, 10, &theme);
        let area = *before.area();
        let wanted = elevation(&before, area);
        for palette in WaterPalette::ALL {
            let mut buf = before.clone();
            Ombre::new(palette).apply(area, &mut buf, &theme);
            for y in area.top()..area.bottom() {
                for x in area.left()..area.right() {
                    let (old, new) = (before[(x, y)].bg, buf[(x, y)].bg);
                    let (Some(old_lum), Some(new_lum)) =
                        (relative_luminance(old), relative_luminance(new))
                    else {
                        continue;
                    };
                    assert!(
                        (new_lum - old_lum).abs() <= 0.005 + f32::EPSILON,
                        "{} {palette:?} at ({x},{y}): {old:?} -> {new:?} moved luminance",
                        profile.name()
                    );
                    for (ink, floor) in audited_inks() {
                        let ink = theme.color(ink).expect("truecolor token ink");
                        let ratio = contrast_ratio(ink, new).expect("resolvable");
                        assert!(
                            ratio >= floor - 1e-4,
                            "{} {palette:?} at ({x},{y}): {ink:?} on {new:?} is {ratio:.3}",
                            profile.name()
                        );
                    }
                }
            }
            assert_eq!(
                elevation(&buf, area),
                wanted,
                "{} {palette:?} re-ordered the grounds",
                profile.name()
            );
        }
    }
}

/// Six profiles keep the audited theme exactly: 256 colors, 16 colors, an
/// unmeasured ground, `NO_COLOR` and ASCII-safe output.
#[test]
fn fallback_profiles_keep_the_audited_theme() {
    for profile in [
        Profile::Dark256,
        Profile::Light256,
        Profile::Ansi16,
        Profile::UnknownGround,
        Profile::NoColor,
        Profile::Ascii,
    ] {
        let theme = profile.theme();
        let before = grounds(40, 10, &theme);
        for palette in WaterPalette::ALL {
            let mut buf = before.clone();
            let area = *buf.area();
            Ombre::new(palette).apply(area, &mut buf, &theme);
            assert_eq!(
                snapshot(&buf),
                snapshot(&before),
                "{} {palette:?}",
                profile.name()
            );
        }
    }
}

/// Glyphs, ink, modifiers, content tints, action fills, custom colors and
/// unpainted cells all survive; only the five structural grounds move.
#[test]
fn inks_tints_fills_and_glyphs_survive_the_pass() {
    let theme = Profile::DarkTrue.theme();
    let mut buf = render(60, 8, |area, buf| {
        buf.set_style(area, theme.bg(Role::Background));
        buf.set_style(Rect::new(0, 0, 20, 4), theme.bg(Role::Surface));
        buf.set_style(Rect::new(20, 0, 10, 4), theme.bg(Role::Selected));
        buf.set_style(Rect::new(30, 0, 10, 4), theme.bg(Role::DiffAddedTint));
        buf.set_style(Rect::new(40, 0, 10, 4), theme.bg(Role::DiffRemovedTint));
        buf.set_style(Rect::new(50, 0, 10, 4), theme.bg(Role::Primary));
        buf.set_style(
            Rect::new(20, 6, 40, 2),
            Style::default().bg(Color::Rgb(7, 8, 9)),
        );
        buf.set_string(0, 0, "Codewhale", theme.fg(Role::Live));
        buf.set_string(
            0,
            1,
            "custom",
            Style::default().fg(Color::Rgb(200, 210, 220)),
        );
        buf[(0, 2)].set_symbol("raw");
        buf[(1, 3)]
            .set_symbol("x")
            .set_style(theme.fg(Role::Primary).add_modifier(Modifier::REVERSED));
    });
    let before = snapshot(&buf);
    let area = *buf.area();
    Ombre::new(WaterPalette::Ocean).apply(area, &mut buf, &theme);
    let after = snapshot(&buf);
    let mut repainted = 0;
    for (index, (old, new)) in before.iter().zip(&after).enumerate() {
        let at = format!("cell {index}");
        assert_eq!(old.0, new.0, "{at}: glyph changed");
        assert_eq!(old.1, new.1, "{at}: ink changed");
        assert_eq!(old.3, new.3, "{at}: modifiers changed");
        if old.2 != new.2 {
            repainted += 1;
            let owned = GROUNDS.iter().any(|role| theme.color(*role) == old.2);
            assert!(owned, "{at}: repainted a background the pass does not own");
        }
    }
    assert!(
        repainted > 0,
        "the finishing pass must move a structural ground"
    );
    // The diff tints, the action fill and the custom ground are untouched.
    for x in [35_u16, 45, 55] {
        assert_eq!(Some(buf[(x, 0)].bg), before[usize::from(x)].2, "x={x}");
    }
    assert_eq!(buf[(25, 6)].bg, Color::Rgb(7, 8, 9));
    assert_eq!(
        snapshot(&buf)[181],
        before[181],
        "reversed selection changed"
    );
}

/// The wash is spatial: a diagonal moves both ways, a vertical does not move
/// across a row, palettes differ, and clipping never re-scales the gradient.
#[test]
fn the_wash_is_spatial_and_anchored_to_the_requested_area() {
    let theme = Profile::DarkTrue.theme();
    let cell = |buf: &Buffer, x: u16, y: u16| buf[(x, y)].bg;
    let area = Rect::new(0, 0, 40, 10);
    let mut diagonal = grounds(40, 10, &theme);
    Ombre::new(WaterPalette::Ocean).apply(area, &mut diagonal, &theme);
    assert_ne!(cell(&diagonal, 0, 0), cell(&diagonal, 39, 0));
    assert_ne!(cell(&diagonal, 0, 0), cell(&diagonal, 0, 1));

    let mut vertical = grounds(40, 10, &theme);
    Ombre::new(WaterPalette::Ocean)
        .direction(OmbreDirection::Vertical)
        .apply(area, &mut vertical, &theme);
    assert_eq!(cell(&vertical, 0, 3), cell(&vertical, 39, 3));
    assert_ne!(cell(&vertical, 0, 0), cell(&vertical, 0, 1));
    assert_eq!(cell(&vertical, 0, 0), cell(&diagonal, 0, 0));
    assert_ne!(cell(&vertical, 20, 0), cell(&diagonal, 20, 0));

    let mut coral = grounds(40, 10, &theme);
    Ombre::new(WaterPalette::Coral).apply(area, &mut coral, &theme);
    assert_ne!(cell(&diagonal, 10, 2), cell(&coral, 10, 2));

    let mut wide = grounds(40, 10, &theme);
    Ombre::new(WaterPalette::Ocean).apply(Rect::new(0, 0, 400, 10), &mut wide, &theme);
    assert_ne!(
        cell(&diagonal, 39, 0),
        cell(&wide, 39, 0),
        "clipping re-scaled the ramp"
    );
}

/// Blank cells carry the gradient without gaps; ink the terminal owns keeps
/// its cell exactly.
#[test]
fn blank_cells_stay_continuous_and_unmeasurable_ink_keeps_its_cell() {
    let theme = Profile::DarkTrue.theme();
    let before = grounds(30, 10, &theme);
    let mut buf = before.clone();
    let area = *buf.area();
    Ombre::new(WaterPalette::Ocean).apply(area, &mut buf, &theme);
    let moved = buf
        .content()
        .iter()
        .zip(before.content())
        .filter(|(new, old)| new.bg != old.bg)
        .count();
    let covered = before
        .content()
        .iter()
        .filter(|cell| {
            GROUNDS
                .iter()
                .any(|role| theme.color(*role) == Some(cell.bg))
        })
        .count();
    assert_eq!(moved, covered, "a blank ground cell kept the old wash");

    let mut inked = grounds(30, 10, &theme);
    let kept = inked[(1, 0)].bg;
    let neighbour = inked[(2, 0)].bg;
    inked[(1, 0)].set_symbol("x");
    Ombre::new(WaterPalette::Ocean).apply(area, &mut inked, &theme);
    assert_eq!(inked[(1, 0)].bg, kept, "Reset ink must keep its cell");
    assert_ne!(
        inked[(2, 0)].bg,
        neighbour,
        "the blank beside it still moves"
    );
}

/// Clipped, offset, zero-sized, outside and `u16::MAX` areas are all safe,
/// and only the visible intersection is touched.
#[test]
fn clipping_offsets_and_u16_max_are_safe() {
    let theme = Profile::DarkTrue.theme();
    let before = grounds(40, 10, &theme);
    let mut clipped = before.clone();
    Ombre::new(WaterPalette::Ocean).apply(Rect::new(5, 3, 20, 4), &mut clipped, &theme);
    for y in 0..10 {
        for x in 0..40 {
            let inside = (5..25).contains(&x) && (3..7).contains(&y);
            assert!(
                inside || clipped[(x, y)].bg == before[(x, y)].bg,
                "({x},{y}) was painted outside the requested area"
            );
        }
    }
    assert_ne!(clipped[(6, 4)].bg, before[(6, 4)].bg);
    assert_eq!(clipped[(25, 4)].bg, before[(25, 4)].bg);

    for area in [
        Rect::new(3, 3, 0, 4),
        Rect::new(3, 3, 4, 0),
        Rect::new(100, 100, 20, 20),
        Rect::new(0, 0, 0, 0),
    ] {
        let mut buf = before.clone();
        Ombre::new(WaterPalette::Ocean).apply(area, &mut buf, &theme);
        assert_eq!(
            snapshot(&buf),
            snapshot(&before),
            "{area:?} painted something"
        );
    }

    let mut huge = before.clone();
    Ombre::new(WaterPalette::Ocean).apply(Rect::new(0, 0, u16::MAX, u16::MAX), &mut huge, &theme);
    for y in 0..10 {
        for x in 0..40 {
            let (old, new) = (before[(x, y)].bg, huge[(x, y)].bg);
            let (Some(old_lum), Some(new_lum)) = (relative_luminance(old), relative_luminance(new))
            else {
                continue;
            };
            assert!(
                (new_lum - old_lum).abs() <= 0.005 + f32::EPSILON,
                "({x},{y})"
            );
        }
    }

    // A buffer that starts somewhere else: the pass follows buffer
    // coordinates, not screen ones.
    let area = Rect::new(10, 4, 30, 10);
    let mut offset = Buffer::empty(area);
    offset.set_style(area, theme.bg(Role::Sidebar));
    let before = snapshot(&offset);
    Ombre::new(WaterPalette::Ocean).apply(Rect::new(12, 6, 20, 3), &mut offset, &theme);
    for (index, (old, new)) in before.iter().zip(snapshot(&offset).iter()).enumerate() {
        let x = area.x + u16::try_from(index).unwrap_or(0) % area.width;
        let y = area.y + u16::try_from(index).unwrap_or(0) / area.width;
        let inside = (12..32).contains(&x) && (6..9).contains(&y);
        assert!(
            inside ^ (old.2 == new.2),
            "({x},{y}): inside={inside} {} -> {}",
            old.2.map_or("none".to_string(), |c| format!("{c:?}")),
            new.2.map_or("none".to_string(), |c| format!("{c:?}"))
        );
    }
}

/// Graphite shows the token grounds: dark truecolor remaps the five
/// structural grounds, paper and an already-graphite theme are untouched.
#[test]
fn graphite_maps_grounds_without_touching_anything_else() {
    let ocean = Profile::DarkTrue.theme();
    let graphite = ocean.ground(Ground::Graphite);
    let before = grounds(40, 10, &ocean);
    let mut buf = before.clone();
    let area = *buf.area();
    Ombre::new(WaterPalette::Graphite).apply(area, &mut buf, &ocean);
    for (index, role) in GROUNDS.iter().enumerate() {
        let x = area.left();
        let y = band_top(area, index);
        assert_eq!(
            buf[(x, y)].bg,
            graphite.color(*role).expect("graphite token"),
            "{role:?}"
        );
        assert_ne!(
            before[(x, y)].bg,
            buf[(x, y)].bg,
            "{role:?} must stop being ocean navy"
        );
    }

    let mut already = before.clone();
    Ombre::new(WaterPalette::Graphite).apply(area, &mut already, &graphite);
    assert_eq!(snapshot(&already), snapshot(&before), "already graphite");

    let light = Profile::LightTrue.theme();
    let mut paper = grounds(40, 10, &light);
    let paper_before = snapshot(&paper);
    Ombre::new(WaterPalette::Graphite).apply(*paper.area(), &mut paper, &light);
    assert_eq!(snapshot(&paper), paper_before, "paper stays native");
}

/// The five gallery entries label every palette, render at their own width
/// and at 40 columns, and keep the frame rules in every profile.
#[test]
fn the_gallery_shows_every_palette_and_keeps_the_rules() {
    let entries: Vec<gallery::Entry> = gallery::entries()
        .into_iter()
        .filter(|entry| entry.name.starts_with("atmosphere-"))
        .collect();
    assert_eq!(entries.len(), WaterPalette::ALL.len());
    for entry in &entries {
        let shown = text(&gallery::render(entry, &Profile::DarkTrue.theme()));
        for palette in WaterPalette::ALL {
            assert!(
                shown.contains(palette.name()),
                "{} is missing the {} label:\n{shown}",
                entry.name,
                palette.name()
            );
        }
        let frames = frames_for(
            entry.name,
            &Profile::ALL,
            &[entry.width, 40],
            entry.height,
            entry.draw,
        );
        assert_frames_keep_the_rules(&frames);
    }
}

/// The pass is idempotent, and `Paint::paint` paints what `apply` paints.
#[test]
fn the_pass_is_idempotent_and_paints_as_a_widget() {
    let theme = Profile::DarkTrue.theme();
    let mut once = grounds(40, 10, &theme);
    let area = *once.area();
    Ombre::new(WaterPalette::Lagoon).apply(area, &mut once, &theme);
    let mut twice = once.clone();
    Ombre::new(WaterPalette::Lagoon).apply(area, &mut twice, &theme);
    assert_eq!(
        snapshot(&twice),
        snapshot(&once),
        "a second pass must settle"
    );

    let mut widget = grounds(40, 10, &theme);
    Ombre::new(WaterPalette::Dusk).paint(area, &mut widget, &theme);
    let mut applied = grounds(40, 10, &theme);
    Ombre::new(WaterPalette::Dusk).apply(area, &mut applied, &theme);
    assert_eq!(snapshot(&widget), snapshot(&applied));
}
