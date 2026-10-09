//! Five small public-API recipes. Run with `cargo run --example recipes`.
//! F1–F5 select a recipe; type and Enter in Composer; m changes motion elsewhere.
//! Esc or Ctrl+C closes. The host owns input, application state and scheduling.
use std::{io, time::Instant};

use codewhale_ratatui::{MotionMode, TextInputOutcome, TextInputState, Theme};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

// recipe:composer:start
pub fn draw_composer(
    area: ratatui::layout::Rect,
    buffer: &mut ratatui::buffer::Buffer,
    theme: &codewhale_ratatui::Theme,
    draft: &codewhale_ratatui::TextInputState,
) -> Option<ratatui::layout::Position> {
    use codewhale_ratatui::{NativeComposer, Paint};
    let area = area.intersection(buffer.area);
    if area.is_empty() {
        return None;
    }
    let composer = NativeComposer::new(draft.text())
        .focused(true)
        .cursor(draft.cursor())
        .can_submit(!draft.is_empty())
        .placeholder("Type a line, then press Enter");
    let height = composer.desired_height(area.width, area.height);
    let slot = ratatui::layout::Rect::new(area.x, area.bottom() - height, area.width, height);
    composer.paint(slot, buffer, theme);
    composer.cursor_position(slot)
}
// recipe:composer:end

// recipe:workbar:start
pub fn draw_workbar(
    area: ratatui::layout::Rect,
    buffer: &mut ratatui::buffer::Buffer,
    theme: &codewhale_ratatui::Theme,
) {
    use codewhale_ratatui::{Paint, Workbar, WorkbarPanel, WorkbarRow};
    // Replace these example rows with your application's current tasks.
    Workbar::new(
        WorkbarPanel::Tasks,
        vec![
            WorkbarRow::new("read", "Read the starter example").mark("✓"),
            WorkbarRow::new("build", "Add a component to your app").mark("●"),
        ],
    )
    .goal("Your first Codewhale component")
    .paint(area, buffer, theme);
}
// recipe:workbar:end

// recipe:ocean:start
pub fn draw_ocean(
    area: ratatui::layout::Rect,
    buffer: &mut ratatui::buffer::Buffer,
    theme: &codewhale_ratatui::Theme,
) {
    use codewhale_ratatui::{
        Message, MotionMode, NativeComposer, OceanColumn, Paint, PostureBar, TerminalShell,
        TuiGround,
    };
    use std::time::Duration;
    let area = area.intersection(buffer.area);
    if area.is_empty() {
        return;
    }
    let composer = NativeComposer::new("").placeholder("One continuous water column");
    let shell = TerminalShell::new(composer.desired_height(area.width, area.height));
    let slots = shell.areas(area);
    shell.paint(area, buffer, theme);
    Message::native("Paint content first; finish ordinary grounds with the water.").paint(
        slots.conversation,
        buffer,
        theme,
    );
    composer.paint(slots.composer, buffer, theme);
    PostureBar::new("ask").paint(slots.posture, buffer, theme);
    let water = OceanColumn::new(Duration::ZERO, MotionMode::Still).viewport(area);
    water.apply(area, buffer, theme);
    if let Some(ground) = theme.tui_ground(TuiGround::Composer).bg {
        water.apply_matching(slots.composer, buffer, theme, ground);
    }
}
// recipe:ocean:end

// recipe:whale:start
pub fn draw_whale(
    area: ratatui::layout::Rect,
    buffer: &mut ratatui::buffer::Buffer,
    theme: &codewhale_ratatui::Theme,
) {
    use codewhale_ratatui::{Paint, Whale, WhaleState};
    // A still pose needs no clock. Narrow and ASCII viewports retain the words.
    Whale::new(WhaleState::Rest)
        .words("Ready when you are")
        .paint(area, buffer, theme);
}
// recipe:whale:end

// recipe:animated-whale:start
pub fn draw_animated_whale(
    area: ratatui::layout::Rect,
    buffer: &mut ratatui::buffer::Buffer,
    theme: &codewhale_ratatui::Theme,
    stage: &mut codewhale_ratatui::whale_motion::Stage,
    now: std::time::Instant,
) {
    use codewhale_ratatui::whale_motion::colored_braille;
    use codewhale_ratatui::{Whale, WhaleState, detect::Appearance};
    // Keep this Stage in the host. Observe its inputs before drawing this frame.
    let area = area.intersection(buffer.area);
    stage.set_visible(area.width >= 16 && area.height >= 9 && !theme.ascii());
    stage.advance(now);
    let grid = colored_braille(
        stage.director(),
        usize::from(area.width.min(32)),
        usize::from(area.height.saturating_sub(1).min(16)),
        theme.caps().appearance != Appearance::Light,
    );
    // The host reports Working; this example has no agent or provider connection.
    grid.paint(&Whale::new(WhaleState::Busy), area, buffer, theme);
}
// recipe:animated-whale:end

