use std::time::Duration;

use codewhale_ratatui::{
    Caps, MotionMode, OceanColumn, OceanRamp, Paint, Role, Theme, TuiGround, TuiInk, TuiPalette,
    color::{ColorDepth, rgb_to_ansi256},
    detect::Appearance,
    gallery,
    testing::{Profile, render},
};
use ratatui::style::{Color, Modifier};
use serde_json::Value;

const INKS: [(TuiInk, &str); 11] = [
    (TuiInk::Working, "status_working"),
    (TuiInk::Success, "success"),
    (TuiInk::Warning, "warning"),
    (TuiInk::PermissionAsk, "permission_ask"),
    (TuiInk::PermissionAutoReview, "permission_auto_review"),
    (TuiInk::PermissionFullAccess, "permission_full_access"),
    (TuiInk::ModeWork, "mode_agent"),
    (TuiInk::ModePlan, "mode_plan"),
    (TuiInk::ModeOperate, "mode_operate"),
    (TuiInk::Soft, "text_soft"),
    (TuiInk::Info, "info"),
];

const GROUNDS: [(TuiGround, &str); 9] = [
    (TuiGround::Surface, "surface_bg"),
    (TuiGround::Panel, "panel_bg"),
    (TuiGround::Elevated, "elevated_bg"),
    (TuiGround::Composer, "composer_bg"),
    (TuiGround::Selection, "selection_bg"),
    (TuiGround::Header, "header_bg"),
    (TuiGround::Footer, "footer_bg"),
    (TuiGround::DiffAdded, "diff_added_bg"),
    (TuiGround::DiffRemoved, "diff_deleted_bg"),
];

fn source() -> Value {
    serde_json::from_str(include_str!("../assets/tui-palettes.json")).unwrap()
}

fn color(value: &Value) -> Color {
    if let Some(rgb) = value.as_array() {
        return Color::Rgb(
            rgb[0].as_u64().unwrap() as u8,
            rgb[1].as_u64().unwrap() as u8,
            rgb[2].as_u64().unwrap() as u8,
        );
    }
    match value.as_str().unwrap() {
        "Color::Reset" => Color::Reset,
        "Color::Blue" => Color::Blue,
        "Color::Cyan" => Color::Cyan,
        "Color::Yellow" => Color::Yellow,
        "Color::Red" => Color::Red,
        "Color::Green" => Color::Green,
        "Color::DarkGray" => Color::DarkGray,
        "Color::LightBlue" => Color::LightBlue,
        "Color::LightCyan" => Color::LightCyan,
        "Color::LightRed" => Color::LightRed,
        "Color::LightYellow" => Color::LightYellow,
        "Color::Magenta" => Color::Magenta,
        name => panic!("unhandled source color {name}"),
    }
}

fn theme(palette: TuiPalette, depth: ColorDepth, appearance: Appearance) -> Theme {
    Theme::new(Caps {
        depth,
        ascii: false,
        appearance,
    })
    .tui_palette(palette)
}

fn adapted(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Indexed(rgb_to_ansi256(r, g, b)),
        value => value,
    }
}

#[test]
fn every_generated_palette_role_ink_and_ground_matches_the_source_export() {
    let source = source();
    let source = source.as_array().unwrap();
    assert_eq!(source.len(), TuiPalette::ALL.len());
    for palette in TuiPalette::ALL {
        let row = source
            .iter()
            .find(|row| row["name"] == palette.name())
            .unwrap();
        assert_eq!(
            row["light"].as_bool().unwrap(),
            palette.light(),
            "{} appearance",
            palette.name()
        );
        let roles = row["roles"].as_array().unwrap();
        assert_eq!(roles.len(), Role::ALL.len());
        for (role, expected) in Role::ALL.into_iter().zip(roles) {
            assert_eq!(
                palette.color(role),
                color(expected),
                "{} {role:?}",
                palette.name()
            );
        }
        for (ink, slot) in INKS {
            assert_eq!(
                palette.ink(ink),
                color(&row["slots"][slot]),
                "{} {ink:?}",
                palette.name()
            );
        }
        for (ground, slot) in GROUNDS {
            assert_eq!(
                palette.ground(ground),
                color(&row["slots"][slot]),
                "{} {ground:?}",
                palette.name()
            );
        }
    }
}

