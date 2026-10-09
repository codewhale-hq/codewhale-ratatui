use codewhale_ratatui::{LineBuffer, TextInputState};
use unicode_segmentation::UnicodeSegmentation;

fn boundary(buffer: &LineBuffer) {
    let cursor = buffer.cursor_byte();
    assert!(
        cursor == buffer.text().len()
            || buffer
                .text()
                .grapheme_indices(true)
                .any(|(at, _)| at == cursor)
    );
}

#[test]
fn combining_input_extends_a_full_visible_grapheme() {
    let mut buffer = LineBuffer::from_text("e").with_limit(1);
    assert!(buffer.insert_char('\u{301}'));
    assert_eq!(buffer.text(), "e\u{301}");
    assert_eq!(buffer.len(), 1);
    boundary(&buffer);
    assert!(!buffer.insert_char('x'));
    assert!(buffer.backspace());
    assert!(buffer.is_empty());
}

#[test]
fn incremental_emoji_is_the_same_as_paste() {
    let mut buffer = LineBuffer::new().with_limit(1);
    for c in "👩‍💻".chars() {
        assert!(buffer.insert_char(c));
        boundary(&buffer);
    }
    assert_eq!(buffer.text(), LineBuffer::from_text("👩‍💻").text());
    assert_eq!(buffer.len(), 1);
    assert!(buffer.backspace());
    assert!(buffer.is_empty());
}

#[test]
fn deleting_a_separator_resegments_regional_indicators() {
    let mut buffer = LineBuffer::from_text("🇦x🇧");
    buffer.set_cursor(1);
    assert!(buffer.delete());
    assert_eq!(buffer.text(), "🇦🇧");
    boundary(&buffer);
    assert_eq!(buffer.cursor(), 1);
    assert!(!buffer.delete());
    assert!(buffer.backspace());
    assert!(buffer.is_empty());
}

#[test]
fn capped_middle_paste_preserves_existing_suffix() {
    let mut buffer = LineBuffer::from_text("ab").with_limit(3);
    buffer.set_cursor(1);
    assert!(buffer.insert_str("海xyz"));
    assert_eq!(buffer.text(), "a海b");
    assert_eq!(buffer.cursor(), 2);
    boundary(&buffer);
}

#[test]
fn invisible_and_control_only_input_still_cannot_enter_a_field() {
    let mut buffer = LineBuffer::new();
    assert!(!buffer.insert_str("\u{301}\u{200d}\u{200b}\n\u{2028}\u{202e}"));
    assert!(buffer.is_empty());
    buffer.insert_str("e");
    assert!(!buffer.insert_str("\u{200b}\u{2028}\u{202e}\n"));
    assert_eq!(buffer.text(), "e");
}

#[test]
fn invisible_paste_prefix_does_not_use_the_visible_text_budget() {
    let mut buffer = LineBuffer::new().with_limit(1);
    let text = format!("{}abc", "\u{200b}\u{2028}".repeat(100));
    assert!(buffer.insert_str(&text));
    assert_eq!(buffer.text(), "a");
    boundary(&buffer);
}

#[test]
fn edits_keep_grapheme_boundaries_through_joins_and_splits() {
    for text in ["🇦x🇧", "🇦🇧x🇨🇩", "e\u{301}海👩‍💻", "a🇦🇧b"] {
        for at in 0..=text.graphemes(true).count() {
            for forward in [false, true] {
                let mut buffer = LineBuffer::from_text(text);
                buffer.set_cursor(at);
                if forward {
                    buffer.delete();
                } else {
                    buffer.backspace();
                }
                boundary(&buffer);
                buffer.insert_char('\u{301}');
                boundary(&buffer);
                while buffer.move_left() {
                    boundary(&buffer);
                }
                while buffer.move_right() {
                    boundary(&buffer);
                }
                while buffer.backspace() {
                    boundary(&buffer);
                }
                assert!(buffer.is_empty());
            }
        }
    }
}

#[test]
fn secret_fields_share_the_correct_unicode_model() {
    let mut state = TextInputState::secret().with_limit(1);
    for c in "👩‍💻".chars() {
        state.buffer_mut().insert_char(c);
    }
    assert_eq!(state.buffer().len(), 1);
    assert!(!format!("{state:?}").contains("👩"));
    boundary(state.buffer());
}
