//! Live subagent presentation. All work and receipts here are demo fixtures;
//! no workers, providers, shell commands or messages are started by this example.

use codewhale_ratatui::{
    AgentCard, CountBar, MotionMode, Role, State, Subagent, SubagentControls, SubagentEvent,
    SubagentIntent, SubagentView, SubagentViewState, Theme,
    testing::{self, Profile},
    whale_motion::{Activity, Context, Inputs, Presence},
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
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

// These phases are explicit example owner data, not a production classifier.
fn performance(presence: Presence, kind: Option<&str>, id: &str) -> Inputs {
    Inputs {
        presence,
        activity: kind.map(|kind| Activity {
            kind: Some(kind.into()),
            observed: true,
            ..Activity::default()
        }),
        context: Context {
            live: true,
            turn_id: Some(id.into()),
            status: (presence == Presence::Done).then(|| "completed".into()),
            ..Context::default()
        },
    }
}

fn agents(step: usize) -> Vec<Subagent<'static>> {
    let (state, verb, task, kind, presence, completed) = match step {
        0 => (
            State::Working,
            "Reading",
            "Inspect the session transport and its callers",
            Some("reading"),
            Presence::Working,
            1,
        ),
        1 => (
            State::Working,
            "Editing",
            "Preserve reconnect receipts in the session view",
            Some("editing"),
            Presence::Working,
            2,
        ),
        2 => (
            State::NeedsYou,
            "Needs you",
            "Choose whether to retry the interrupted upload",
            None,
            Presence::NeedsYou,
            2,
        ),
        3 => (
            State::Working,
            "Checking",
            "Verify the reconnect path and retained history",
            Some("executing"),
            Presence::Working,
            3,
        ),
        _ => (
            State::Done,
            "Done",
            "Reconnect receipts stay attached to the session",
            None,
            Presence::Done,
            4,
        ),
    };
    let mut author = Subagent::new(
        "author",
        AgentCard::new("Harbor", state)
            .role("Implementation")
            .route("Owner-selected model")
            .task(task)
            .status_word(verb),
    );
    author.performance = Some(performance(presence, kind, "author-turn"));
    author.elapsed = Some(Duration::from_secs(42 + step as u64 * 18));
    author.tokens = Some(format!("{} tokens", 8400 + step * 2100).into());
    author.checking = step == 3;
    author.progress = Some(CountBar::new(completed, 4).state(state).label("steps"));
    author.controls = SubagentControls {
        open: true,
        message: state != State::Done,
        stop: state == State::Working,
    };
    let events = [
        (
            "00:42",
            State::Done,
            "Read the transport and existing retry handling",
        ),
        (
            "01:00",
            State::Working,
            "Updated reconnect receipts in two files",
        ),
        (
            "01:18",
            State::NeedsYou,
            "Interrupted upload needs a retry decision",
        ),
        (
            "01:36",
            State::Working,
            "Running the focused session checks",
        ),
        (
            "01:54",
            State::Done,
            "18 checks passed; patch ready for review",
        ),
    ];
    author.events = events
        .into_iter()
        .take(step.min(4) + 1)
        .map(|(when, state, message)| SubagentEvent::new(when, state, message))
        .collect();
    let review_state = if step >= 4 {
        State::Done
    } else {
        State::Working
    };
    let mut review = Subagent::new(
        "review",
        AgentCard::new("Reef", review_state)
            .role("Independent review")
            .task(if step >= 4 {
                "No material findings in the reconnect patch"
            } else {
                "Read the patch and inspect permission boundaries"
            }),
    );
    review.elapsed = Some(Duration::from_secs(31 + step as u64 * 10));
    review.tokens = Some("6.2k tokens".into());
    review.performance = Some(performance(
        if step >= 4 {
            Presence::Done
        } else {
            Presence::Working
        },
        (step < 4).then_some("reading"),
        "review-turn",
    ));
    review.controls = SubagentControls {
        open: true,
        message: step < 4,
        stop: step < 4,
    };
    review.events = vec![SubagentEvent::new(
        "00:31",
        review_state,
        if step >= 4 {
            "Reviewed the patch; no material findings"
        } else {
            "Checking that only the owner can authorize retry"
        },
    )];
    let test_state = if step >= 4 {
        State::Done
    } else if step >= 3 {
        State::Working
    } else {
        State::Ready
    };
    let mut checks = Subagent::new(
        "checks",
        AgentCard::new("Current", test_state)
            .role("Validation")
            .task(match test_state {
                State::Done => "18 focused checks passed",
                State::Working => "Run reconnect and retained-history checks",
                _ => "Waiting for the implementation patch",
            })
            .status_word(if test_state == State::Ready {
                "Queued"
            } else {
                test_state.word()
            }),
    );
    checks.performance = Some(performance(
        match test_state {
            State::Done => Presence::Done,
            State::Working => Presence::Working,
            _ => Presence::Idle,
        },
        (test_state == State::Working).then_some("executing"),
        "checks-turn",
    ));
    checks.checking = test_state == State::Working;
    checks.progress = Some(
        CountBar::new(
            if step >= 4 {
                18
            } else if step >= 3 {
                12
            } else {
                0
            },
            18,
        )
        .state(test_state)
        .label("checks"),
    );
    checks.controls.open = true;
    checks.controls.stop = test_state == State::Working;
    checks.events = vec![SubagentEvent::new(
        "01:36",
        test_state,
        checks.card.task.to_string(),
    )];
    let mut docs = Subagent::new(
        "notes",
        AgentCard::new("Tide", State::Done)
            .role("Documentation")
            .task("Recorded the existing reconnect behavior"),
    );
    docs.elapsed = Some(Duration::from_secs(28));
    docs.tokens = Some("3.1k tokens".into());
    docs.performance = Some(performance(Presence::Done, None, "notes-turn"));
    docs.controls.open = true;
    docs.events = vec![SubagentEvent::new(
        "00:28",
        State::Done,
        "Saved the transport notes for implementation and review",
    )];
    vec![author, review, checks, docs]
}

