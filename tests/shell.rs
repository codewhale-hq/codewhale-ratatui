use codewhale_ratatui::TerminalShell;
use ratatui::layout::Rect;
#[test]
fn source_slot_order_and_height_budget_hold_at_offset_and_tiny_sizes() {
    for (width, height) in [(0, 0), (1, 1), (40, 12), (80, 24), (112, 38)] {
        let area = Rect::new(7, 11, width, height);
        let slots = TerminalShell::new(5)
            .pending_rows(3)
            .workflow_rows(6)
            .workbar_rows(5)
            .areas(area);
        let ordered = [
            slots.conversation,
            slots.pending,
            slots.composer,
            slots.posture,
            slots.workflows,
            slots.metrics,
            slots.workbar,
        ];
        let mut y = area.y;
        for slot in ordered {
            assert_eq!(slot.y, y);
            assert_eq!(slot.width, width);
            y = y.saturating_add(slot.height);
            assert!(y <= area.bottom());
        }
        assert_eq!(y, area.bottom());
        assert!(slots.pending.height <= 4);
        if height >= 12 {
            assert!(slots.conversation.height >= 3);
            assert_eq!(slots.posture.height, 1);
            assert_eq!(slots.metrics.height, 1);
        }
    }
}

#[test]
fn terminal_shell_narrow_width_bounds_40_to_80_cols() {
    let theme = codewhale_ratatui::testing::Profile::DarkTrue.theme();
    for width in (40u16..=80).step_by(8) {
        let area = Rect::new(0, 0, width, 24);
        let shell = TerminalShell::new(4)
            .pending_rows(2)
            .workflow_rows(2)
            .workbar_rows(6);
        let slots = shell.areas(area);
        assert_eq!(slots.conversation.width, width);
        assert_eq!(slots.workbar.width, width);
        assert_eq!(slots.composer.height, 4);
        assert!(slots.conversation.height >= 3);
        let buf = codewhale_ratatui::testing::render(width, 24, |rect, buf| {
            codewhale_ratatui::Paint::paint(&shell, rect, buf, &theme);
        });
        assert_eq!(buf.area.width, width);
    }
}
