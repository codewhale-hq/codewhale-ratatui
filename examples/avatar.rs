//! Avatar authoring preview. --pack avatar.json opens local artwork only; it
//! never installs, enables or executes a plugin. --frames DIR exports actual
//! Ratatui buffers for every authored action/view and terminal profile.
use codewhale_ratatui::{
    Paint, Role, Theme,
    avatar::Pack,
    avatar_sprite::Sprite,
    testing::{self, Profile},
    whale_girl,
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

fn load(path: &Path) -> Result<(Pack, Vec<u8>, usize, usize), Box<dyn std::error::Error>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(codewhale_ratatui::avatar::MAX_PACK_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let pack = Pack::parse(&bytes).map_err(io::Error::other)?;
    let mut tiles = Vec::new();
    let (w, h) = (u32::from(pack.tile_width), u32::from(pack.tile_height));
    let scale = 96. / f64::from(w.max(h));
    let (tw, th) = (
        (f64::from(w) * scale).round().max(1.) as u32,
        (f64::from(h) * scale).round().max(1.) as u32,
    );
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
            tiles.extend(
                image::imageops::resize(&tile, tw, th, image::imageops::FilterType::Lanczos3)
                    .into_raw(),
            );
        }
    }
    Ok((pack, tiles, tw as usize, th as usize))
}
struct Screen<'a> {
    pack: &'a Pack,
    pixels: &'a [u8],
    w: usize,
    h: usize,
    action: &'a str,
    frame: f64,
    reduced: bool,
}
impl Paint for Screen<'_> {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let area = area.intersection(buf.area);
        buf.set_style(area, theme.fg(Role::Primary));
        if area.height < 4 {
            return;
        }
        let sampled = self
            .pack
            .sample("rest", self.frame, self.reduced, None, Some(self.action));
        let art = Rect::new(
            area.x,
            area.y.saturating_add(2),
            area.width,
            area.height.saturating_sub(4),
        );
        if let Ok(sprite) = Sprite::new(self.pack, self.pixels, self.w, self.h, sampled.index) {
            sprite.paint(art, buf, theme);
        }
        buf.set_stringn(
            area.x,
            area.y,
            format!(
                "{} / {}{}",
                self.pack.name,
                self.action,
                if self.reduced { " / still" } else { "" }
            ),
            usize::from(area.width),
            theme.fg(Role::Primary),
        );
        buf.set_stringn(
            area.x,
            area.bottom().saturating_sub(1),
            "Left/Right action  V view  Space motion  Esc close",
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
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pack" => file = Some(PathBuf::from(args.next().ok_or("--pack needs a file")?)),
            "--frames" => {
                frames = Some(PathBuf::from(
                    args.next().ok_or("--frames needs a directory")?,
                ))
            }
            _ => return Err("usage: avatar [--pack avatar.json] [--frames DIR]".into()),
        }
    }
    let (pack, pixels, w, h) = if let Some(file) = file {
        load(&file)?
    } else {
        (
            whale_girl::pack().clone(),
            whale_girl::TERMINAL.to_vec(),
            96,
            96,
        )
    };
    let actions: Vec<_> = pack.actions.keys().map(String::as_str).collect();
    let views: Vec<_> = pack.views.values().map(String::as_str).collect();
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
                    let screen = Screen {
                        pack: &pack,
                        pixels: &pixels,
                        w,
                        h,
                        action,
                        frame: *frame,
                        reduced: step == 0,
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
    let started = Instant::now();
    let mut index = 0;
    let mut reduced = false;
    loop {
        let screen = Screen {
            pack: &pack,
            pixels: &pixels,
            w,
            h,
            action: actions[index],
            frame: started.elapsed().as_secs_f64() * 30.,
            reduced,
        };
        terminal.draw(|f| screen.paint(f.area(), f.buffer_mut(), &theme))?;
        if event::poll(if reduced {
            Duration::from_secs(60)
        } else {
            Duration::from_millis(100)
        })? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => break,
                    KeyCode::Left => index = (index + actions.len() - 1) % actions.len(),
                    KeyCode::Right => index = (index + 1) % actions.len(),
                    KeyCode::Char(' ') => reduced = !reduced,
                    KeyCode::Char('v') => {
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
            }
        }
    }
    Ok(())
}
