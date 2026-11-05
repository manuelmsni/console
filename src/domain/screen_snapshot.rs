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
