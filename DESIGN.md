# Codewhale terminal components

The current Codewhale TUI is the source for native components, layout,
backgrounds and ombré. The Ratatui library develops their terminal presentation;
keep application data, actions and persistence in the host. Optional
compositions can explore useful alternatives without changing the native
baseline. [VIEWS.md](VIEWS.md) inventories the terminal screens.

## The water

`OceanColumn` adapts the existing Underwater treatment from
`CodeWhale/crates/tui/src/tui/ocean.rs` at
`a79ce5c4d5ed1a5f7032185710c27343a900351c`:

| Position | Ground |
|---|---|
| Surface | `#102A45` |
| Middle | `#0A1E33` |
| Deep | `#061320` |

One absolute vertical column connects the workspace, transcript and composer.
It does not restart inside each widget. Caller-reported waiting and approval
add the native warm cast; failure stays steady; completion breathes and settles.
Selections, elevated panels, diffs and code retain their own backgrounds.
The caller supplies phase, context percentage, motion policy and elapsed time.
Use `OceanColumn::viewport` for the shell bounds and `apply_matching` for each
native chrome band's base ground. Keep elevated and semantic surfaces outside
those bands; the finishing pass preserves their fills.
The field paints measured truecolor grounds. Underwater retains its original
stops; other painted themes derive depth and reflected light from their own
background, surface and accent colors. Light themes use a pale accent reflection
and a completion breath that returns to paper without clipping to white.
Terminal-owned grounds and fallback profiles retain their selected colors.

`Ombre` offers additional artistic treatments over completed component buffers:
Ocean uses the logo's blue pair, Lagoon uses Live into Primary, Dusk uses Primary
into Attention, Coral uses Danger into Attention, and Graphite maps the actual
desktop graphite grounds. These are spatial materials, never alternative state
colors. Paper uses pale washes. A caller can supply elapsed time to `apply_at`
for a slow, anchored drift during active work. Graphite stays neutral; reduced
and still motion retain the fixed composition. The finishing pass preserves glyphs, foregrounds,
content fills and caller colors, and keeps a text cell's original background
whenever a replacement cannot preserve its contrast floor. All fallback
profiles pass through unchanged.

## Work has priority

The default native layout has no permanent top header or right sidebar.
Conversation, pending input, the rounded composer, posture, workflow progress,
metrics and workbar form one vertical layout. The workbar has Tasks, Fleet,
Jobs, Files, Notes, Context, Git and Cost, with optional top and side placement.
Its row facts and keyboard outcomes belong to the caller.

Native palette slots retain their separate Working, Success, Warning,
permission and mode colors. Marks and words preserve meaning under no color
or ASCII. Narrow surfaces shed secondary information before essential input,
permission and model/context information.

Shared panels use rounded Unicode edges and retain plain ASCII equivalents.
Focus remains visible in the existing gutter of unbordered panels, without
moving content. Selected tabs and segmented controls carry a quiet ground
alongside their shape and weight cues. Count bars resolve reported progress
to eighth-cell steps in Unicode, retaining whole cells in ASCII and exact
counts in words. Long action hints wrap through the shared text layout.

`WorkspaceFrame`, `ContextRibbon`, author-heading messages, desktop-style
cards and the extra `Ombre` palettes are optional compositions. They are not
substitutes for the TUI baseline.

## Life and motion

The C-shaped Codewhale retains the native client's character. Its terminal
presentation can develop independently: the face and silhouette lead, while
the cove and foreground water recede. The
`whale_motion` core adapts its existing pure Rust springs, authored clips,
props, particles, scene and cove, rather than adding another performance model.
Provenance and selected conformance fixtures live in
[assets/whale-motion](assets/whale-motion/PROVENANCE.md).

One session-scoped `Stage` receives explicit owner inputs and advances on the
host's clock. Changing session resets the performance. Hiding a surface stops
painting; resuming discards missed motion. Braille cadence is capped at six
paints per second in action and two at rest. The full-color `WhalePet` uses
the GPUI hero cadence: 30 paints per second in action, eight at rest. These
are scheduling ceilings; the host owns the redraw policy. Reduced motion
uses the authored poster and schedules no redraw. Every pet surface keeps
readable state words; a host that already presents them can hide the redundant
caption with empty words. Compact response views reclaim the scenery area
when the artwork cannot fit, and schedule no hidden pet animation.

`WhalePet` paints the shared cove and contour rig into a bounded,
antialiased RGBA image, then reuses the avatar half-block painter. The full
expressive rig retains its eyes and gaze at small sizes, with four samples per
axis up to 64 pixels; larger art uses two samples and grows to 160 pixels.
Avatar resizes average premultiplied color so fine details and translucent
edges survive downsampling. This keeps
the two-stop body gradient, light eye/throat apertures, prop colors and partial
opacity of entering/exiting shapes. Scenery is painted behind the whale and
near water in front, in the native client's order. Paper scenes give the near
current a sea-glass tint so it remains visible without covering the face.
The same Stage supplies
resting pointer attention and decorative water ripples. Monochrome, ANSI-16
and unknown grounds retain Braille; ASCII and tiny viewports retain words.
`examples/pet.rs` is the live action tour and reproducible buffer export.

Braille has one foreground per cell. The colored adapter samples native ink at
visible dots, chooses the majority shape, and resolves ties by paint order.
The monochrome packed geometry remains an independent conformance oracle.
The studio paints the character after the water, using the optional
`paint_with_contrast` floor of 3:1 for decorative ink. It mixes only insufficient
inks toward the theme foreground; ordinary `paint` retains native ink exactly.
Fish, rare jellyfish and bubbles occupy open water around text; their bounded
populations and caller-clock motion settle under reduced or still policies.

## Host boundaries and verification

Components own paint and local presentation geometry. The host owns Engine
events, permission decisions, queue mutation, Markdown parsing, clipboard,
link navigation, persistence and the event loop. `Transcript` accepts typed
authored blocks; `CodeBlock` keeps original source separate from safe display.

The live showcase and captured previews use the same renderer. Every catalog
entry is exported in nine terminal profiles and included in the generated
README's dark and light boards. Tests cover clipping, Unicode, safe display,
color fallbacks, motion lifecycle and canonical native geometry. Hosted CI
checks source, snapshots and media hashes. These checks establish library
behavior. Engine adoption is separate work.
