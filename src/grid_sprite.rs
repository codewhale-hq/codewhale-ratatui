//! Codewhale's pixel sprites: hand-written character grids painted with
//! terminal half-blocks, one column per cell and two cells per row.
//!
//! The grids in `assets/sprites/` are byte copies of the design source. One
//! line-oriented grammar describes both characters (it is documented at the top
//! of `whale.grid`): `ink` lines map a character to a theme token and a
//! `fill`/`open` level, `sprite` blocks are rows of ink keys, and a `pose`
//! composes sprites onto a fixed `stage` at named `slot`s.
//!
//! Nothing here animates and nothing owns a clock. A layer written as frames
//! (`bubbles.0`, `bubbles.1`, ...) paints its still, which is the last frame.
//!
//! A sprite is decoration beside real text. It writes no words, so the heading
//! or status text next to it must carry the meaning, and it must never be the
//! only carrier of state. A host with a plain or screen-reader mode leaves the
//! sprite out; the text beside it is complete without it.
//!
//! The half-block rules are those of [`crate::avatar_sprite::Sprite`]: token
//! colors only on a measured ground at 256 colors or better, exact RGB at
//! truecolor and the xterm cube at 256, the cell's own ground kept behind a
//! half cell, and one `Primary` ink everywhere else. Unlike that widget, a grid
//! is never resampled: a pose that does not fit its area is not painted.
use std::sync::OnceLock;

use crate::{
    Paint, Role, Theme,
    color::{self, ColorDepth},
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
};

/// The whale: mark, favicon cut, companion and hero.
pub const WHALE_GRID: &str = include_str!("../assets/sprites/whale.grid");
/// The whale girl: small and hero, eight poses each.
pub const WHALE_GIRL_GRID: &str = include_str!("../assets/sprites/whale-girl.grid");

/// The widest and tallest stage or sprite a grid may declare, in cells.
const MAX_SIDE: u16 = 256;

/// Where an ink sits between the theme's lightest and darkest ink, or which
/// role it reads directly.
#[derive(Clone, Copy)]
enum Tone {
    Role(Role),
    /// The lightest of the theme's body ink and grounds.
    Lightest,
    /// The darkest of the theme's body ink and grounds.
    Darkest,
}

/// A theme token an `ink` line may name. Every token resolves from the
/// [`Theme`] at paint time: a role where one matches, otherwise a fixed mix of
/// two roles. No sprite color is stored anywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GridToken {
    Whale,
    WhaleLite,
    WhaleDeep,
    WhalePleat,
    Cream,
    Line,
    Text,
    Panel,
    Warn,
    GirlHair,
    GirlPale,
    GirlSkin,
    GirlBlush,
    GirlCloth,
    GirlGold,
}

impl GridToken {
    pub const COUNT: usize = 15;
    pub const ALL: [GridToken; Self::COUNT] = [
        Self::Whale,
        Self::WhaleLite,
        Self::WhaleDeep,
        Self::WhalePleat,
        Self::Cream,
        Self::Line,
        Self::Text,
        Self::Panel,
        Self::Warn,
        Self::GirlHair,
        Self::GirlPale,
        Self::GirlSkin,
        Self::GirlBlush,
        Self::GirlCloth,
        Self::GirlGold,
    ];

    /// The name an `ink` line uses, as in the design's `theme.tokens`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Whale => "whale",
            Self::WhaleLite => "whale-lite",
            Self::WhaleDeep => "whale-deep",
            Self::WhalePleat => "whale-pleat",
            Self::Cream => "cream",
            Self::Line => "line",
            Self::Text => "text",
            Self::Panel => "panel",
            Self::Warn => "warn",
            Self::GirlHair => "girl-hair",
            Self::GirlPale => "girl-pale",
            Self::GirlSkin => "girl-skin",
            Self::GirlBlush => "girl-blush",
            Self::GirlCloth => "girl-cloth",
            Self::GirlGold => "girl-gold",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|token| token.name() == name)
    }

    /// `(base, toward, percent)`: the ink is `base`, moved `percent` of the
    /// way to `toward`. Zero percent is the role itself, so the whale is
    /// exactly `Primary`: blue means the whale is present.
    const fn recipe(self) -> (Tone, Tone, u32) {
        use Tone::{Darkest, Lightest};
        const fn role(role: Role) -> Tone {
            Tone::Role(role)
        }
        match self {
            Self::Whale => (role(Role::Primary), Lightest, 0),
            Self::WhaleLite => (role(Role::Primary), Lightest, 35),
            Self::WhaleDeep => (role(Role::Primary), Darkest, 40),
            Self::WhalePleat => (Lightest, role(Role::Primary), 30),
            Self::Cream => (Lightest, Lightest, 0),
            Self::Line => (Darkest, Darkest, 0),
            Self::Text => (role(Role::Foreground), Lightest, 0),
            Self::Panel => (role(Role::Surface), Lightest, 0),
            Self::Warn => (role(Role::Attention), Lightest, 0),
            Self::GirlHair => (role(Role::Primary), Darkest, 55),
            Self::GirlPale => (Lightest, role(Role::Primary), 40),
            Self::GirlSkin => (Lightest, role(Role::Danger), 14),
            Self::GirlBlush => (Lightest, role(Role::Danger), 45),
            Self::GirlCloth => (Darkest, role(Role::Primary), 30),
            Self::GirlGold => (role(Role::Attention), Lightest, 20),
        }
    }
}

