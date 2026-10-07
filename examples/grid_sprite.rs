//! Every pose of both pixel characters, still: `cargo run --example grid_sprite`.
//!
//! Prints one sheet per theme with ANSI color and exits; nothing animates and
//! no key is read. `--text` prints the same sheets as plain glyphs from a
//! `TestBackend` and names a sheet whose glyphs repeat an earlier one instead
//! of printing it again. `--svg DIR` also writes each sheet as an SVG.
//! `--width N` sets the columns (default 120).
//!
//! Each sprite sits under its pose name. The name is the content; the sprite
//! is decoration and is never the only carrier of a state.

use std::{error::Error, path::PathBuf};

use codewhale_ratatui::{
    GridPose, GridSheet, GridSprite, Paint, Role, Theme, TuiPalette,
    testing::{self, Profile},
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};

const GAP: u16 = 2;

enum Item<'a> {
    Heading(String),
    Pose(&'a GridPose),
}

/// The sheet: a heading per body, then its poses flowed left to right.
struct Sheet<'a> {
    items: Vec<Item<'a>>,
}

impl<'a> Sheet<'a> {
    fn new() -> Self {
        let mut items = Vec::new();
        for (character, sheet) in [
            ("Whale", GridSheet::whale()),
            ("Whale girl", GridSheet::whale_girl()),
        ] {
            let mut body = "";
            for pose in sheet.poses() {
                if pose.body() != body {
                    body = pose.body();
                    items.push(Item::Heading(format!(
                        "{character} / {body}  {}x{} cells, {} columns by {} rows",
                        pose.width(),
                        pose.height(),
                        pose.width(),
                        pose.rows()
                    )));
                }
                items.push(Item::Pose(pose));
            }
        }
        Self { items }
    }

    /// Where each item goes at `width`, and the rows the sheet needs.
    fn place(&self, width: u16) -> (Vec<Rect>, u16) {
        let (mut x, mut y, mut tall) = (0u16, 0u16, 0u16);
        let mut placed = Vec::with_capacity(self.items.len());
        for item in &self.items {
            let (w, h) = match item {
                Item::Heading(_) => (width, 1),
                Item::Pose(pose) => (pose.width().max(pose.name().len() as u16), pose.rows() + 1),
            };
            if x > 0 && (x + w > width || matches!(item, Item::Heading(_))) {
                (x, y, tall) = (0, y + tall + 1, 0);
            }
            placed.push(Rect::new(x, y, w, h));
            if matches!(item, Item::Heading(_)) {
                y += 2;
            } else {
                (x, tall) = (x + w + GAP, tall.max(h));
            }
        }
        (placed, y + tall)
    }
}

impl Paint for Sheet<'_> {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        buf.set_style(area, theme.bg(Role::Background));
        let (placed, _) = self.place(area.width);
        for (item, at) in self.items.iter().zip(placed) {
            let at = Rect::new(area.x + at.x, area.y + at.y, at.width, at.height);
            match item {
                Item::Heading(words) => {
                    buf.set_stringn(
                        at.x,
                        at.y,
                        words,
                        at.width.into(),
                        theme.fg(Role::Foreground),
                    );
                }
                Item::Pose(pose) => {
                    buf.set_stringn(
                        at.x,
                        at.y,
                        pose.name(),
                        at.width.into(),
                        theme.fg(Role::Muted),
                    );
                    let art = Rect::new(at.x, at.y + 1, pose.width(), pose.rows());
                    GridSprite::new(pose).paint(art, buf, theme);
                }
            }
        }
    }
}

fn themes() -> Vec<(String, Theme)> {
    let native = |profile: Profile, palette: TuiPalette| {
        (
            format!("{palette:?} palette, {}", profile.name()),
            profile.theme().tui_palette(palette),
        )
    };
    let mut themes = vec![
        native(Profile::DarkTrue, TuiPalette::Underwater),
        native(Profile::LightTrue, TuiPalette::WhaleLight),
    ];
    themes.extend(
        Profile::ALL
            .into_iter()
            .map(|profile| (format!("tokens, {}", profile.name()), profile.theme())),
    );
    themes
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|at| args.get(at + 1))
    };
    let plain = args.iter().any(|a| a == "--text");
    let width = value("--width").and_then(|w| w.parse().ok()).unwrap_or(120);
    let svg = value("--svg").map(PathBuf::from);
    if let Some(dir) = &svg {
        std::fs::create_dir_all(dir)?;
    }

    let sheet = Sheet::new();
    let (_, height) = sheet.place(width);
    let mut seen: Vec<(String, String)> = Vec::new();
    for (name, theme) in themes() {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| frame.render_widget(sheet.themed(&theme), frame.area()))?;
        let buf = terminal.backend().buffer();
        if let Some(dir) = &svg {
            let file = name.replace([' ', ','], "-").to_lowercase();
            std::fs::write(dir.join(format!("{file}.svg")), testing::svg(buf, &theme))?;
        }
        println!("==== {name} ====");
        if !plain {
            println!("{}\n", testing::ansi(buf));
            continue;
        }
        let text = testing::text(buf);
        match seen.iter().find(|(_, earlier)| *earlier == text) {
            Some((first, _)) => {
                println!("(the same glyphs as \"{first}\"; only the colors differ)\n")
            }
            None => {
                println!("{text}\n");
                seen.push((name, text));
            }
        }
    }
    Ok(())
}
