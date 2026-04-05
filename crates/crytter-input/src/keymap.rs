/// Encode a browser keyboard event into terminal escape sequence bytes.
///
/// `key` — the `KeyboardEvent.key` value (e.g. "a", "Enter", "ArrowUp")
/// `ctrl` — Ctrl key held
/// `alt` — Alt/Option key held
/// `shift` — Shift key held
/// `app_cursor` — application cursor mode (DECCKM) is active
///
/// Returns `None` for keys that don't produce terminal output (e.g. bare Shift).
pub fn encode_key(
    key: &str,
    ctrl: bool,
    alt: bool,
    _shift: bool,
    app_cursor: bool,
) -> Option<Vec<u8>> {
    // Alt wraps the output in ESC prefix
    let wrap_alt = |bytes: Vec<u8>| -> Vec<u8> {
        if alt {
            let mut out = vec![0x1b];
            out.extend(bytes);
            out
        } else {
            bytes
        }
    };

    // Dead key (^, ~, `) — no character yet, wait for composition
    if key == "Dead" {
        return None;
    }

    // Single printable character (len() is bytes, chars().count() handles UTF-8)
    if key.chars().count() == 1 {
        let c = key.chars().next().unwrap();

        if ctrl {
            // Ctrl+letter → C0 control codes (0x01-0x1A)
            if c.is_ascii_alphabetic() {
                let code = (c.to_ascii_uppercase() as u8) - b'A' + 1;
                return Some(wrap_alt(vec![code]));
            }
            // Ctrl+special
            return match c {
                '[' | '3' => Some(vec![0x1b]),       // ESC
                '\\' | '4' => Some(vec![0x1c]),      // FS
                ']' | '5' => Some(vec![0x1d]),       // GS
                '6' => Some(vec![0x1e]),              // RS
                '/' | '7' => Some(vec![0x1f]),        // US
                ' ' | '2' | '@' => Some(vec![0x00]), // NUL
                _ => None,
            };
        }

        if c.is_ascii() || !c.is_control() {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            // Alt+letter/digit = ESC prefix (terminal modifier: Alt+b, Alt+f, etc.)
            // Alt+anything else = composed character (AZERTY: |, ~, {, }, €) — no ESC
            if alt && !c.is_ascii_alphanumeric() {
                return Some(s.as_bytes().to_vec());
            }
            return Some(wrap_alt(s.as_bytes().to_vec()));
        }

        return None;
    }

    // Special keys
    let result = match key {
        "Enter" => vec![0x0d],
        "Backspace" => {
            if ctrl {
                vec![0x08]
            } else {
                vec![0x7f]
            }
        }
        "Tab" => vec![0x09],
        "Escape" => vec![0x1b],
        "Delete" => vec![0x1b, b'[', b'3', b'~'],

        // Arrow keys
        "ArrowUp" => arrow_key(b'A', app_cursor),
        "ArrowDown" => arrow_key(b'B', app_cursor),
        "ArrowRight" => arrow_key(b'C', app_cursor),
        "ArrowLeft" => arrow_key(b'D', app_cursor),

        // Navigation
        "Home" => vec![0x1b, b'[', b'H'],
        "End" => vec![0x1b, b'[', b'F'],
        "PageUp" => vec![0x1b, b'[', b'5', b'~'],
        "PageDown" => vec![0x1b, b'[', b'6', b'~'],
        "Insert" => vec![0x1b, b'[', b'2', b'~'],

        // Function keys
        "F1" => vec![0x1b, b'O', b'P'],
        "F2" => vec![0x1b, b'O', b'Q'],
        "F3" => vec![0x1b, b'O', b'R'],
        "F4" => vec![0x1b, b'O', b'S'],
        "F5" => vec![0x1b, b'[', b'1', b'5', b'~'],
        "F6" => vec![0x1b, b'[', b'1', b'7', b'~'],
        "F7" => vec![0x1b, b'[', b'1', b'8', b'~'],
        "F8" => vec![0x1b, b'[', b'1', b'9', b'~'],
        "F9" => vec![0x1b, b'[', b'2', b'0', b'~'],
        "F10" => vec![0x1b, b'[', b'2', b'1', b'~'],
        "F11" => vec![0x1b, b'[', b'2', b'3', b'~'],
        "F12" => vec![0x1b, b'[', b'2', b'4', b'~'],

        // Ignore modifier-only keys
        "Shift" | "Control" | "Alt" | "Meta" | "CapsLock" | "NumLock" => return None,

        _ => return None,
    };

    Some(wrap_alt(result))
}