/// One ink: the token it is painted with, and whether it prints where the
/// terminal has one color (`fill`) or stays empty there (`open`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GridInk {
    pub token: GridToken,
    pub fill: bool,
}

/// One pose of one body, composed onto its stage: the still frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridPose {
    body: String,
    name: String,
    width: u16,
    height: u16,
    cells: Vec<Option<GridInk>>,
}

impl GridPose {
    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Stage width in cells, which is also the columns it paints.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Stage height in cells. It paints [`GridPose::rows`] terminal rows.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Terminal rows the pose paints: two cells per row.
    #[must_use]
    pub const fn rows(&self) -> u16 {
        self.height.div_ceil(2)
    }

    /// The ink at a stage cell, or `None` where it is empty or off the stage.
    #[must_use]
    pub fn cell(&self, x: u16, y: u16) -> Option<GridInk> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells[usize::from(y) * usize::from(self.width) + usize::from(x)]
    }
}

struct GridRows {
    name: String,
    width: u16,
    rows: Vec<String>,
}

/// One parsed `.grid` file: its inks, its sprites as written, and every pose
/// composed to its still frame.
pub struct GridSheet {
    inks: Vec<(char, GridInk)>,
    sprites: Vec<GridRows>,
    poses: Vec<GridPose>,
}

/// A sprite, or the frames named after it, and the slot it attaches to.
type Layer<'a> = (&'a str, Option<&'a str>);

fn size(text: &str, at: &str) -> Result<(u16, u16), String> {
    let parsed = text
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse::<u16>().ok()?, h.parse::<u16>().ok()?)))
        .filter(|(w, h)| (1..=MAX_SIDE).contains(w) && (1..=MAX_SIDE).contains(h));
    parsed.ok_or_else(|| format!("{at}: expected <w>x<h> from 1x1 to {MAX_SIDE}x{MAX_SIDE}"))
}