#[test]
fn truecolor_and_256_adapt_only_rgb_and_keep_native_terminal_colors() {
    for palette in TuiPalette::ALL {
        let appearance = if palette.light() {
            Appearance::Light
        } else {
            Appearance::Dark
        };
        let truecolor = theme(palette, ColorDepth::TrueColor, appearance);
        let ansi256 = theme(palette, ColorDepth::Ansi256, appearance);
        for role in Role::ALL {
            assert_eq!(truecolor.color(role), Some(palette.color(role)));
            assert_eq!(ansi256.color(role), Some(adapted(palette.color(role))));
        }
        for (ink, _) in INKS {
            assert_eq!(truecolor.tui_ink(ink).fg, Some(palette.ink(ink)));
            assert_eq!(ansi256.tui_ink(ink).fg, Some(adapted(palette.ink(ink))));
        }
        for (ground, _) in GROUNDS {
            assert_eq!(
                truecolor.tui_ground(ground).bg,
                Some(palette.ground(ground))
            );
            assert_eq!(
                ansi256.tui_ground(ground).bg,
                Some(adapted(palette.ground(ground)))
            );
        }
    }
}

#[test]
fn terminal_owned_shells_keep_reset_instead_of_receiving_an_assumed_ground() {
    for palette in [
        TuiPalette::Whale,
        TuiPalette::WhaleLight,
        TuiPalette::Terminal,
    ] {
        for depth in [ColorDepth::TrueColor, ColorDepth::Ansi256] {
            let theme = theme(
                palette,
                depth,
                if palette.light() {
                    Appearance::Light
                } else {
                    Appearance::Dark
                },
            );
            for role in [Role::Sidebar, Role::Background, Role::Surface] {
                assert_eq!(theme.color(role), Some(Color::Reset));
                assert_eq!(theme.bg(role).bg, Some(Color::Reset));
            }
            for ground in [
                TuiGround::Surface,
                TuiGround::Panel,
                TuiGround::Composer,
                TuiGround::Header,
                TuiGround::Footer,
            ] {
                assert_eq!(
                    theme.tui_ground(ground).bg,
                    Some(Color::Reset),
                    "{} {ground:?} remains terminal-owned",
                    palette.name()
                );
            }
        }
    }
    let terminal = theme(
        TuiPalette::Terminal,
        ColorDepth::TrueColor,
        Appearance::Dark,
    );
    assert_eq!(terminal.color(Role::Primary), Some(Color::Blue));
    assert_eq!(terminal.color(Role::Foreground), Some(Color::Reset));
    assert_eq!(terminal.tui_ink(TuiInk::Working).fg, Some(Color::Cyan));
    for (ground, _) in GROUNDS {
        assert_eq!(terminal.tui_ground(ground).bg, Some(Color::Reset));
    }
}

