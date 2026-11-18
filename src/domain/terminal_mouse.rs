//! Qué hacen la rueda y los clics sobre la terminal. Depende de lo que haya
//! pedido el programa que está dentro:
//!
//! - Si quiere el ratón (Claude Code a pantalla completa, vim, htop...), se le
//!   cuentan los giros de la rueda y los clics, y él decide (subir por su
//!   conversación, pulsar "Jump to bottom", elegir una opción...). Arrastrar,
//!   en cambio, sigue seleccionando texto en la app, para poder copiar siempre.
//! - Si usa la pantalla alternativa sin pedir el ratón (less, man...), la
//!   rueda le manda flechas, como hacen xterm y Windows Terminal: esa pantalla
//!   no tiene historial que mover.
//! - Si no, la rueda mueve el historial de la pestaña.

use super::terminal_grid::GridPosition;
use super::terminal_keys::{encode_key_press, InputModes, KeyModifiers, TerminalKey};

/// Las terminales mandan un aviso por cada "clic" de la rueda, que suele
/// equivaler a unas 3 líneas de historial.
const LINES_PER_WHEEL_EVENT: i32 = 3;

/// Botones con los que xterm cuenta el ratón.
const LEFT_BUTTON: u32 = 0;
/// En el formato antiguo, soltar no dice qué botón se soltó.
const BUTTON_RELEASED: u32 = 3;
/// Moverse sin ningún botón pulsado: "soltado" más 32, la marca de movimiento.
const POINTER_MOVED: u32 = 35;
const WHEEL_UP_BUTTON: u32 = 64;
const WHEEL_DOWN_BUTTON: u32 = 65;

/// Cómo quiere el programa que se le escriban las posiciones del ratón.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseEncoding {
    /// El formato original de xterm: un byte por número (posiciones hasta la 223).
    SingleBytes,
    /// Igual, pero cada número como un carácter UTF-8 (pantallas más anchas).
    Utf8,
    /// `ESC [ < botón ; columna ; fila M` (o `m` al soltar), el de los programas modernos.
    Sgr,
}

/// `true` si desplazarse es cosa del programa (tiene su propio historial o
/// ninguno), no del historial de la pestaña.
pub fn program_handles_scrolling(modes: InputModes) -> bool {
    modes.mouse_reporting.is_some() || modes.alternate_screen
}

/// Lo que hay que mandar al programa por un clic con el botón izquierdo
/// (pulsar y soltar), o nada si no ha pedido el ratón.
pub fn encode_click(position: GridPosition, modes: InputModes) -> Option<Vec<u8>> {
    let encoding = modes.mouse_reporting?;
    let mut bytes = encode_mouse_event(LEFT_BUTTON, ButtonState::Pressed, position, encoding);
    bytes.extend(encode_mouse_event(LEFT_BUTTON, ButtonState::Released, position, encoding));
    Some(bytes)
}

/// Lo que hay que mandar al programa porque el ratón ha pasado a otra celda
/// (sin pulsar nada), o nada si no lo ha pedido.
pub fn encode_pointer_motion(position: GridPosition, modes: InputModes) -> Option<Vec<u8>> {
    let encoding = modes.mouse_reporting.filter(|_| modes.reports_pointer_motion)?;
    Some(encode_mouse_event(POINTER_MOVED, ButtonState::Pressed, position, encoding))
}

#[derive(Debug, PartialEq, Eq)]
pub enum WheelEffect {
    /// Positivo: hacia líneas más antiguas.
    ScrollHistory(i32),
    SendToProgram(Vec<u8>),
    /// Todavía no ha girado lo bastante para un aviso al programa.
    Nothing,
}

/// La rueda de una pestaña: recuerda las líneas que aún no llegan a un aviso.
#[derive(Debug, Default)]
pub struct MouseWheel {
    lines_not_yet_reported: i32,
}

