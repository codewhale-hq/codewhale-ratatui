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

Both built-ins share the preview carrier: `--character whale` selects the
original contour whale; Tab changes character interactively. Custom pack
clips appear alongside the built-in activity choices.

Clips may opt into `motion`: `breathe`, `work`, `hop`, or `sleep` (`none` is
the default). These bounded presets sample the existing clock and deform the
sprite about its feet. Reduced motion and named views use an identity
transform. Terminal precision depends on the cell grid. The optional
`compact` rectangle declares face framing for small native/browser carriers.
Older hosts without these fields reject them; update the shared contract in
all consumers together.
