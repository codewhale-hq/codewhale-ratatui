//! Roster viewer: empty by default, or a read-only view of Engine roster JSON.
//! `--demo` explicitly selects sample states. It never starts agent work.

use codewhale_ratatui::{
    AgentCard, MotionMode, Role, State, Subagent, SubagentEvent, SubagentUsage, SubagentView,
    SubagentViewState, SubagentViewWords, Theme,
    testing::{self, Profile},
    text,
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
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

// Presentation adapter for the existing Engine AgentRosterRow, not a new
// lifecycle authority. See crates/tui/src/agent_roster.rs and the protocol's
// EventMsg::AgentList. New states remain unknown instead of becoming success.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RosterState {
    Running,
    Waiting,
    Parked,
    Done,
    Failed,
    Cancelled,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct RosterRow {
    worker_id: String,
    display_name: String,
    model: String,
    state: RosterState,
    status: String,
    activity: Option<String>,
    outcome: Option<String>,
    millis: Option<u64>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost_microusd: Option<u64>,
    steps_taken: u32,
}

impl RosterRow {
    fn project(self) -> Subagent<'static> {
        let (state, word) = match self.state {
            RosterState::Running => match self.status.as_str() {
                "queued" => (State::Ready, "Queued"),
                "starting" => (State::Working, "Starting"),
                "model_wait" => (State::Working, "Waiting for model"),
                "running_tool" => (State::Working, "Running tool"),
                _ => (State::Working, "Running"),
            },
            RosterState::Waiting => (State::NeedsYou, "Waiting for you"),
            RosterState::Parked => (State::Stopped, "Parked"),
            RosterState::Done => (State::Done, "Done"),
            RosterState::Failed => (State::Failed, "Failed"),
            RosterState::Cancelled => (State::Stopped, "Cancelled"),
            RosterState::Unknown => (State::Unknown, "Unknown"),
        };
        let mut card = AgentCard::new(self.display_name, state).status_word(word);
        if !self.model.is_empty() {
            card = card.route(self.model);
        }
        if let Some(activity) = self.activity {
            card = card.task(activity);
        }
        let mut agent = Subagent::new(self.worker_id, card);
        agent.elapsed = self.millis.map(Duration::from_millis);
        agent.usage = Some(SubagentUsage {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cost_microusd: self.cost_microusd,
        });
        agent.steps = Some(self.steps_taken);
        agent.outcome = self.outcome.map(Into::into);
        agent.performance = Some(Inputs {
            presence: match state {
                State::Working => Presence::Working,
                State::NeedsYou => Presence::NeedsYou,
                State::Done => Presence::Done,
                _ => Presence::Idle,
            },
            activity: None,
            context: Context::default(),
        });
        // AgentList has no timestamped event history, action permissions or
        // classified activity. Do not manufacture any of them from its text.
        agent
    }
}

struct Snapshot {
    agents: Vec<Subagent<'static>>,
    owner: Option<String>,
    summary: String,
}

fn parse_roster(bytes: &[u8]) -> Result<Snapshot, Box<dyn std::error::Error>> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    // Accept a direct row array, AgentList itself, or its EventEnvelope.
    let event = value
        .get("event")
        .filter(|v| v.is_object())
        .unwrap_or(&value);
    let rows = if event.is_array() {
        event
    } else {
        event
            .get("roster")
            .ok_or("expected AgentList.roster or an array of AgentRosterRow")?
    };
    let rows: Vec<RosterRow> = serde_json::from_value(rows.clone())?;
    let mut ids = HashSet::new();
    for row in &rows {
        if row.worker_id.trim().is_empty() || !ids.insert(row.worker_id.as_str()) {
            return Err("worker IDs must be nonempty and unique".into());
        }
    }
    let mut counts = Vec::new();
    for (state, word) in [
        (RosterState::Running, "running"),
        (RosterState::Waiting, "waiting"),
        (RosterState::Parked, "parked"),
        (RosterState::Done, "done"),
        (RosterState::Failed, "failed"),
        (RosterState::Cancelled, "cancelled"),
        (RosterState::Unknown, "unknown"),
    ] {
        let count = rows.iter().filter(|row| row.state == state).count();
        if count > 0 {
            counts.push(format!("{count} {word}"));
        }
    }
    counts.push(format!("{} total", rows.len()));
    Ok(Snapshot {
        agents: rows.into_iter().map(RosterRow::project).collect(),
        owner: event
            .get("owner_session_id")
            .or_else(|| event.get("session_id"))
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        summary: counts.join(" / "),
    })
}

