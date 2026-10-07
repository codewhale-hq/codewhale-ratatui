//! Avatar authoring preview. --character names a built-in (whale, whale-girl,
//! pixel-whale, pixel-whale-girl). --pack avatar.json opens local artwork only;
//! it never installs, enables or executes a plugin. --pixel-cell N says that
//! pack is pixel art drawn with N page pixels per art cell, so it is sampled
//! on its own grid instead of being smoothed. --frames DIR exports actual
//! Ratatui buffers for every authored action/view and terminal profile.
use codewhale_ratatui::{
    Paint, Role, Theme,
    avatar::{Crop, Pack},
    avatar_builtin::{self, Builtin},
    avatar_sprite::Sprite,
    testing::{self, Profile},
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, buffer::Buffer, layout::Rect};
use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// A local pack decoded for the terminal.
struct Loaded {
    pack: Pack,
    pixels: Vec<u8>,
    w: usize,
    h: usize,
    /// Page pixels per art cell when the pack is pixel art.
    cell: Option<u32>,
}

/// What the carrier shows: the contour whale, a built-in sprite character or
/// a local pack.
#[derive(Clone, Copy)]
enum Character<'a> {
    Contour,
    Builtin(&'static Builtin),
    Loaded(&'a Loaded),
}
impl Character<'_> {
    fn pack(&self) -> Option<&Pack> {
        match self {
            Self::Contour => None,
            Self::Builtin(builtin) => Some(builtin.pack()),
            Self::Loaded(loaded) => Some(&loaded.pack),
        }
    }
    fn name(&self) -> &str {
        self.pack().map_or("Whale", |pack| &pack.name)
    }
    fn actions(&self) -> Vec<&str> {
        self.pack()
            .map_or(codewhale_ratatui::avatar::ACTS.to_vec(), |pack| {
                pack.actions.keys().map(String::as_str).collect()
            })
    }
    /// The whole character, or its compact crop when only that keeps a
    /// pixel-art pack cell for cell in `area`.
    fn sprite(&self, frame: usize, area: Rect) -> Option<Sprite<'_>> {
        let (full, compact) = match self {
            Self::Contour => return None,
            Self::Builtin(builtin) => (builtin.sprite(frame).ok()?, builtin.compact(frame)),
            Self::Loaded(loaded) => {
                let sprite =
                    Sprite::new(&loaded.pack, &loaded.pixels, loaded.w, loaded.h, frame).ok()?;
                match loaded.cell {
                    None => (sprite, None),
                    Some(cell) => (
                        sprite.pixel_art(),
                        loaded.pack.compact.map(|c| {
                            let cell = cell as u16;
                            sprite
                                .crop(Crop {
                                    x: c.x / cell,
                                    y: c.y / cell,
                                    width: c.width / cell,
                                    height: c.height / cell,
                                })
                                .pixel_art()
                        }),
                    ),
                }
            }
        };
        Some(match compact {
            Some(compact) if compact.reduction(area) < full.reduction(area) => compact,
            _ => full,
        })
    }
}

