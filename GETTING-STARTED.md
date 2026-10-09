# Your first Codewhale component

Codewhale Ratatui lets you use one part of Codewhale's terminal interface, or
compose a whole terminal workspace. Start with an editable composer, then add
the parts your app needs. You supply the data and actions; the library supplies
the presentation and input helpers.

## Try the editable starter

You need Rust 1.89 or newer and a terminal. Clone this repository to run its
examples:

```sh
git clone https://github.com/Hmbown/codewhale-ratatui
cd codewhale-ratatui
cargo run --locked --example starter
```

Type a line into the composer and press **Enter**. The example echoes your text
above it. **Esc** or **Ctrl+C** closes the app. Paste works too. It is a local
example; it does not send your text to a model.

The complete [starter source](examples/starter.rs) shows terminal cleanup,
Unicode editing, paste, the native workbar and the Underwater background.

For smaller, separate pieces, run:

```sh
cargo run --locked --example recipes
```

**F1** opens the editable composer, **F2** the Tasks workbar, **F3** the native
water column, **F4** a still whale, and **F5** an animated whale. In F5, press
**m** to compare Full, Reduced and Still motion. **Esc** closes. Each
[draw function](examples/recipes.rs) uses public library APIs and can be copied
independently into your app.

## Add a component to your app

The Git dependency below does not require a crates.io release. Your application
chooses its Ratatui terminal backend. This example uses Crossterm 0.29:

```toml
[dependencies]
codewhale-ratatui = { git = "https://github.com/Hmbown/codewhale-ratatui" }
ratatui = { version = "0.30.2", default-features = false, features = ["std", "crossterm_0_29"] }
crossterm = "0.29"
```

Commit your application's lockfile to keep the selected Git revision stable.
The `cargo run --example …` commands in this guide run the cloned library's
examples. Adding the dependency to another project does not copy those examples.

For a static widget, keep a theme outside your draw callback and pass it when
you paint:

```rust
use codewhale_ratatui::{NativeComposer, Paint, Theme};

let theme = Theme::detect().tui();
let composer = NativeComposer::new("Review the changes").focused(true);
// Inside your application's drawing callback:
// frame.render_widget(composer.themed(&theme), frame.area());
```

`Paint` supplies `.themed(&theme)` for Ratatui's `render_widget`. Components
also expose `.paint(area, buffer, &theme)` for composing into one buffer. Most
components are display values; rebuilding one from your current state during
draw is fine. Keep editing, selection and animation state between frames.

## Add the animated pet

Run `cargo run --locked --example pet` for the current full-color whale and
its cove. The tour covers all 17 authored activities. Use **← / →** to choose
an action, **Space** for still motion, **C** for scenery, **B** for Braille,
**V** for the rig's viewing direction, **P** for terminal profiles and **Q**
to close. Pointer movement draws its attention; clicking the water makes ripples.

Keep one `Stage` per session, report your app's real activity with
`Stage::observe`, and advance it from your existing event loop:

```rust
use codewhale_ratatui::{Theme, WhalePet, whale_motion::Stage};

fn draw_pet(frame: &mut ratatui::Frame<'_>, theme: &Theme, stage: &mut Stage) {
    stage.advance(std::time::Instant::now());
    frame.render_stateful_widget(WhalePet::new(theme), frame.area(), stage);
}
```

Use `stage.cadence(Tier::Hero)` to schedule color frames and `Tier::Terminal`
for Braille. `None` means no animation wakeup is needed. Reduced motion uses
the authored poster; `stage.set_visible(false)` suspends hidden animation.
The component keeps the activity caption visible in monochrome, ASCII and
small viewports. The [pet example](examples/pet.rs) includes the complete
input, pointer mapping, capability and terminal-cleanup code.

For a whale-centered work surface with reply and agent panes, use `PetMode`
and retain a `PetModeState` between frames. Try the idle-only example with
`cargo run --locked --example pet_mode`: **L** toggles motion and **Q** exits.
Your host supplies the real session, transcript and agent data.
[Avatar packs](AVATARS.md) provide the optional 2D Whale girl and custom artwork.

## Connect input to your state

Drawing a composer does not handle keyboard input. Keep `TextInputState` in
your application, draw its text and cursor, and act on its input outcomes:

```rust
use codewhale_ratatui::{NativeComposer, TextInputOutcome, TextInputState};

let mut draft = TextInputState::new();
// In your drawing callback:
let composer = NativeComposer::new(draft.text())
    .cursor(draft.cursor())
    .focused(true)
    .can_submit(!draft.is_empty());
// Paint composer, then set the terminal cursor from composer.cursor_position(area).

// In your keyboard handler:
// match draft.handle_key(key) {
//     TextInputOutcome::Submitted => { send(draft.text()); draft.clear(); }
//     TextInputOutcome::Cancelled => close_composer(),
//     TextInputOutcome::Changed | TextInputOutcome::Ignored => {}
// }
// In your bracketed-paste handler: draft.paste(&text);
```

The [starter](examples/starter.rs) is the runnable version. Your app decides
what submitting or cancelling means. For lists, persist `ListState` or
`PickerState` and use `render_stateful_widget`. For the workbar, persist
`WorkbarState`, use `.apply(workbar)` when drawing, and react to
`.handle_key(key, rows, visible_rows)`. `WorkbarOutcome::Activate(id)` reports
the chosen row; your app performs its action.

## Choose the parts you need

