# Composable whale avatars

`avatar::Pack` is Codewhale's shared, declarative avatar pack v1. It maps
existing whale activity states and named views to timed clips. Pass the
Director's existing 30Hz frame value into `pack.sample(...)`; the returned
index selects a frame, with reduced motion always using the authored poster.
There is no plugin loader, activity classifier or extra clock in the renderer.

`whale_girl` includes 36 frames: two for each of the 17 existing states and a
side/back view. The hair remains continuous into two flukes at each lock's tip.
Native PNG pages retain detailed art; `whale_girl::TERMINAL` is a predecoded
96px derivative for terminal cells.
The Rust package includes the portable player, manifest and terminal frames.
Full-resolution PNG pages remain in the repository and native app; the terminal
renderer does not use them. Custom local packs still load their own PNG pages.

`avatar_sprite::Sprite::new` paints an entire sequence of row-major RGBA tiles;
`Sprite::page` paints a single page using a global frame index. Constructors
check the complete byte geometry and bounds. Paint fits the available rectangle,
preserves aspect ratio, clips to the buffer and respects `NO_COLOR`, ASCII-safe,
ANSI16, ANSI256 and truecolor capabilities.

Run `cargo run --example avatar` for the built-in character. Left/right cycle
actions, V cycles views, Space toggles motion, Q exits. `--pack path/avatar.json`
previews your own local pack without installing or executing a plugin.
`--frames output-dir` exports actual terminal buffers for all actions/views in
four capability profiles. The showcase example also supports F11 to switch
between the whale and Whale girl.

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

## Prepared dot whale

`DotWhale::new(points, materials)` paints a prepared point field supplied by the
shared pet owner. The canonical character has 980 points, each paired with a
final `[r, g, b, alpha]` material. The painter accepts one to 980 finite points
inside `[-1, 1]`, matching material counts, RGB channels in `0..=255`, and alpha
in `0..=1`. It fits that fixed envelope into terminal braille cells and adapts
ink to the existing theme. It has no simulation, activity classifier or clock.

Supply an honest `.caption(...)` and optional `.action_id(...)`; the default is
`Activity unobserved`. Captions remain available in ASCII, narrow layouts and
when point data cannot be painted. The caller owns source/freshness validation
and selects the owner's `still` pose for reduced motion; stopping redraws of a
moving pose does not select Still.

The example reads a prepared owner JSON capture and reloads changed file data:

```sh
cargo run --example dot_whale -- --frame /path/to/owner-frame.json
cargo run --example dot_whale -- --frame /path/to/owner-frame.json --still --svg whale-still.svg
```

Space toggles Still; Q closes. If the capture has no `still` pose, the example
says `Still frame unavailable`. Its caption uses supplied typed activity and
freshness metadata. A captured or synthetic preview is not a live Engine
session, and this example does not connect to the owner transport itself.
