/// One USB HID keyboard report's worth of input: a modifier byte plus a
/// single keycode. `modifier = 0x02` is Left Shift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub modifier: u8,
    pub keycode: u8,
}

/// Left Shift modifier bit, per USB HID Boot Keyboard report.
const SHIFT: u8 = 0x02;

/// Map a printable ASCII character (0x20-0x7E), `\n`, or `\t` to the
/// US-QWERTY HID Usage Table (Keyboard/Keypad page 0x07) keycode that
/// types it. Returns `None` for anything outside that set.
pub fn char_to_key(c: char) -> Option<KeyEvent> {
    let (modifier, keycode): (u8, u8) = match c {
        '\n' => (0, 0x28), // Return
        '\t' => (0, 0x2B), // Tab
        ' ' => (0, 0x2C),

        'a'..='z' => (0, 0x04 + (c as u8 - b'a')),
        'A'..='Z' => (SHIFT, 0x04 + (c as u8 - b'A')),

        '1' => (0, 0x1E),
        '2' => (0, 0x1F),
        '3' => (0, 0x20),
        '4' => (0, 0x21),
        '5' => (0, 0x22),
        '6' => (0, 0x23),
        '7' => (0, 0x24),
        '8' => (0, 0x25),
        '9' => (0, 0x26),
        '0' => (0, 0x27),

        '!' => (SHIFT, 0x1E), // shift+1
        '@' => (SHIFT, 0x1F), // shift+2
        '#' => (SHIFT, 0x20), // shift+3
        '$' => (SHIFT, 0x21), // shift+4
        '%' => (SHIFT, 0x22), // shift+5
        '^' => (SHIFT, 0x23), // shift+6
        '&' => (SHIFT, 0x24), // shift+7
        '*' => (SHIFT, 0x25), // shift+8
        '(' => (SHIFT, 0x26), // shift+9
        ')' => (SHIFT, 0x27), // shift+0

        '-' => (0, 0x2D),
        '_' => (SHIFT, 0x2D),
        '=' => (0, 0x2E),
        '+' => (SHIFT, 0x2E),
        '[' => (0, 0x2F),
        '{' => (SHIFT, 0x2F),
        ']' => (0, 0x30),
        '}' => (SHIFT, 0x30),
        '\\' => (0, 0x31),
        '|' => (SHIFT, 0x31),
        ';' => (0, 0x33),
        ':' => (SHIFT, 0x33),
        '\'' => (0, 0x34),
        '"' => (SHIFT, 0x34),
        '`' => (0, 0x35),
        '~' => (SHIFT, 0x35),
        ',' => (0, 0x36),
        '<' => (SHIFT, 0x36),
        '.' => (0, 0x37),
        '>' => (SHIFT, 0x37),
        '/' => (0, 0x38),
        '?' => (SHIFT, 0x38),

        _ => return None,
    };
    Some(KeyEvent { modifier, keycode })
}

/// Whether `c` can actually be typed on the badge, via `UsbHid::send_str`.
///
/// This intentionally does *not* delegate to [`char_to_key`]. `send_str`
/// drives its own char->keycode table server-side
/// (`char_to_hid_code_us101` in `usb-bao1x/src/mappings.rs`, at xous-core
/// rev 616bf65f6e379165464f50b1e79ec42aff77a683), which covers printable
/// ASCII (0x20-0x7E) and `\n` (mapped to Enter) -- but has **no** case for
/// `\t`: an untyped Tab falls through to `_ => log::warn!("Ignoring
/// unhandled character...")` and is silently dropped. `char_to_key` above
/// still maps `\t` (it's a general US-QWERTY table, unused for device
/// typing now that we go through `send_str`), so `is_typeable` must be
/// narrower than "`char_to_key` succeeds" to keep the preview's
/// untypeable-character count accurate.
pub fn is_typeable(c: char) -> bool {
    matches!(c, '\u{20}'..='\u{7E}' | '\n')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lowercase_letter_no_shift() {
        let k = char_to_key('a').unwrap();
        assert_eq!(k.modifier, 0x00);
        assert_eq!(k.keycode, 0x04); // HID 'a'
    }
    #[test]
    fn uppercase_letter_has_shift() {
        let k = char_to_key('A').unwrap();
        assert_eq!(k.modifier, 0x02); // Left Shift
        assert_eq!(k.keycode, 0x04);
    }
    #[test]
    fn digit_one() {
        let k = char_to_key('1').unwrap();
        assert_eq!(k.modifier, 0x00);
        assert_eq!(k.keycode, 0x1E);
    }
    #[test]
    fn shifted_symbol_at() {
        let k = char_to_key('@').unwrap();
        assert_eq!(k.modifier, 0x02);
        assert_eq!(k.keycode, 0x1F); // '2' key + shift
    }
    #[test]
    fn newline_is_enter() {
        let k = char_to_key('\n').unwrap();
        assert_eq!(k.keycode, 0x28); // Return
    }
    #[test]
    fn non_ascii_is_untypeable() {
        assert!(char_to_key('é').is_none());
        assert!(!is_typeable('😀'));
    }
    #[test]
    fn tab_is_not_typeable_on_real_hardware() {
        // usb-bao1x's char_to_hid_code_us101 (rev 616bf65f6e379165464f50b1e79ec42aff77a683,
        // services/usb-bao1x/src/mappings.rs) has no case for '\t': it falls
        // through to `_ => log::warn!(...)` and is silently dropped by
        // send_str. is_typeable must match that, even though char_to_key
        // (unused for device typing, kept as a reference US-QWERTY table)
        // still maps '\t' to the Tab keycode.
        assert!(!is_typeable('\t'));
        assert!(char_to_key('\t').is_some()); // char_to_key itself is unchanged
    }
    #[test]
    fn space_and_newline_are_typeable() {
        assert!(is_typeable(' '));
        assert!(is_typeable('\n'));
    }
}
