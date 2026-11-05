//! Interpretación de la salida de terminal con la librería `vt100`.

use crate::application::ports::{TerminalEmulator, TerminalEmulatorFactory};
use crate::domain::screen_snapshot::{ScreenCell, ScreenSnapshot, TerminalColor};
use crate::domain::terminal_grid::{GridPosition, TerminalSize};
use crate::domain::terminal_keys::InputModes;

/// Cuántas líneas antiguas se recuerdan para poder subir con la rueda.
const HISTORY_LINES_KEPT: usize = 10_000;

pub struct Vt100EmulatorFactory;

impl TerminalEmulatorFactory for Vt100EmulatorFactory {
    fn create_emulator(&self, size: TerminalSize) -> Box<dyn TerminalEmulator> {
        Box::new(Vt100Emulator { parser: vt100::Parser::new(size.rows, size.columns, HISTORY_LINES_KEPT) })
    }
}

struct Vt100Emulator {
    parser: vt100::Parser,
}

impl TerminalEmulator for Vt100Emulator {
    fn process_shell_output(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    fn resize(&mut self, size: TerminalSize) {
        self.parser.set_size(size.rows, size.columns);
    }

    fn snapshot(&self) -> ScreenSnapshot {
        let screen = self.parser.screen();
        let (row_count, column_count) = screen.size();

        let rows = (0..row_count)
            .map(|row| {
                (0..column_count)
                    .map(|column| screen.cell(row, column).map_or_else(ScreenCell::blank, screen_cell_from))
                    .collect()
            })
            .collect();

        let is_viewing_history = screen.scrollback() > 0;
        let visible_cursor = if screen.hide_cursor() || is_viewing_history {
            None
        } else {
            let (row, column) = screen.cursor_position();
            Some(GridPosition { row, column })
        };

        ScreenSnapshot { rows, visible_cursor }
    }

    fn text_between(&self, first: GridPosition, last: GridPosition) -> String {
        // vt100 no incluye la columna final, por eso el +1.
        self.parser.screen().contents_between(first.row, first.column, last.row, last.column + 1)
    }

    fn input_modes(&self) -> InputModes {
        let screen = self.parser.screen();
        InputModes { application_cursor_keys: screen.application_cursor(), bracketed_paste: screen.bracketed_paste() }
    }

    fn scroll_history_by(&mut self, lines: i32) {
        let current_offset = self.parser.screen().scrollback() as i64;
        let new_offset = (current_offset + lines as i64).max(0) as usize;
        self.parser.set_scrollback(new_offset);
    }

    fn scroll_to_newest_output(&mut self) {
        self.parser.set_scrollback(0);
    }

    fn line_at_cursor(&self) -> String {
        let screen = self.parser.screen();
        let (_, column_count) = screen.size();
        let (cursor_row, _) = screen.cursor_position();
        // Si la línea es más larga que la pantalla, empieza en alguna fila de más arriba.
        let mut first_row = cursor_row;
        while first_row > 0 && screen.row_wrapped(first_row - 1) {
            first_row -= 1;
        }
        screen.contents_between(first_row, 0, cursor_row, column_count)
    }

    fn replayable_state(&mut self) -> Vec<u8> {
        let (row_count, _) = self.parser.screen().size();
        let mut bytes = Vec::new();
        // El historial de scroll se escribe como texto y se empuja hacia arriba
        // con tantos saltos de línea como filas tiene la pantalla menos una:
        // justo los necesarios para que quede entero encima, sin líneas vacías.
        for line in self.lines_above_screen() {
            bytes.extend(line.as_bytes());
            bytes.extend(b"\r\n");
        }
        bytes.extend("\r\n".repeat(row_count.saturating_sub(1) as usize).as_bytes());

        // La pantalla visible, con colores, cursor y modos, tal cual está.
        let offset_being_viewed = self.parser.screen().scrollback();
        self.parser.set_scrollback(0);
        bytes.extend(self.parser.screen().state_formatted());
        self.parser.set_scrollback(offset_being_viewed);
        bytes
    }
}

impl Vt100Emulator {
    /// Las líneas que ya salieron por arriba de la pantalla, de la más antigua a la más reciente.
    fn lines_above_screen(&mut self) -> Vec<String> {
        let offset_being_viewed = self.parser.screen().scrollback();
        let (_, column_count) = self.parser.screen().size();

        // vt100 solo deja leer lo que se ve, así que se recorre el historial
        // desplazando la vista una línea cada vez y leyendo la fila de arriba.
        self.parser.set_scrollback(usize::MAX);
        let history_length = self.parser.screen().scrollback();
        let mut lines = Vec::new();
        for offset in (1..=history_length).rev() {
            self.parser.set_scrollback(offset);
            lines.push(self.parser.screen().rows(0, column_count).next().unwrap_or_default());
        }
        self.parser.set_scrollback(offset_being_viewed);
        lines
    }