fn read_roster(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    const LIMIT: u64 = 8 * 1024 * 1024;
    let mut bytes = Vec::new();
    File::open(path)?.take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err("roster file exceeds 8 MiB".into());
    }
    Ok(bytes)
}

fn samples(step: usize) -> Vec<Subagent<'static>> {
    let state = [
        State::Working,
        State::Working,
        State::NeedsYou,
        State::Working,
        State::Done,
    ][step.min(4)];
    let mut agents = Vec::new();
    for (id, status) in [
        ("A", state),
        ("B", State::Ready),
        ("C", State::Stopped),
        ("D", State::Done),
    ] {
        let mut agent = Subagent::new(
            id,
            AgentCard::new(format!("Sample worker {id}"), status)
                .task(format!("{} state", status.word())),
        );
        agent.performance = Some(Inputs {
            presence: match status {
                State::Working => Presence::Working,
                State::NeedsYou => Presence::NeedsYou,
                State::Done => Presence::Done,
                _ => Presence::Idle,
            },
            activity: None,
            context: Context {
                live: true,
                turn_id: Some(id.into()),
                ..Context::default()
            },
        });
        agent.events = vec![SubagentEvent::new(
            "Sample",
            status,
            format!("{} state", status.word()),
        )];
        if status == State::Done {
            agent.outcome = Some("Sample output.\n\nThe viewer preserves the full result or error supplied by the owner.\nUse O to switch between output and activity; scroll with PageUp/PageDown or the mouse wheel.".into());
        }
        agents.push(agent);
    }
    agents
}

struct Viewer {
    state: SubagentViewState,
    snapshot: Snapshot,
    path: Option<PathBuf>,
    bytes: Vec<u8>,
    watch: bool,
    sample: bool,
    step: usize,
    motion: MotionMode,
    profile: Profile,
    changed: Instant,
    polled: Instant,
    notice: String,
}

impl Viewer {
    fn new(
        profile: Profile,
        now: Instant,
        sample: bool,
        path: Option<PathBuf>,
        watch: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut viewer = Self {
            state: SubagentViewState::default(),
            snapshot: Snapshot {
                agents: Vec::new(),
                owner: None,
                summary: "0 total".into(),
            },
            path,
            bytes: Vec::new(),
            watch,
            sample,
            step: 0,
            motion: if sample {
                MotionMode::Full
            } else {
                MotionMode::Reduced
            },
            profile,
            changed: now,
            polled: now,
            notice: "Load roster JSON with --roster FILE; use --demo for sample states".into(),
        };
        if viewer.path.is_some()
            && let Err(error) = viewer.reload()
        {
            if !watch {
                return Err(error);
            }
            viewer.notice = error.to_string();
        }
        if sample {
            viewer.set_sample(0, now);
        }
        Ok(viewer)
    }

    fn set_sample(&mut self, step: usize, now: Instant) {
        self.step = step.min(4);
        self.snapshot.agents = samples(self.step);
        self.snapshot.summary = "Sample states / no agent work is running".into();
        self.notice = "SAMPLE DATA / no reported usage, model routes or test results".into();
        self.changed = now;
    }

