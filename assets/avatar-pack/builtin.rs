//! The sprite characters built into this crate, as packs of the one avatar
//! system. The shared contract (`contract.rs`) names one built-in sprite pack
//! and has no field for pixel art, and it must stay identical in every host
//! that vendors it. So the list of built-ins, and which of them are hand-drawn
//! pixel art, live here beside it.
use crate::avatar::{Character, Crop, Pack, RegisteredPack};
use crate::avatar_sprite::Sprite;
use std::sync::OnceLock;

/// One built-in sprite character and its predecoded terminal tiles.
pub struct Builtin {
    /// The picker key, `codewhale:<pack id>`.
    pub key: &'static str,
    /// Row-major RGBA tiles in manifest order, `width` by `height` each.
    pub pixels: &'static [u8],
    pub width: usize,
    pub height: usize,
    /// Hand-drawn pixel art: one terminal pixel is one art cell, and it is
    /// painted cell for cell or at a whole-number reduction, never smoothed.
    pub pixel_art: bool,
    /// The rectangle of a terminal tile the drawings use. A pixel character
    /// is fitted by its art, not by the transparent margin of its tile.
    pub art: Option<Crop>,
    pack: fn() -> &'static Pack,
}

macro_rules! pack {
    ($name:ident, $file:literal) => {
        fn $name() -> &'static Pack {
            static PACK: OnceLock<Pack> = OnceLock::new();
            PACK.get_or_init(|| {
                Pack::parse(include_bytes!($file)).expect("validated built-in avatar")
            })
        }
    };
}
pack!(pixel_whale, "../pixel-whale/avatar.json");
pack!(pixel_whale_girl, "../pixel-whale-girl/avatar.json");

static ALL: [Builtin; 3] = [
    Builtin {
        key: crate::avatar::GIRL_KEY,
        pixels: crate::whale_girl::TERMINAL,
        width: crate::whale_girl::TILE,
        height: crate::whale_girl::TILE,
        pixel_art: false,
        art: None,
        pack: crate::whale_girl::pack,
    },
    // 26 by 22 cells standing on the floor of a 32-cell tile.
    Builtin {
        key: "codewhale:pixel-whale",
        pixels: include_bytes!("../pixel-whale/terminal.rgba"),
        width: 32,
        height: 32,
        pixel_art: true,
        art: Some(Crop {
            x: 3,
            y: 10,
            width: 26,
            height: 22,
        }),
        pack: pixel_whale,
    },
    // 39 by 52 cells standing on the floor of a 56-cell tile.
    Builtin {
        key: "codewhale:pixel-whale-girl",
        pixels: include_bytes!("../pixel-whale-girl/terminal.rgba"),
        width: 56,
        height: 56,
        pixel_art: true,
        art: Some(Crop {
            x: 8,
            y: 4,
            width: 39,
            height: 52,
        }),
        pack: pixel_whale_girl,
    },
];

/// Every built-in sprite character, in picker order. The contour whale is
/// not a sprite pack and is not listed; it stays first in [`characters`].
pub fn all() -> &'static [Builtin] {
    &ALL
}

/// A built-in by picker key (`codewhale:pixel-whale`) or pack id
/// (`pixel-whale`).
pub fn find(name: &str) -> Option<&'static Builtin> {
    ALL.iter().find(|b| b.key == name || b.id() == name)
}

/// The picker list: the contour whale, every built-in sprite character, then
/// reviewed plugin packs.
pub fn characters(registered: &[RegisteredPack]) -> Vec<Character> {
    let mut out = crate::avatar::characters(ALL[0].pack(), registered);
    for (at, builtin) in ALL.iter().enumerate().skip(1) {
        let pack = builtin.pack();
        out.insert(
            at + 1,
            Character {
                key: builtin.key.into(),
                name: pack.name.clone(),
                actions: pack.actions.keys().cloned().collect(),
                views: pack.views.keys().cloned().collect(),
            },
        );
    }
    out
}