fn main() -> io::Result<()> {
    use codewhale_ratatui::{
        Role,
        whale_motion::{Context, Inputs, Presence, Stage, Tier},
    };
    use crossterm::{
        event::{DisableBracketedPaste, EnableBracketedPaste},
        execute,
    };
    struct PasteMode;
    impl Drop for PasteMode {
        fn drop(&mut self) {
            let _ = execute!(io::stdout(), DisableBracketedPaste);
        }
    }
    ratatui::run(|terminal| {
        execute!(io::stdout(), EnableBracketedPaste)?;
        let _paste_mode = PasteMode;
        let theme = Theme::detect().tui();
        let mut draft = TextInputState::new();
        let mut stage = Stage::new();
        let mut recipe = 1;
        let mut motion = MotionMode::Full;
        let mut submitted = String::new();
        loop {
            let now = Instant::now();
            stage.observe(
                Some("recipe-example"),
                Inputs {
                    presence: Presence::Working,
                    activity: None,
                    context: Context::default(),
                },
                !motion.animates(),
            );
            stage.set_visible(recipe == 5);
            terminal.draw(|frame| {
                let area = frame.area();
                if area.height < 3 {
                    stage.set_visible(false);
                    return;
                }
                let body =
                    ratatui::layout::Rect::new(area.x, area.y + 1, area.width, area.height - 2);
                let buffer = frame.buffer_mut();
                buffer.set_style(
                    area,
                    theme.bg(Role::Background).patch(theme.fg(Role::Foreground)),
                );
                let help = format!(
                    "F1 Composer  F2 Workbar  F3 Water  F4 Whale  F5 Live / {motion:?} / Esc close"
                );
                buffer.set_stringn(
                    area.x,
                    area.y,
                    help,
                    usize::from(area.width),
                    theme.fg(Role::Muted),
                );
                buffer.set_stringn(
                    area.x,
                    area.bottom() - 1,
                    &submitted,
                    usize::from(area.width),
                    theme.fg(Role::Foreground),
                );
                let cursor = match recipe {
                    1 => draw_composer(body, buffer, &theme, &draft),
                    2 => {
                        draw_workbar(body, buffer, &theme);
                        None
                    }
                    3 => {
                        draw_ocean(body, buffer, &theme);
                        None
                    }
                    4 => {
                        draw_whale(body, buffer, &theme);
                        None
                    }
                    _ => {
                        draw_animated_whale(body, buffer, &theme, &mut stage, now);
                        None
                    }
                };
                if let Some(cursor) = cursor {
                    frame.set_cursor_position(cursor);
                }
            })?;
            if let Some(wait) = stage.cadence(Tier::Terminal)
                && !event::poll(wait)?
            {
                continue;
            }
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Esc if key.is_press() => break,
                    KeyCode::Char('c')
                        if key.is_press() && key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        break;
                    }
                    KeyCode::F(n @ 1..=5) if key.is_press() => recipe = n,
                    KeyCode::Char('m') if key.is_press() && recipe != 1 => {
                        motion = match motion {
                            MotionMode::Full => MotionMode::Reduced,
                            MotionMode::Reduced => MotionMode::Still,
                            MotionMode::Still => MotionMode::Full,
                        }
                    }
                    _ if recipe == 1 && draft.handle_key(key) == TextInputOutcome::Submitted => {
                        submitted = format!("You typed: {}", draft.text());
                        draft.clear();
                    }
                    _ => {}
                },
                Event::Paste(text) if recipe == 1 => {
                    draft.paste(&text);
                }
                _ => {}
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codewhale_ratatui::{
        testing::Profile,
        whale_motion::{Context, Inputs, Presence, Stage, Tier},
    };
    use ratatui::{buffer::Buffer, layout::Rect, style::Color};

    fn words(buffer: &Buffer) -> String {
        let mut text = String::new();
        for y in buffer.area.y..buffer.area.bottom() {
            for x in buffer.area.x..buffer.area.right() {
                text.push_str(buffer[(x, y)].symbol());
            }
        }
        text
    }

    fn working_stage(reduced: bool) -> Stage {
        let mut stage = Stage::new();
        stage.observe(
            Some("test-session"),
            Inputs {
                presence: Presence::Working,
                activity: None,
                context: Context::default(),
            },
            reduced,
        );
        stage
    }

    #[test]
    fn composer_keeps_unicode_draft_and_returns_a_visible_cursor() {
        let draft = TextInputState::with_text("海🌊 cafe\u{301}");
        let before = (draft.text().to_owned(), draft.cursor());
        let area = Rect::new(7, 5, 64, 12);
        let mut buffer = Buffer::empty(area);
        let cursor = draw_composer(area, &mut buffer, &Profile::DarkTrue.theme().tui(), &draft)
            .expect("editable composer has a cursor");
        assert!(area.contains(cursor));
        assert_eq!(
            (draft.text(), draft.cursor()),
            (before.0.as_str(), before.1)
        );
        let rendered = words(&buffer);
        assert!(rendered.contains("海"));
        assert!(rendered.contains("🌊"));
        assert!(rendered.contains("cafe\u{301}"));
    }

    #[test]
    fn native_recipes_show_tasks_water_and_the_whale_caption() {
        let area = Rect::new(0, 0, 80, 24);
        let theme = Profile::DarkTrue.theme().tui();
        let mut tasks = Buffer::empty(area);
        draw_workbar(area, &mut tasks, &theme);
        let task_words = words(&tasks);
        assert!(task_words.contains("Tasks"));
        assert!(task_words.contains("Read the starter example"));
        assert!(task_words.contains("Add a component to your app"));

        let mut ocean = Buffer::empty(area);
        draw_ocean(area, &mut ocean, &theme);
        let ocean_words = words(&ocean);
        assert!(ocean_words.contains("Paint content first"));
        assert!(ocean_words.contains("One continuous water column"));
        assert_ne!(ocean[(0, 0)].bg, ocean[(0, area.bottom() - 1)].bg);

        let mut whale = Buffer::empty(area);
        draw_whale(area, &mut whale, &theme);
        let whale_words = words(&whale);
        assert!(whale_words.contains("Ready when you are"));
        assert!(
            whale_words
                .chars()
                .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
        );
    }

    #[test]
    fn every_recipe_preserves_cells_outside_tiny_and_clipped_viewports() {
        let bounds = Rect::new(8, 6, 36, 20);
        let theme = Profile::DarkTrue.theme().tui();
        let draft = TextInputState::with_text("海🌊");
        for area in [
            Rect::new(12, 8, 24, 14),
            Rect::new(30, 20, 30, 20),
            Rect::new(9, 7, 1, 1),
            Rect::new(10, 9, 0, 0),
        ] {
            for recipe in 1..=5 {
                let mut buffer = Buffer::empty(bounds);
                for y in bounds.y..bounds.bottom() {
                    for x in bounds.x..bounds.right() {
                        buffer[(x, y)]
                            .set_symbol("!")
                            .set_fg(Color::Magenta)
                            .set_bg(Color::Blue);
                    }
                }
                let before = buffer.clone();
                match recipe {
                    1 => {
                        draw_composer(area, &mut buffer, &theme, &draft);
                    }
                    2 => draw_workbar(area, &mut buffer, &theme),
                    3 => draw_ocean(area, &mut buffer, &theme),
                    4 => draw_whale(area, &mut buffer, &theme),
                    _ => draw_animated_whale(
                        area,
                        &mut buffer,
                        &theme,
                        &mut working_stage(false),
                        Instant::now(),
                    ),
                }
                for y in bounds.y..bounds.bottom() {
                    for x in bounds.x..bounds.right() {
                        if !area.contains((x, y).into()) {
                            assert_eq!(
                                buffer[(x, y)],
                                before[(x, y)],
                                "recipe {recipe}, {area:?}, ({x}, {y})"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn live_whale_schedules_only_visible_full_motion_art() {
        let theme = Profile::DarkTrue.theme().tui();
        let area = Rect::new(0, 0, 32, 17);
        let mut buffer = Buffer::empty(area);
        let mut reduced = working_stage(true);
        draw_animated_whale(area, &mut buffer, &theme, &mut reduced, Instant::now());
        assert!(words(&buffer).contains("Working"));
        assert_eq!(reduced.cadence(Tier::Terminal), None);

        let mut stage = working_stage(false);
        draw_animated_whale(area, &mut buffer, &theme, &mut stage, Instant::now());
        assert!(stage.cadence(Tier::Terminal).is_some());
        let tiny = Rect::new(0, 0, 14, 3);
        let mut fallback = Buffer::empty(tiny);
        draw_animated_whale(tiny, &mut fallback, &theme, &mut stage, Instant::now());
        assert!(words(&fallback).contains("Working"));
        assert_eq!(stage.cadence(Tier::Terminal), None);
        draw_animated_whale(
            area,
            &mut buffer,
            &Profile::Ascii.theme(),
            &mut stage,
            Instant::now(),
        );
        assert_eq!(stage.cadence(Tier::Terminal), None);
    }
}