#[test]
fn low_color_and_unknown_ground_retain_the_host_capability_fallbacks() {
    for palette in TuiPalette::ALL {
        for profile in [
            Profile::Ansi16,
            Profile::NoColor,
            Profile::Ascii,
            Profile::UnknownGround,
        ] {
            let base = profile.theme();
            let native = base.tui_palette(palette);
            for role in Role::ALL {
                assert_eq!(
                    native.fg(role),
                    base.fg(role),
                    "{} {profile:?} {role:?}",
                    palette.name()
                );
                assert_eq!(native.bg(role).bg, None);
            }
            for (ink, _) in INKS {
                assert_eq!(native.tui_ink(ink), base.fg(ink.fallback_role()));
            }
            for (ground, _) in GROUNDS {
                assert_eq!(
                    native.tui_ground(ground),
                    base.bg(ground.fallback_role()),
                    "{} {profile:?} {ground:?}",
                    palette.name()
                );
                assert_eq!(native.tui_ground(ground).bg, None);
            }
        }
    }
    let monochrome = Profile::NoColor.theme().tui_palette(TuiPalette::Dracula);
    assert!(
        monochrome
            .fg(Role::Muted)
            .add_modifier
            .contains(Modifier::DIM)
    );
    assert!(
        monochrome
            .tui_ink(TuiInk::Soft)
            .add_modifier
            .contains(Modifier::DIM)
    );
}

#[test]
fn native_ocean_is_exclusive_to_known_dark_truecolor_underwater() {
    for palette in TuiPalette::ALL {
        let theme = theme(
            palette,
            ColorDepth::TrueColor,
            if palette.light() {
                Appearance::Light
            } else {
                Appearance::Dark
            },
        );
        assert_eq!(
            OceanRamp::for_theme(&theme).is_some(),
            palette == TuiPalette::Underwater
        );
        if palette != TuiPalette::Underwater {
            let mut buf = render(40, 12, |area, buf| {
                buf.set_style(area, theme.bg(Role::Background))
            });
            let before = buf.clone();
            OceanColumn::new(Duration::from_secs(3), MotionMode::Full)
                .paint(buf.area, &mut buf, &theme);
            assert_eq!(buf, before, "{} keeps its own ground", palette.name());
        }
    }
    for profile in [
        Profile::LightTrue,
        Profile::Dark256,
        Profile::Ansi16,
        Profile::NoColor,
        Profile::UnknownGround,
    ] {
        assert!(
            OceanRamp::for_theme(&profile.theme().tui_palette(TuiPalette::Underwater)).is_none()
        );
    }
}

#[test]
fn each_native_palette_has_a_named_gallery_entry_that_paints_its_own_preset() {
    let entries = gallery::entries();
    for palette in TuiPalette::ALL {
        let name = format!("tui-theme-{}", palette.name());
        let entry = entries.iter().find(|entry| entry.name == name).unwrap();
        assert_eq!(entry.width, 104);
        assert_eq!(entry.height, 24);
        for profile in [Profile::DarkTrue, Profile::LightTrue] {
            let buf = gallery::render(entry, &profile.theme());
            assert_eq!(
                buf[(0, 3)].bg,
                palette.color(Role::Background),
                "{} ignores the surrounding board appearance",
                palette.name()
            );
            assert_eq!(buf[(0, 0)].bg, palette.color(Role::Sidebar));
        }
    }
}

#[test]
fn native_palette_gallery_keeps_bounds_and_fallback_profiles() {
    let entries = gallery::entries();
    for entry in entries
        .iter()
        .filter(|entry| entry.name.starts_with("tui-theme-"))
    {
        for profile in [
            Profile::Ansi16,
            Profile::NoColor,
            Profile::Ascii,
            Profile::UnknownGround,
        ] {
            let theme = profile.theme();
            for width in [1, 12, 40, 104] {
                let buf = gallery::render_at(entry, &theme, width, entry.height);
                if profile == Profile::Ascii {
                    assert!(
                        codewhale_ratatui::testing::text(&buf).is_ascii(),
                        "{} {width}",
                        entry.name
                    );
                }
                for cell in &buf.content {
                    assert_eq!(
                        cell.bg,
                        Color::Reset,
                        "{} {profile:?} must leave the ground alone",
                        entry.name
                    );
                    if matches!(profile, Profile::NoColor | Profile::Ascii) {
                        assert_eq!(cell.fg, Color::Reset);
                    } else {
                        assert!(!matches!(cell.fg, Color::Rgb(..) | Color::Indexed(_)));
                    }
                }
            }
        }
    }
}
