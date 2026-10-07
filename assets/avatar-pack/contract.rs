//! Avatar pack v1: presentation data, never executable rendering or lifecycle.
//! Native, terminal and the extension host vendor this exact contract.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ACTS: [&str; 17] = [
    "rest", "listen", "think", "busy", "read", "search", "write", "run", "browse", "talk", "pod",
    "needs", "done", "hmm", "computer", "connect", "sleep",
];
pub const MAX_PNG_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_PACK_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Pack {
    pub version: u8,
    pub id: String,
    pub name: String,
    pub atlases: Vec<String>,
    pub columns: u16,
    pub rows: u16,
    pub tile_width: u16,
    pub tile_height: u16,
    pub actions: BTreeMap<String, Action>,
    pub states: BTreeMap<String, String>,
    #[serde(default)]
    pub views: BTreeMap<String, String>,
    /// Optional pixel crop within every tile, used for compact carriers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact: Option<Crop>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

pub const WHALE_KEY: &str = "codewhale:whale";
pub const GIRL_KEY: &str = "codewhale:whale-girl";
pub const WHALE_VIEWS: [&str; 3] = ["emblem", "cruise", "open"];

/// One picker contract for both built-in renderers and reviewed sprite packs.
/// A contour character does not pretend to have a PNG atlas.
#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    pub key: String,
    pub name: String,
    pub actions: Vec<String>,
    pub views: Vec<String>,
}
pub fn characters(girl: &Pack, registered: &[RegisteredPack]) -> Vec<Character> {
    let sprite = |key: &str, pack: &Pack| Character {
        key: key.into(),
        name: pack.name.clone(),
        actions: pack.actions.keys().cloned().collect(),
        views: pack.views.keys().cloned().collect(),
    };
    let mut out = vec![
        Character {
            key: WHALE_KEY.into(),
            name: "Whale".into(),
            actions: ACTS.iter().map(|s| (*s).into()).collect(),
            views: WHALE_VIEWS.iter().map(|s| (*s).into()).collect(),
        },
        sprite(GIRL_KEY, girl),
    ];
    out.extend(
        registered
            .iter()
            .filter(|p| p.key.starts_with("plugin:"))
            .map(|p| sprite(&p.key, &p.pack)),
    );
    out
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Action {
    pub frames: Vec<u16>,
    pub durations_ms: Vec<u16>,
    pub poster: u16,
    #[serde(default)]
    pub repeat: bool,
    #[serde(default, skip_serializing_if = "Motion::is_none")]
    pub motion: Motion,
}

/// Bounded presentation presets; plugins never supply executable animation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Motion {
    #[default]
    None,
    Breathe,
    Work,
    Hop,
    Sleep,
}
impl Motion {
    fn is_none(&self) -> bool {
        *self == Self::None
    }
    fn sample(self, seconds: f64, within_frame: f64) -> Transform {
        if self == Self::None {
            return Transform::default();
        }
        let (amplitude, period) = match self {
            Self::Sleep => (0.008, 4.8),
            Self::Work => (0.015, 1.3),
            _ => (0.012, 3.2),
        };
        let breath = amplitude * (seconds * std::f64::consts::TAU / period).sin();
        // One arrival, then quiet work; no extra clock and no cross-fade ghosts.
        let entry = if seconds < 0.5 {
            -0.055 * (seconds * std::f64::consts::TAU / 0.32).sin() * (-seconds * 8.).exp()
        } else {
            0.
        };
        let accent = if self == Self::Work && within_frame < 0.12 {
            -0.018 * (within_frame / 0.12 * std::f64::consts::PI).sin()
        } else {
            0.
        };
        let (lift, landing) = if self == Self::Hop && (0.12..0.98).contains(&seconds) {
            let t = ((seconds - 0.12) % 0.43) / 0.43;
            if t < 0.78 {
                let p = t / 0.78;
                (-5. * 4. * p * (1. - p), 0.)
            } else {
                (
                    0.,
                    -0.075 * ((t - 0.78) / 0.22 * std::f64::consts::PI).sin(),
                )
            }
        } else {
            (0., 0.)
        };
        let squash = (breath + entry + accent + landing).clamp(-0.10, 0.08);
        Transform {
            scale_x: 1. - squash * 0.6,
            scale_y: 1. + squash,
            lift,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub scale_x: f64,
    pub scale_y: f64,
    pub lift: f64,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            scale_x: 1.,
            scale_y: 1.,
            lift: 0.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub index: usize,
    pub transform: Transform,
}
impl Frame {
    fn still(index: usize) -> Self {
        Self {
            index,
            transform: Transform::default(),
        }
    }
}

pub fn slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

pub fn relative_asset(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | ':'))
        && value
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != "..")
}