| I want to… | Start with | Runnable reference |
| --- | --- | --- |
| Edit a draft | `NativeComposer`, `TextInputState` | [starter](examples/starter.rs), recipes F1 |
| Show tasks or an agent fleet | `Workbar`, `WorkbarRow`, `WorkbarAgent`, `WorkbarState` | recipes F2, [showcase](examples/showcase.rs) |
| Compose the native terminal layout | `TerminalShell`, `Message::native`, `PostureBar`, `MetricsLine` | [starter](examples/starter.rs), recipes F3 |
| Let someone choose an item | `List`, `Picker`, `SessionList`, their selection states | [gallery](examples/gallery.rs), [native view recipes](src/gallery/native_views.rs) |
| Show a decision or a result | `ApprovalCard`, `Receipt`, `Diff`, `Transcript` | [showcase](examples/showcase.rs), [component guide](COMPONENTS.md) |
| Add Codewhale's background | `Theme::tui`, `TuiPalette`, `OceanColumn` | recipes F3 |
| Add a still or animated whale | `Whale`, `WhaleState`, `whale_motion::Stage` | recipes F4/F5 |
| Add fish, jellyfish and bubbles | `Habitat`, `FishSchool`, `Jellyfish`, `BubbleField` | [habitat](examples/habitat.rs) |
| Show work in progress | `Spinner`, `VerificationSpinner`, `MotionSet`, `FrameBudget` | [motion](examples/motion.rs) |
| Add the native animated whale and cove | `WhalePet`, `Stage` | [pet](examples/pet.rs) |
| Compose the full pet work surface | `PetMode`, `PetModeState` | [pet mode](examples/pet_mode.rs) |

The [view guide](VIEWS.md) maps Codewhale's terminal screens to these parts.
The [gallery](examples/gallery.rs) shows variations at different widths and
terminal capabilities. Gallery data is illustrative; use your own data in
your application.

## Use the native colors and background

`Theme::detect().tui()` selects Underwater on a dark terminal and WhaleLight
on a light one. To choose another native preset explicitly:

```rust
use codewhale_ratatui::{Theme, TuiPalette};
let theme = Theme::detect().tui_palette(TuiPalette::TokyoNight);
```

Keep the theme outside the draw callback. Replace it when your user's theme
or terminal settings change. `Theme::detect()` without `.tui()` selects the
library's role-token theme instead of a native TUI palette.

For Underwater, paint the shell and foreground components first, then apply
one `OceanColumn` with `.viewport(full_shell_area)`. Use `.apply_matching`
for the composer's base ground, so the depth continues through both areas.
The [water recipe](examples/recipes.rs) shows the finishing order. The native
field respects terminal capabilities and preserves selections, code and other
semantic backgrounds; other palettes retain their own grounds.

`WorkspaceFrame`, desktop-style cards and additional `Ombre` palettes are
optional compositions. `TerminalShell`, `NativeComposer`, `Workbar`, native
palettes and `OceanColumn` are the starting point for Codewhale's TUI look.

## Bring the whale and water to life

`Whale::new(WhaleState::Rest).paint(…)` draws a still pose. It needs no timer.
You may override its caption with `.words("Ready when you are")`. A whale's
art needs at least **16 columns × 8 rows plus one caption row**. Narrow or
ASCII-safe terminals show readable state words instead.

For the animated character, keep **one `whale_motion::Stage` in your host**.
Report your current `Inputs` with `.observe(session_id, inputs, reduced_motion)`;
advance it with your current `Instant`; then rasterize with `colored_braille`
and paint the resulting `ColoredGrid`. The F5 recipe shows this with a
declared example Working state. Change its presence and caption together when
your application's state changes. Creating a new Stage every frame restarts
the performance.

Use `stage.cadence(Tier::Terminal)` to decide when another frame is due. It
returns `None` while hidden or under reduced motion, so your host can wait for
input. Call `stage.set_visible(false)` when the surface is hidden. A different
session ID resets the performance; resuming does not replay missed animation.
Full motion animates. Reduced and Still policies keep a readable static cue.

Spinners take your measured `Duration`, verb and `MotionMode`; they begin
moving only after 400 ms. The [motion example](examples/motion.rs) combines
their deadlines with `FrameBudget` and stops scheduling settled transitions.
There is no built-in animation thread or event loop.

For marine life, paint foreground content first and `Habitat` afterwards.
Use `.protected(overlay_rectangles)` to keep creatures out of blank dialog
space. They need clear water around their whole silhouette; crowded screens
may show fewer creatures. Reduced and Still motion remove ambient life.
Repaint the underlying surface every frame, because whale dots and marine
life are transparent. See the [habitat example](examples/habitat.rs).

## If a preview looks different

- **Only whale words appear:** check the available art size and ASCII policy.
- **The whale is still:** use the animated recipe, keep its Stage between
  frames, and let your event loop schedule the returned cadence.
- **No ombré appears:** use the native Underwater truecolor theme; limited-color
  and other theme profiles preserve their supported grounds.
- **Fish are absent:** allow a clear habitat area, use Full motion and protect
  only the rectangles that need protection.
- **A control draws but does nothing:** connect its input state and outcomes
  to your application's actions. Rendering does not perform those actions.

For a full interactive tour, run `cargo run --locked --example showcase`.
For focused studies, run `cargo run --locked --example habitat` or
`cargo run --locked --example motion`. Their key controls are documented at
the top of each source file.
