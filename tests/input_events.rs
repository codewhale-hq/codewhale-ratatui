//! Activation is deliberate; holding a key may repeat editing/navigation.
use codewhale_ratatui::{
    FormField, FormOutcome, FormState, ListOutcome, ListState, Picker, PickerItem, PickerOutcome,
    PickerState, TextInputOutcome, TextInputState,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;

fn key(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> KeyEvent {
    KeyEvent::new_with_kind(code, modifiers, kind)
}

fn accidental_activation() -> Vec<KeyEvent> {
    let mut events = Vec::new();
    for code in [KeyCode::Enter, KeyCode::Esc] {
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            events.push(key(code, KeyModifiers::NONE, kind));
        }
        for modifiers in [
            KeyModifiers::SHIFT,
            KeyModifiers::CONTROL,
            KeyModifiers::ALT,
            KeyModifiers::SUPER,
        ] {
            events.push(key(code, modifiers, KeyEventKind::Press));
        }
    }
    events
}

#[test]
fn inputs_leave_modified_and_held_activation_to_the_host() {
    let mut input = TextInputState::with_text("keep 海洋");
    for event in accidental_activation() {
        assert_eq!(
            input.handle_key(event),
            TextInputOutcome::Ignored,
            "{event:?}"
        );
        assert_eq!(input.text(), "keep 海洋");
    }
    assert_eq!(
        input.handle_key(key(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Press)),
        TextInputOutcome::Submitted
    );
    assert_eq!(
        input.handle_key(key(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Press)),
        TextInputOutcome::Cancelled
    );
}

#[test]
fn repeated_typing_and_movement_still_edit() {
    let mut input = TextInputState::new();
    for c in "海洋".chars() {
        assert_eq!(
            input.handle_key(key(
                KeyCode::Char(c),
                KeyModifiers::NONE,
                KeyEventKind::Repeat
            )),
            TextInputOutcome::Changed
        );
    }
    assert_eq!(
        input.handle_key(key(KeyCode::Left, KeyModifiers::NONE, KeyEventKind::Repeat)),
        TextInputOutcome::Changed
    );
    assert_eq!(input.buffer().cursor(), 1);
    assert_eq!(input.text(), "海洋");
}

#[test]
fn altgr_typing_follows_the_platform_event_contract() {
    let mut input = TextInputState::new();
    let outcome = input.handle_key(key(
        KeyCode::Char('€'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
        KeyEventKind::Press,
    ));
    if cfg!(windows) {
        assert_eq!(outcome, TextInputOutcome::Changed);
        assert_eq!(input.text(), "€");
    } else {
        assert_eq!(outcome, TextInputOutcome::Ignored);
        assert!(input.text().is_empty());
    }
}

#[test]
fn forms_do_not_submit_cancel_or_toggle_on_a_held_key() {
    let mut form = FormState::new(vec![FormField::check("Keep result", false)]);
    for event in accidental_activation().into_iter().chain([
        key(KeyCode::Char(' '), KeyModifiers::NONE, KeyEventKind::Repeat),
        key(KeyCode::Char(' '), KeyModifiers::SUPER, KeyEventKind::Press),
    ]) {
        assert_eq!(form.handle_key(event), FormOutcome::Ignored, "{event:?}");
        assert_eq!(form.checked(0), Some(false));
        assert!(!form.is_dirty());
    }
    assert_eq!(
        form.handle_key(key(
            KeyCode::Char(' '),
            KeyModifiers::NONE,
            KeyEventKind::Press
        )),
        FormOutcome::Changed
    );
    assert_eq!(form.checked(0), Some(true));
}

#[test]
fn form_navigation_respects_host_shortcuts() {
    let mut form = FormState::new(vec![FormField::text("First"), FormField::text("Second")]);
    for code in [KeyCode::Up, KeyCode::Down, KeyCode::Tab, KeyCode::BackTab] {
        assert_eq!(
            form.handle_key(key(code, KeyModifiers::CONTROL, KeyEventKind::Press)),
            FormOutcome::Ignored
        );
        assert_eq!(form.focus(), 0);
    }
    assert_eq!(
        form.handle_key(key(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Repeat)),
        FormOutcome::Changed
    );
    assert_eq!(form.focus(), 1);
    assert_eq!(
        form.handle_key(key(
            KeyCode::BackTab,
            KeyModifiers::SHIFT,
            KeyEventKind::Press
        )),
        FormOutcome::Changed
    );
    assert_eq!(form.focus(), 0);
}

#[test]
fn lists_and_basic_pickers_repeat_navigation_only() {
    let rows = ["First", "Second", "Third"];
    let area = Rect::new(0, 0, 20, 3);
    let mut list = ListState::default();
    let mut picker = PickerState::default();
    for event in accidental_activation().into_iter().chain([
        key(KeyCode::Char(' '), KeyModifiers::NONE, KeyEventKind::Repeat),
        key(KeyCode::Down, KeyModifiers::CONTROL, KeyEventKind::Press),
    ]) {
        assert_eq!(
            list.handle_key(event, &rows, area),
            ListOutcome::Ignored,
            "{event:?}"
        );
        assert_eq!(
            picker.handle_key(event, rows.len(), area.height),
            PickerOutcome::Ignored
        );
        assert_eq!(list, ListState::default());
        assert_eq!(picker, PickerState::default());
    }
    let down = key(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Repeat);
    assert_eq!(list.handle_key(down, &rows, area), ListOutcome::Moved);
    assert_eq!(
        picker.handle_key(down, rows.len(), area.height),
        PickerOutcome::Moved
    );
    assert_eq!(list.selected, 1);
    assert_eq!(picker.selected, 1);
}

#[test]
fn query_pickers_repeat_query_input_but_never_toggle_or_choose() {
    let items = [PickerItem::new("First"), PickerItem::new("Second")];
    let picker = Picker::new(&items, PickerState::default()).query("", 0);
    let mut state = PickerState::default();
    for event in accidental_activation().into_iter().chain([
        key(
            KeyCode::Char(' '),
            KeyModifiers::CONTROL,
            KeyEventKind::Repeat,
        ),
        key(KeyCode::Tab, KeyModifiers::SUPER, KeyEventKind::Press),
    ]) {
        assert_eq!(
            state.handle_picker_key(event, &picker, 3),
            PickerOutcome::Ignored,
            "{event:?}"
        );
        assert_eq!(state, PickerState::default());
    }
    for code in [KeyCode::Char(' '), KeyCode::Char('a'), KeyCode::Backspace] {
        assert_eq!(
            state.handle_picker_key(
                key(code, KeyModifiers::NONE, KeyEventKind::Repeat),
                &picker,
                3
            ),
            PickerOutcome::Query
        );
    }
    assert_eq!(
        state.handle_picker_key(
            key(
                KeyCode::Char(' '),
                KeyModifiers::CONTROL,
                KeyEventKind::Press
            ),
            &picker,
            3
        ),
        PickerOutcome::Toggled(0)
    );
}