impl MouseWheel {
    /// `lines` positivo es girar hacia arriba (hacia lo más antiguo);
    /// `pointer` es la celda sobre la que está el ratón.
    pub fn turn(&mut self, lines: i32, pointer: GridPosition, modes: InputModes) -> WheelEffect {
        if let Some(encoding) = modes.mouse_reporting {
            self.lines_not_yet_reported += lines;
            let events = self.lines_not_yet_reported / LINES_PER_WHEEL_EVENT;
            self.lines_not_yet_reported -= events * LINES_PER_WHEEL_EVENT;
            let button = if events > 0 { WHEEL_UP_BUTTON } else { WHEEL_DOWN_BUTTON };
            let bytes: Vec<u8> = (0..events.abs())
                .flat_map(|_| encode_mouse_event(button, ButtonState::Pressed, pointer, encoding))
                .collect();
            return if bytes.is_empty() { WheelEffect::Nothing } else { WheelEffect::SendToProgram(bytes) };
        }
        self.lines_not_yet_reported = 0;
        if lines == 0 {
            return WheelEffect::Nothing;
        }
        if modes.alternate_screen {
            let key = if lines > 0 { TerminalKey::ArrowUp } else { TerminalKey::ArrowDown };
            let arrow = encode_key_press(key, KeyModifiers::NONE, modes);
            return WheelEffect::SendToProgram(arrow.repeat(lines.unsigned_abs() as usize));
        }
        WheelEffect::ScrollHistory(lines)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonState {
    Pressed,
    Released,
}

/// Las posiciones del ratón se cuentan desde 1, no desde 0.
fn encode_mouse_event(button: u32, state: ButtonState, position: GridPosition, encoding: MouseEncoding) -> Vec<u8> {
    let column = u32::from(position.column) + 1;
    let row = u32::from(position.row) + 1;
    match encoding {
        MouseEncoding::Sgr => {
            let final_letter = if state == ButtonState::Pressed { 'M' } else { 'm' };
            format!("\x1b[<{button};{column};{row}{final_letter}").into_bytes()
        }
        MouseEncoding::SingleBytes | MouseEncoding::Utf8 => {
            let button = if state == ButtonState::Pressed { button } else { BUTTON_RELEASED };
            // Cada número va sumado a 32, para que sea un carácter que se pueda escribir.
            let numbers = [button, column, row].map(|number| 32 + number);
            let mut bytes = b"\x1b[M".to_vec();
            if encoding == MouseEncoding::SingleBytes {
                bytes.extend(numbers.map(|number| number.min(255) as u8));
            } else {
                let characters: String = numbers.iter().map(|&number| char::from_u32(number.min(2047)).unwrap_or(' ')).collect();
                bytes.extend(characters.as_bytes());
            }
            bytes
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POINTER: GridPosition = GridPosition { row: 4, column: 9 };

    fn program_wanting_the_mouse(encoding: MouseEncoding) -> InputModes {
        InputModes { mouse_reporting: Some(encoding), alternate_screen: true, ..InputModes::default() }
    }

    #[test]
    fn without_a_program_asking_for_it_the_wheel_moves_the_history() {
        let mut wheel = MouseWheel::default();
        assert_eq!(wheel.turn(3, POINTER, InputModes::default()), WheelEffect::ScrollHistory(3));
        assert_eq!(wheel.turn(-2, POINTER, InputModes::default()), WheelEffect::ScrollHistory(-2));
        assert!(!program_handles_scrolling(InputModes::default()));
    }

    #[test]
    fn a_program_that_wants_the_mouse_receives_the_wheel_where_the_pointer_is() {
        let mut wheel = MouseWheel::default();
        let modes = program_wanting_the_mouse(MouseEncoding::Sgr);
        assert_eq!(wheel.turn(3, POINTER, modes), WheelEffect::SendToProgram(b"\x1b[<64;10;5M".to_vec()));
        assert_eq!(wheel.turn(-3, POINTER, modes), WheelEffect::SendToProgram(b"\x1b[<65;10;5M".to_vec()));
        assert!(program_handles_scrolling(modes));
    }

    #[test]
    fn small_turns_add_up_until_they_make_a_whole_event() {
        let mut wheel = MouseWheel::default();
        let modes = program_wanting_the_mouse(MouseEncoding::Sgr);
        assert_eq!(wheel.turn(2, POINTER, modes), WheelEffect::Nothing);
        assert_eq!(wheel.turn(1, POINTER, modes), WheelEffect::SendToProgram(b"\x1b[<64;10;5M".to_vec()));
        assert_eq!(wheel.turn(7, POINTER, modes), WheelEffect::SendToProgram(b"\x1b[<64;10;5M".repeat(2)));
    }

    #[test]
    fn older_programs_get_the_position_in_single_bytes_or_utf8() {
        let mut wheel = MouseWheel::default();
        let bytes = wheel.turn(3, POINTER, program_wanting_the_mouse(MouseEncoding::SingleBytes));
        assert_eq!(bytes, WheelEffect::SendToProgram(vec![0x1b, b'[', b'M', 32 + 64, 32 + 10, 32 + 5]));

        // Columna 300: no cabe en un byte, pero sí en un carácter UTF-8.
        let far_right = GridPosition { row: 0, column: 299 };
        let bytes = wheel.turn(-3, far_right, program_wanting_the_mouse(MouseEncoding::Utf8));
        let expected = format!("\x1b[M{}{}{}", char::from(32 + 65), char::from_u32(32 + 300).unwrap(), char::from(32 + 1));
        assert_eq!(bytes, WheelEffect::SendToProgram(expected.into_bytes()));
    }

    #[test]
    fn on_the_alternate_screen_without_mouse_the_wheel_sends_arrows() {
        let mut wheel = MouseWheel::default();
        let modes = InputModes { alternate_screen: true, application_cursor_keys: true, ..InputModes::default() };
        assert_eq!(wheel.turn(2, POINTER, modes), WheelEffect::SendToProgram(b"\x1bOA\x1bOA".to_vec()));
        assert_eq!(wheel.turn(-1, POINTER, modes), WheelEffect::SendToProgram(b"\x1bOB".to_vec()));
        assert!(program_handles_scrolling(modes));
    }

    #[test]
    fn pointer_motion_reaches_only_programs_that_asked_for_it() {
        let clicks_only = program_wanting_the_mouse(MouseEncoding::Sgr);
        assert_eq!(encode_pointer_motion(POINTER, clicks_only), None);
        let every_move = InputModes { reports_pointer_motion: true, ..clicks_only };
        assert_eq!(encode_pointer_motion(POINTER, every_move), Some(b"\x1b[<35;10;5M".to_vec()));
    }

    #[test]
    fn a_click_reaches_the_program_only_if_it_asked_for_the_mouse() {
        assert_eq!(encode_click(POINTER, InputModes::default()), None);
        assert_eq!(
            encode_click(POINTER, program_wanting_the_mouse(MouseEncoding::Sgr)),
            Some(b"\x1b[<0;10;5M\x1b[<0;10;5m".to_vec())
        );
        assert_eq!(
            encode_click(POINTER, program_wanting_the_mouse(MouseEncoding::SingleBytes)),
            Some(vec![0x1b, b'[', b'M', 32, 32 + 10, 32 + 5, 0x1b, b'[', b'M', 32 + 3, 32 + 10, 32 + 5])
        );
    }
}