impl Pack {
    pub fn page(&self, frame: usize) -> usize {
        frame / (usize::from(self.columns).max(1) * usize::from(self.rows).max(1))
    }
    pub fn local_frame(&self, frame: usize) -> usize {
        frame % (usize::from(self.columns).max(1) * usize::from(self.rows).max(1))
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_PACK_BYTES {
            return Err("Avatar manifest exceeds 128 KiB".into());
        }
        let pack: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid avatar manifest: {e}"))?;
        pack.validate()?;
        Ok(pack)
    }

    pub fn validate(&self) -> Result<(), String> {
        let count = u32::from(self.columns)
            .saturating_mul(u32::from(self.rows))
            .saturating_mul(self.atlases.len().try_into().unwrap_or(u32::MAX));
        if self.version != 1
            || !slug(&self.id)
            || self.name.trim().is_empty()
            || self.name.len() > 96
            || self.name.chars().any(char::is_control)
            || self.atlases.is_empty()
            || self.atlases.len() > 16
            || self
                .atlases
                .iter()
                .any(|a| !relative_asset(a) || !a.ends_with(".png"))
            || count == 0
            || count > 128
            || self.tile_width == 0
            || self.tile_height == 0
            || u32::from(self.tile_width) * u32::from(self.columns) > 2048
            || u32::from(self.tile_height) * u32::from(self.rows) > 2048
            || self.actions.is_empty()
            || self.actions.len() > 64
            || self.views.len() > 8
            || !self.states.contains_key("rest")
        {
            return Err("Invalid avatar identity, atlas dimensions, or action limits".into());
        }
        if self.compact.is_some_and(|c| {
            c.width == 0
                || c.height == 0
                || u32::from(c.x) + u32::from(c.width) > u32::from(self.tile_width)
                || u32::from(c.y) + u32::from(c.height) > u32::from(self.tile_height)
        }) {
            return Err("Avatar compact crop exceeds tile bounds".into());
        }
        for (id, action) in &self.actions {
            if !slug(id)
                || action.frames.is_empty()
                || action.frames.len() > 64
                || action.frames.len() != action.durations_ms.len()
                || u32::from(action.poster) >= count
                || action.frames.iter().any(|&f| u32::from(f) >= count)
                || action
                    .durations_ms
                    .iter()
                    .any(|&d| !(80..=10_000).contains(&d))
                || action
                    .durations_ms
                    .iter()
                    .map(|&d| u32::from(d))
                    .sum::<u32>()
                    > 30_000
            {
                return Err(format!("Invalid avatar action {id}"));
            }
        }
        for (state, action) in &self.states {
            if !ACTS.contains(&state.as_str()) || !self.actions.contains_key(action) {
                return Err(format!("Invalid avatar state binding {state}"));
            }
        }
        for (view, action) in &self.views {
            if !slug(view) || !self.actions.contains_key(action) {
                return Err(format!("Invalid avatar view binding {view}"));
            }
        }
        Ok(())
    }

    /// Check compressed size and dimensions before any client invokes a codec.
    pub fn validate_png(&self, png: &[u8]) -> Result<(), String> {
        self.validate()?;
        if png.len() < 33
            || png.len() > MAX_PNG_BYTES
            || &png[..8] != b"\x89PNG\r\n\x1a\n"
            || png[8..12] != 13u32.to_be_bytes()
            || &png[12..16] != b"IHDR"
        {
            return Err("Avatar atlas must be a bounded PNG".into());
        }
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        if width != u32::from(self.columns) * u32::from(self.tile_width)
            || height != u32::from(self.rows) * u32::from(self.tile_height)
        {
            return Err("Avatar atlas dimensions do not match the manifest".into());
        }
        Ok(())
    }

