//! A live roster and focused agent, over facts supplied by the host.
//! Selection and scroll are local; execution, history and permissions are not.

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Paragraph, StatefulWidget, Widget, Wrap},
};

use super::workbench::row;
use crate::{
    AgentCard, CountBar, MotionMode, Paint, Role, State, Theme, VerificationSpinner, WhalePet,
    duration, glyphs, spin, text,
    whale_motion::{Inputs, Stage, Tier},
};

/// A reported event. Keep these in chronological order; the view follows the
/// newest event until the person scrolls back. Text is sanitized before paint.
#[derive(Clone, Debug)]
pub struct SubagentEvent<'a> {
    pub when: Cow<'a, str>,
    pub state: State,
    pub message: Cow<'a, str>,
}

impl<'a> SubagentEvent<'a> {
    #[must_use]
    pub fn new(
        when: impl Into<Cow<'a, str>>,
        state: State,
        message: impl Into<Cow<'a, str>>,
    ) -> Self {
        Self {
            when: when.into(),
            state,
            message: message.into(),
        }
    }
}

/// Capabilities the owner currently allows. An intent still needs the host's
/// normal validation and permission path; a component never executes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubagentControls {
    pub open: bool,
    pub message: bool,
    pub stop: bool,
}

/// One retained roster entry. IDs must be nonempty and unique in this roster.
/// The view never expires a worker or infers success from its counts.
#[derive(Clone, Debug)]
pub struct Subagent<'a> {
    pub id: Cow<'a, str>,
    pub card: AgentCard<'a>,
    pub elapsed: Option<Duration>,
    pub tokens: Option<Cow<'a, str>>,
    pub progress: Option<CountBar>,
    pub events: Vec<SubagentEvent<'a>>,
    pub performance: Option<Inputs>,
    pub checking: bool,
    pub controls: SubagentControls,
}

impl<'a> Subagent<'a> {
    #[must_use]
    pub fn new(id: impl Into<Cow<'a, str>>, card: AgentCard<'a>) -> Self {
        Self {
            id: id.into(),
            card,
            elapsed: None,
            tokens: None,
            progress: None,
            events: Vec::new(),
            performance: None,
            checking: false,
            controls: SubagentControls::default(),
        }
    }

    fn usage(&self) -> String {
        [
            self.elapsed
                .map(duration)
                .or_else(|| self.card.elapsed.as_deref().map(str::to_owned)),
            self.tokens.as_deref().map(str::to_owned),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" / ")
    }
}

/// Returned to the host only. In particular, Stop is a request, not a state
/// transition: the reported worker stays Working until its owner says otherwise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubagentIntent {
    Open(String),
    Message(String),
    Stop(String),
}

/// Local presentation state for one roster. Feed a new roster when the session
/// changes (or replace this state), retaining completed entries in owner data.
/// Deliberately not Clone: the canonical Stage owns a unique performance clock.
/// Keep one state per surface instead of copying an in-flight Director.
pub struct SubagentViewState {
    selected: Option<String>,
    index: usize,
    offset: usize,
    event_top: Option<usize>,
    event_end: usize,
    detail: bool,
    visible: bool,
    now: Option<Instant>,
    working: HashMap<String, (bool, Instant)>,
    motion: MotionMode,
    stage: Stage,
    observed: Option<(String, State)>,
    flourish_until: Option<Instant>,
    shown: usize,
    pet_visible: bool,
}

impl Default for SubagentViewState {
    fn default() -> Self {
        Self {
            selected: None,
            index: 0,
            offset: 0,
            event_top: None,
            event_end: 0,
            detail: false,
            visible: true,
            now: None,
            working: HashMap::new(),
            motion: MotionMode::Full,
            stage: Stage::new(),
            observed: None,
            flourish_until: None,
            shown: 0,
            pet_visible: false,
        }
    }
}

impl SubagentViewState {
    #[must_use]
    pub fn selected_id(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        self.selected.as_ref().map(|_| self.index)
    }

