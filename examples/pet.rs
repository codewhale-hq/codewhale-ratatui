//! Live GPUI whale pet. Arrow keys preview the authored actions; mouse motion
//! follows the water and a left click makes a ripple. All inputs are fixtures.
//! --frames DIR exports actual Ratatui buffers for reproducible animation.

use codewhale_ratatui::{
    PetStyle, Role, Theme, WhalePet, WhaleState,
    testing::{self, Profile},
    whale_motion::{Activity, Context, Inputs, Presence, Stage, Tier, rig},
};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseButton, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, buffer::Buffer, layout::Rect};
use std::{
    io,
    path::Path,
    time::{Duration, Instant},
};

// Explicit demo owner facts, in the public catalogue's order. Production
// hosts supply their existing owner inputs instead of classifying any text.
fn inputs(index: usize, turn: usize) -> Inputs {
    let state = WhaleState::ALL[index];
    let (presence, kind) = match state {
        WhaleState::Rest => (Presence::Idle, None),
        WhaleState::Listen => (Presence::Listening, None),
        WhaleState::Think => (Presence::Thinking, None),
        WhaleState::NeedsYou => (Presence::NeedsYou, None),
        WhaleState::Done => (Presence::Done, None),
        WhaleState::Stuck => (Presence::Stuck, None),
        WhaleState::Asleep => (Presence::Offline, None),
        WhaleState::Read => (Presence::Working, Some("reading")),
        WhaleState::Search => (Presence::Working, Some("searching")),
        WhaleState::Write => (Presence::Working, Some("editing")),
        WhaleState::Run => (Presence::Working, Some("executing")),
        WhaleState::Browse => (Presence::Working, Some("browsing")),
        WhaleState::Talk => (Presence::Working, Some("responding")),
        WhaleState::Pod { .. } => (Presence::Working, Some("delegating")),
        WhaleState::Computer => (Presence::Working, Some("computer")),
        WhaleState::Connect => (Presence::Working, Some("network")),
        WhaleState::Busy => (Presence::Working, Some("unknown")),
    };
    Inputs {
        presence,
        activity: kind.map(|kind| Activity {
            kind: Some(kind.into()),
            observed: kind != "unknown",
            parallel: Some(3.),
            ..Activity::default()
        }),
        context: Context {
            live: true,
            turn_id: Some(format!("pet-preview-{turn}")),
            status: Some(
                if state == WhaleState::Stuck {
                    "failed"
                } else {
                    "completed"
                }
                .into(),
            ),
            now_ms: Some(1000.),
            failed_at_ms: Some(0.),
        },
    }
}

struct Demo {
    stage: Stage,
    index: usize,
    turn: usize,
    reduced: bool,
    cove: bool,
    style: PetStyle,
    view: usize,
    profile: Profile,
    automatic: bool,
    changed: Instant,
}

impl Demo {
    fn new(profile: Profile, now: Instant) -> Self {
        let mut demo = Self {
            stage: Stage::new(),
            index: 0,
            turn: 0,
            reduced: false,
            cove: true,
            style: PetStyle::Color,
            view: 0,
            profile,
            automatic: true,
            changed: now,
        };
        demo.observe();
        demo
    }

    fn observe(&mut self) {
        self.stage.observe(
            Some("pet-preview"),
            inputs(self.index, self.turn),
            self.reduced,
        );
    }

    fn select(&mut self, index: usize, now: Instant) {
        self.index = index;
        self.turn += 1;
        self.changed = now;
        self.observe();
    }

    fn pet_area(area: Rect) -> Rect {
        Rect::new(
            area.x,
            area.y.saturating_add(2),
            area.width,
            area.height.saturating_sub(6),
        )
        .intersection(area)
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        let theme = self.profile.theme();
        buf.set_style(
            area,
            theme.bg(Role::Background).patch(theme.fg(Role::Primary)),
        );
        if area.is_empty() {
            return;
        }
        let title = if area.width >= 66 {
            format!(
                "Codewhale · companion    {} / {}{}",
                self.profile.name(),
                if self.reduced { "Still" } else { "Animated" },
                if self.automatic { " / tour" } else { "" }
            )
        } else {
            format!(
                "Codewhale · {}",
                if self.reduced { "Still" } else { "Animated" }
            )
        };
        buf.set_stringn(
            area.x,
            area.y,
            title,
            usize::from(area.width),
            theme.fg(Role::Primary),
        );
        let pet_area = Self::pet_area(area);
        let direction = [
            rig::MARK_DIRECTION,
            rig::CRUISE_DIRECTION,
            rig::OPEN_DIRECTION,
        ][self.view];
        WhalePet::new(&theme)
            .cove(self.cove)
            .style(self.style)
            .direction(direction)
            .paint(pet_area, buf, &mut self.stage);
        if area.height >= 5 {
            let caption = "Move over the water · click for a ripple";
            let x = area.x + area.width.saturating_sub(caption.len() as u16) / 2;
            buf.set_stringn(
                x,
                area.bottom() - 3,
                caption,
                usize::from(area.right() - x),
                theme.fg(Role::Muted),
            );
            let hints = if area.width >= 76 {
                "← → action   A tour   Space motion   C cove   B Braille   V view   P theme   Q close"
            } else {
                "← → action  A tour  Space motion  P theme  Q close"
            };
            buf.set_stringn(
                area.x,
                area.bottom() - 1,
                hints,
                usize::from(area.width),
                theme.fg(Role::Muted),
            );
        }
    }
}

