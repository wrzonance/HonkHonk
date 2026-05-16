use iced::keyboard::{self, key::Named};

/// Formats a key press event into a portal-compatible trigger string.
///
/// Returns `Some("Meta+1")` for valid combos, `None` for:
/// - Bare non-F-key characters without a modifier
/// - Modifier-only presses (Ctrl, Alt, Shift, Super alone)
/// - Escape (caller treats this as Cancel)
/// - Unidentified keys
///
/// F-keys (F1–F12) are accepted without a modifier.
pub fn format_combo(modifiers: keyboard::Modifiers, key: &keyboard::Key) -> Option<String> {
    let has_modifier = modifiers.control()
        || modifiers.alt()
        || modifiers.shift()
        || modifiers.logo();

    let key_str: String = match key {
        keyboard::Key::Named(named) => match named {
            Named::Escape
            | Named::Control
            | Named::Alt
            | Named::Shift
            | Named::Super
            | Named::Meta => return None,
            // F-keys: valid without modifier
            Named::F1 => "F1".into(),
            Named::F2 => "F2".into(),
            Named::F3 => "F3".into(),
            Named::F4 => "F4".into(),
            Named::F5 => "F5".into(),
            Named::F6 => "F6".into(),
            Named::F7 => "F7".into(),
            Named::F8 => "F8".into(),
            Named::F9 => "F9".into(),
            Named::F10 => "F10".into(),
            Named::F11 => "F11".into(),
            Named::F12 => "F12".into(),
            // Named keys that need a modifier
            Named::Space if has_modifier => "Space".into(),
            Named::Enter if has_modifier => "Return".into(),
            Named::Tab if has_modifier => "Tab".into(),
            Named::Delete if has_modifier => "Delete".into(),
            Named::Backspace if has_modifier => "Backspace".into(),
            Named::Home if has_modifier => "Home".into(),
            Named::End if has_modifier => "End".into(),
            Named::PageUp if has_modifier => "PageUp".into(),
            Named::PageDown if has_modifier => "PageDown".into(),
            Named::ArrowUp if has_modifier => "Up".into(),
            Named::ArrowDown if has_modifier => "Down".into(),
            Named::ArrowLeft if has_modifier => "Left".into(),
            Named::ArrowRight if has_modifier => "Right".into(),
            _ => return None,
        },
        keyboard::Key::Character(c) if has_modifier => c.to_uppercase(),
        keyboard::Key::Character(_) | keyboard::Key::Unidentified => return None,
    };

    let mut parts: Vec<&str> = Vec::new();
    if modifiers.control() {
        parts.push("Ctrl");
    }
    if modifiers.alt() {
        parts.push("Alt");
    }
    if modifiers.shift() {
        parts.push("Shift");
    }
    if modifiers.logo() {
        parts.push("Meta");
    }
    parts.push(&key_str);
    Some(parts.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::{self, key::Named};

    fn key(named: Named) -> keyboard::Key {
        keyboard::Key::Named(named)
    }

    fn ch(c: &str) -> keyboard::Key {
        keyboard::Key::Character(c.into())
    }

    fn mods(ctrl: bool, alt: bool, shift: bool, logo: bool) -> keyboard::Modifiers {
        let mut m = keyboard::Modifiers::empty();
        if ctrl {
            m |= keyboard::Modifiers::CTRL;
        }
        if alt {
            m |= keyboard::Modifiers::ALT;
        }
        if shift {
            m |= keyboard::Modifiers::SHIFT;
        }
        if logo {
            m |= keyboard::Modifiers::LOGO;
        }
        m
    }

    #[test]
    fn meta_plus_digit() {
        let result = format_combo(mods(false, false, false, true), &ch("1"));
        assert_eq!(result.as_deref(), Some("Meta+1"));
    }

    #[test]
    fn ctrl_alt_f5() {
        let result = format_combo(mods(true, true, false, false), &key(Named::F5));
        assert_eq!(result.as_deref(), Some("Ctrl+Alt+F5"));
    }

    #[test]
    fn f1_without_modifier_is_valid() {
        let result = format_combo(keyboard::Modifiers::empty(), &key(Named::F1));
        assert_eq!(result.as_deref(), Some("F1"));
    }

    #[test]
    fn f12_without_modifier_is_valid() {
        let result = format_combo(keyboard::Modifiers::empty(), &key(Named::F12));
        assert_eq!(result.as_deref(), Some("F12"));
    }

    #[test]
    fn bare_letter_without_modifier_is_none() {
        let result = format_combo(keyboard::Modifiers::empty(), &ch("a"));
        assert!(result.is_none());
    }

    #[test]
    fn bare_digit_without_modifier_is_none() {
        let result = format_combo(keyboard::Modifiers::empty(), &ch("1"));
        assert!(result.is_none());
    }

    #[test]
    fn escape_is_none() {
        let result = format_combo(keyboard::Modifiers::empty(), &key(Named::Escape));
        assert!(result.is_none());
    }

    #[test]
    fn modifier_only_ctrl_is_none() {
        let result = format_combo(mods(true, false, false, false), &key(Named::Control));
        assert!(result.is_none());
    }

    #[test]
    fn ctrl_shift_a_uppercases_character() {
        let result = format_combo(mods(true, false, true, false), &ch("a"));
        assert_eq!(result.as_deref(), Some("Ctrl+Shift+A"));
    }

    #[test]
    fn modifier_order_is_ctrl_alt_shift_meta() {
        let result = format_combo(mods(true, true, true, true), &ch("x"));
        assert_eq!(result.as_deref(), Some("Ctrl+Alt+Shift+Meta+X"));
    }

    #[test]
    fn meta_space() {
        let result = format_combo(mods(false, false, false, true), &key(Named::Space));
        assert_eq!(result.as_deref(), Some("Meta+Space"));
    }

    #[test]
    fn bare_space_is_none() {
        let result = format_combo(keyboard::Modifiers::empty(), &key(Named::Space));
        assert!(result.is_none());
    }
}
