//! Generate `src/roles.rs` from the vendored design tokens, and check it.
//!
//!   cargo test --test generated                       # fails if roles.rs is stale
//!   CODEWHALE_BLESS=1 cargo test --test generated     # rewrites roles.rs
//!
//! Colors come only from `vendor/codewhale-design/tokens.json`, the one
//! token source the desktop app and the web read too. The generator adds
//! what a terminal needs that the tokens cannot say:
//!
//! - which xterm 256-color index to show for each role. It starts from the
//!   nearest fixed index and, where quantizing breaks the design's own
//!   contrast rules (the same pairs `generate.py` enforces in truecolor),
//!   moves the ink to the nearest index that holds them. Grounds are never
//!   moved, except the diff tints, which need a hue the gray ramp lacks.
//! - four roles the tokens do not name, derived from tokens that they do:
//!   `Hint` and `Dim` (`muted_foreground` receded toward `background`) and
//!   the diff tints (`live` / `danger` at 16% over `background`). See
//!   [`DERIVED`].

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use codewhale_ratatui::color::{
    blend, contrast_ratio, indexed_rgb, rgb, rgb_to_ansi256, tint_keeping_luminance,
};
use codewhale_ratatui::theme::{LOGO_BOTTOM, OCEAN_TINT};
use ratatui::style::Color;

/// Every role, in enum order: variant, `tokens.json` key, doc comment.
const ROLES: &[(&str, &str, &str)] = &[
    (
        "Sidebar",
        "sidebar",
        "The deepest ground: rails and things put away.",
    ),
    ("Background", "background", "The stage."),
    ("Surface", "surface", "A raised panel."),
    ("Hover", "hover", "A row under the pointer."),
    ("Selected", "selected", "The selected row."),
    ("Foreground", "foreground", "Body ink."),
    (
        "Muted",
        "muted_foreground",
        "Ink that recedes: details, verbs in hints, asides.",
    ),
    ("Border", "border", "Quiet lines that separate."),
    (
        "BorderStrong",
        "border_strong",
        "Edges that identify a control or a decision.",
    ),
    ("Primary", "primary", "Actions, focus and links."),
    (
        "PrimaryForeground",
        "primary_foreground",
        "Ink on a `Primary` ground.",
    ),
    ("Live", "live", "Work happening now, and work done."),
    ("Attention", "attention", "Needs you."),
    ("Danger", "danger", "Failed or destructive."),
    (
        "Hint",
        "hint",
        "Ink one step quieter than `Muted`: asides and placeholders. Derived: `muted_foreground` receded toward `background` until it just holds 4.5:1 on every ground.",
    ),
    (
        "Dim",
        "dim",
        "The quietest ink: disabled and decorative words, never the only carrier of meaning. Derived: `muted_foreground` receded toward `background` until it just holds 3:1 on every ground.",
    ),
    (
        "DiffAddedTint",
        "diff_added_tint",
        "Ground behind an added line, under `Foreground` ink. Derived: `live` at 16% over `background`.",
    ),
    (
        "DiffRemovedTint",
        "diff_removed_tint",
        "Ground behind a removed line, under `Foreground` ink. Derived: `danger` at 16% over `background`.",
    ),
];

/// Roles with no `tokens.json` key: derived here from tokens that exist, so
/// a token change reaches them without a hand edit. Their keys are not
/// token names; [`derive`] fills them in after the tokens are read.
const DERIVED: &[&str] = &["hint", "dim", "diff_added_tint", "diff_removed_tint"];

/// How far a diff tint moves from `background` toward its hue, in percent.
/// The design allows at most 16%, behind `Foreground` only.
const TINT_PERCENT: u32 = 16;

/// The diff tints and the state hue each is made from.
const TINTS: &[(&str, &str)] = &[("diff_added_tint", "live"), ("diff_removed_tint", "danger")];

/// The design's contrast rules (`generate.py`): text at 4.5:1 and control
/// edges at 3:1 on every ground, and ink on a primary fill at 4.5:1.
const GROUNDS: &[&str] = &["background", "surface", "sidebar", "hover", "selected"];
const TEXT: &[&str] = &[
    "foreground",
    "muted_foreground",
    "primary",
    "live",
    "attention",
    "danger",
];
const EDGES: &[&str] = &["border_strong"];

