//! Traduce las teclas de egui a las teclas de terminal del dominio.

use eframe::egui::{Key, Modifiers};

use crate::domain::terminal_keys::{KeyModifiers, TerminalKey};

/// Devuelve `None` para las teclas que llegan como texto normal
/// (letras, números, símbolos), que se envían por otro camino.
pub fn terminal_key_for(key: Key, modifiers: Modifiers) -> Option<TerminalKey> {
    // AltGr (para @, #, €...) no llega aquí como Ctrl: winit ya lo filtra y
    // esos caracteres entran como texto normal. Ctrl+Alt+letra se ignora.
    let is_plain_ctrl = modifiers.ctrl && !modifiers.alt;
    if is_plain_ctrl {
        if let Some(letter) = letter_of(key) {
            return Some(TerminalKey::ControlLetter(letter));
        }
        if key == Key::Space {
            return Some(TerminalKey::ControlSpace);
        }
    }

    let terminal_key = match key {
        Key::Enter => TerminalKey::Enter,
        Key::Backspace => TerminalKey::Backspace,
        Key::Tab => TerminalKey::Tab,
        Key::Escape => TerminalKey::Escape,
        Key::ArrowUp => TerminalKey::ArrowUp,
        Key::ArrowDown => TerminalKey::ArrowDown,
        Key::ArrowLeft => TerminalKey::ArrowLeft,
        Key::ArrowRight => TerminalKey::ArrowRight,
        Key::Home => TerminalKey::Home,
        Key::End => TerminalKey::End,
        Key::PageUp => TerminalKey::PageUp,
        Key::PageDown => TerminalKey::PageDown,
        Key::Insert => TerminalKey::Insert,
        Key::Delete => TerminalKey::Delete,
        Key::F1 => TerminalKey::Function(1),
        Key::F2 => TerminalKey::Function(2),
        Key::F3 => TerminalKey::Function(3),
        Key::F4 => TerminalKey::Function(4),
        Key::F5 => TerminalKey::Function(5),
        Key::F6 => TerminalKey::Function(6),
        Key::F7 => TerminalKey::Function(7),
        Key::F8 => TerminalKey::Function(8),
        Key::F9 => TerminalKey::Function(9),
        Key::F10 => TerminalKey::Function(10),
        Key::F11 => TerminalKey::Function(11),
        Key::F12 => TerminalKey::Function(12),
        _ => return None,
    };
    Some(terminal_key)
}

pub fn terminal_modifiers_of(modifiers: Modifiers) -> KeyModifiers {
    KeyModifiers { shift: modifiers.shift, alt: modifiers.alt, ctrl: modifiers.ctrl }
}

fn letter_of(key: Key) -> Option<char> {
    let mut characters = key.name().chars();
    match (characters.next(), characters.next()) {
        (Some(letter), None) if letter.is_ascii_alphabetic() => Some(letter),
        _ => None,
    }
}