    /// One caller-supplied clock; reduced motion chooses the authored poster.
    /// A custom action can be previewed by name without changing Engine state.
    pub fn sample(
        &self,
        state: &str,
        frame: f64,
        reduced: bool,
        view: Option<&str>,
        action: Option<&str>,
    ) -> Frame {
        let named = action
            .filter(|a| self.actions.contains_key(*a))
            .or_else(|| view.and_then(|v| self.views.get(v).map(String::as_str)))
            .or_else(|| self.states.get(state).map(String::as_str))
            .or_else(|| self.states.get("rest").map(String::as_str));
        let Some(clip) = named.and_then(|name| self.actions.get(name)) else {
            return Frame::still(0);
        };
        if reduced || !frame.is_finite() {
            return Frame::still(clip.poster as usize);
        }
        let total: u64 = clip.durations_ms.iter().map(|&d| u64::from(d)).sum();
        if total == 0 {
            return Frame::still(clip.poster as usize);
        }
        let seconds = (frame.max(0.) / 30.).min(31_536_000.);
        let elapsed = (seconds * 1000.) as u64;
        let mut elapsed = if clip.repeat {
            elapsed % total
        } else {
            elapsed.min(total - 1)
        };
        for (&index, &duration) in clip.frames.iter().zip(&clip.durations_ms) {
            if elapsed < u64::from(duration) {
                return Frame {
                    index: index as usize,
                    transform: if view.is_some() {
                        Transform::default()
                    } else {
                        clip.motion.sample(seconds, elapsed as f64 / 1000.)
                    },
                };
            }
            elapsed -= u64::from(duration);
        }
        Frame::still(clip.poster as usize)
    }
}

/// A reviewed Native registration projected by the Engine. No owner token,
/// host path, JavaScript, HTML, external URL or input handler crosses this API.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RegisteredPack {
    pub key: String,
    pub handle: u64,
    pub content_hash: String,
    pub pack: Pack,
}