    /// Select by identity, preserving focus across owner reordering. Returns
    /// false for an absent, empty or ambiguous ID rather than choosing a worker.
    pub fn select(&mut self, agents: &[Subagent<'_>], id: &str) -> bool {
        if id.is_empty() || agents.iter().filter(|a| a.id == id).count() != 1 {
            return false;
        }
        let index = agents.iter().position(|a| a.id == id).unwrap_or(0);
        if self.selected.as_deref() != Some(id) {
            self.event_top = None;
            self.event_end = 0;
        }
        self.selected = Some(id.to_owned());
        self.index = index;
        true
    }

    /// Observe explicit owner inputs and advance the existing whale clock once.
    /// Paint never reads or advances time. Reduced/Still and terminal states
    /// settle; an observed Working -> Done earns one brief native flourish.
    pub fn update(&mut self, agents: &[Subagent<'_>], now: Instant, motion: MotionMode) {
        self.now = Some(now);
        self.motion = motion;
        // Presentation onsets only: no worker lifecycle or elapsed-time claims
        // are derived from this cache. Drop entries as soon as work stops.
        let active: HashSet<_> = agents
            .iter()
            .filter(|a| a.card.status.state == State::Working)
            .map(|a| a.id.as_ref())
            .collect();
        self.working
            .retain(|id, _| self.visible && active.contains(id.as_str()));
        if self.visible {
            for agent in agents
                .iter()
                .filter(|a| a.card.status.state == State::Working)
            {
                let phase = self
                    .working
                    .entry(agent.id.to_string())
                    .or_insert((agent.checking, now));
                if phase.0 != agent.checking {
                    *phase = (agent.checking, now);
                }
            }
        }
        let retained = self
            .selected
            .clone()
            .is_some_and(|id| self.select(agents, &id));
        if !retained {
            self.selected = None;
            if let Some(agent) = agents.get(self.index.min(agents.len().saturating_sub(1))) {
                self.select(agents, &agent.id);
            }
        }
        let Some(agent) = self.agent(agents) else {
            self.stage.set_visible(false);
            self.observed = None;
            self.flourish_until = None;
            return;
        };
        let identity = (agent.id.to_string(), agent.card.status.state);
        if self.observed.as_ref() != Some(&identity) {
            self.flourish_until = self
                .observed
                .as_ref()
                .filter(|(id, status)| {
                    id == &identity.0 && *status == State::Working && identity.1 == State::Done
                })
                .and_then(|_| now.checked_add(Duration::from_millis(1400)));
            self.observed = Some(identity);
        }
        let active = agent.card.status.state == State::Working
            || self.flourish_until.is_some_and(|end| now < end);
        self.stage
            .set_visible(self.visible && agent.performance.is_some());
        if let Some(inputs) = &agent.performance {
            self.stage.observe(
                Some(&agent.id),
                inputs.clone(),
                !motion.animates() || !active,
            );
            self.stage.advance(now);
        }
    }

    fn agent<'a, 'b>(&self, agents: &'a [Subagent<'b>]) -> Option<&'a Subagent<'b>> {
        let agent = agents.get(self.index)?;
        let id = self.selected.as_deref()?;
        (agent.id == id && !id.is_empty() && agents.iter().filter(|a| a.id == id).count() == 1)
            .then_some(agent)
    }

    /// Hiding stops animation; resuming discards the hidden interval.
    pub fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        self.stage.set_visible(visible);
        self.working.clear();
    }

    fn mark_elapsed(&self, agent: &Subagent<'_>) -> Duration {
        self.now
            .zip(self.working.get(agent.id.as_ref()).map(|(_, start)| *start))
            .map_or(Duration::ZERO, |(now, start)| {
                now.saturating_duration_since(start)
            })
    }

