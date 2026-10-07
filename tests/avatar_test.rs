use codewhale_ratatui::{Paint, avatar_sprite::Sprite, testing::Profile, whale_girl};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
#[test]
fn avatar_obeys_clip_bounds_and_terminal_color_caps() {
    let sprite = Sprite::new(whale_girl::pack(), whale_girl::TERMINAL, 96, 96, 0).unwrap();
    for profile in Profile::ALL {
        let theme = profile.theme();
        for area in [
            Rect::new(0, 0, 0, 0),
            Rect::new(0, 0, 1, 1),
            Rect::new(1, 1, 30, 15),
            Rect::new(65500, 65500, 10, 10),
        ] {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 34, 20));
            sprite.paint(area, &mut buffer, &theme);
            let clipped = area.intersection(buffer.area);
            for y in 0..20 {
                for x in 0..34 {
                    let c = &buffer[(x, y)];
                    if !clipped.contains((x, y).into()) {
                        assert_eq!(c.symbol(), " ");
                        assert_eq!(c.fg, Color::Reset);
                    }
                    if matches!(profile, Profile::NoColor | Profile::Ascii | Profile::Ansi16) {
                        assert!(!matches!(c.fg, Color::Rgb(..) | Color::Indexed(..)));
                    }
                    if profile == Profile::Ascii {
                        assert!(c.symbol().is_ascii());
                    }
                }
            }
        }
    }
}
#[test]
fn avatar_refuses_invalid_buffer_dimensions_and_frames() {
    assert!(Sprite::new(whale_girl::pack(), &[], 96, 96, 0).is_err());
    assert!(
        Sprite::new(
            whale_girl::pack(),
            whale_girl::TERMINAL,
            96,
            96,
            whale_girl::FRAMES
        )
        .is_err()
    );
    assert!(Sprite::new(whale_girl::pack(), whale_girl::TERMINAL, usize::MAX, 96, 0).is_err());
}

#[test]
fn avatar_page_render_matches_the_same_global_frame() {
    let pack = whale_girl::pack();
    let frame = 31;
    let per_page = usize::from(pack.columns) * usize::from(pack.rows);
    let tile_bytes = 96 * 96 * 4;
    let start = pack.page(frame) * per_page * tile_bytes;
    let page = &whale_girl::TERMINAL[start..start + per_page * tile_bytes];
    let whole = Sprite::new(pack, whale_girl::TERMINAL, 96, 96, frame).unwrap();
    let paged = Sprite::page(pack, page, 96, 96, frame).unwrap();
    let area = Rect::new(0, 0, 64, 32);
    let mut a = Buffer::empty(area);
    let mut b = Buffer::empty(area);
    whole.paint(area, &mut a, &Profile::DarkTrue.theme());
    paged.paint(area, &mut b, &Profile::DarkTrue.theme());
    assert_eq!(a, b);
    assert!(Sprite::page(pack, page, 96, 96, whale_girl::FRAMES).is_err());
}