impl Builtin {
    pub fn pack(&self) -> &'static Pack {
        (self.pack)()
    }
    /// The pack id, which is also the name a person types.
    pub fn id(&self) -> &'static str {
        self.key.strip_prefix("codewhale:").unwrap_or(self.key)
    }
    /// One global frame, ready to paint: pixel art keeps its cell grid and
    /// is fitted by its drawing.
    pub fn sprite(&self, frame: usize) -> Result<Sprite<'static>, String> {
        let sprite = Sprite::new(self.pack(), self.pixels, self.width, self.height, frame)?;
        let sprite = self.art.map_or(sprite, |art| sprite.crop(art));
        Ok(if self.pixel_art {
            sprite.pixel_art()
        } else {
            sprite
        })
    }
    /// The same frame through the pack's `compact` crop, for a carrier too
    /// small for the whole character. `None` when the pack declares none.
    pub fn compact(&self, frame: usize) -> Option<Sprite<'static>> {
        let pack = self.pack();
        let crop = pack.compact?;
        // The crop is in page pixels; terminal tiles are smaller by a whole
        // number for pixel art and by a fraction for painted art.
        let scale = |value: u16, tile: u16, side: usize| {
            (usize::from(value) * side / usize::from(tile).max(1)) as u16
        };
        let sprite = Sprite::new(pack, self.pixels, self.width, self.height, frame).ok()?;
        let sprite = sprite.crop(Crop {
            x: scale(crop.x, pack.tile_width, self.width),
            y: scale(crop.y, pack.tile_height, self.height),
            width: scale(crop.width, pack.tile_width, self.width),
            height: scale(crop.height, pack.tile_height, self.height),
        });
        Some(if self.pixel_art {
            sprite.pixel_art()
        } else {
            sprite
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avatar::{ACTS, GIRL_KEY, WHALE_KEY};

    #[test]
    fn every_builtin_is_a_valid_pack_with_whole_terminal_tiles() {
        for builtin in all() {
            let pack = builtin.pack();
            pack.validate().unwrap();
            assert_eq!(builtin.key, format!("codewhale:{}", pack.id));
            let frames = usize::from(pack.columns) * usize::from(pack.rows) * pack.atlases.len();
            assert_eq!(
                builtin.pixels.len(),
                frames * builtin.width * builtin.height * 4,
                "{}",
                builtin.key
            );
            for act in ACTS {
                assert!(pack.states.contains_key(act), "{} {act}", builtin.key);
            }
            for frame in 0..frames {
                assert!(builtin.sprite(frame).is_ok());
            }
            assert!(builtin.sprite(frames).is_err());
            assert_eq!(find(builtin.key).map(|b| b.key), Some(builtin.key));
            assert_eq!(find(builtin.id()).map(|b| b.key), Some(builtin.key));
        }
        assert!(find("whale").is_none());
    }

    #[test]
    fn pixel_art_tiles_are_whole_cells_and_stay_inside_their_art_bounds() {
        for builtin in all().iter().filter(|b| b.pixel_art) {
            let pack = builtin.pack();
            // A page tile is a whole number of art cells, the same both ways.
            assert_eq!(usize::from(pack.tile_width) % builtin.width, 0);
            assert_eq!(
                usize::from(pack.tile_width) / builtin.width,
                usize::from(pack.tile_height) / builtin.height
            );
            // Motion presets move art by parts of a cell; pixel packs have none.
            assert!(
                pack.actions
                    .values()
                    .all(|a| a.motion == crate::avatar::Motion::None)
            );
            let art = builtin.art.expect("pixel art declares its bounds");
            let (mut left, mut top, mut right, mut bottom) = (usize::MAX, usize::MAX, 0, 0);
            for (at, pixel) in builtin.pixels.as_chunks::<4>().0.iter().enumerate() {
                assert!(matches!(pixel[3], 0 | 255), "no soft edges");
                if pixel[3] == 255 {
                    let (x, y) = (at % builtin.width, at / builtin.width % builtin.height);
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x + 1);
                    bottom = bottom.max(y + 1);
                }
            }
            assert_eq!(
                (left, top, right - left, bottom - top),
                (
                    usize::from(art.x),
                    usize::from(art.y),
                    usize::from(art.width),
                    usize::from(art.height)
                ),
                "{}",
                builtin.key
            );
        }
    }

    #[test]
    fn the_picker_lists_the_whale_then_builtins_then_plugins() {
        let registered = RegisteredPack {
            key: "plugin:studio:studio".into(),
            handle: 3,
            content_hash: "hash".into(),
            pack: pixel_whale().clone(),
        };
        let choices = characters(&[registered]);
        assert_eq!(
            choices.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
            [
                WHALE_KEY,
                GIRL_KEY,
                "codewhale:pixel-whale",
                "codewhale:pixel-whale-girl",
                "plugin:studio:studio"
            ]
        );
        assert_eq!(choices[2].name, "Pixel whale");
        assert_eq!(choices[3].name, "Pixel whale girl");
        // A pack with only a front view is a whole character.
        assert_eq!(choices[2].views, ["front"]);
    }

    #[test]
    fn a_compact_crop_maps_from_page_pixels_to_terminal_pixels() {
        let girl = find("pixel-whale-girl").unwrap();
        let full = girl.sprite(2).unwrap();
        let compact = girl.compact(2).unwrap();
        let area = ratatui::layout::Rect::new(0, 0, 36, 18);
        assert_eq!(compact.reduction(area), 1);
        assert_eq!(full.reduction(area), 2);
        assert!(find("pixel-whale").unwrap().compact(2).is_none());
        // The painted girl's crop survives the fractional mapping.
        assert!(find("whale-girl").unwrap().compact(0).is_some());
    }
}