    /// Todo el texto (historial de scroll y pantalla), para comprobarlo en los tests.
    #[cfg(test)]
    fn all_text_lines(&mut self) -> Vec<String> {
        let (_, column_count) = self.parser.screen().size();
        let mut lines = self.lines_above_screen();
        self.parser.set_scrollback(0);
        lines.extend(self.parser.screen().rows(0, column_count));
        while lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.pop();
        }
        lines
    }
}

fn screen_cell_from(cell: &vt100::Cell) -> ScreenCell {
    let mut foreground = terminal_color_from(cell.fgcolor(), TerminalColor::DefaultForeground);
    let mut background = terminal_color_from(cell.bgcolor(), TerminalColor::DefaultBackground);
    if cell.inverse() {
        std::mem::swap(&mut foreground, &mut background);
    }
    ScreenCell { text: cell.contents().to_string(), foreground, background, is_bold: cell.bold() }
}

fn terminal_color_from(color: vt100::Color, default: TerminalColor) -> TerminalColor {
    match color {
        vt100::Color::Default => default,
        vt100::Color::Idx(index) => TerminalColor::Indexed(index),
        vt100::Color::Rgb(red, green, blue) => TerminalColor::Rgb(red, green, blue),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_emulator() -> Vt100Emulator {
        Vt100Emulator { parser: vt100::Parser::new(5, 20, HISTORY_LINES_KEPT) }
    }

    fn emulator_showing(output: &str) -> Vt100Emulator {
        let mut emulator = empty_emulator();
        emulator.process_shell_output(output.as_bytes());
        emulator
    }

    fn at(row: u16, column: u16) -> GridPosition {
        GridPosition { row, column }
    }

    #[test]
    fn selected_cells_become_plain_text_without_colors() {
        let emulator = emulator_showing("\x1b[31mhola\x1b[0m mundo");
        assert_eq!(emulator.text_between(at(0, 0), at(0, 3)), "hola");
        assert_eq!(emulator.text_between(at(0, 5), at(0, 9)), "mundo");
    }

    #[test]
    fn selection_across_rows_keeps_line_breaks() {
        let emulator = emulator_showing("primera\r\nsegunda");
        assert_eq!(emulator.text_between(at(0, 0), at(1, 6)), "primera\nsegunda");
    }

    #[test]
    fn long_line_at_cursor_is_read_whole() {
        let emulator = emulator_showing(r"C:\>echo una linea bastante larga");
        assert_eq!(emulator.line_at_cursor(), r"C:\>echo una linea bastante larga");
    }

    #[test]
    fn all_text_includes_lines_that_scrolled_off_screen() {
        let output: Vec<String> = (1..=8).map(|number| format!("linea {number}")).collect();
        let mut emulator = emulator_showing(&output.join("\r\n"));
        assert_eq!(emulator.all_text_lines(), output);
    }

    /// Lo que hace una pestaña restaurada: el texto viejo se empuja hacia
    /// arriba y la consola nueva (ConPTY) se coloca arriba y borra la pantalla.
    #[test]
    fn restored_text_survives_when_the_new_console_clears_the_screen() {
        let pushed_into_history = format!("C:\\>dir antiguo{}", "\r\n".repeat(5));
        let new_console_starting = "\x1b[H\x1b[2JC:\\>";
        let mut emulator = emulator_showing(&(pushed_into_history + new_console_starting));

        assert_eq!(emulator.all_text_lines().first().map(String::as_str), Some(r"C:\>dir antiguo"));
        assert_eq!(emulator.line_at_cursor(), r"C:\>", "la consola nueva queda arriba, lista para escribir");
    }

    /// Lo que se usa al compactar el diario: un emulador nuevo que reproduce
    /// la foto debe quedar igual (historial de scroll, pantalla y colores).
    #[test]
    fn replayable_state_rebuilds_the_same_screen_and_history() {
        let output: Vec<String> = (1..=8).map(|number| format!("linea {number}")).collect();
        let mut original = emulator_showing(&(output.join("\r\n") + "\r\n\x1b[31mrojo\x1b[0m"));

        let mut rebuilt = empty_emulator();
        rebuilt.process_shell_output(&original.replayable_state());

        assert_eq!(rebuilt.all_text_lines(), original.all_text_lines());
        assert_eq!(rebuilt.snapshot(), original.snapshot());
    }

    #[test]
    fn inverse_text_swaps_colors() {
        let emulator = emulator_showing("\x1b[7mX");
        let cell = &emulator.snapshot().rows[0][0];
        assert_eq!(cell.foreground, TerminalColor::DefaultBackground);
        assert_eq!(cell.background, TerminalColor::DefaultForeground);
    }
}
