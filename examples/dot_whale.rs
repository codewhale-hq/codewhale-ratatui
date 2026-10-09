use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    time::Duration,
};

use codewhale_ratatui::{
    DotWhale, Paint, Role, Theme,
    testing::{self, Profile},
    text,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, buffer::Buffer, layout::Rect};
use serde::Deserialize;

#[derive(Deserialize)]
struct Pose {
    points: Vec<[f64; 2]>,
    materials: Vec<[f64; 4]>,
    #[serde(default)]
    style: PoseStyle,
}

#[derive(Default, Deserialize)]
struct PoseStyle {
    #[serde(default)]
    hollow: bool,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Activity {
    freshness: Option<String>,
    authoritative_presence: Option<String>,
    activity_kind: Option<String>,
    action_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnerFrame {
    #[serde(flatten)]
    pose: Pose,
    still: Option<Pose>,
    #[serde(default)]
    activity: Activity,
    producer_connected: Option<bool>,
}

impl OwnerFrame {
    fn caption(&self) -> &str {
        if self.activity.freshness.as_deref() != Some("fresh")
            || self.producer_connected == Some(false)
        {
            return "Activity unobserved";
        }
        match self.activity.authoritative_presence.as_deref() {
            Some("needs_you") => "Waiting for you",
            Some("done") => "Work complete",
            Some("idle") => "Idle",
            _ => match self.activity.activity_kind.as_deref() {
                Some("reading") => "Reading",
                Some("editing") => "Editing",
                Some("searching") => "Searching",
                Some("testing") => "Running tests",
                Some("executing") => "Running a command",
                Some("browsing") => "Browsing",
                Some("computer") => "Using the computer",
                Some("memory") => "Retrieving context",
                Some("thinking") => "Thinking",
                Some("responding") => "Writing a response",
                Some("delegating") => "Coordinating agents",
                Some("tool") => "Using a tool",
                _ => "Activity unobserved",
            },
        }
    }

    fn draw(&self, area: Rect, buf: &mut Buffer, theme: &Theme, still: bool, error: Option<&str>) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        buf.set_style(
            area,
            theme.bg(Role::Background).patch(theme.fg(Role::Primary)),
        );
        let pose = if still {
            self.still.as_ref()
        } else {
            Some(&self.pose)
        };
        let label = if still && pose.is_none() {
            "Still frame unavailable"
        } else {
            self.caption()
        };
        let action = (self.activity.freshness.as_deref() == Some("fresh")
            && self.producer_connected != Some(false))
        .then_some(self.activity.action_id.as_deref())
        .flatten();
        let empty = Pose {
            points: Vec::new(),
            materials: Vec::new(),
            style: PoseStyle::default(),
        };
        let pose = pose.unwrap_or(&empty);
        let whale = DotWhale::new(&pose.points, &pose.materials)
            .caption(label)
            .action_id(action)
            .hollow(
                pose.style.hollow
                    || self.activity.freshness.as_deref() != Some("fresh")
                    || self.producer_connected == Some(false),
            );
        if area.height < 5 {
            whale.paint(area, buf, theme);
            return;
        }
        buf.set_stringn(
            area.x,
            area.y,
            if still {
                "Prepared owner frame / Still"
            } else {
                "Prepared owner frame"
            },
            usize::from(area.width),
            theme.fg(Role::Primary),
        );
        whale.paint(
            Rect::new(
                area.x,
                area.y + 2,
                area.width,
                area.height.saturating_sub(4),
            ),
            buf,
            theme,
        );
        let footer = text::display_safe(
            error.unwrap_or("Space still / Q close / file updates are reloaded"),
        );
        buf.set_stringn(
            area.x,
            area.bottom().saturating_sub(1),
            footer,
            usize::from(area.width),
            theme.fg(if error.is_some() {
                Role::Danger
            } else {
                Role::Muted
            }),
        );
    }
}

fn load(path: &Path) -> Result<(Vec<u8>, OwnerFrame), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(512 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 512 * 1024 {
        return Err("prepared frame exceeds 512 KiB".into());
    }
    let frame: OwnerFrame = serde_json::from_slice(&bytes)?;
    for pose in std::iter::once(&frame.pose).chain(frame.still.iter()) {
        if pose.points.is_empty()
            || pose.points.len() > 980
            || pose.points.len() != pose.materials.len()
            || pose
                .points
                .iter()
                .flatten()
                .any(|n| !n.is_finite() || !(-1.0..=1.0).contains(n))
            || pose.materials.iter().any(|m| {
                m[..3]
                    .iter()
                    .any(|n| !n.is_finite() || !(0.0..=255.0).contains(n))
                    || !m[3].is_finite()
                    || !(0.0..=1.0).contains(&m[3])
            })
        {
            return Err("invalid prepared points or materials".into());
        }
    }
    Ok((bytes, frame))
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (mut path, mut svg, mut profile) = (None, None, None);
    let mut still = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--frame" => {
                path = Some(PathBuf::from(
                    args.next().ok_or("--frame requires an owner JSON file")?,
                ))
            }
            "--svg" => {
                svg = Some(PathBuf::from(
                    args.next().ok_or("--svg requires an output file")?,
                ))
            }
            "--profile" => {
                profile = Some(
                    Profile::from_name(&args.next().ok_or("--profile requires a name")?)
                        .ok_or("unknown terminal profile")?,
                )
            }
            "--still" => still = true,
            _ => {
                return Err(
                    "usage: dot_whale --frame FILE [--svg FILE] [--profile NAME] [--still]".into(),
                );
            }
        }
    }
    let path =
        path.ok_or("usage: dot_whale --frame FILE [--svg FILE] [--profile NAME] [--still]")?;
    let (mut bytes, mut owner) = load(&path)?;
    if let Some(svg) = svg {
        let theme = profile.unwrap_or(Profile::DarkTrue).theme();
        let buffer = testing::render(100, 34, |area, buf| {
            owner.draw(area, buf, &theme, still, None)
        });
        std::fs::write(svg, testing::svg(&buffer, &theme))?;
        return Ok(());
    }
    codewhale_ratatui::detect::probe_terminal_background();
    let theme = profile.map_or_else(Theme::detect, |p| p.theme());
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut error: Option<String> = None;
    let mut dirty = true;
    loop {
        if dirty {
            terminal
                .draw(|f| owner.draw(f.area(), f.buffer_mut(), &theme, still, error.as_deref()))?;
            dirty = false;
        }
        if event::poll(Duration::from_millis(33))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char(' ') => {
                        still = !still;
                        dirty = true;
                    }
                    _ => {}
                },
                Event::Resize(..) => dirty = true,
                _ => {}
            }
        }
        match load(&path) {
            Ok((next, frame)) => {
                if next != bytes || error.is_some() {
                    bytes = next;
                    owner = frame;
                    error = None;
                    dirty = true;
                }
            }
            Err(failure) => {
                let next = failure.to_string();
                if error.as_deref() != Some(next.as_str()) {
                    error = Some(next);
                    dirty = true;
                }
            }
        }
    }
    Ok(())
}