    /// The host waits for this duration or input, whichever arrives first.
    /// Only visible active rows/the focused pet ask for frames. Settled,
    /// hidden, empty and Reduced/Still views ask for no timed redraws.
    #[must_use]
    pub fn next_frame_in(&self, agents: &[Subagent<'_>], theme: &Theme) -> Option<Duration> {
        if !self.visible || !self.motion.animates() {
            return None;
        }
        let marks = agents
            .iter()
            .skip(self.offset)
            .take(self.shown)
            .filter(|a| a.card.status.state == State::Working)
            .filter_map(|agent| spin::next_frame_in(self.mark_elapsed(agent), self.motion))
            .min();
        let pet = self
            .pet_visible
            .then(|| {
                self.stage.cadence(if theme.paints_grounds() {
                    Tier::Hero
                } else {
                    Tier::Terminal
                })
            })
            .flatten();
        [marks, pet].into_iter().flatten().min()
    }

    /// Arrow/Home/End navigation, Tab to the next NeedsYou/Failed worker,
    /// Left/Right for narrow details, PageUp/PageDown for event history.
    /// Enter/M/X return capability-gated Open/Message/Stop intents.
    pub fn handle_key(&mut self, agents: &[Subagent<'_>], key: KeyEvent) -> Option<SubagentIntent> {
        if !self.visible
            || key.kind == KeyEventKind::Release
            || !key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
        {
            return None;
        }
        let index = self
            .selected
            .as_deref()
            .and_then(|id| agents.iter().position(|a| a.id == id))
            .unwrap_or(0);
        if let Some(id) = self.selected.clone() {
            self.select(agents, &id);
        }
        let target = match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(index.saturating_sub(1)),
            KeyCode::Down | KeyCode::Char('j') => {
                Some((index + 1).min(agents.len().saturating_sub(1)))
            }
            KeyCode::Home => Some(0),
            KeyCode::End => Some(agents.len().saturating_sub(1)),
            KeyCode::Tab | KeyCode::BackTab => (1..=agents.len())
                .map(|step| {
                    if key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT) {
                        (index + agents.len() - step) % agents.len()
                    } else {
                        (index + step) % agents.len()
                    }
                })
                .find(|i| {
                    matches!(
                        agents[*i].card.status.state,
                        State::NeedsYou | State::Failed
                    )
                }),
            KeyCode::Right => {
                self.detail = true;
                None
            }
            KeyCode::Char(' ') => {
                self.detail = !self.detail;
                None
            }
            KeyCode::Left => {
                self.detail = false;
                None
            }
            KeyCode::PageUp => {
                self.event_top = Some(self.event_top.unwrap_or(self.event_end).saturating_sub(5));
                None
            }
            KeyCode::PageDown => {
                self.event_top = self.event_top.and_then(|top| {
                    let next = top.saturating_add(5);
                    (next < self.event_end).then_some(next)
                });
                None
            }
            _ => None,
        };
        if let Some(agent) = target.and_then(|i| agents.get(i)) {
            self.select(agents, &agent.id);
        }
        let agent = self.agent(agents)?;
        // Holding an action key must not repeat a stop/message request.
        if key.kind != KeyEventKind::Press {
            return None;
        }
        match key.code {
            KeyCode::Enter if agent.controls.open => {
                Some(SubagentIntent::Open(agent.id.to_string()))
            }
            KeyCode::Char('m') if agent.controls.message => {
                Some(SubagentIntent::Message(agent.id.to_string()))
            }
            KeyCode::Char('x') if agent.controls.stop => {
                Some(SubagentIntent::Stop(agent.id.to_string()))
            }
            _ => None,
        }
    }
}

/// Caller-owned labels and hints, including localization. State labels remain
/// those on each existing AgentCard; count wording can be supplied separately.
#[derive(Clone, Debug)]
pub struct SubagentViewWords<'a> {
    pub title: Cow<'a, str>,
    pub empty: Cow<'a, str>,
    pub activity: Cow<'a, str>,
    pub no_events: Cow<'a, str>,
    pub no_performance: Cow<'a, str>,
    pub hints: Cow<'a, str>,
    pub open: Cow<'a, str>,
    pub message: Cow<'a, str>,
    pub stop: Cow<'a, str>,
    /// Override the default English summary with the owner's localized counts.
    pub summary: Option<Cow<'a, str>>,
}