    fn reload(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(path) = &self.path else {
            return Ok(false);
        };
        let bytes = read_roster(path)?;
        let notice = format!(
            "{} {} / read only",
            if self.watch { "Watching" } else { "Snapshot" },
            path.display()
        );
        if bytes == self.bytes {
            let changed = self.notice != notice;
            self.notice = notice;
            return Ok(changed);
        }
        let snapshot = parse_roster(&bytes)?;
        // A new session must never inherit selection, history scroll or the
        // old pet's performance clock from another conversation.
        if snapshot.owner != self.snapshot.owner {
            self.state = SubagentViewState::default();
        }
        self.snapshot = snapshot;
        self.bytes = bytes;
        self.notice = notice;
        Ok(true)
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, now: Instant) {
        let theme = self.profile.theme();
        self.state.update(&self.snapshot.agents, now, self.motion);
        let inner = Rect {
            height: area.height.saturating_sub(2),
            ..area
        };
        SubagentView::new(&self.snapshot.agents, &theme)
            .words(SubagentViewWords {
                summary: Some(self.snapshot.summary.as_str().into()),
                empty: if self.path.is_some() {
                    "No agents in this roster".into()
                } else {
                    "No roster loaded".into()
                },
                no_events: "No event history in this snapshot".into(),
                ..SubagentViewWords::default()
            })
            .render(inner, buf, &mut self.state);
        if area.height >= 2 {
            buf.set_style(
                Rect::new(area.x, area.bottom() - 2, area.width, 2),
                theme.bg(Role::Sidebar),
            );
            for (offset, label, role) in [
                (2, self.notice.clone(), Role::Muted),
                (
                    1,
                    format!(
                        "{}L motion   P theme   Q close / {:?}",
                        if self.sample {
                            "N sample state   R replay   "
                        } else if self.path.is_some() {
                            "R reload   "
                        } else {
                            ""
                        },
                        self.motion
                    ),
                    Role::Primary,
                ),
            ] {
                buf.set_stringn(
                    area.x,
                    area.bottom() - offset,
                    text::display_safe(&label),
                    usize::from(area.width),
                    theme.fg(role),
                );
            }
        }
    }
}

