//! The full pet surface at rest. This preview has no Engine, fabricated work,
//! response, usage or agents. The actual terminal integration lives in `/pet on`.
use codewhale_ratatui::{
    MotionMode, PetMode, PetModeState, State, StatusMark, Theme,
    testing::{self, Profile},
    whale_motion::{Context, Inputs, Presence},
};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal, backend::CrosstermBackend, buffer::Buffer, layout::Rect, widgets::StatefulWidget,
};
use std::{
    io,
    path::Path,
    time::{Duration, Instant},
};
fn update(state: &mut PetModeState, now: Instant, motion: MotionMode) {
    state.update(
        Some("idle-preview"),
        Inputs {
            presence: Presence::Idle,
            activity: None,
            context: Context::default(),
        },
        &[],
        now,
        motion,
    );
}
fn draw(state: &mut PetModeState, theme: &Theme, area: Rect, buf: &mut Buffer) {
    let mut view = PetMode::new(theme, StatusMark::new(State::Ready).word("Idle"));
    view.hints = "Idle preview · L motion · Q close".into();
    view.render(area, buf, state);
}
fn export(dir: &Path, profile: Profile) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut state = PetModeState::default();
    state.set_visible(true);
    let theme = profile.theme();
    let now = Instant::now();
    let mut manifest = Vec::new();
    for index in 0..200 {
        let elapsed_ms = index * 80;
        update(
            &mut state,
            now + Duration::from_millis(elapsed_ms),
            MotionMode::Full,
        );
        let buf = testing::render(100, 36, |area, buf| draw(&mut state, &theme, area, buf));
        let file = format!("frame-{index:03}.svg");
        std::fs::write(dir.join(&file), testing::svg(&buf, &theme))?;
        manifest.push(serde_json::json!({"file":file,"elapsed_ms":elapsed_ms,"width":100,"height":36,"profile":profile.name()}));
    }
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )
}
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            crossterm::cursor::Show
        );
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (mut frames, mut profile) = (None, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--frames" => frames = Some(args.next().ok_or("--frames requires a directory")?),
            "--profile" => {
                profile = Some(
                    Profile::from_name(&args.next().ok_or("--profile requires a name")?)
                        .ok_or("unknown terminal profile")?,
                )
            }
            _ => return Err("usage: pet_mode [--frames DIR] [--profile NAME]".into()),
        }
    }
    if let Some(dir) = frames {
        return export(Path::new(&dir), profile.unwrap_or(Profile::DarkTrue)).map_err(Into::into);
    }
    codewhale_ratatui::detect::probe_terminal_background();
    let theme = profile.map_or_else(Theme::detect, |p| p.theme());
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut state = PetModeState::default();
    state.set_visible(true);
    let mut motion = MotionMode::Full;
    let mut dirty = true;
    loop {
        update(&mut state, Instant::now(), motion);
        if dirty {
            terminal.draw(|frame| draw(&mut state, &theme, frame.area(), frame.buffer_mut()))?;
        }
        let wait = state.next_frame_in(&[], &theme);
        dirty = false;
        if event::poll(wait.unwrap_or(Duration::from_millis(500)))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Esc | KeyCode::Char('q') => break,
                    KeyCode::Char('l') => {
                        motion = if motion.animates() {
                            MotionMode::Reduced
                        } else {
                            MotionMode::Full
                        };
                        dirty = true;
                    }
                    _ => {}
                },
                Event::Resize(..) => dirty = true,
                Event::Mouse(mouse) => dirty = state.handle_mouse(&[], mouse),
                _ => {}
            }
        } else if wait.is_some() {
            dirty = true;
        }
    }
    Ok(())
}