impl Default for SubagentViewWords<'_> {
    fn default() -> Self {
        Self {
            title: "Subagents".into(),
            empty: "No agents reported".into(),
            activity: "Activity".into(),
            no_events: "No events reported".into(),
            no_performance: "No activity animation reported".into(),
            hints: "Up/Down select   Tab attention   Space details   PgUp/PgDn history".into(),
            open: "Enter open".into(),
            message: "M message".into(),
            stop: "X stop".into(),
            summary: None,
        }
    }
}

/// A dynamic extension of the existing Fleet/AgentCard presentation. Wide
/// terminals show roster + detail; narrow terminals switch between them.
///
/// ```no_run
/// use codewhale_ratatui::{AgentCard, MotionMode, State, Subagent, SubagentView, SubagentViewState, Theme};
/// let agents = vec![Subagent::new("review", AgentCard::new("Review", State::Working).task("Check the patch"))];
/// let mut state = SubagentViewState::default();
/// let theme = Theme::detect();
/// state.update(&agents, std::time::Instant::now(), MotionMode::Full);
/// # fn draw(frame: &mut ratatui::Frame<'_>, agents: &[Subagent<'_>], theme: &Theme, state: &mut SubagentViewState) {
/// frame.render_stateful_widget(SubagentView::new(agents, theme), frame.area(), state);
/// # }
/// ```
pub struct SubagentView<'a> {
    agents: &'a [Subagent<'a>],
    theme: &'a Theme,
    words: SubagentViewWords<'a>,
}

