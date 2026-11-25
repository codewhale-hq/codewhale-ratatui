use codewhale_ratatui::{
    Depth, Paint, Panel, Role,
    testing::{Profile, render, text},
};
use ratatui::layout::Rect;

#[test]
fn tonal_panel_is_borderless_when_grounds_differ_and_quiet_rail_otherwise() {
    for profile in Profile::ALL {
        for theme in [profile.theme(), profile.theme().without_base_ground()] {
            let buf = render(24, 4, |area, buf| {
                let body = Panel::tonal().title("Summary").draw(area, buf, &theme);
                assert!(body.width <= area.width);
                assert!(body.height <= area.height);
            });
            let rendered = text(&buf);
            assert!(
                !rendered.starts_with(['╭', '+']),
                "{}: tonal panel must never draw a full box border",
                profile.name()
            );

            let first_sym = buf[(0, 0)].symbol();
            let has_rail = first_sym == "▏" || first_sym == "|";
            let has_ground = theme.grounds_differ(Role::Surface, Role::Background);
            assert!(
                has_rail || has_ground,
                "{}: tonal panel must separate via Surface ground or quiet left rail",
                profile.name()
            );

            if has_rail {
                let expected_sym = if theme.ascii() { "|" } else { "▏" };
                assert_eq!(first_sym, expected_sym, "{}", profile.name());
                if let Some(border_color) = theme.color(Role::Border) {
                    assert_eq!(buf[(0, 0)].fg, border_color, "{}", profile.name());
                }
                if let Some(strong) = theme.color(Role::BorderStrong)
                    && theme.color(Role::Border) != Some(strong)
                {
                    assert_ne!(
                        buf[(0, 0)].fg,
                        strong,
                        "{}: tonal rail must never use BorderStrong",
                        profile.name()
                    );
                }
            }
        }
    }

    let dark = Profile::DarkTrue.theme();
    let buf = render(24, 4, |area, buf| {
        Panel::tonal().title("Notes").paint(area, buf, &dark);
    });
    assert_eq!(buf[(0, 0)].symbol(), " ");
    assert_eq!(buf[(0, 0)].bg, dark.bg(Role::Surface).bg.unwrap());

    let ansi16 = Profile::Ansi16.theme();
    let buf = render(24, 4, |area, buf| {
        Panel::tonal().title("Notes").paint(area, buf, &ansi16);
    });
    assert_eq!(buf[(0, 0)].symbol(), "▏");

    let ascii = Profile::Ascii.theme();
    let buf = render(24, 4, |area, buf| {
        Panel::tonal().title("Notes").paint(area, buf, &ascii);
    });
    assert_eq!(buf[(0, 0)].symbol(), "|");
}

#[test]
fn tonal_panel_body_matches_draw_across_narrow_widths() {
    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in 0..=24 {
            let area = Rect::new(0, 0, width, 4);
            let panel = Panel::tonal().title("Tonal");
            let expected = panel.body(area, &theme);
            let mut actual = Rect::default();
            let _ = render(width.max(1), 4, |_, buf| {
                actual = panel.draw(area, buf, &theme);
            });
            assert_eq!(actual, expected, "{} width={width}", profile.name());
        }
    }
    assert_eq!(Depth::Tonal.ground(), Role::Surface);
}