impl GridSheet {
    /// The whale, parsed once from [`WHALE_GRID`].
    #[must_use]
    pub fn whale() -> &'static Self {
        static SHEET: OnceLock<GridSheet> = OnceLock::new();
        SHEET.get_or_init(|| Self::parse(WHALE_GRID).expect("whale.grid is checked by tests"))
    }

    /// The whale girl, parsed once from [`WHALE_GIRL_GRID`].
    #[must_use]
    pub fn whale_girl() -> &'static Self {
        static SHEET: OnceLock<GridSheet> = OnceLock::new();
        SHEET.get_or_init(|| {
            Self::parse(WHALE_GIRL_GRID).expect("whale-girl.grid is checked by tests")
        })
    }

    /// Read a grid. Like the design's reference parser, this rejects anything
    /// it could not draw: a ragged row, a character with no `ink` line, an
    /// unknown token, an ink without `fill` or `open`, a pose naming a missing
    /// stage, sprite or slot, and a layer that leaves its stage. It also
    /// rejects a name defined twice and a canvas over 256 cells a side.
    pub fn parse(source: &str) -> Result<Self, String> {
        let mut inks: Vec<(char, GridInk)> = Vec::new();
        let mut sprites: Vec<GridRows> = Vec::new();
        let mut stages: Vec<(&str, u16, u16)> = Vec::new();
        let mut slots: Vec<(&str, &str, u16, u16)> = Vec::new();
        let mut layouts: Vec<(&str, &str, Vec<Layer<'_>>)> = Vec::new();
        let lines: Vec<&str> = source.split('\n').collect();
        let mut next = 0;
        while next < lines.len() {
            let line = lines[next];
            next += 1;
            let at = format!("line {next}");
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                [] => {}
                [first, ..] if first.starts_with('#') => {}
                ["ink", key, token, level] => {
                    let mut chars = key.chars();
                    let (Some(key), None) = (chars.next(), chars.next()) else {
                        return Err(format!("{at}: an ink key is one character"));
                    };
                    let token = GridToken::from_name(token)
                        .ok_or_else(|| format!("{at}: unknown token {token}"))?;
                    let fill = match *level {
                        "fill" => true,
                        "open" => false,
                        _ => return Err(format!("{at}: an ink is fill or open")),
                    };
                    if key == '.' || inks.iter().any(|(k, _)| *k == key) {
                        return Err(format!("{at}: ink {key} is already taken"));
                    }
                    inks.push((key, GridInk { token, fill }));
                }
                ["sprite", name, dimensions] => {
                    let (width, height) = size(dimensions, &at)?;
                    let rows = lines
                        .get(next..next + usize::from(height))
                        .ok_or_else(|| format!("{at}: sprite {name} is cut short"))?;
                    next += usize::from(height);
                    for row in rows {
                        if row.chars().count() != usize::from(width) {
                            return Err(format!("{at}: ragged row in {name}: {row:?}"));
                        }
                        if let Some(c) = row
                            .chars()
                            .find(|c| *c != '.' && !inks.iter().any(|(k, _)| k == c))
                        {
                            return Err(format!("{at}: no ink for {c:?} in {name}"));
                        }
                    }
                    if sprites.iter().any(|s| s.name == *name) {
                        return Err(format!("{at}: sprite {name} is defined twice"));
                    }
                    sprites.push(GridRows {
                        name: (*name).to_string(),
                        width,
                        rows: rows.iter().map(|r| (*r).to_string()).collect(),
                    });
                }
                ["stage", body, dimensions] => {
                    let (width, height) = size(dimensions, &at)?;
                    if stages.iter().any(|(b, ..)| b == body) {
                        return Err(format!("{at}: stage {body} is defined twice"));
                    }
                    stages.push((body, width, height));
                }
                ["slot", body, slot, x, y] => {
                    let (Ok(x), Ok(y)) = (x.parse(), y.parse()) else {
                        return Err(format!("{at}: a slot is <body> <slot> <x> <y>"));
                    };
                    if slots.iter().any(|(b, s, ..)| b == body && s == slot) {
                        return Err(format!("{at}: slot {body} {slot} is defined twice"));
                    }
                    slots.push((body, slot, x, y));
                }
                ["pose", body, pose, layers @ ..] if !layers.is_empty() => {
                    if layouts.iter().any(|(b, p, _)| b == body && p == pose) {
                        return Err(format!("{at}: pose {body} {pose} is defined twice"));
                    }
                    let layers = layers
                        .iter()
                        .map(|layer| match layer.split_once('@') {
                            Some((sprite, slot)) => (sprite, Some(slot)),
                            None => (*layer, None),
                        })
                        .collect();
                    layouts.push((body, pose, layers));
                }
                _ => return Err(format!("{at}: unknown line: {line}")),
            }
        }

        let ink = |key: char| inks.iter().find(|(k, _)| *k == key).map(|(_, ink)| *ink);
        let mut poses = Vec::with_capacity(layouts.len());
        for (body, pose, layers) in layouts {
            let at = format!("pose {body} {pose}");
            let &(_, width, height) = stages
                .iter()
                .find(|(b, ..)| *b == body)
                .ok_or_else(|| format!("{at}: no stage"))?;
            let mut cells = vec![None; usize::from(width) * usize::from(height)];
            for (layer, slot) in layers {
                let (x, y) = match slot {
                    None => (0, 0),
                    Some(slot) => slots
                        .iter()
                        .find(|(b, s, ..)| *b == body && *s == slot)
                        .map(|&(_, _, x, y)| (x, y))
                        .ok_or_else(|| format!("{at}: no slot {slot}"))?,
                };
                // A layer is one sprite, or frames `<layer>.0`, `<layer>.1`...
                // Every frame must stay on the stage; only the last is drawn.
                let mut frames: Vec<&GridRows> =
                    sprites.iter().filter(|s| s.name == layer).collect();
                if frames.is_empty() {
                    while let Some(frame) = sprites
                        .iter()
                        .find(|s| s.name == format!("{layer}.{}", frames.len()))
                    {
                        frames.push(frame);
                    }
                }
                let still = *frames
                    .last()
                    .ok_or_else(|| format!("{at}: no sprite {layer}"))?;
                if frames.iter().any(|f| {
                    u32::from(x) + u32::from(f.width) > u32::from(width)
                        || u32::from(y) + f.rows.len() as u32 > u32::from(height)
                }) {
                    return Err(format!("{at}: {layer} leaves the {width}x{height} stage"));
                }
                for (row, line) in still.rows.iter().enumerate() {
                    for (column, key) in line.chars().enumerate() {
                        if key != '.' {
                            cells[(usize::from(y) + row) * usize::from(width)
                                + usize::from(x)
                                + column] = ink(key);
                        }
                    }
                }
            }
            poses.push(GridPose {
                body: body.to_string(),
                name: pose.to_string(),
                width,
                height,
                cells,
            });
        }
        Ok(Self {
            inks,
            sprites,
            poses,
        })
    }

    /// The inks in file order, each with the character that names it.
    #[must_use]
    pub fn inks(&self) -> &[(char, GridInk)] {
        &self.inks
    }

    /// Sprite names in file order.
    pub fn sprite_names(&self) -> impl Iterator<Item = &str> {
        self.sprites.iter().map(|s| s.name.as_str())
    }

    /// A sprite's rows exactly as written, ink keys and `.`.
    #[must_use]
    pub fn sprite_rows(&self, name: &str) -> Option<&[String]> {
        self.sprites
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.rows.as_slice())
    }

    /// Every pose in file order. Bodies are namespaced by file: both
    /// characters have a `hero`.
    #[must_use]
    pub fn poses(&self) -> &[GridPose] {
        &self.poses
    }

    #[must_use]
    pub fn pose(&self, body: &str, pose: &str) -> Option<&GridPose> {
        self.poses.iter().find(|p| p.body == body && p.name == pose)
    }
}

