use codewhale_ratatui::{Caps, Message, NativeComposer, Paint, Theme, Themed};
use codewhale_ratatui::{color::ColorDepth, detect::Appearance};
use ratatui::{Terminal, backend::TestBackend};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = Theme::new(Caps {
        depth: ColorDepth::TrueColor,
        ascii: false,
        appearance: Appearance::Dark,
    })
    .tui();
    let components: Vec<Box<dyn Paint>> = vec![
        Box::new(Message::native("Use your own backend and state.")),
        Box::new(NativeComposer::new("A borrowed component")),
    ];
    let mut terminal = Terminal::new(TestBackend::new(40, 6))?;
    for component in &components {
        let widget = Themed::new(component.as_ref(), &theme);
        terminal.draw(|frame| frame.render_widget(&widget, frame.area()))?;
        let first = terminal.backend().buffer().clone();
        terminal.draw(|frame| frame.render_widget(&widget, frame.area()))?;
        assert_eq!(&first, terminal.backend().buffer());
    }
    Ok(())
}
