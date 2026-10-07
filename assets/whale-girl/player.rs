//! Built-in art adapter over the shared avatar-pack contract.
//! No separate action vocabulary or animation clock lives here.
use crate::avatar::{Frame, Pack};
use std::sync::OnceLock;
pub const TERMINAL: &[u8] = include_bytes!("terminal.rgba");
pub const TILE: usize = 96;
pub const FRAMES: usize = 64;
pub fn pack() -> &'static Pack {
    static PACK: OnceLock<Pack> = OnceLock::new();
    PACK.get_or_init(|| {
        Pack::parse(include_bytes!("avatar.json")).expect("validated built-in avatar")
    })
}
pub fn sample(
    act: &str,
    frame: f64,
    reduced: bool,
    view: Option<&str>,
    action: Option<&str>,
) -> Frame {
    pack().sample(act, frame, reduced, view, action)
}
pub fn pixel(index: usize, x: usize, y: usize) -> [u8; 4] {
    if index >= FRAMES || x >= TILE || y >= TILE {
        return [0; 4];
    }
    let at = ((index * TILE + y) * TILE + x) * 4;
    TERMINAL[at..at + 4].try_into().expect("four channels")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_poses_keep_headroom_through_motion_and_gaze() {
        // Inspect actual alpha coverage, not the transparent tile rectangle.
        // This catches a complete drawing being cropped during a hop/stretch.
        let bounds: Vec<_> = (0..FRAMES)
            .map(|frame| {
                let (mut left, mut top, mut right, mut bottom) = (TILE, TILE, 0, 0);
                for y in 0..TILE {
                    for x in 0..TILE {
                        if pixel(frame, x, y)[3] >= 40 {
                            left = left.min(x);
                            top = top.min(y);
                            right = right.max(x + 1);
                            bottom = bottom.max(y + 1);
                        }
                    }
                }
                let at = |n| n as f64 / TILE as f64 * 124. - 62.;
                (at(left), at(top), at(right), at(bottom))
            })
            .collect();
        for (action, clip) in &pack().actions {
            let duration: u32 = clip.durations_ms.iter().map(|&d| u32::from(d)).sum();
            for tick in 0..duration * 60 / 1000 + 1 {
                let frame = sample("rest", f64::from(tick) / 2., false, None, Some(action));
                let (l, t, r, b) = bounds[frame.index];
                let motion = frame.transform;
                let y = |v| 62. + (v - 62.) * motion.scale_y + motion.lift;
                // Shared carriers may add at most three design units of gaze.
                assert!(
                    l * motion.scale_x - 3. >= -62.
                        && r * motion.scale_x + 3. <= 62.
                        && y(t) - 3. >= -62.
                        && y(b) + 3. <= 62.,
                    "{action} clips frame {} at tick {tick}",
                    frame.index
                );
            }
        }
    }

    #[test]
    fn girl_has_distinct_frames_for_all_native_actions_and_authored_views() {
        assert_eq!(TERMINAL.len(), FRAMES * TILE * TILE * 4);
        for act in crate::avatar::ACTS {
            let clip = &pack().actions[&pack().states[act]];
            assert!(
                clip.frames
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    >= 3,
                "{act} has at least three authored poses"
            );
            let tile = |index: u16| {
                &TERMINAL[index as usize * TILE * TILE * 4..(index as usize + 1) * TILE * TILE * 4]
            };
            assert_ne!(
                tile(clip.frames[0]),
                tile(clip.frames[1]),
                "{act} poses differ"
            );
            for index in &clip.frames {
                let pixels = tile(*index);
                assert!(
                    pixels.as_chunks::<4>().0.iter().any(|p| p[3] > 200),
                    "{act} paints"
                );
                assert!(
                    pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 0),
                    "{act} is transparent"
                );
            }
            assert_eq!(
                sample(act, 10000., true, None, None).index,
                clip.poster as usize
            );
        }
        assert_ne!(
            sample("rest", 0., false, Some("side"), None),
            sample("rest", 0., false, Some("back"), None)
        );
    }
}
