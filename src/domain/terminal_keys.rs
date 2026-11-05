//! Cómo se le cuentan las teclas especiales a un programa de terminal.
//! Las terminales no reciben "flecha arriba": reciben secuencias de bytes
//! como `ESC [ A`. Este módulo hace esa traducción.

/// Teclas que no son simplemente texto escrito.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalKey {
    Enter,
    Backspace,
    Tab,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    /// F1 a F12.
    Function(u8),
    /// Ctrl + una letra, por ejemplo Ctrl+C para interrumpir.
    ControlLetter(char),
    ControlSpace,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

impl KeyModifiers {
    pub const NONE: Self = Self { shift: false, alt: false, ctrl: false };

    /// Número con el que xterm codifica los modificadores (1 = ninguno).
    fn xterm_code(self) -> u8 {
        1 + self.shift as u8 + 2 * self.alt as u8 + 4 * self.ctrl as u8
    }
}

/// Modos que el programa en ejecución puede activar y que cambian
/// los bytes que hay que enviarle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputModes {
    /// Algunos programas (vim, less...) piden las flechas en formato `ESC O A`.
    pub application_cursor_keys: bool,
    /// El programa quiere saber qué texto es pegado y cuál tecleado.
    pub bracketed_paste: bool,
}

pub fn encode_key_press(key: TerminalKey, modifiers: KeyModifiers, modes: InputModes) -> Vec<u8> {
    let modifier_code = modifiers.xterm_code();
    let sequence = match key {
        TerminalKey::Enter if modifiers.alt => "\x1b\r".to_string(),
        TerminalKey::Enter => "\r".to_string(),
        TerminalKey::Backspace if modifiers.ctrl => "\x08".to_string(),
        TerminalKey::Backspace => "\x7f".to_string(),
        TerminalKey::Tab if modifiers.shift => "\x1b[Z".to_string(),
        TerminalKey::Tab => "\t".to_string(),
        TerminalKey::Escape => "\x1b".to_string(),
        TerminalKey::ArrowUp => cursor_key_sequence('A', modifier_code, modes),
        TerminalKey::ArrowDown => cursor_key_sequence('B', modifier_code, modes),
        TerminalKey::ArrowRight => cursor_key_sequence('C', modifier_code, modes),
        TerminalKey::ArrowLeft => cursor_key_sequence('D', modifier_code, modes),
        TerminalKey::Home => cursor_key_sequence('H', modifier_code, modes),
        TerminalKey::End => cursor_key_sequence('F', modifier_code, modes),
        TerminalKey::Insert => numbered_key_sequence(2, modifier_code),
        TerminalKey::Delete => numbered_key_sequence(3, modifier_code),
        TerminalKey::PageUp => numbered_key_sequence(5, modifier_code),
        TerminalKey::PageDown => numbered_key_sequence(6, modifier_code),
        TerminalKey::Function(number) => function_key_sequence(number, modifier_code),
        TerminalKey::ControlLetter(letter) => control_character_for(letter).to_string(),
        TerminalKey::ControlSpace => "\0".to_string(),
    };
    sequence.into_bytes()
}

/// Prepara texto pegado para enviarlo a la terminal. Los saltos de línea se
/// envían como Enter (`\r`), igual que si se pulsara la tecla.
pub fn encode_paste(text: &str, modes: InputModes) -> Vec<u8> {
    let text_with_terminal_newlines = text.replace("\r\n", "\r").replace('\n', "\r");
    if modes.bracketed_paste {
        format!("\x1b[200~{text_with_terminal_newlines}\x1b[201~").into_bytes()
    } else {
        text_with_terminal_newlines.into_bytes()
    }
}

/// Sustituye lo escrito en el prompt por otro texto: Fin lleva el cursor al
/// final y Ctrl+Inicio borra todo lo que hay a su izquierda (así funciona
/// tanto en cmd como en PowerShell). No se usa Esc porque, seguido de texto,
/// la consola lo interpretaría como Alt+letra.
pub fn encode_prompt_line_replacement(new_text: &str) -> Vec<u8> {
    let end_key = "\x1b[F";
    let ctrl_home_key = "\x1b[1;5H";
    format!("{end_key}{ctrl_home_key}{new_text}").into_bytes()
}

fn cursor_key_sequence(final_letter: char, modifier_code: u8, modes: InputModes) -> String {
    if modifier_code > 1 {
        format!("\x1b[1;{modifier_code}{final_letter}")
    } else if modes.application_cursor_keys {
        format!("\x1bO{final_letter}")
    } else {
        format!("\x1b[{final_letter}")
    }
}

fn numbered_key_sequence(key_number: u8, modifier_code: u8) -> String {
    if modifier_code > 1 {
        format!("\x1b[{key_number};{modifier_code}~")
    } else {
        format!("\x1b[{key_number}~")
    }
}

fn function_key_sequence(function_number: u8, modifier_code: u8) -> String {
    let key_number = match function_number {
        1..=4 => {
            let final_letter = ['P', 'Q', 'R', 'S'][(function_number - 1) as usize];
            return if modifier_code > 1 {
                format!("\x1b[1;{modifier_code}{final_letter}")
            } else {
                format!("\x1bO{final_letter}")
            };
        }
        5 => 15,
        6 => 17,
        7 => 18,
        8 => 19,
        9 => 20,
        10 => 21,
        11 => 23,
        12 => 24,
        _ => return String::new(),
    };
    numbered_key_sequence(key_number, modifier_code)
}

/// Ctrl+A es el byte 1, Ctrl+B el 2... hasta Ctrl+Z, el 26.
fn control_character_for(letter: char) -> char {
    char::from(letter.to_ascii_uppercase() as u8 & 0x1f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_is_the_interrupt_byte() {
        let bytes = encode_key_press(TerminalKey::ControlLetter('c'), KeyModifiers::NONE, InputModes::default());
        assert_eq!(bytes, vec![3]);
    }

    #[test]
    fn arrows_follow_application_cursor_mode() {
        let normal = encode_key_press(TerminalKey::ArrowUp, KeyModifiers::NONE, InputModes::default());
        let application = encode_key_press(
            TerminalKey::ArrowUp,
            KeyModifiers::NONE,
            InputModes { application_cursor_keys: true, ..InputModes::default() },
        );
        assert_eq!(normal, b"\x1b[A");
        assert_eq!(application, b"\x1bOA");
    }

    #[test]
    fn ctrl_arrow_includes_modifier_code() {
        let ctrl = KeyModifiers { ctrl: true, ..KeyModifiers::NONE };
        let bytes = encode_key_press(TerminalKey::ArrowLeft, ctrl, InputModes::default());
        assert_eq!(bytes, b"\x1b[1;5D");
    }

    #[test]
    fn paste_is_wrapped_when_program_asks_for_it() {
        let modes = InputModes { bracketed_paste: true, ..InputModes::default() };
        assert_eq!(encode_paste("a\r\nb", modes), b"\x1b[200~a\rb\x1b[201~");
    }
}