/// Derived inks and their floors on every ground. `hint` is text and holds
/// the design's 4.5:1; `dim` is secondary chrome (disabled or decorative,
/// never the only carrier of meaning) and holds 3:1, as the engine's palette
/// audits its own hint and dim inks.
const DERIVED_INKS: &[(&str, f32)] = &[("hint", 4.5), ("dim", 3.0)];

/// Every ink that gets a 256-color index of its own, in settling order.
fn inks() -> Vec<&'static str> {
    TEXT.iter()
        .chain(DERIVED_INKS.iter().map(|(k, _)| k).collect::<Vec<_>>())
        .chain(EDGES)
        .copied()
        .collect()
}

/// What a diff tint must keep legible, as `(ink key, floor)`: body ink and
/// the tint's own state hue at 4.5:1; `muted_foreground` (line numbers) at
/// 3:1, the floor the 256-color cube can still meet on a green.
fn tint_floors(tint: &str) -> Vec<(&'static str, f32)> {
    let hue = TINTS.iter().find(|(t, _)| *t == tint).expect("a tint").1;
    vec![("foreground", 4.5), ("muted_foreground", 3.0), (hue, 4.5)]
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap_or_else(|_| panic!("bad hex {s}"))
}

fn idx_color(i: u8) -> Color {
    Color::Indexed(i)
}

/// Contrast floors the ink `key` must hold, as `(ground key, floor)`.
fn floors(key: &str) -> Vec<(&'static str, f32)> {
    if TEXT.contains(&key) {
        GROUNDS.iter().map(|g| (*g, 4.5)).collect()
    } else if EDGES.contains(&key) {
        GROUNDS.iter().map(|g| (*g, 3.0)).collect()
    } else if let Some((_, floor)) = DERIVED_INKS.iter().find(|(k, _)| *k == key) {
        GROUNDS.iter().map(|g| (*g, *floor)).collect()
    } else if key == "primary_foreground" {
        vec![("primary", 4.5)]
    } else {
        Vec::new()
    }
}

/// Every `(ink, ground, floor)` the kit audits, for the truecolor tables.
fn audited() -> Vec<(&'static str, &'static str, f32)> {
    let mut pairs = Vec::new();
    for ink in TEXT
        .iter()
        .chain(EDGES)
        .chain(["primary_foreground"].iter())
    {
        for (ground, floor) in floors(ink) {
            pairs.push((*ink, ground, floor));
        }
    }
    for (ink, _) in DERIVED_INKS {
        for (ground, floor) in floors(ink) {
            pairs.push((*ink, ground, floor));
        }
    }
    for (tint, _) in TINTS {
        for (ink, floor) in tint_floors(tint) {
            pairs.push((ink, *tint, floor));
        }
    }
    pairs
}

/// Fail if any audited pair breaks its floor in this truecolor table.
fn audit(table: &BTreeMap<String, u32>, what: &str) {
    for (ink, ground, floor) in audited() {
        let ratio = contrast_ratio(rgb(table[ink]), rgb(table[ground])).expect("rgb");
        assert!(
            ratio >= floor,
            "{what}: {ink} on {ground} is {ratio:.2}:1, needs {floor}:1"
        );
    }
}

