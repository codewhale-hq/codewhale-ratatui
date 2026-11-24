# Changelog

## Unreleased

- Mirrors the Engine's vendored 0.2.1 adaptations. The posture bar keeps the turn clock while a turn runs on a narrow row: its shed ladder drops the session clock, permission key, hint and counts first (cap warning and mode still outrank the clock), and the right notice is clipped against the permission chip plus the turn clock instead of covering it. `NativeComposer` exposes its paint, uses the full available height without a menu, and bounds wrapping for long non-ASCII drafts. `WorkflowRun.total` and the total argument to `WorkflowRun::outcomes` use `u64`. The workbar has a Terminal panel and a nine-item order. `motion` adds the activity sweep (`ACTIVITY_SWEEP_*`, `activity_sweep_strength`) driven by the 5.4.0 motion tokens. The package version is 0.2.1.

- The habitat now implements the whole Codewhale ocean for the Engine's Underwater field. New `Shoal` (a far shoal of minnows), `MarineSnow` (single braille motes drifting down) and `DistantWhale` (a small whale crossing the deep every few minutes, first about twelve seconds in) join `FishSchool`, `Jellyfish` and `BubbleField`, with `Habitat::shoal`, `snow` and `whale` switches. Every creature now lifts a low, quantised glow out of the water it sits on (`ink` and `presence` builders on each part) on truecolor themes with known grounds, so a frame changes only the cells a creature moves through; the school is slower with tail poses that step with travel, there are six fish at most and one to three bubble streams, the jellyfish pulses in eight poses and visits about a third of the time, it stays away while the whale passes unless held open, and every creature keeps one row and two columns clear of text (including wide graphemes). Adds `color::glow_over`. README boards, the habitat animation and the studio animations are regenerated.

- Upgrade to Codewhale design tokens 5.4.0 (CURRENT_DECISIONS §226, §228.5): add `Role::Subagent` and `Role::LivePeak` (`Role::COUNT` 19 to 21), resolve the light 256-color `Hover`/`Background` index-255 collision, make `Role::ansi16(appearance)` appearance-aware (`LightBlue`/`LightCyan`/`LightYellow`/`LightRed` on Dark, `Blue`/`Cyan`/`Yellow`/`Red` on Light/Unknown), and update `paint_working_glow` to use the `5.4.0` raised-cosine sweep tokens (`9`-cell half-width, `0.78` strength, `1800ms` active / `600ms` rest).

- The Ocean column's waiting, approval and failure tints now ease off near the
  surface where they would push `Hint` below 4.5:1 or `Dim` below 3:1; deeper
  rows keep the full warm tint. The README boards and animations are
  regenerated for the current tokens.
- Data displays stay honest at the edges. `data_number` no longer prints a
  small value as `0` or `-0` (`0.0042`, `4.2e-7`) and rounds across a unit
  (`999950` is `1M`, with a `T` tier). A `DataTrend` with more points than
  dot columns draws each column from its lowest to its highest value, so a
  spike is never dropped. A `DataBars` value above zero always draws at least
  an eighth of a cell, and a narrow row cuts the label before the value.
- Add data displays: `DataSparkline`, `DataBars` and `DataTrend` (a braille
  line chart). The words carry every value; the ink draws in from a caller
  progress (`MotionStep::drawn`) and is at its end state under reduced or
  still motion. Bars glide from their previous values.
- Add `DataDisplay`, the same displays as JSON an agent can send:
  `DataDisplay::from_json` validates it (finite numbers, at most 1024 points
  or 24 bars, labels up to 120 characters), `describe` says the same facts as
  plain text, and `DATA_DISPLAY_SCHEMA` is its tool schema. Try
  `cargo run --example data`.
- Move to Codewhale design tokens 2.3.0. `Primary` is now the neutral ink of
  filled actions and switches; a new `Accent` role carries the one quiet blue
  for links, focus and selection (what 1.x called `primary`), so components
  that drew links or focus in `Primary` now use `Accent`. The diff tints come
  from the tokens' `diff_add` / `diff_remove`; `ocean_depth` has no role.
  `Role::COUNT` is 19. The 16 named TUI palettes give `Primary` and `Accent`
  the same blue, so a host on a named palette sees no colour change.
  `ansi16` maps `Accent` to `Blue`. Snapshots, README boards and animations
  are regenerated.
- Keep `DotWhale` dense in large panes. Its scale is capped at 32 braille dots
  (about four lit dots per cell, as at 80x18), and the whale and caption are
  centred together. Each cell takes the colour and alpha of its strongest dot
  instead of an alpha-weighted mean. Hollow (unobserved) frames use
  the theme's `Muted` ink instead of `DIM`.
- `Sprite` scales by one integer factor (n, or 1/n) on both axes, with the
  same rounding for width and rows. Every source pixel stays whole. A sprite
  that is slightly too large for its area now shrinks by a whole factor.
- `PetMode::style` and `PetMode::cove` reach the `WhalePet` fallback, so a
  host can force the Braille whale or drop the translucent cove. A forced
  Braille whale is scheduled at the Terminal cadence.
- Known limits: `DotWhale` does not grow past the cap, so a large pane shows a
  whale of the same size with more space around it. Both painters still assume
  a 1:2 cell aspect. On profiles without colour, `Muted` falls back to `DIM`.
- Add private vulnerability reporting guidance and contributor setup/check
  instructions; include both guides in the source package.
- Feature the animated whale and full pet view in the README and catalogue,
  with five gallery entries and the current dark/light activity previews.
- Simplify the native showcase: concise conversation, sparse ambient life,
  muted metrics and optional workflow progress, with the original workbar
  given room for its rows.
- Present a still native preview first and organize every dark/light component
  preview into named collections, with animations available on demand.
- Reduce gallery framing and use native terminal palette colors around the
  unchanged Ratatui buffers.

- Render themed components by reference, and use `Themed::new` with a
  heterogeneous collection of `dyn Paint` components.
- Render lists and pickers with Ratatui's `StatefulWidget`, persisting the
  viewport's scroll offset in application-owned state.
- Keep backend selection with the consuming application; interactive examples
  enable Crossterm separately from the library dependency.
- Add a small native starter app, a standalone consumer check, rendering
  benchmarks, public documentation/package gates and Windows test coverage.
- Fix incremental Unicode editing and grapheme cursor boundaries after
  deletion.
- Keep modified navigation/activation keys with the host and prevent held-key
  repeats from submitting, choosing, cancelling or toggling input controls.
- Reuse contrast calculations within native ocean text runs while preserving
  each cell's original color and contrast policy.
- Enable the previously exempt whale bounds check now that its renderer clips.
- Limit source packages to library sources, examples, documentation and
  required native assets.

## 0.1.0 development baseline

- Native Codewhale workbar, composer, footer, conversation, instrument surfaces
  and session list; 16 source palettes and the Underwater ocean column.
- Input, navigation, decisions, results, motion and marine-life components,
  plus optional workspace compositions and additional ombrés.
- Source view guide, 195-entry gallery across nine terminal profiles, and
  generated dark/light README previews and animations.

This version has not been published to a package registry.
