# Composable whale avatars

`avatar::Pack` is Codewhale's shared, declarative avatar pack v1. It maps
existing whale activity states and named views to timed clips. Pass the
Director's existing 30Hz frame value into `pack.sample(...)`; the returned
index selects a frame, with reduced motion always using the authored poster.
There is no plugin loader, activity classifier or extra clock in the renderer.

`whale_girl` includes 64 active, cel-shaded chibi poses: all 17 existing states,
front/side/back views and a custom wave action. The hair remains continuous into two flukes at each lock's tip.
Native PNG pages retain detailed art; `whale_girl::TERMINAL` is a predecoded
96px derivative for terminal cells.
The Rust package includes the portable player, manifest and terminal frames.
Full-resolution PNG pages remain in the repository and native app; the terminal
renderer does not use them. Custom local packs still load their own PNG pages.

## Built-in characters

Codewhale has one pet system. Every character other than the original whale
is a pack of it, and `avatar_builtin::all()` lists the ones this crate ships:

| `--character` | Key | Art | Terminal tile | Drawn |
|---|---|---|---|---|
| `whale` | `codewhale:whale` | The Braille contour whale (`Whale`, `whale_motion`). Not a pack. | none | 17 states, three views |
| `whale-girl` | `codewhale:whale-girl` | Painted, 768px pages | 96 by 96, smoothed | 64 poses, front/side/back |
| `pixel-whale` | `codewhale:pixel-whale` | Pixel art, 8 page pixels per cell | 32 by 32, one pixel per cell | 8 actions, front only |
| `pixel-whale-girl` | `codewhale:pixel-whale-girl` | Pixel art, 4 page pixels per cell | 56 by 56, one pixel per cell | 8 actions, front only |

The Braille contour whale remains the default terminal whale. The pixel whale
and pixel whale girl are choices beside it, not replacements, and nothing
selects them unless the host or the person does.

`avatar_builtin::find("pixel-whale")` takes a key or an id.
`builtin.sprite(frame)` returns a `Sprite` ready to paint and
`avatar_builtin::characters(&registered)` is the picker list: the whale, the
built-ins, then reviewed plugin packs. The shared contract file
(`assets/avatar-pack/contract.rs`) names one built-in sprite pack and has no
pixel-art field; it is vendored byte for byte by the app and the Engine, so
the list and the pixel-art flag live beside it in `builtin.rs` until all three
can change together.

The two pixel characters are drawn by hand as text grids, one character per
cell. Their pages bake the navy theme's colors, so they do not follow the
terminal theme the way the contour whale does. Eight actions are drawn (`rest`,
`think`, `read`, `write`, `run`, `needs`, `done`, `sleep`); only `think` and
`done` have more than one frame, and the other nine states borrow the nearest
drawing. They have a `front` view only, which is a valid pack. They declare
no `motion`.

### Pixel art stays on its grid

`Sprite::pixel_art()` never resamples at a fraction. One source pixel becomes
one half-block pixel, so a terminal cell shows two art cells, upper and lower:

- When the area holds the art, it is painted cell for cell and centred. It is
  not enlarged.
- When it does not, every `Sprite::reduction(area)`th pixel is kept (2, 3, ...):
  the largest whole-number size that fits. Columns stay even, detail is lost.
- An empty area paints nothing.

`Sprite::crop` paints one rectangle of a tile. Built-in pixel characters use
it to fit by their drawing instead of the transparent margin of the tile
(26 by 22 cells for the whale, 39 by 52 for the girl: 11 and 26 terminal
rows). `builtin.compact(frame)` applies the pack's `compact` crop the same
way; the pixel whale girl declares her head, 36 by 36 cells, for a carrier
too small for all of her. `examples/avatar.rs` switches to it when only the
crop stays cell for cell.

Without color (`NO_COLOR`, 16 colors, an unmeasured ground, ASCII-safe) every
sprite, pixel art included, falls back to one ink and the density letters
`.`, `:`, `+`, `#`, one per terminal cell. That is half the vertical detail
and no hand-authored fill map.