/// The color of every token on one theme, resolved once per paint.
struct Inks([Option<Color>; GridToken::COUNT]);

impl Inks {
    fn new(theme: &Theme) -> Self {
        // The extremes are measured, not assumed from the appearance: a
        // native palette may leave its grounds to the terminal, and those
        // have no color to paint an ink with.
        let extreme = |roles: &[Role], lightest: bool| {
            let measured = roles
                .iter()
                .filter_map(|role| Some((*role, color::relative_luminance(theme.token(*role))?)));
            if lightest {
                measured.max_by(|a, b| a.1.total_cmp(&b.1))
            } else {
                measured.min_by(|a, b| a.1.total_cmp(&b.1))
            }
            .map(|(role, _)| role)
        };
        let lightest = extreme(
            &[
                Role::Foreground,
                Role::Surface,
                Role::Background,
                Role::Selected,
                Role::Hover,
            ],
            true,
        );
        let darkest = extreme(
            &[
                Role::Foreground,
                Role::Sidebar,
                Role::Background,
                Role::Surface,
            ],
            false,
        );
        let role = |tone| match tone {
            Tone::Role(role) => Some(role),
            Tone::Lightest => lightest,
            Tone::Darkest => darkest,
        };
        Self(GridToken::ALL.map(|token| {
            let (base, toward, percent) = token.recipe();
            let base = role(base)?;
            let mixed = (percent > 0)
                .then(|| {
                    let a = color::resolvable_rgb(theme.token(base))?;
                    let b = color::resolvable_rgb(theme.token(role(toward)?))?;
                    let mix = |a: u8, b: u8| {
                        ((u32::from(a) * (100 - percent) + u32::from(b) * percent + 50) / 100) as u8
                    };
                    let (r, g, b) = (mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2));
                    Some(if theme.depth() == ColorDepth::Ansi256 {
                        Color::Indexed(color::rgb_to_ansi256(r, g, b))
                    } else {
                        Color::Rgb(r, g, b)
                    })
                })
                .flatten();
            // A role the terminal owns is left unpainted, like an empty cell.
            mixed
                .or_else(|| theme.color(base))
                .filter(|color| *color != Color::Reset)
        }))
    }

    fn of(&self, ink: Option<GridInk>) -> Option<Color> {
        self.0[ink?.token as usize]
    }
}

