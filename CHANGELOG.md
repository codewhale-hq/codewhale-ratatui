# Changelog

## Unreleased

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