struct Demo {
    state: SubagentViewState,
    step: usize,
    motion: MotionMode,
    profile: Profile,
    tour: bool,
    changed: Instant,
    notice: String,
}

impl Demo {
    fn new(profile: Profile, now: Instant) -> Self {
        Self {
            state: SubagentViewState::default(),
            step: 0,
            motion: MotionMode::Full,
            profile,
            tour: true,
            changed: now,
            notice: "Demo data / no workers are started".into(),
        }
    }
    fn advance(&mut self, now: Instant) {
        self.step = (self.step + 1).min(4);
        self.changed = now;
        if self.step == 4 {
            self.tour = false;
        }
    }
    fn draw(&mut self, area: Rect, buf: &mut Buffer, now: Instant) {
        let theme = self.profile.theme();
        let agents = agents(self.step);
        self.state.update(&agents, now, self.motion);
        let inner = Rect {
            height: area.height.saturating_sub(2),
            ..area
        };
        SubagentView::new(&agents, &theme).render(inner, buf, &mut self.state);
        if area.height >= 2 {
            buf.set_style(
                Rect::new(area.x, area.bottom() - 2, area.width, 2),
                theme.bg(Role::Sidebar),
            );
            buf.set_stringn(
                area.x,
                area.bottom() - 2,
                &self.notice,
                usize::from(area.width),
                theme.fg(Role::Muted),
            );
            let label = format!(
                "N phase   A tour   R replay   L motion   P theme   Q close / {} / {:?}",
                self.profile.name(),
                self.motion
            );
            buf.set_stringn(
                area.x,
                area.bottom() - 1,
                label,
                usize::from(area.width),
                theme.fg(Role::Primary),
            );
        }
    }
    fn intent(&mut self, intent: SubagentIntent) {
        self.notice = match intent {
            SubagentIntent::Open(id) => {
                format!("Demo: {id} transcript requested / use Left/Right for its activity pane")
            }
            SubagentIntent::Message(id) => {
                format!("Demo: message intent for {id} / no message sent")
            }
            SubagentIntent::Stop(id) => {
                format!("Demo: stop intent for {id} / status awaits owner confirmation")
            }
        };
    }
}