/// Upper half, lower half, both.
const BLOCKS: [&str; 3] = ["▀", "▄", "█"];
const ASCII_BLOCKS: [&str; 3] = ["\"", "_", "#"];

/// One pose, painted whole and still. See the module documentation for the
/// decoration rule: pair it with text and never let it carry state alone.
///
/// ```
/// use codewhale_ratatui::{GridSprite, Paint, Theme};
/// # fn draw(area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
/// if let Some(whale) = GridSprite::whale("companion", "rest") {
///     whale.paint(area, buf, theme);
/// }
/// # }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct GridSprite<'a> {
    pose: &'a GridPose,
}

impl<'a> GridSprite<'a> {
    #[must_use]
    pub const fn new(pose: &'a GridPose) -> Self {
        Self { pose }
    }

    /// Columns the sprite needs.
    #[must_use]
    pub const fn columns(&self) -> u16 {
        self.pose.width
    }

    /// Rows the sprite needs.
    #[must_use]
    pub const fn rows(&self) -> u16 {
        self.pose.rows()
    }

    /// Whether `area` holds the whole sprite. [`Paint::paint`] draws nothing
    /// when it does not, so a host can give the room to text instead.
    #[must_use]
    pub const fn fits(&self, area: Rect) -> bool {
        area.width >= self.columns() && area.height >= self.rows()
    }
}

impl GridSprite<'static> {
    /// A pose of the whale: `mark`, `favicon`, `companion` or `hero`.
    #[must_use]
    pub fn whale(body: &str, pose: &str) -> Option<Self> {
        GridSheet::whale().pose(body, pose).map(Self::new)
    }

    /// A pose of the whale girl: `small` or `hero`.
    #[must_use]
    pub fn whale_girl(body: &str, pose: &str) -> Option<Self> {
        GridSheet::whale_girl().pose(body, pose).map(Self::new)
    }
}

impl Paint for GridSprite<'_> {
    fn paint(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        // Pixel art is drawn at one size. A pose is centred whole in the area
        // it was given or left out; the buffer may still clip that placement.
        if !self.fits(area) {
            return;
        }
        let left = area.x + (area.width - self.columns()) / 2;
        let top = area.y + (area.height - self.rows()) / 2;
        let visible = Rect::new(left, top, self.columns(), self.rows()).intersection(buf.area);
        let inks = (theme.caps().paints_tokens() && !theme.ascii()).then(|| Inks::new(theme));
        let blocks = if theme.ascii() { ASCII_BLOCKS } else { BLOCKS };
        for y in visible.top()..visible.bottom() {
            for x in visible.left()..visible.right() {
                let (column, row) = (x - left, (y - top) * 2);
                let upper = self.pose.cell(column, row);
                let lower = self.pose.cell(column, row + 1);
                let cell = &mut buf[(x, y)];
                let Some(inks) = &inks else {
                    // The one-ink map for NO_COLOR, 16 colors, an unmeasured
                    // ground and ASCII: fill inks print, open inks are empty.
                    let on = |ink: Option<GridInk>| ink.is_some_and(|ink| ink.fill);
                    let symbol = match (on(upper), on(lower)) {
                        (true, false) => blocks[0],
                        (false, true) => blocks[1],
                        (true, true) => blocks[2],
                        (false, false) => continue,
                    };
                    cell.set_symbol(symbol).set_style(theme.fg(Role::Primary));
                    cell.modifier = Modifier::empty();
                    continue;
                };
                // Only the foreground is set for a half or a one-ink cell, so
                // the ground already behind the sprite shows through.
                match (inks.of(upper), inks.of(lower)) {
                    (Some(a), Some(b)) if a == b => cell.set_symbol(blocks[2]).set_fg(a),
                    (Some(a), Some(b)) => cell.set_symbol(blocks[0]).set_fg(a).set_bg(b),
                    (Some(a), None) => cell.set_symbol(blocks[0]).set_fg(a),
                    (None, Some(b)) => cell.set_symbol(blocks[1]).set_fg(b),
                    (None, None) => continue,
                };
                cell.modifier = Modifier::empty();
            }
        }
    }

    fn height(&self, _width: u16, _theme: &Theme) -> u16 {
        self.rows()
    }
}
