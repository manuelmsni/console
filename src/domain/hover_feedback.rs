//! Saber si lo que hay bajo el ratón se puede pulsar, para poner la mano.
//!
//! Los programas de terminal no le dicen a la terminal qué es un botón. Pero
//! los que piden el movimiento del ratón (Claude Code a pantalla completa)
//! iluminan lo que se puede pulsar al pasar por encima: le cambian el fondo
//! sin cambiar el texto. Así que se recuerda cómo estaba la fila del ratón al
//! llegar a ella y, si la celda bajo el ratón ha cambiado de fondo pero no de
//! texto, es que el programa ha reaccionado: se puede pulsar.

use super::screen_snapshot::{ScreenSnapshot, TerminalColor};
use super::terminal_grid::GridPosition;

#[derive(Debug, Default)]
pub struct HoverFeedback {
    /// La fila del ratón tal como estaba al llegar a ella.
    row_before_hover: Option<RowContents>,
}

#[derive(Debug, PartialEq)]
struct RowContents {
    row: u16,
    texts: Vec<String>,
    backgrounds: Vec<TerminalColor>,
}

impl HoverFeedback {
    /// Hay que llamarla al cambiar el ratón de celda, antes de contárselo al
    /// programa (para que la fila se recuerde aún sin iluminar). `None`: el
    /// ratón ya no está sobre la terminal.
    pub fn pointer_moved(&mut self, position: Option<GridPosition>, screen: &ScreenSnapshot) {
        self.row_before_hover = match position {
            Some(position) if self.remembered_row() == Some(position.row) => return,
            Some(position) => row_contents(screen, position.row),
            None => None,
        };
    }

    /// `true` si el programa ha iluminado la celda bajo el ratón.
    pub fn program_highlights(&mut self, position: GridPosition, screen: &ScreenSnapshot) -> bool {
        let Some(now) = row_contents(screen, position.row) else { return false };
        let Some(before) = &self.row_before_hover else { return false };
        if before.row != position.row {
            return false;
        }
        if before.texts != now.texts {
            // La fila ha cambiado por otra cosa (llega texto, se ha desplazado...):
            // se toma como nuevo punto de partida.
            self.row_before_hover = Some(now);
            return false;
        }
        let column = usize::from(position.column);
        before.backgrounds.get(column) != now.backgrounds.get(column)
    }

    fn remembered_row(&self) -> Option<u16> {
        self.row_before_hover.as_ref().map(|contents| contents.row)
    }
}

fn row_contents(screen: &ScreenSnapshot, row: u16) -> Option<RowContents> {
    let cells = screen.rows.get(usize::from(row))?;
    Some(RowContents {
        row,
        // Una celda vacía y un espacio se ven igual: al iluminar una fila, los
        // programas suelen rellenarla con espacios de color.
        texts: cells.iter().map(|cell| cell.text.trim().to_string()).collect(),
        backgrounds: cells.iter().map(|cell| cell.background).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::screen_snapshot::ScreenCell;

    const BUTTON_COLOR: TerminalColor = TerminalColor::Rgb(55, 55, 55);
    const HOVERED_BUTTON_COLOR: TerminalColor = TerminalColor::Rgb(70, 70, 70);

    /// Una pantalla de una fila: "ab" normal y "[ok]" como botón, en `button_color`.
    fn screen_with_button(button_color: TerminalColor, button_text: &str) -> ScreenSnapshot {
        let mut row: Vec<ScreenCell> = "ab".chars().map(|letter| cell(letter, TerminalColor::DefaultBackground)).collect();
        row.extend(button_text.chars().map(|letter| cell(letter, button_color)));
        ScreenSnapshot { rows: vec![row], visible_cursor: None }
    }

    fn cell(letter: char, background: TerminalColor) -> ScreenCell {
        ScreenCell { text: letter.to_string(), background, ..ScreenCell::blank() }
    }

    fn at(column: u16) -> GridPosition {
        GridPosition { row: 0, column }
    }

    #[test]
    fn a_button_lit_up_under_the_pointer_can_be_clicked() {
        let mut hover = HoverFeedback::default();
        hover.pointer_moved(Some(at(3)), &screen_with_button(BUTTON_COLOR, "[ok]"));

        let lit_up = screen_with_button(HOVERED_BUTTON_COLOR, "[ok]");
        assert!(hover.program_highlights(at(3), &lit_up));

        // Dentro del mismo botón sigue valiendo, y fuera de él no.
        hover.pointer_moved(Some(at(4)), &lit_up);
        assert!(hover.program_highlights(at(4), &lit_up));
        hover.pointer_moved(Some(at(0)), &screen_with_button(BUTTON_COLOR, "[ok]"));
        assert!(!hover.program_highlights(at(0), &screen_with_button(BUTTON_COLOR, "[ok]")));
    }

    /// Como las opciones de una lista de Claude Code: al iluminarla rellena con
    /// espacios de color hasta el final de la fila, donde antes no había nada.
    #[test]
    fn a_highlight_that_fills_empty_cells_with_spaces_still_counts() {
        let mut hover = HoverFeedback::default();
        let mut option = screen_with_button(TerminalColor::DefaultBackground, "3. ok");
        option.rows[0].extend((0..4).map(|_| ScreenCell::blank()));
        hover.pointer_moved(Some(at(4)), &option);

        let mut lit_up = screen_with_button(HOVERED_BUTTON_COLOR, "3. ok");
        lit_up.rows[0].extend((0..4).map(|_| cell(' ', HOVERED_BUTTON_COLOR)));
        assert!(hover.program_highlights(at(4), &lit_up));
        assert!(hover.program_highlights(at(9), &lit_up), "también en el relleno");
    }

    #[test]
    fn something_that_does_not_react_to_the_pointer_is_not_clickable() {
        let mut hover = HoverFeedback::default();
        let screen = screen_with_button(BUTTON_COLOR, "[ok]");
        hover.pointer_moved(Some(at(3)), &screen);
        assert!(!hover.program_highlights(at(3), &screen));
    }

    #[test]
    fn new_text_on_the_row_is_not_mistaken_for_a_highlight() {
        let mut hover = HoverFeedback::default();
        hover.pointer_moved(Some(at(3)), &screen_with_button(BUTTON_COLOR, "[ok]"));
        let other_text = screen_with_button(HOVERED_BUTTON_COLOR, "[no]");
        assert!(!hover.program_highlights(at(3), &other_text));
        assert!(!hover.program_highlights(at(3), &other_text), "lo nuevo pasa a ser el punto de partida");
    }

    #[test]
    fn with_the_pointer_away_nothing_is_clickable() {
        let mut hover = HoverFeedback::default();
        hover.pointer_moved(Some(at(3)), &screen_with_button(BUTTON_COLOR, "[ok]"));
        hover.pointer_moved(None, &screen_with_button(BUTTON_COLOR, "[ok]"));
        assert!(!hover.program_highlights(at(3), &screen_with_button(HOVERED_BUTTON_COLOR, "[ok]")));
    }
}
