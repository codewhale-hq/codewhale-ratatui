//! Bounded RGBA avatar tiles painted with terminal half-blocks. This renderer
//! neither loads plugins nor owns a timer. Hosts supply reviewed pixels and
//! sample an action with the same Director used by the rest of their UI.
use crate::{
    Paint, Role, Theme,
    color::{self, ColorDepth},
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

/// Row-major RGBA pixels within each tile, tiles in manifest order.
/// Hosts decode PNG once and concatenate its tiles; terminal authors can also
/// ship predecoded tiles. Construction validates the complete byte geometry.
#[derive(Clone, Copy)]
pub struct Sprite<'a> {
    pixels: &'a [u8],
    width: usize,
    height: usize,
    frame: usize,
    transform: crate::avatar::Transform,
}
impl<'a> Sprite<'a> {
    pub fn new(
        pack: &crate::avatar::Pack,
        pixels: &'a [u8],
        width: usize,
        height: usize,
        frame: usize,
    ) -> Result<Self, String> {
        pack.validate()?;
        let count = usize::from(pack.columns) * usize::from(pack.rows) * pack.atlases.len();
        Self::tiles(pixels, width, height, frame, count)
    }
    pub fn page(
        pack: &crate::avatar::Pack,
        pixels: &'a [u8],
        width: usize,
        height: usize,
        frame: usize,
    ) -> Result<Self, String> {
        pack.validate()?;
        if pack.page(frame) >= pack.atlases.len() {
            return Err("avatar frame exceeds page count".into());
        }
        Self::tiles(
            pixels,
            width,
            height,
            pack.local_frame(frame),
            usize::from(pack.columns) * usize::from(pack.rows),
        )
    }
    fn tiles(
        pixels: &'a [u8],
        width: usize,
        height: usize,
        frame: usize,
        count: usize,
    ) -> Result<Self, String> {
        if width == 0
            || height == 0
            || width > 2048
            || height > 2048
            || frame >= count
            || width
                .checked_mul(height)
                .and_then(|n| n.checked_mul(4))
                .and_then(|n| n.checked_mul(count))
                != Some(pixels.len())
            || pixels.len() > 16 * 1024 * 1024
        {
            return Err("avatar pixels do not match bounded tile dimensions".into());
        }
        Ok(Self {
            pixels,
            width,
            height,
            frame,
            transform: Default::default(),
        })
    }
    pub fn with_transform(mut self, transform: crate::avatar::Transform) -> Self {
        if transform.scale_x.is_finite()
            && transform.scale_y.is_finite()
            && transform.lift.is_finite()
        {
            self.transform = crate::avatar::Transform {
                scale_x: transform.scale_x.clamp(0.90, 1.08),
                scale_y: transform.scale_y.clamp(0.90, 1.08),
                lift: transform.lift.clamp(-8., 8.),
            };
        }
        self
    }
    fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        // Inverse sample about the feet; sub-cell movement stays on the same
        // caller clock and does not resize the surrounding terminal layout.
        let t = self.transform;
        let x = (x as f64 - self.width as f64 / 2.) / t.scale_x + self.width as f64 / 2.;
        let y = (y as f64 - self.height as f64 - t.lift / 124. * self.height as f64) / t.scale_y
            + self.height as f64;
        if x < 0. || y < 0. || x >= self.width as f64 || y >= self.height as f64 {
            return [0; 4];
        }
        let (x, y) = (x as usize, y as usize);
        let offset = ((self.frame * self.height + y) * self.width + x) * 4;
        self.pixels[offset..offset + 4]
            .try_into()
            .expect("validated RGBA pixel")
    }
}
impl Paint for Sprite<'_> {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        // One cell is approximately twice as tall as wide. Fit without changing
        // the artwork's aspect ratio and never allocate from terminal geometry.
        let scale = (f64::from(area.width) / self.width as f64)
            .min(f64::from(area.height) * 2. / self.height as f64);
        let width = ((self.width as f64 * scale).floor() as u16)
            .max(1)
            .min(area.width);
        let rows = ((self.height as f64 * scale / 2.).ceil() as u16)
            .max(1)
            .min(area.height);
        let left = area.x + (area.width - width) / 2;
        let top = area.y + (area.height - rows) / 2;
        let colored = theme.caps().paints_tokens() && !theme.ascii();
        for y in 0..rows {
            for x in 0..width {
                let sx = (usize::from(x) * self.width / usize::from(width)).min(self.width - 1);
                let sy = |half: usize| {
                    ((usize::from(y) * 2 + half) * self.height / (usize::from(rows) * 2))
                        .min(self.height - 1)
                };
                let a = self.pixel(sx, sy(0));
                let b = self.pixel(sx, sy(1));
                if a[3] < 40 && b[3] < 40 {
                    continue;
                }
                let cell = &mut buf[(left + x, top + y)];
                if !colored {
                    // A legible one-ink density rendering for NO_COLOR, unknown
                    // ground and ASCII-safe profiles; no forced palette escapes.
                    let alpha = u16::from(a[3]) + u16::from(b[3]);
                    let shade = (u16::from(a[0])
                        + u16::from(a[1])
                        + u16::from(a[2])
                        + u16::from(b[0])
                        + u16::from(b[1])
                        + u16::from(b[2]))
                        / 6;
                    let symbol = if alpha < 160 {
                        "."
                    } else if shade > 220 {
                        ":"
                    } else if shade > 130 {
                        "+"
                    } else {
                        "#"
                    };
                    cell.set_symbol(symbol).set_style(theme.fg(Role::Primary));
                    continue;
                }
                let bg = color::resolvable_rgb(cell.bg);
                let blend = |p: [u8; 4]| {
                    let rgb = if let Some((r, g, b)) = bg {
                        let mix = |fg: u8, bg: u8| {
                            ((u32::from(fg) * u32::from(p[3])
                                + u32::from(bg) * (255 - u32::from(p[3])))
                                / 255) as u8
                        };
                        (mix(p[0], r), mix(p[1], g), mix(p[2], b))
                    } else {
                        (p[0], p[1], p[2])
                    };
                    if theme.caps().depth == ColorDepth::Ansi256 {
                        Color::Indexed(color::rgb_to_ansi256(rgb.0, rgb.1, rgb.2))
                    } else {
                        Color::Rgb(rgb.0, rgb.1, rgb.2)
                    }
                };
                let background = cell.bg;
                match (a[3] >= 40, b[3] >= 40) {
                    (true, true) => {
                        cell.set_symbol("▀")
                            .set_style(Style::default().fg(blend(a)).bg(blend(b)));
                    }
                    (true, false) => {
                        cell.set_symbol("▀")
                            .set_style(Style::default().fg(blend(a)).bg(background));
                    }
                    (false, true) => {
                        cell.set_symbol("▄")
                            .set_style(Style::default().fg(blend(b)).bg(background));
                    }
                    _ => {}
                }
            }
        }
    }
}

/// The original whale in the same avatar carrier. Uses the existing rig,
/// terminal ink/capability handling and caller-owned Director.
pub struct Contour<'a> {
    pub director: &'a crate::whale_motion::Director,
    pub view: Option<&'a str>,
}
impl Paint for Contour<'_> {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        use crate::whale_motion::{View, rasterize_colored, rig, scene};
        let area = area.intersection(buf.area);
        let cols = usize::from(area.width.min(64));
        let rows = usize::from(area.height.saturating_sub(1).min(32));
        let mut view = View::hero((cols * 2).min(rows * 4) as f64, 1.);
        view.dir = match self.view {
            Some("cruise") => rig::CRUISE_DIRECTION,
            Some("open") => rig::OPEN_DIRECTION,
            _ => rig::MARK_DIRECTION,
        };
        let parts = scene(self.director, view);
        rasterize_colored(
            &parts,
            cols,
            rows,
            0.5,
            theme.caps().appearance != crate::detect::Appearance::Light,
        )
        .paint(
            &crate::Whale::new(crate::WhaleState::Rest).words(""),
            area,
            buf,
            theme,
        );
    }
}