fn export(dir: &Path, profile: Profile) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let started = Instant::now();
    let mut demo = Demo::new(profile, started);
    demo.stage.advance(started);
    let mut manifest = Vec::new();
    for index in 0..200 {
        let elapsed_ms = index * 60;
        let now = started + Duration::from_millis(elapsed_ms);
        let action = [0, 4, 6, 10, 12][index as usize / 40];
        if action != demo.index {
            demo.select(action, now);
        }
        demo.stage.advance(now);
        if index == 8 {
            demo.stage.cove_tap(25., 35.);
        }
        let theme = demo.profile.theme();
        let buf = testing::render(88, 36, |area, buf| demo.draw(area, buf));
        let file = format!("frame-{index:03}.svg");
        std::fs::write(dir.join(&file), testing::svg(&buf, &theme))?;
        manifest.push(serde_json::json!({"file": file, "elapsed_ms": elapsed_ms,
            "profile": profile.name(), "width": 88, "height": 36}));
    }
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "Exported {} live pet frames to {}",
        manifest.len(),
        dir.display()
    );
    Ok(())
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            LeaveAlternateScreen,
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
            _ => return Err("usage: pet [--profile NAME] [--frames DIR]".into()),
        }
    }
    if let Some(dir) = frames {
        return export(Path::new(&dir), profile.unwrap_or(Profile::DarkTrue)).map_err(Into::into);
    }
    codewhale_ratatui::detect::probe_terminal_background();
    // Preserve NO_COLOR, ASCII and unmeasured-ground policy unless the person
    // explicitly selects a preview profile.
    let profile = profile.unwrap_or_else(|| {
        let caps = Theme::detect().caps();
        Profile::ALL
            .into_iter()
            .find(|p| p.caps() == caps)
            .unwrap_or(Profile::UnknownGround)
    });
    let started = Instant::now();
    let mut demo = Demo::new(profile, started);
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    loop {
        let now = Instant::now();
        if demo.automatic
            && !demo.reduced
            && now.duration_since(demo.changed) >= Duration::from_secs(4)
        {
            demo.select((demo.index + 1) % WhaleState::ALL.len(), now);
        }
        demo.stage.advance(now);
        terminal.draw(|frame| demo.draw(frame.area(), frame.buffer_mut()))?;
        let tier = if demo.style == PetStyle::Color {
            Tier::Hero
        } else {
            Tier::Terminal
        };
        let mut wait = demo.stage.cadence(tier).unwrap_or(Duration::from_secs(60));
        if demo.automatic && !demo.reduced {
            wait = wait.min(Duration::from_secs(4).saturating_sub(demo.changed.elapsed()));
        }
        if !event::poll(wait)? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Left | KeyCode::Right => {
                    demo.automatic = false;
                    let next = if key.code == KeyCode::Left {
                        (demo.index + WhaleState::ALL.len() - 1) % WhaleState::ALL.len()
                    } else {
                        (demo.index + 1) % WhaleState::ALL.len()
                    };
                    demo.select(next, Instant::now());
                }
                KeyCode::Char('a') => {
                    demo.automatic = !demo.automatic;
                    demo.changed = Instant::now();
                }
                KeyCode::Char(' ') => {
                    demo.reduced = !demo.reduced;
                    demo.changed = Instant::now();
                    demo.observe();
                }
                KeyCode::Char('c') => demo.cove = !demo.cove,
                KeyCode::Char('b') => {
                    demo.style = if demo.style == PetStyle::Color {
                        PetStyle::Braille
                    } else {
                        PetStyle::Color
                    }
                }
                KeyCode::Char('v') => demo.view = (demo.view + 1) % 3,
                KeyCode::Char('p') => {
                    let current = Profile::ALL
                        .iter()
                        .position(|p| *p == demo.profile)
                        .unwrap_or(0);
                    demo.profile = Profile::ALL[(current + 1) % Profile::ALL.len()];
                }
                _ => {}
            },
            Event::Mouse(mouse) if demo.cove && !demo.reduced => {
                let area = Demo::pet_area(terminal.size()?.into());
                if let Some((x, y)) = WhalePet::point(area, mouse.column, mouse.row) {
                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => demo.stage.cove_tap(x, y),
                        MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                            demo.stage.cove_observe(x, y)
                        }
                        _ => {}
                    }
                } else {
                    demo.stage.cove_leave();
                }
            }
            _ => {}
        }
    }
    Ok(())
}
