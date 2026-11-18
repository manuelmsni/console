use super::terminal_grid::GridPosition;

/// Color de una celda tal como lo pide el programa que corre en la terminal.
/// La traducción a colores reales de pantalla la decide la interfaz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalColor {
    DefaultForeground,
    DefaultBackground,
    /// Uno de los 256 colores de la paleta clásica de terminal.
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenCell {
    /// Vacío si la celda está en blanco.
    pub text: String,
    pub foreground: TerminalColor,
    pub background: TerminalColor,
    pub is_bold: bool,
}

impl ScreenCell {
    pub fn blank() -> Self {
        Self {
            text: String::new(),
            foreground: TerminalColor::DefaultForeground,
            background: TerminalColor::DefaultBackground,
            is_bold: false,
        }
    }
}

/// Foto de lo que se ve ahora mismo en la terminal, lista para dibujar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenSnapshot {
    pub rows: Vec<Vec<ScreenCell>>,
    /// `None` cuando el programa oculta el cursor o se está viendo el historial.
    pub visible_cursor: Option<GridPosition>,
}

impl ScreenSnapshot {
    /// La última fila con algo: texto o un fondo de color (los cuadros de
    /// escritura de algunos programas son solo fondo). `None` si está vacía.
    pub fn last_row_in_use(&self) -> Option<u16> {
        let is_in_use = |cell: &ScreenCell| !cell.text.trim().is_empty() || cell.background != TerminalColor::DefaultBackground;
        let row = self.rows.iter().rposition(|row| row.iter().any(is_in_use))?;
        u16::try_from(row).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen_of(rows: &[&str]) -> ScreenSnapshot {
        let rows = rows
            .iter()
            .map(|row| row.chars().map(|letter| ScreenCell { text: letter.to_string(), ..ScreenCell::blank() }).collect())
            .collect();
        ScreenSnapshot { rows, visible_cursor: None }
    }

    #[test]
    fn the_last_row_in_use_ignores_blank_rows_below_it() {
        assert_eq!(screen_of(&["hola", "  ", "adios", "   ", ""]).last_row_in_use(), Some(2));
        assert_eq!(screen_of(&["", "  "]).last_row_in_use(), None);
    }

    #[test]
    fn a_row_with_only_a_colored_background_is_in_use() {
        let mut screen = screen_of(&["hola", "    "]);
        screen.rows[1][2].background = TerminalColor::Indexed(8);
        assert_eq!(screen.last_row_in_use(), Some(1));
    }
}