fn nearest(value: u32) -> u8 {
    rgb_to_ansi256((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn dist(a: (u8, u8, u8), value: u32) -> u32 {
    let d = |x: u8, y: u8| {
        let v = i32::from(x) - i32::from(y);
        (v * v) as u32
    };
    d(a.0, (value >> 16) as u8) + d(a.1, (value >> 8) as u8) + d(a.2, value as u8)
}

/// State hues keep their hue at 256 colors: a green that quantizes to gray
/// stops saying "live".
const HUES: &[&str] = &["primary", "live", "attention", "danger"];

/// Hue in degrees and chroma (max - min channel) of an RGB triple.
fn hue_chroma((r, g, b): (u8, u8, u8)) -> (f32, u8) {
    let (rf, gf, bf) = (f32::from(r), f32::from(g), f32::from(b));
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let c = f32::from(max - min);
    if c == 0.0 {
        return (0.0, 0);
    }
    let h = if max == r {
        ((gf - bf) / c).rem_euclid(6.0)
    } else if max == g {
        (bf - rf) / c + 2.0
    } else {
        (rf - gf) / c + 4.0
    };
    (h * 60.0, max - min)
}

fn split(value: u32) -> (u8, u8, u8) {
    ((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

/// Degrees between the hue of `rgb` and the hue of `value`.
fn hue_gap(rgb: (u8, u8, u8), value: u32) -> f32 {
    let (want, _) = hue_chroma(split(value));
    let (got, _) = hue_chroma(rgb);
    let diff = (want - got).abs();
    diff.min(360.0 - diff)
}

/// Whether index `i` still reads as the hue of `value`.
fn keeps_hue(i: u8, value: u32) -> bool {
    let rgb = indexed_rgb(i).expect("fixed index");
    let (_, chroma) = hue_chroma(rgb);
    chroma >= 40 && hue_gap(rgb, value) <= 35.0
}

fn pack(color: Color) -> u32 {
    match color {
        Color::Rgb(r, g, b) => (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b),
        other => panic!("not an RGB color: {other:?}"),
    }
}

/// `ink` mixed with `ground` at `percent` of `ink`, as `0xRRGGBB`.
fn mix(ink: u32, ground: u32, percent: u32) -> u32 {
    pack(blend(rgb(ink), rgb(ground), percent as f32 / 100.0))
}

/// The four roles the tokens do not name, added to a token table.
///
/// `dim` is `muted_foreground` receded toward `background` as far as it
/// still holds 3:1 on every ground in `grounds`, and `hint` is the same at
/// 4.5:1, so the ink ladder runs Foreground, Muted, Hint, Dim, each a
/// visible step apart. The diff tints are `live` and `danger` at
/// [`TINT_PERCENT`] over `background`. `grounds` lists every ground the
/// inks must hold on (the blue ombre's included); it never changes the
/// derivation otherwise.
fn derive(colors: &mut BTreeMap<String, u32>, mode: &str, grounds: &[u32]) {
    let (muted, background) = (colors["muted_foreground"], colors["background"]);
    let holds = |value: u32, floor: f32| {
        grounds
            .iter()
            .all(|g| contrast_ratio(rgb(value), rgb(*g)).expect("rgb") >= floor)
    };
    let weight = |floor: f32| {
        (0..=100u32)
            .rev()
            .take_while(|w| holds(mix(muted, background, *w), floor))
            .last()
            .unwrap_or_else(|| panic!("{mode}: muted_foreground does not hold {floor}:1"))
    };
    let (hint, dim) = (weight(4.5), weight(3.0));
    colors.insert("hint".into(), mix(muted, background, hint));
    colors.insert("dim".into(), mix(muted, background, dim));
    for (name, hue) in TINTS {
        colors.insert((*name).into(), mix(colors[*hue], background, TINT_PERCENT));
    }
    for (a, b) in [("muted_foreground", "hint"), ("hint", "dim")] {
        let ratio = contrast_ratio(rgb(colors[a]), rgb(colors[b])).expect("rgb");
        assert!(
            ratio >= 1.1,
            "{mode}: {a} and {b} are not a visible step apart ({ratio:.2})"
        );
    }
}

/// The 256-color table for one appearance, and a note per adjusted role.
fn table_256(colors: &BTreeMap<String, u32>, mode: &str) -> (BTreeMap<String, u8>, Vec<String>) {
    let mut table: BTreeMap<String, u8> = colors
        .iter()
        .map(|(k, v)| (k.clone(), nearest(*v)))
        .collect();
    let mut notes = Vec::new();
    // Inks over grounds first; ink on primary last, so it sees primary's
    // final index.
    let mut order = inks();
    order.push("primary_foreground");
    // Inks settled so far: a later ink never lands on one of these.
    let mut done: Vec<&str> = Vec::new();
    for key in order {
        let rules = floors(key);
        let holds = |i: u8, table: &BTreeMap<String, u8>| {
            rules.iter().all(|(g, floor)| {
                contrast_ratio(idx_color(i), idx_color(table[*g])).is_some_and(|r| r >= *floor)
            })
        };
        let hue = HUES.contains(&key);
        let start = table[key];
        // Two inks sharing one color read as one: the later one moves.
        let collides = key != "primary_foreground" && done.iter().any(|k| table[*k] == start);
        if holds(start, &table) && (!hue || keeps_hue(start, colors[key])) && !collides {
            done.push(key);
            continue;
        }
        let value = colors[key];
        // Never land on an index another ink already shows: two states
        // sharing one color would read as one state.
        let taken: Vec<u8> = inks()
            .into_iter()
            .filter(|k| *k != key)
            .map(|k| table[k])
            .collect();
        let best = (16..=255u8)
            .filter(|i| !taken.contains(i) && holds(*i, &table))
            .filter(|i| !hue || keeps_hue(*i, value))
            .min_by_key(|i| (dist(indexed_rgb(*i).expect("fixed index"), value), *i))
            .unwrap_or_else(|| panic!("{mode} {key}: no 256-color index holds its contrast"));
        let why = if !holds(start, &table) {
            "failed its contrast floor"
        } else if hue && !keeps_hue(start, value) {
            "lost its hue"
        } else {
            "shared an index with another ink"
        };
        notes.push(format!("{mode} {key}: {start} -> {best} (nearest {why})"));
        table.insert(key.to_string(), best);
        done.push(key);
    }
    // Diff tints are grounds, but the cube has no gray-green or gray-red
    // that dark, so the nearest index would be a plain gray. They take the
    // nearest index that keeps their hue and the floors on the final inks.
    for (tint, hue_key) in TINTS {
        let rules = tint_floors(tint);
        let want = colors[*tint];
        let best = (16..=255u8)
            .filter(|i| {
                rules.iter().all(|(ink, floor)| {
                    contrast_ratio(idx_color(table[*ink]), idx_color(*i))
                        .is_some_and(|r| r >= *floor)
                })
            })
            .filter(|i| keeps_hue(*i, colors[*hue_key]))
            // Closest hue first, in 10-degree steps, so a green stays green
            // rather than drifting to the nearer teal; then the nearest color.
            .min_by_key(|i| {
                let rgb = indexed_rgb(*i).expect("fixed index");
                (
                    hue_gap(rgb, colors[*hue_key]) as u32 / 10,
                    dist(rgb, want),
                    *i,
                )
            })
            .unwrap_or_else(|| panic!("{mode} {tint}: no 256-color index holds its contrast"));
        notes.push(format!(
            "{mode} {tint}: {} -> {best} (the cube has no tint this faint; nearest index that keeps the hue of {hue_key})",
            table[*tint]
        ));
        table.insert((*tint).to_string(), best);
    }
    let hues: Vec<u8> = ["primary", "live", "attention", "danger"]
        .iter()
        .map(|k| table[*k])
        .collect();
    for (i, a) in hues.iter().enumerate() {
        assert!(
            !hues[i + 1..].contains(a),
            "{mode}: two state hues share 256-color index {a}"
        );
    }
    // The ink ladder keeps its steps: no two inks share an index.
    let shown: Vec<(&str, u8)> = inks().into_iter().map(|k| (k, table[k])).collect();
    for (i, (a, ai)) in shown.iter().enumerate() {
        for (b, bi) in &shown[i + 1..] {
            assert!(ai != bi, "{mode}: {a} and {b} share 256-color index {ai}");
        }
    }
    (table, notes)
}

/// Roles the blue ombre tints: the grounds and the quiet line.
const OCEAN_TINTED: &[&str] = &[
    "sidebar",
    "background",
    "surface",
    "hover",
    "selected",
    "border",
];

/// The five ombre grounds, tinted from the dark token grounds.
fn ocean_grounds(dark: &BTreeMap<String, u32>) -> Vec<u32> {
    GROUNDS
        .iter()
        .map(|g| tint_keeping_luminance(dark[*g], LOGO_BOTTOM, OCEAN_TINT))
        .collect()
}

/// The dark table with its grounds and quiet line tinted toward the logo's
/// deep blue at their own luminance. The diff tints follow: the same mix of
/// state hue over the ombre's own background, so they sit on it. Every
/// truecolor contrast floor must still hold; a failure here means the tint
/// needs review, not a new ink.
fn ocean(dark: &BTreeMap<String, u32>) -> BTreeMap<String, u32> {
    let mut table: BTreeMap<String, u32> = dark
        .iter()
        .map(|(k, v)| {
            let v = if OCEAN_TINTED.contains(&k.as_str()) {
                tint_keeping_luminance(*v, LOGO_BOTTOM, OCEAN_TINT)
            } else {
                *v
            };
            (k.clone(), v)
        })
        .collect();
    for (name, hue) in TINTS {
        table.insert(
            (*name).into(),
            mix(table[*hue], table["background"], TINT_PERCENT),
        );
    }
    audit(&table, "ocean");
    table
}

fn render(json: &serde_json::Value) -> String {
    let version = json["version"].as_str().expect("version");
    let mut modes = BTreeMap::new();
    for mode in ["dark", "light"] {
        let obj = json["colors"][mode].as_object().expect("colors");
        let keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        for key in &keys {
            assert!(
                !DERIVED.contains(key),
                "tokens.json now names `{key}`; read it instead of deriving it (tests/generated.rs DERIVED)"
            );
            assert!(
                ROLES.iter().any(|(_, k, _)| k == key),
                "tokens.json color `{key}` has no Role; add it to ROLES in tests/generated.rs"
            );
        }
        for (_, key, _) in ROLES.iter().filter(|(_, k, _)| !DERIVED.contains(k)) {
            assert!(
                keys.contains(key),
                "Role token `{key}` is missing from tokens.json {mode}"
            );
        }
        let mut colors: BTreeMap<String, u32> = obj
            .iter()
            .map(|(k, v)| (k.clone(), hex(v.as_str().expect("hex string"))))
            .collect();
        // The ink ladder must hold on every ground it can sit on; for dark,
        // that includes the blue ombre's.
        let mut grounds: Vec<u32> = GROUNDS.iter().map(|g| colors[*g]).collect();
        if mode == "dark" {
            grounds.extend(ocean_grounds(&colors));
        }
        derive(&mut colors, mode, &grounds);
        audit(&colors, mode);
        modes.insert(mode, colors);
    }

    let mut out = String::new();
    let w = &mut out;
    writeln!(
        w,
        "// Generated from vendor/codewhale-design/tokens.json {version} by tests/generated.rs."
    )
    .unwrap();
    writeln!(
        w,
        "// Do not edit. Regenerate: CODEWHALE_BLESS=1 cargo test --test generated"
    )
    .unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "/// The design tokens version these roles were generated from."
    )
    .unwrap();
    writeln!(w, "pub const TOKENS_VERSION: &str = \"{version}\";").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "/// What a color is for. Components name roles; they never name colors."
    )
    .unwrap();
    writeln!(
        w,
        "/// Names follow `tokens.json`, so one vocabulary covers the desktop app,"
    )
    .unwrap();
    writeln!(
        w,
        "/// the web and the terminal. Four roles the tokens do not name are"
    )
    .unwrap();
    writeln!(w, "/// derived from ones they do ([`Role::is_derived`]).").unwrap();
    writeln!(
        w,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]"
    )
    .unwrap();
    writeln!(w, "pub enum Role {{").unwrap();
    for (variant, _, doc) in ROLES {
        writeln!(w, "    /// {doc}").unwrap();
        writeln!(w, "    {variant},").unwrap();
    }
    writeln!(w, "}}").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "impl Role {{").unwrap();
    writeln!(w, "    pub const COUNT: usize = {};", ROLES.len()).unwrap();
    writeln!(w, "    pub const ALL: [Role; Self::COUNT] = [").unwrap();
    for (variant, _, _) in ROLES {
        writeln!(w, "        Role::{variant},").unwrap();
    }
    writeln!(w, "    ];").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "    /// The `tokens.json` key this role reads, or `None` for a role the"
    )
    .unwrap();
    writeln!(
        w,
        "    /// kit derives from other tokens ([`Role::is_derived`])."
    )
    .unwrap();
    writeln!(w, "    #[must_use]").unwrap();
    writeln!(
        w,
        "    pub const fn token_name(self) -> Option<&'static str> {{"
    )
    .unwrap();
    writeln!(w, "        match self {{").unwrap();
    for (variant, key, _) in ROLES {
        if DERIVED.contains(key) {
            writeln!(w, "            Role::{variant} => None,").unwrap();
        } else {
            writeln!(w, "            Role::{variant} => Some(\"{key}\"),").unwrap();
        }
    }
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "    /// Position in [`Role::ALL`] and in the color tables."
    )
    .unwrap();
    writeln!(w, "    #[must_use]").unwrap();
    writeln!(w, "    pub const fn index(self) -> usize {{").unwrap();
    writeln!(w, "        self as usize").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "    /// Whether the kit derives this role from other tokens, because"
    )
    .unwrap();
    writeln!(w, "    /// `tokens.json` does not name it.").unwrap();
    writeln!(w, "    #[must_use]").unwrap();
    writeln!(w, "    pub const fn is_derived(self) -> bool {{").unwrap();
    let derived: Vec<String> = ROLES
        .iter()
        .filter(|(_, k, _)| DERIVED.contains(k))
        .map(|(v, _, _)| format!("Role::{v}"))
        .collect();
    writeln!(w, "        matches!(self, {})", derived.join(" | ")).unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "}}").unwrap();

    for mode in ["dark", "light"] {
        let colors = &modes[mode];
        let upper = mode.to_uppercase();
        writeln!(w).unwrap();
        writeln!(
            w,
            "/// {mode} token colors, `0xRRGGBB`, indexed by [`Role::index`]."
        )
        .unwrap();
        writeln!(w, "pub(crate) const {upper}: [u32; Role::COUNT] = [").unwrap();
        for (variant, key, _) in ROLES {
            writeln!(w, "    0x{:06x}, // {variant}", colors[*key]).unwrap();
        }
        writeln!(w, "];").unwrap();
        let (table, notes) = table_256(colors, mode);
        writeln!(w).unwrap();
        writeln!(
            w,
            "/// {mode} xterm 256-color indices (16..=255 only; 0..=15 belong to the"
        )
        .unwrap();
        writeln!(
            w,
            "/// user's profile). Nearest index, except where contrast needed a move:"
        )
        .unwrap();
        if notes.is_empty() {
            writeln!(w, "/// none in this table.").unwrap();
        }
        for note in &notes {
            writeln!(w, "/// - {note}").unwrap();
        }
        writeln!(w, "pub(crate) const {upper}_256: [u8; Role::COUNT] = [").unwrap();
        for (variant, key, _) in ROLES {
            writeln!(w, "    {}, // {variant}", table[*key]).unwrap();
        }
        writeln!(w, "];").unwrap();
    }

    let ocean = ocean(&modes["dark"]);
    writeln!(w).unwrap();
    writeln!(
        w,
        "/// The blue ombre (`Ground::Ocean`): the dark table with its grounds and"
    )
    .unwrap();
    writeln!(
        w,
        "/// quiet line tinted {OCEAN_TINT} toward `LOGO_BOTTOM` at their own luminance,"
    )
    .unwrap();
    writeln!(
        w,
        "/// so every contrast floor holds. The diff tints are re-mixed over its own"
    )
    .unwrap();
    writeln!(
        w,
        "/// background. Truecolor only; 256 colors use `DARK_256`."
    )
    .unwrap();
    writeln!(w, "pub(crate) const OCEAN: [u32; Role::COUNT] = [").unwrap();
    for (variant, key, _) in ROLES {
        writeln!(w, "    0x{:06x}, // {variant}", ocean[*key]).unwrap();
    }
    writeln!(w, "];").unwrap();
    out
}

