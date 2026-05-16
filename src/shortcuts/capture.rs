use iced::keyboard::{self, key::Named};

/// Maps F-key variants to their string representations.
fn fkey_name(named: &Named) -> Option<&'static str> {
    match named {
        Named::F1 => Some("F1"),
        Named::F2 => Some("F2"),
        Named::F3 => Some("F3"),
        Named::F4 => Some("F4"),
        Named::F5 => Some("F5"),
        Named::F6 => Some("F6"),
        Named::F7 => Some("F7"),
        Named::F8 => Some("F8"),
        Named::F9 => Some("F9"),
        Named::F10 => Some("F10"),
        Named::F11 => Some("F11"),
        Named::F12 => Some("F12"),
        _ => None,
    }
}

/// Maps named keys that require a modifier to their string representations.
fn named_key_with_modifier(named: &Named) -> Option<&'static str> {
    match named {
        Named::Space => Some("Space"),
        Named::Enter => Some("Return"),
        Named::Tab => Some("Tab"),
        Named::Delete => Some("Delete"),
        Named::Backspace => Some("Backspace"),
        Named::Home => Some("Home"),
        Named::End => Some("End"),
        Named::PageUp => Some("PageUp"),
        Named::PageDown => Some("PageDown"),
        Named::ArrowUp => Some("Up"),
        Named::ArrowDown => Some("Down"),
        Named::ArrowLeft => Some("Left"),
        Named::ArrowRight => Some("Right"),
        _ => None,
    }
}

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
    let has_modifier =
        modifiers.control() || modifiers.alt() || modifiers.shift() || modifiers.logo();

    let key_str: String = match key {
        keyboard::Key::Named(named) => match named {
            Named::Escape
            | Named::Control
            | Named::Alt
            | Named::Shift
            | Named::Super
            | Named::Meta => return None,
            _ => {
                if let Some(s) = fkey_name(named) {
                    s.to_owned()
                } else if has_modifier {
                    named_key_with_modifier(named)?.to_owned()
                } else {
                    return None;
                }
            }
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