`tools/pixel-avatar.py <pack directory> --cell N` writes `terminal.rgba` and
`manifest.json` from the pages and refuses a page that is not flat color on
that grid; `--check` fails when either is stale. `tests/pixel_avatar.rs`
checks the hashes, compares the tiles with the page pixel by pixel, and
compares the painted buffer with the poses as written in the grid.

## Painting

`avatar_sprite::Sprite::new` paints an entire sequence of row-major RGBA tiles;
`Sprite::page` paints a single page using a global frame index. Constructors
check the complete byte geometry and bounds. Paint fits the available rectangle,
preserves aspect ratio, clips to the buffer and respects `NO_COLOR`, ASCII-safe,
ANSI16, ANSI256 and truecolor capabilities.

Run `cargo run --example avatar` for the built-in characters, or
`cargo run --example avatar -- --character pixel-whale` to open on one. Tab
changes character, Left/Right cycle actions, V cycles views, Space toggles
motion, Q exits. `--pack path/avatar.json` previews your own local pack
without installing or executing a plugin; add `--pixel-cell N` when it is
pixel art drawn with N page pixels per cell, so it is read on its own grid
instead of being smoothed to 96px. `--frames output-dir` exports actual
terminal buffers for the chosen character's actions in four capability
profiles. The showcase example also supports F11 to step through the whale
and each built-in character.

A minimal pack:

```json
{
  "version": 1, "id": "my-whale", "name": "My whale",
  "atlases": ["page-00.png"], "columns": 2, "rows": 1,
  "tileWidth": 256, "tileHeight": 256,
  "actions": {"wave": {"frames": [0,1], "durationsMs": [700,300], "poster": 0, "repeat": true}},
  "states": {"rest": "wave", "done": "wave"},
  "views": {"front": "wave"}
}
```

Pages share grid dimensions. Global indices walk page order, then row-major
cell order. `rest` is required; unknown activity falls back to it. Valid states:
rest, listen, think, busy, read, search, write, run, browse, talk, pod, needs,
done, hmm, computer, connect, sleep. Custom actions have their own names; clients
can preview those clips without changing Engine truth.

Bounds are enforced before decoding: 128KiB manifest, 16 pages, 128 total
frames, 64 actions, eight views, 2048px per page axis, 4MiB PNG, 80–10,000ms per
frame and 30s total per action. Decode PNGs once, split into tiles, and preserve
alpha. Hosts must separately validate provenance and registration lifetime.

In Codewhale's newest Native plugin system, expose an `avatars` Cordis service
injection and call `ctx.avatars.registerPack({path:'avatars/avatar.json'})`.
The Engine admits only art within the already reviewed Native bundle, binds
it to the live owner/scope and withdraws it on disable/revoke/failure. Ratatui
only receives reviewed presentation data. In the Engine TUI use `/pet avatar`,
`/pet avatar whale-girl`, `/pet action read`, `/pet view back`, and
`/pet action live`. Selection and preview are session-local in the TUI.

## Start from one sheet

```sh
cargo run --example create_avatar -- --sheet art.png --out my-avatar \
  --id my-avatar --name 'My avatar' --columns 2 --rows 2 --actions rest,wave
cargo run --example avatar -- --pack my-avatar/avatars/avatar.json
```

Rows name actions, columns hold frames. The new directory contains a Native
plugin manifest, a Cordis `ctx.avatars.registerPack` entry, art and bindings.
All 17 activity states get a valid fallback. Review/install uses the existing
Native plugin workflow; preview does not grant authority. The generator
requires a new output directory and never overwrites existing work.

Every character shares the preview carrier: `--character whale` selects the
original contour whale; Tab changes character interactively. With `--pack`
the carrier holds the whale and that pack.

Clips may opt into `motion`: `breathe`, `work`, `hop`, or `sleep` (`none` is
the default). These bounded presets sample the existing clock and deform the
sprite about its feet. Reduced motion and named views use an identity
transform. Terminal precision depends on the cell grid. The optional
`compact` rectangle declares face framing for small native/browser carriers.
Older hosts without these fields reject them; update the shared contract in
all consumers together.
