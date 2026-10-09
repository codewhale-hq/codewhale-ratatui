use codewhale_ratatui::{
    List, ListState, NativeComposer, Paint, Picker, PickerItem, PickerState, Theme, Themed,
    testing::Profile,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

fn theme() -> Theme {
    Profile::DarkTrue.theme().tui()
}

#[test]
fn borrowed_widgets_render_twice_without_consuming_the_component() {
    let theme = theme();
    let component = NativeComposer::new("A reusable component");
    let widget = component.themed(&theme);
    let area = Rect::new(0, 0, 40, 5);
    let mut first = Buffer::empty(area);
    let mut second = Buffer::empty(area);
    (&widget).render(area, &mut first);
    (&widget).render(area, &mut second);
    assert_eq!(first, second);
    widget.render(area, &mut second);
    assert_eq!(first, second);
}

#[test]
fn erased_components_use_the_same_paint_contract() {
    let theme = theme();
    let components: Vec<Box<dyn Paint>> = vec![Box::new(NativeComposer::new("Borrowed"))];
    let area = Rect::new(7, 5, 40, 5);
    let mut actual = Buffer::empty(area);
    let mut expected = Buffer::empty(area);
    Themed::new(components[0].as_ref(), &theme).render(area, &mut actual);
    components[0].paint(area, &mut expected, &theme);
    assert_eq!(actual, expected);
}

#[test]
fn list_state_persists_the_rendered_offset_through_resize() {
    let theme = theme();
    let rows: Vec<_> = (0..20).map(|i| format!("Row {i}")).collect();
    let list = List::new(&rows, ListState::new(0));
    let widget = list.themed(&theme);
    let mut state = ListState::new(19);
    let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
    terminal
        .draw(|f| f.render_stateful_widget(&widget, f.area(), &mut state))
        .unwrap();
    assert_eq!(state.offset, 16);
    terminal.backend_mut().resize(40, 8);
    terminal
        .draw(|f| f.render_stateful_widget(&widget, f.area(), &mut state))
        .unwrap();
    assert_eq!(state.offset, 12);
    assert_eq!(state.selected, 19);
}

#[test]
fn picker_state_accounts_for_query_rows_and_partial_buffers() {
    let theme = theme();
    let items: Vec<_> = (0..20)
        .map(|i| PickerItem::new(format!("Item {i}")))
        .collect();
    let picker = Picker::new(&items, PickerState::new(0)).query("", 0);
    let widget = picker.themed(&theme);
    let area = Rect::new(7, 5, 40, 5);
    let mut buf = Buffer::empty(area);
    let mut state = PickerState::new(19);
    StatefulWidget::render(&widget, Rect::new(7, 5, 40, 20), &mut buf, &mut state);
    assert_eq!(state.selected, 19);
    assert_eq!(state.offset, 16);
    let mut expected = Buffer::empty(area);
    Picker::new(&items, state)
        .query("", 0)
        .paint(area, &mut expected, &theme);
    assert_eq!(buf, expected);
}

#[test]
fn empty_viewports_preserve_state_and_empty_data_settles_it() {
    let theme = theme();
    let rows = ["one"];
    let mut state = ListState {
        selected: 4,
        offset: 3,
    };
    let original = state;
    let mut buf = Buffer::empty(Rect::new(7, 5, 10, 4));
    StatefulWidget::render(
        List::new(&rows, ListState::default()).themed(&theme),
        Rect::new(0, 0, 2, 2),
        &mut buf,
        &mut state,
    );
    assert_eq!(state, original);
    let empty: [&str; 0] = [];
    let area = buf.area;
    StatefulWidget::render(
        List::new(&empty, ListState::default()).themed(&theme),
        area,
        &mut buf,
        &mut state,
    );
    assert_eq!(state, ListState::default());
}