fn export(
    dir: &Path,
    profile: Profile,
    width: u16,
    height: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let started = Instant::now();
    let mut demo = Demo::new(profile, started);
    let mut manifest = Vec::new();
    for index in 0..200 {
        let elapsed_ms = index * 80;
        let now = started + Duration::from_millis(elapsed_ms);
        demo.step = (index / 40) as usize;
        if width < 88 && index == 80 {
            demo.state.handle_key(
                &agents(demo.step),
                event::KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            );
        }
        let theme = profile.theme();
        let buf = testing::render(width, height, |area, buf| demo.draw(area, buf, now));
        let file = format!("frame-{index:03}.svg");
        std::fs::write(dir.join(&file), testing::svg(&buf, &theme))?;
        manifest.push(serde_json::json!({"file":file,"elapsed_ms":elapsed_ms,"profile":profile.name(),"width":width,"height":height}));
    }
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "Exported {} subagent frames to {}",
        manifest.len(),
        dir.display()
    );
    Ok(())
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
    let (mut frames, mut profile) = (None, None);
    let (mut width, mut height) = (112, 38);
    while let Some(arg) = args.next() {
        match arg.as_str() {
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
            _ => {
                return Err(
                    "usage: subagents [--profile NAME] [--frames DIR] [--size COLSxROWS]".into(),
                );
            }
        }
    }
    if let Some(dir) = frames {
        return export(
            Path::new(&dir),
            profile.unwrap_or(Profile::DarkTrue),
            width,
            height,
        );
    }
    codewhale_ratatui::detect::probe_terminal_background();
    let profile = profile.unwrap_or_else(|| {
        let caps = Theme::detect().caps();
        Profile::ALL
            .into_iter()
            .find(|p| p.caps() == caps)
            .unwrap_or(Profile::UnknownGround)
    });
    let mut demo = Demo::new(profile, Instant::now());
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    loop {
        let now = Instant::now();
        if demo.tour && now.saturating_duration_since(demo.changed) >= Duration::from_secs(4) {
            demo.advance(now);
        }
        terminal.draw(|frame| demo.draw(frame.area(), frame.buffer_mut(), now))?;
        let mut wait = demo
            .state
            .next_frame_in(&agents(demo.step), &demo.profile.theme())
            .unwrap_or(Duration::from_secs(60));
        if demo.tour {
            wait = wait.min(Duration::from_secs(4).saturating_sub(demo.changed.elapsed()));
        }
        if !event::poll(wait)? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('n') => {
                    demo.tour = false;
                    demo.advance(Instant::now());
                }
                KeyCode::Char('r') => {
                    demo.step = 0;
                    demo.tour = true;
                    demo.changed = Instant::now();
                }
                KeyCode::Char('a') => {
                    demo.tour = !demo.tour;
                    demo.changed = Instant::now();
                }
                KeyCode::Char('l') => {
                    demo.motion = if demo.motion.animates() {
                        MotionMode::Reduced
                    } else {
                        MotionMode::Full
                    };
                }
                KeyCode::Char('p') => {
                    let at = Profile::ALL
                        .iter()
                        .position(|p| *p == demo.profile)
                        .unwrap_or(0);
                    demo.profile = Profile::ALL[(at + 1) % Profile::ALL.len()];
                }
                _ => {
                    if let Some(intent) = demo.state.handle_key(&agents(demo.step), key) {
                        demo.intent(intent);
                    }
                }
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
    Ok(())
}