impl<'a> SubagentView<'a> {
    #[must_use]
    pub fn new(agents: &'a [Subagent<'a>], theme: &'a Theme) -> Self {
        Self {
            agents,
            theme,
            words: SubagentViewWords::default(),
        }
    }

    #[must_use]
    pub fn words(mut self, words: SubagentViewWords<'a>) -> Self {
        self.words = words;
        self
    }

    /// Pure layout also used by host mouse routing. Detail replaces the roster
    /// on narrow screens; neither is squeezed into an unreadable column.
    #[must_use]
    pub fn areas(area: Rect, detail: bool) -> (Rect, Rect) {
        let body = Rect::new(
            area.x,
            area.y.saturating_add(area.height.min(3)),
            area.width,
            area.height.saturating_sub(5),
        );
        if body.width >= 88 && body.height >= 9 {
            let width = (u32::from(body.width) * 2 / 5).clamp(34, 48) as u16;
            (
                Rect { width, ..body },
                Rect::new(
                    body.x + width + 3,
                    body.y,
                    body.width - width - 3,
                    body.height,
                ),
            )
        } else if detail && body.height >= 5 {
            (Rect::default(), body)
        } else {
            (body, Rect::default())
        }
    }

    fn line(&self, area: Rect, y: u16, value: &str, role: Role, buf: &mut Buffer) {
        if y >= area.height {
            return;
        }
        row(
            Rect {
                y: area.y + y,
                height: 1,
                ..area
            },
            buf,
            &Line::styled(
                text::truncate(
                    &text::display_safe(value),
                    usize::from(area.width),
                    self.theme.ascii(),
                )
                .into_owned(),
                self.theme.fg(role),
            ),
        );
    }

    fn mark(&self, agent: &Subagent<'_>, state: &SubagentViewState) -> &'static str {
        if agent.card.status.state != State::Working {
            agent.card.status.glyph(self.theme)
        } else if agent.checking {
            VerificationSpinner::frame(state.mark_elapsed(agent), state.motion, self.theme.ascii())
        } else {
            spin::frame(state.mark_elapsed(agent), state.motion, self.theme.ascii())
        }
    }

    fn roster(&self, area: Rect, buf: &mut Buffer, state: &mut SubagentViewState) {
        if area.is_empty() {
            state.shown = 0;
            return;
        }
        let size = if area.height >= 8 { 4 } else { 2 };
        // Reserve a final row for the range whenever entries overflow.
        let capacity = usize::from(area.height / size).max(1);
        let clipped = self.agents.len() > capacity;
        state.shown = usize::from(area.height.saturating_sub(u16::from(clipped)) / size).max(1);
        state.offset = state
            .offset
            .min(self.agents.len().saturating_sub(state.shown));
        if state.index < state.offset {
            state.offset = state.index;
        }
        if state.index >= state.offset + state.shown {
            state.offset = state.index + 1 - state.shown;
        }
        for (index, agent) in self
            .agents
            .iter()
            .enumerate()
            .skip(state.offset)
            .take(state.shown)
        {
            let y = ((index - state.offset) as u16) * size;
            let entry = Rect::new(area.x, area.y + y, area.width, size.min(area.height - y));
            let selected = state.selected_id() == Some(&*agent.id);
            if selected {
                buf.set_style(entry, self.theme.bg(Role::Selected));
            }
            let marker = if selected {
                glyphs::pick(glyphs::SELECTION, self.theme.ascii())
            } else {
                " "
            };
            let status = format!(
                "{} {}",
                self.mark(agent, state),
                text::display_safe(&agent.card.status.word)
            );
            let tail = text::width(&status).min(usize::from(entry.width.saturating_sub(4))) as u16;
            let name_width = entry.width.saturating_sub(tail + 3);
            row(
                Rect {
                    width: name_width,
                    height: 1,
                    ..entry
                },
                buf,
                &Line::styled(
                    format!(
                        "{marker} {}",
                        text::truncate(
                            &text::display_safe(&agent.card.title),
                            usize::from(name_width.saturating_sub(2)),
                            self.theme.ascii()
                        )
                    ),
                    self.theme
                        .fg(if selected {
                            Role::Primary
                        } else {
                            Role::Foreground
                        })
                        .add_modifier(Modifier::BOLD),
                ),
            );
            self.line(
                Rect::new(entry.right() - tail, entry.y, tail, 1),
                0,
                &status,
                agent.card.status.state.role(),
                buf,
            );
            let body = Rect::new(
                entry.x + entry.width.min(2),
                entry.y,
                entry.width.saturating_sub(2),
                entry.height,
            );
            self.line(body, 1, &agent.card.task, Role::Foreground, buf);
            if size == 4 {
                let usage = agent.usage();
                let facts: Vec<&str> = [
                    agent.card.role.as_deref(),
                    Some(usage.as_str()).filter(|s| !s.is_empty()),
                ]
                .into_iter()
                .flatten()
                .collect();
                self.line(body, 2, &facts.join(" / "), Role::Muted, buf);
            }
        }
        if clipped {
            self.line(
                area,
                area.height - 1,
                &format!(
                    "{}-{} / {}",
                    state.offset + 1,
                    (state.offset + state.shown).min(self.agents.len()),
                    self.agents.len()
                ),
                Role::Muted,
                buf,
            );
        }
    }

    fn details(&self, area: Rect, buf: &mut Buffer, state: &mut SubagentViewState) {
        state.pet_visible = false;
        if area.is_empty() {
            return;
        }
        let Some(agent) = state.agent(self.agents) else {
            return;
        };
        self.line(area, 0, &agent.card.title, Role::Primary, buf);
        row(
            Rect {
                y: area.y + 1,
                height: 1,
                ..area
            },
            buf,
            &Line::from(agent.card.status.spans(self.theme)),
        );
        let identity: Vec<_> = [agent.card.role.as_deref(), agent.card.route.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        self.line(area, 2, &identity.join(" / "), Role::Muted, buf);
        if area.height >= 5 {
            Paragraph::new(text::display_safe(&agent.card.task).into_owned())
                .style(self.theme.fg(Role::Foreground))
                .wrap(Wrap { trim: false })
                .render(Rect::new(area.x, area.y + 3, area.width, 2), buf);
        } else {
            self.line(area, 3, &agent.card.task, Role::Foreground, buf);
        }
        if area.height >= 7
            && let Some(progress) = &agent.progress
        {
            progress.paint(
                Rect::new(area.x, area.y + 5, area.width, 1),
                buf,
                self.theme,
            );
        }
        self.line(area, 6, &agent.usage(), Role::Muted, buf);
        let pet_height = if area.height >= 19
            && area.width >= 26
            && agent.performance.is_some()
            && !self.theme.ascii()
        {
            area.height.saturating_sub(14).clamp(9, 16)
        } else {
            0
        };
        if pet_height > 0 {
            let pet = Rect::new(area.x, area.y + 7, area.width, pet_height);
            WhalePet::new(self.theme)
                .words(agent.card.status.word.to_string())
                .paint(pet, buf, &mut state.stage);
            state.pet_visible = true;
        }
        let start = 7 + pet_height;
        if area.height <= start {
            return;
        }
        let log = Rect::new(area.x, area.y + start, area.width, area.height - start);
        self.line(log, 0, &self.words.activity, Role::Primary, buf);
        let header = 1 + u16::from(agent.performance.is_none());
        if agent.performance.is_none() {
            self.line(log, 1, &self.words.no_performance, Role::Muted, buf);
        }
        if log.height <= header {
            return;
        }
        let body = Rect::new(log.x, log.y + header, log.width, log.height - header);
        if agent.events.is_empty() {
            self.line(body, 0, &self.words.no_events, Role::Muted, buf);
            return;
        }
        let lines: Vec<_> = agent
            .events
            .iter()
            .map(|event| {
                Line::from(vec![
                    Span::styled(
                        format!("{} ", text::display_safe(&event.when)),
                        self.theme.fg(Role::Muted),
                    ),
                    Span::styled(
                        format!("{} ", glyphs::pick(event.state.glyph(), self.theme.ascii())),
                        self.theme.fg(event.state.role()),
                    ),
                    Span::styled(
                        text::display_safe(&event.message).into_owned(),
                        self.theme.fg(Role::Foreground),
                    ),
                ])
            })
            .collect();
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        let end = paragraph
            .line_count(body.width)
            .saturating_sub(usize::from(body.height));
        state.event_end = end;
        state.event_top = state.event_top.map(|top| top.min(end));
        paragraph
            .scroll((
                state.event_top.unwrap_or(end).min(usize::from(u16::MAX)) as u16,
                0,
            ))
            .render(body, buf);
    }
}

impl StatefulWidget for SubagentView<'_> {
    type State = SubagentViewState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let area = area.intersection(buf.area);
        state.shown = 0;
        state.pet_visible = false;
        if area.is_empty() || !state.visible {
            return;
        }
        buf.set_style(
            area,
            self.theme
                .bg(Role::Background)
                .patch(self.theme.fg(Role::Foreground)),
        );
        self.line(area, 0, &self.words.title, Role::Primary, buf);
        let count = |status| {
            self.agents
                .iter()
                .filter(|a| a.card.status.state == status)
                .count()
        };
        let summary = self
            .words
            .summary
            .as_deref()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "{} working / {} need you / {} done / {} failed / {} total",
                    count(State::Working),
                    count(State::NeedsYou),
                    count(State::Done),
                    count(State::Failed),
                    self.agents.len()
                )
            });
        self.line(area, 1, &summary, Role::Muted, buf);
        let (roster, detail) = Self::areas(area, state.detail);
        if self.agents.is_empty() {
            self.line(
                if roster.is_empty() { detail } else { roster },
                0,
                &self.words.empty,
                Role::Muted,
                buf,
            );
        } else {
            self.roster(roster, buf, state);
            self.details(detail, buf, state);
        }
        if area.height >= 3 {
            self.line(area, area.height - 1, &self.words.hints, Role::Muted, buf);
            let mut actions = Vec::new();
            if let Some(agent) = state.agent(self.agents) {
                if agent.controls.open {
                    actions.push(self.words.open.as_ref());
                }
                if agent.controls.message {
                    actions.push(self.words.message.as_ref());
                }
                if agent.controls.stop {
                    actions.push(self.words.stop.as_ref());
                }
            }
            self.line(
                area,
                area.height - 2,
                &actions.join("   "),
                Role::Primary,
                buf,
            );
        }
    }
}