/// Arrow key encoding — normal mode vs application cursor mode (DECCKM).
fn arrow_key(code: u8, app_cursor: bool) -> Vec<u8> {
    if app_cursor {
        vec![0x1b, b'O', code] // SS3
    } else {
        vec![0x1b, b'[', code] // CSI
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_char() {
        assert_eq!(encode_key("a", false, false, false, false), Some(vec![b'a']));
        assert_eq!(encode_key("Z", false, false, false, false), Some(vec![b'Z']));
    }

    #[test]
    fn ctrl_c() {
        assert_eq!(
            encode_key("c", true, false, false, false),
            Some(vec![0x03])
        );
    }

    #[test]
    fn ctrl_a() {
        assert_eq!(
            encode_key("a", true, false, false, false),
            Some(vec![0x01])
        );
    }

    #[test]
    fn enter() {
        assert_eq!(
            encode_key("Enter", false, false, false, false),
            Some(vec![0x0d])
        );
    }

    #[test]
    fn backspace() {
        assert_eq!(
            encode_key("Backspace", false, false, false, false),
            Some(vec![0x7f])
        );
    }

    #[test]
    fn arrow_keys_normal() {
        assert_eq!(
            encode_key("ArrowUp", false, false, false, false),
            Some(vec![0x1b, b'[', b'A'])
        );
    }

    #[test]
    fn arrow_keys_app_cursor() {
        assert_eq!(
            encode_key("ArrowUp", false, false, false, true),
            Some(vec![0x1b, b'O', b'A'])
        );
    }

    #[test]
    fn alt_wraps_with_esc() {
        assert_eq!(
            encode_key("a", false, true, false, false),
            Some(vec![0x1b, b'a'])
        );
    }

    #[test]
    fn function_keys() {
        assert_eq!(
            encode_key("F1", false, false, false, false),
            Some(vec![0x1b, b'O', b'P'])
        );
        assert_eq!(
            encode_key("F12", false, false, false, false),
            Some(vec![0x1b, b'[', b'2', b'4', b'~'])
        );
    }

    #[test]
    fn modifier_only_returns_none() {
        assert_eq!(encode_key("Shift", false, false, true, false), None);
        assert_eq!(encode_key("Control", true, false, false, false), None);
        assert_eq!(encode_key("Alt", false, true, false, false), None);
    }

    #[test]
    fn accented_characters() {
        assert_eq!(encode_key("é", false, false, false, false), Some(vec![0xC3, 0xA9]));
        assert_eq!(encode_key("ç", false, false, false, false), Some(vec![0xC3, 0xA7]));
    }

    #[test]
    fn dead_key_returns_none() {
        assert_eq!(encode_key("Dead", false, false, false, false), None);
    }

    #[test]
    fn alt_composed_chars() {
        // AZERTY Alt combos — should NOT wrap with ESC
        assert_eq!(encode_key("|", false, true, false, false), Some(vec![b'|']));
        assert_eq!(encode_key("~", false, true, false, false), Some(vec![b'~']));
        assert_eq!(encode_key("{", false, true, false, false), Some(vec![b'{']));
        assert_eq!(encode_key("}", false, true, false, false), Some(vec![b'}']));
        assert_eq!(encode_key("[", false, true, false, false), Some(vec![b'[']));
        assert_eq!(encode_key("]", false, true, false, false), Some(vec![b']']));
        assert_eq!(encode_key("\\", false, true, false, false), Some(vec![b'\\']));
        assert_eq!(encode_key("@", false, true, false, false), Some(vec![b'@']));
        assert_eq!(encode_key("#", false, true, false, false), Some(vec![b'#']));
        assert_eq!(encode_key("€", false, true, false, false), Some(vec![0xE2, 0x82, 0xAC]));
        // Alt+letter: wrap with ESC (real terminal modifier: Alt+b, Alt+f, etc.)
        assert_eq!(encode_key("a", false, true, false, false), Some(vec![0x1b, b'a']));
        assert_eq!(encode_key("f", false, true, false, false), Some(vec![0x1b, b'f']));
    }

    #[test]
    fn delete_key() {
        assert_eq!(
            encode_key("Delete", false, false, false, false),
            Some(vec![0x1b, b'[', b'3', b'~'])
        );
    }
}