fn export(
    dir: &Path,
    mut viewer: Viewer,
    width: u16,
    height: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let started = Instant::now();
    let mut manifest = Vec::new();
    for index in 0..200 {
        let elapsed_ms = index * 80;
        let now = started + Duration::from_millis(elapsed_ms);
        if viewer.sample {
            viewer.set_sample((index / 40) as usize, now);
        }
        if width < 88 && index == 80 {
            viewer.state.handle_key(
                &viewer.snapshot.agents,
                event::KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            );
        }
        let theme = viewer.profile.theme();
        let buf = testing::render(width, height, |area, buf| viewer.draw(area, buf, now));
        let file = format!("frame-{index:03}.svg");
        std::fs::write(dir.join(&file), testing::svg(&buf, &theme))?;
        manifest.push(serde_json::json!({"file":file,"elapsed_ms":elapsed_ms,"profile":viewer.profile.name(),"width":width,"height":height}));
    }
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "Exported {} roster frames to {}",
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
    let (mut frames, mut profile, mut path) = (None, None, None);
    let (mut sample, mut watch) = (false, false);
    let (mut width, mut height) = (112, 38);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demo" => sample = true,
            "--watch" => watch = true,
            "--roster" => {
                path = Some(PathBuf::from(
                    args.next().ok_or("--roster requires a file")?,
                ))
            }
            "--size" => {
                let size = args.next().ok_or("--size requires COLSxROWS")?;
                let (cols, rows) = size.split_once('x').ok_or("--size requires COLSxROWS")?;
                width = cols.parse()?;
                height = rows.parse()?;
                if !(1..=160).contains(&width) || !(1..=80).contains(&height) {
                    return Err("export size must be 1..160 columns and 1..80 rows".into());
                }
            }
            "--frames" => frames = Some(args.next().ok_or("--frames requires a directory")?),
            "--profile" => {
                profile = Some(
                    Profile::from_name(&args.next().ok_or("--profile requires a name")?)
                        .ok_or("unknown terminal profile")?,
                )
            }
            "--help" | "-h" => {
                println!(
                    "subagents [--roster FILE [--watch] | --demo] [--profile NAME] [--frames DIR] [--size COLSxROWS]\n\nReads an AgentRosterRow array or AgentList JSON, including EventEnvelope.\nR reloads a file. --watch checks it twice a second. No provider calls or worker controls.\nWithout a file or --demo, the viewer is empty. --demo shows explicitly labelled sample states."
                );
                return Ok(());
            }
            _ => return Err("unknown option; use --help".into()),
        }
    }
    if sample && path.is_some() {
        return Err("choose --demo or --roster, not both".into());
    }
    if watch && path.is_none() {
        return Err("--watch requires --roster FILE".into());
    }
    if frames.is_none() {
        codewhale_ratatui::detect::probe_terminal_background();
    }
    let profile = profile.unwrap_or_else(|| {
        if frames.is_some() {
            return Profile::DarkTrue;
        }
        let caps = Theme::detect().caps();
        Profile::ALL
            .into_iter()
            .find(|p| p.caps() == caps)
            .unwrap_or(Profile::UnknownGround)
    });
    let mut viewer = Viewer::new(profile, Instant::now(), sample, path, watch)?;
    if let Some(dir) = frames {
        return export(Path::new(&dir), viewer, width, height);
    }
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut dirty = true;
    loop {
        let now = Instant::now();
        if viewer.sample
            && viewer.step < 4
            && now.saturating_duration_since(viewer.changed) >= Duration::from_secs(4)
        {
            viewer.set_sample(viewer.step + 1, now);
            dirty = true;
        }
        if viewer.watch
            && now.saturating_duration_since(viewer.polled) >= Duration::from_millis(500)
        {
            viewer.polled = now;
            match viewer.reload() {
                Ok(changed) => dirty |= changed,
                Err(error) => {
                    let notice = format!("Cannot read roster: {error} / showing last snapshot");
                    dirty |= viewer.notice != notice;
                    viewer.notice = notice;
                }
            }
        }
        if dirty {
            terminal.draw(|frame| viewer.draw(frame.area(), frame.buffer_mut(), now))?;
        }
        let animation = viewer
            .state
            .next_frame_in(&viewer.snapshot.agents, &viewer.profile.theme());
        let mut wait = animation.unwrap_or(Duration::from_secs(60));
        if viewer.sample && viewer.step < 4 {
            wait = wait.min(
                Duration::from_secs(4)
                    .saturating_sub(now.saturating_duration_since(viewer.changed)),
            );
        }
        if viewer.watch {
            wait = wait.min(
                Duration::from_millis(500)
                    .saturating_sub(now.saturating_duration_since(viewer.polled)),
            );
        }
        if !event::poll(wait)? {
            dirty = animation.is_some();
            continue;
        }
        dirty = true;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('n') if viewer.sample => {
                    viewer.set_sample(viewer.step + 1, Instant::now())
                }
                KeyCode::Char('r') if viewer.sample => viewer.set_sample(0, Instant::now()),
                KeyCode::Char('r') => {
                    if let Err(error) = viewer.reload() {
                        viewer.notice =
                            format!("Cannot read roster: {error} / showing last snapshot");
                    }
                }
                KeyCode::Char('l') => {
                    viewer.motion = if viewer.motion.animates() {
                        MotionMode::Reduced
                    } else {
                        MotionMode::Full
                    }
                }
                KeyCode::Char('p') => {
                    let at = Profile::ALL
                        .iter()
                        .position(|p| *p == viewer.profile)
                        .unwrap_or(0);
                    viewer.profile = Profile::ALL[(at + 1) % Profile::ALL.len()];
                }
                _ => {
                    viewer.state.handle_key(&viewer.snapshot.agents, key);
                }
            },
            Event::Mouse(mouse) => {
                viewer.state.handle_mouse(&viewer.snapshot.agents, mouse);
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(id: &str, state: &str, status: &str) -> serde_json::Value {
        json!({"worker_id": id, "display_name": id, "model": "", "state": state,
            "status": status, "steps_taken": 0, "run_id": format!("run-{id}"),
            "input_tokens": 0, "output_tokens": null, "cost_microusd": null})
    }

    #[test]
    fn accepts_engine_arrays_agent_list_and_envelope_without_inventing_missing_receipts() {
        let rows = json!([
            row("waiting", "waiting", "waiting_for_user"),
            row("parked", "parked", "waiting_for_user"),
            row("cancelled", "cancelled", "interrupted"),
            row("future", "future_state", "future_status")
        ]);
        for value in [
            rows.clone(),
            json!({"event":"agent_list", "owner_session_id":"owner", "roster":rows.clone()}),
            json!({"seq":1,"event":{"event":"agent_list", "owner_session_id":"owner", "roster":rows}}),
        ] {
            let snapshot = parse_roster(&serde_json::to_vec(&value).unwrap()).unwrap();
            assert!(snapshot.summary.contains("1 parked"));
            assert!(snapshot.summary.contains("1 cancelled"));
            assert_eq!(snapshot.agents[0].card.status.state, State::NeedsYou);
            assert_eq!(snapshot.agents[1].card.status.word, "Parked");
            assert_eq!(snapshot.agents[2].card.status.word, "Cancelled");
            assert_eq!(snapshot.agents[3].card.status.state, State::Unknown);
            for agent in snapshot.agents {
                assert_eq!(agent.elapsed, None);
                assert_eq!(agent.usage.unwrap().input_tokens, Some(0));
                assert_eq!(agent.usage.unwrap().output_tokens, None);
                assert!(agent.card.route.is_none());
                assert!(agent.events.is_empty());
                assert!(agent.outcome.is_none());
                assert_eq!(agent.controls, Default::default());
            }
        }
    }

    #[test]
    fn rejects_ambiguous_identity_and_non_roster_input() {
        for value in [
            json!([
                row("same", "done", "completed"),
                row("same", "failed", "failed")
            ]),
            json!([row(" ", "done", "completed")]),
            json!({"agents":[]}),
        ] {
            assert!(parse_roster(&serde_json::to_vec(&value).unwrap()).is_err());
        }
    }

    #[test]
    fn reload_keeps_good_data_recovers_same_bytes_and_resets_changed_owner() {
        let path = std::env::temp_dir().join(format!(
            "subagent-roster-test-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let good = serde_json::to_vec(&json!({"event":"agent_list", "owner_session_id":"first", "roster":[row("a", "done", "completed")]})).unwrap();
        std::fs::write(&path, &good).unwrap();
        let mut viewer = Viewer::new(
            Profile::Ascii,
            Instant::now(),
            false,
            Some(path.clone()),
            true,
        )
        .unwrap();
        viewer
            .state
            .update(&viewer.snapshot.agents, Instant::now(), MotionMode::Reduced);
        assert_eq!(viewer.state.selected_id(), Some("a"));
        std::fs::write(&path, b"{\"roster\": [").unwrap();
        assert!(viewer.reload().is_err());
        assert_eq!(viewer.snapshot.agents[0].id, "a");
        assert_eq!(viewer.state.selected_id(), Some("a"));
        viewer.notice = "Cannot read roster / showing last snapshot".into();
        std::fs::write(&path, &good).unwrap();
        assert!(viewer.reload().unwrap());
        assert!(viewer.notice.starts_with("Watching "));
        std::fs::write(&path, serde_json::to_vec(&json!({"event":"agent_list", "owner_session_id":"second", "roster":[row("b", "running", "running")]})).unwrap()).unwrap();
        assert!(viewer.reload().unwrap());
        assert_eq!(viewer.state.selected_id(), None);
        assert_eq!(viewer.snapshot.agents[0].id, "b");
        std::fs::remove_file(path).unwrap();
    }
}