#[test]
fn roles_match_the_vendored_tokens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(root.join("vendor/codewhale-design/tokens.json"))
        .expect("vendored tokens.json");
    let json: serde_json::Value = serde_json::from_str(&source).expect("tokens.json parses");
    let expected = render(&json);
    let target = root.join("src/roles.rs");
    if std::env::var_os("CODEWHALE_BLESS").is_some() {
        std::fs::write(&target, &expected).expect("write src/roles.rs");
        return;
    }
    let actual = std::fs::read_to_string(&target).unwrap_or_default();
    assert!(
        actual == expected,
        "src/roles.rs is stale for tokens {}; run CODEWHALE_BLESS=1 cargo test --test generated",
        json["version"]
    );
}

/// The vendored `tokens.rs` (from `generate.py`) and our roles agree, so the
/// two generated views of one token file cannot drift apart.
#[test]
fn vendored_tokens_rs_agrees_with_roles() {
    use codewhale_ratatui::{Role, tokens};
    let t = codewhale_ratatui::testing::Profile::DarkTrue.theme();
    assert_eq!(t.token(Role::BorderStrong), rgb(tokens::DARK.border_strong));
    let t = codewhale_ratatui::testing::Profile::LightTrue.theme();
    assert_eq!(t.token(Role::Primary), rgb(tokens::LIGHT.primary));
    assert_eq!(codewhale_ratatui::theme::TOKENS_VERSION, tokens::VERSION);
}

/// `generate.py --check` validates the vendored folder against its own
/// digest. It needs Python; where Python is missing the check says so.
#[test]
fn vendored_folder_passes_its_own_check() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = root.join("vendor/codewhale-design/generate.py");
    match std::process::Command::new("python3")
        .arg(&script)
        .arg("--check")
        .output()
    {
        Ok(out) => assert!(
            out.status.success(),
            "generate.py --check failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(err) => eprintln!("skipped: python3 unavailable ({err})"),
    }
}