fn load(path: &Path, cell: Option<u32>) -> Result<Loaded, Box<dyn std::error::Error>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(codewhale_ratatui::avatar::MAX_PACK_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let pack = Pack::parse(&bytes).map_err(io::Error::other)?;
    let mut tiles = Vec::new();
    let (w, h) = (u32::from(pack.tile_width), u32::from(pack.tile_height));
    // Painted art is smoothed down to a 96px terminal tile. Pixel art keeps
    // its own grid: one terminal pixel per art cell, read at the cell centre.
    let (tw, th) = match cell {
        Some(cell) if cell == 0 || w % cell != 0 || h % cell != 0 => {
            return Err(format!("a {w}x{h} tile is not a whole number of {cell}px cells").into());
        }
        Some(cell) => (w / cell, h / cell),
        None => {
            let scale = 96. / f64::from(w.max(h));
            (
                (f64::from(w) * scale).round().max(1.) as u32,
                (f64::from(h) * scale).round().max(1.) as u32,
            )
        }
    };
    for atlas in &pack.atlases {
        bytes.clear();
        std::fs::File::open(path.parent().unwrap_or(Path::new(".")).join(atlas))?
            .take(codewhale_ratatui::avatar::MAX_PNG_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        pack.validate_png(&bytes).map_err(io::Error::other)?;
        let png = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?.to_rgba8();
        for i in 0..u32::from(pack.columns) * u32::from(pack.rows) {
            let tile = image::imageops::crop_imm(
                &png,
                i % u32::from(pack.columns) * w,
                i / u32::from(pack.columns) * h,
                w,
                h,
            )
            .to_image();
            if let Some(cell) = cell {
                for y in 0..th {
                    for x in 0..tw {
                        tiles.extend(tile.get_pixel(x * cell + cell / 2, y * cell + cell / 2).0);
                    }
                }
            } else {
                tiles.extend(
                    image::imageops::resize(&tile, tw, th, image::imageops::FilterType::Lanczos3)
                        .into_raw(),
                );
            }
        }
    }
    Ok(Loaded {
        pack,
        pixels: tiles,
        w: tw as usize,
        h: th as usize,
        cell,
    })
}
struct Screen<'a> {
    character: Character<'a>,
    action: &'a str,
    frame: f64,
    reduced: bool,
    director: &'a codewhale_ratatui::whale_motion::Director,
    view: Option<&'a str>,
}
impl Paint for Screen<'_> {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let area = area.intersection(buf.area);
        buf.set_style(area, theme.fg(Role::Primary));
        if area.height < 4 {
            return;
        }
        let sampled = self
            .character
            .pack()
            .map(|pack| pack.sample("rest", self.frame, self.reduced, None, Some(self.action)));
        let art = Rect::new(
            area.x,
            area.y.saturating_add(2),
            area.width,
            area.height.saturating_sub(4),
        );
        if let Some(sampled) = sampled {
            if let Some(sprite) = self.character.sprite(sampled.index, art) {
                sprite
                    .with_transform(sampled.transform)
                    .paint(art, buf, theme);
            }
        } else {
            codewhale_ratatui::avatar_sprite::Contour {
                director: self.director,
                view: self.view,
            }
            .paint(art, buf, theme);
        }
        buf.set_stringn(
            area.x,
            area.y,
            format!(
                "{} / {}{}",
                self.character.name(),
                self.action,
                if self.reduced { " / still" } else { "" }
            ),
            usize::from(area.width),
            theme.fg(Role::Primary),
        );
        buf.set_stringn(
            area.x,
            area.bottom().saturating_sub(1),
            "Tab character  Left/Right action  V view  Space motion  Esc close",
            usize::from(area.width),
            theme.fg(Role::Muted),
        );
    }
}
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut file = None;
    let mut frames = None;
    let mut named = None;
    let mut cell = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--character" => named = Some(args.next().ok_or("--character needs a name")?),
            "--pack" => file = Some(PathBuf::from(args.next().ok_or("--pack needs a file")?)),
            "--pixel-cell" => {
                cell = Some(
                    args.next()
                        .and_then(|n| n.parse::<u32>().ok())
                        .ok_or("--pixel-cell needs a number of page pixels")?,
                )
            }
            "--frames" => {
                frames = Some(PathBuf::from(
                    args.next().ok_or("--frames needs a directory")?,
                ))
            }
            _ => {
                return Err(
                    "usage: avatar [--character whale|whale-girl|pixel-whale|pixel-whale-girl] \
                     [--pack avatar.json [--pixel-cell N]] [--frames DIR]"
                        .into(),
                );
            }
        }
    }
    if cell.is_some() && file.is_none() {
        return Err("--pixel-cell describes a --pack".into());
    }
    // One carrier for every character: the contour whale, then a local pack
    // when one is named, otherwise every built-in sprite character.
    let loaded = file.map(|file| load(&file, cell)).transpose()?;
    let mut characters = vec![Character::Contour];
    match &loaded {
        Some(loaded) => characters.push(Character::Loaded(loaded)),
        None => characters.extend(avatar_builtin::all().iter().map(Character::Builtin)),
    }
    let mut chosen = match named.as_deref() {
        // A local pack or, with no flag, the first sprite character.
        None => 1,
        Some("whale") => 0,
        Some(name) => characters
            .iter()
            .position(|c| matches!(c, Character::Builtin(b) if b.id() == name))
            .ok_or_else(|| {
                let names: Vec<_> = avatar_builtin::all().iter().map(Builtin::id).collect();
                format!("--character expects whale, {}", names.join(", "))
            })?,
    };
    let mut actions = characters[chosen].actions();
    if let Some(dir) = frames {
        std::fs::create_dir_all(&dir)?;
        for profile in [
            Profile::DarkTrue,
            Profile::LightTrue,
            Profile::NoColor,
            Profile::Ascii,
        ] {
            let theme = profile.theme();
            for action in &actions {
                for (step, frame) in [0., 18., 42.].iter().enumerate() {
                    let mut director = codewhale_ratatui::whale_motion::Director::for_preview(
                        codewhale_ratatui::whale_motion::Act::from_id(action)
                            .unwrap_or(codewhale_ratatui::whale_motion::Act::Rest),
                        step == 0,
                    );
                    for _ in 0..(*frame as usize) {
                        director.step(1. / 30.);
                    }
                    let screen = Screen {
                        character: characters[chosen],
                        action,
                        frame: *frame,
                        reduced: step == 0,
                        director: &director,
                        view: None,
                    };
                    let buf = testing::render(64, 36, |area, buf| screen.paint(area, buf, &theme));
                    std::fs::write(
                        dir.join(format!("{}-{action}-{step}.svg", profile.name())),
                        testing::svg(&buf, &theme),
                    )?;
                }
            }
        }
        println!("Exported avatar actions and views to {}", dir.display());
        return Ok(());
    }
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let _guard = TerminalGuard;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let theme = Theme::detect();
    let mut started = Instant::now();
    let mut last = started;
    let mut original_view = 0;
    let mut director = codewhale_ratatui::whale_motion::Director::for_preview(
        codewhale_ratatui::whale_motion::Act::Rest,
        false,
    );
    let mut index = 0;
    let mut reduced = false;
    loop {
        let now = Instant::now();
        let mut dt = now.duration_since(last).as_secs_f64().min(1.);
        last = now;
        while dt > 1e-9 {
            let step = dt.min(1. / 30.);
            director.step(step);
            dt -= step;
        }
        let screen = Screen {
            character: characters[chosen],
            action: actions[index],
            frame: started.elapsed().as_secs_f64() * 30.,
            reduced,
            director: &director,
            view: Some(codewhale_ratatui::avatar::WHALE_VIEWS[original_view]),
        };
        terminal.draw(|f| screen.paint(f.area(), f.buffer_mut(), &theme))?;
        if event::poll(if reduced {
            Duration::from_secs(60)
        } else {
            Duration::from_millis(100)
        })? && let Event::Key(key) = event::read()?
        {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => break,
                KeyCode::Left => index = (index + actions.len() - 1) % actions.len(),
                KeyCode::Right => index = (index + 1) % actions.len(),
                KeyCode::Char(' ') => reduced = !reduced,
                KeyCode::Tab => {
                    chosen = (chosen + 1) % characters.len();
                    index = 0;
                    actions = characters[chosen].actions();
                }
                KeyCode::Char('v') if matches!(characters[chosen], Character::Contour) => {
                    original_view = (original_view + 1) % 3;
                }
                KeyCode::Char('v') => {
                    let views: Vec<_> = characters[chosen]
                        .pack()
                        .map(|pack| pack.views.values().map(String::as_str).collect())
                        .unwrap_or_default();
                    let current = views
                        .iter()
                        .position(|v| *v == actions[index])
                        .map_or(0, |i| (i + 1) % views.len().max(1));
                    if let Some(action) = views.get(current) {
                        index = actions.iter().position(|a| a == action).unwrap_or(index);
                    }
                }
                _ => {}
            }
            started = Instant::now();
            director = codewhale_ratatui::whale_motion::Director::for_preview(
                codewhale_ratatui::whale_motion::Act::from_id(actions[index])
                    .unwrap_or(codewhale_ratatui::whale_motion::Act::Rest),
                reduced,
            );
        }
    }
    Ok(())
}