impl RegisteredPack {
    pub fn image_key(&self, page: usize) -> String {
        format!("{}:{}:{}:{page}", self.key, self.content_hash, self.handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example() -> Pack {
        Pack::parse(br#"{"version":1,"id":"studio","name":"Studio avatar","atlases":["art/atlas.png"],"columns":2,"rows":1,"tileWidth":8,"tileHeight":8,"actions":{"wave":{"frames":[0,1],"durationsMs":[100,200],"poster":1,"repeat":true}},"states":{"rest":"wave","done":"wave"},"views":{"side":"wave"}}"#).unwrap()
    }
    #[test]
    fn motion_is_optional_bounded_and_freezes_with_reduced_motion() {
        let mut pack = example();
        let baseline = pack.sample("rest", 24., false, None, None);
        assert_eq!(baseline.transform, Transform::default());
        pack.actions.get_mut("wave").unwrap().motion = Motion::Breathe;
        let t = pack.sample("rest", 24., false, None, None).transform;
        assert!((t.scale_y - 1.012).abs() < 1e-10);
        assert!((t.scale_x - 0.9928).abs() < 1e-10);
        assert_eq!(
            pack.sample("rest", 24., true, None, None).transform,
            Transform::default()
        );
        assert_eq!(
            pack.sample("rest", f64::NAN, false, None, None).transform,
            Transform::default()
        );
        assert_eq!(
            pack.sample("rest", 24., false, Some("side"), None)
                .transform,
            Transform::default()
        );
        for motion in [Motion::Breathe, Motion::Work, Motion::Hop, Motion::Sleep] {
            pack.actions.get_mut("wave").unwrap().motion = motion;
            for frame in (0..3000).map(|n| n as f64 / 10.).chain([f64::MAX]) {
                let t = pack.sample("rest", frame, false, None, None).transform;
                assert!((0.90..=1.08).contains(&t.scale_y));
                assert!((0.90..=1.08).contains(&t.scale_x));
                assert!((-5.0..=0.0).contains(&t.lift));
                assert!(!(t.scale_x > 1. && t.scale_y > 1.));
            }
        }
        let mut json = serde_json::to_value(&pack).unwrap();
        json["actions"]["wave"]["motion"] = "javascript".into();
        assert!(Pack::parse(&serde_json::to_vec(&json).unwrap()).is_err());
    }

    #[test]
    fn catalog_keeps_builtins_and_custom_actions_together() {
        let pack = example();
        let registered = RegisteredPack {
            key: "plugin:studio:studio".into(),
            handle: 3,
            content_hash: "hash".into(),
            pack: pack.clone(),
        };
        let choices = characters(&pack, &[registered]);
        assert_eq!(
            choices.iter().map(|p| p.key.as_str()).collect::<Vec<_>>(),
            [WHALE_KEY, GIRL_KEY, "plugin:studio:studio"]
        );
        assert_eq!(choices[0].actions, ACTS);
        assert_eq!(choices[0].views, WHALE_VIEWS);
        assert_eq!(choices[2].actions, ["wave"]);
        assert_eq!(choices[2].views, ["side"]);
    }
    #[test]
    fn compact_crop_is_optional_and_confined_to_each_tile() {
        let mut pack = example();
        assert_eq!(pack.compact, None);
        pack.compact = Some(Crop {
            x: 2,
            y: 1,
            width: 6,
            height: 7,
        });
        assert!(pack.validate().is_ok());
        for crop in [
            Crop {
                x: 3,
                y: 1,
                width: 6,
                height: 7,
            },
            Crop {
                x: 0,
                y: 0,
                width: 0,
                height: 1,
            },
            Crop {
                x: u16::MAX,
                y: 0,
                width: 1,
                height: 1,
            },
        ] {
            pack.compact = Some(crop);
            assert!(pack.validate().is_err());
        }
    }
    #[test]
    fn avatar_named_actions_share_clock_and_reduced_posters() {
        let p = example();
        assert_eq!(p.sample("rest", 0., false, None, None).index, 0);
        assert_eq!(p.sample("done", 3., false, None, None).index, 1);
        assert_eq!(p.sample("unknown", 9., false, None, None).index, 0);
        for time in [0., f64::INFINITY, f64::NAN, 5000.] {
            assert_eq!(p.sample("rest", time, true, None, None).index, 1);
        }
        assert_eq!(
            p.sample("rest", 0., false, Some("side"), Some("missing"))
                .index,
            0
        );
        let mut p = p;
        p.actions.get_mut("wave").unwrap().repeat = false;
        assert_eq!(p.sample("rest", 10000., false, None, Some("wave")).index, 1);
    }
    #[test]
    fn avatar_limits_and_references_are_validated_before_rendering() {
        for path in [
            "/absolute.png",
            "../outside.png",
            "a/../b.png",
            "https://a.png",
            "a\\b.png",
            "a//b.png",
            "a/./b.png",
        ] {
            let mut p = example();
            p.atlases = vec![path.into()];
            assert!(p.validate().is_err(), "{path}");
        }
        let mut p = example();
        p.actions.get_mut("wave").unwrap().frames[0] = 2;
        assert!(p.validate().is_err());
        let mut p = example();
        p.actions.get_mut("wave").unwrap().durations_ms[0] = 0;
        assert!(p.validate().is_err());
        let mut p = example();
        p.states.insert("made-up-owner-state".into(), "wave".into());
        assert!(p.validate().is_err());
        let mut p = example();
        p.tile_width = 2049;
        assert!(p.validate().is_err());
        let mut p = example();
        p.columns = u16::MAX;
        p.rows = u16::MAX;
        p.atlases.push("another.png".into());
        assert!(p.validate().is_err());
        let mut p = example();
        p.actions.clear();
        assert_eq!(p.sample("rest", 3., false, None, None).index, 0);
    }
    #[test]
    fn avatar_image_dimensions_are_checked_without_invoking_a_codec() {
        let p = example();
        let mut header = vec![0u8; 33];
        header[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        header[8..12].copy_from_slice(&13u32.to_be_bytes());
        header[12..16].copy_from_slice(b"IHDR");
        header[16..20].copy_from_slice(&16u32.to_be_bytes());
        header[20..24].copy_from_slice(&8u32.to_be_bytes());
        assert!(p.validate_png(&header).is_ok());
        header[20..24].copy_from_slice(&4096u32.to_be_bytes());
        assert!(p.validate_png(&header).is_err());
        assert!(p.validate_png(&[]).is_err());
    }
}
