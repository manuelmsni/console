/// Tamaño de una terminal medido en celdas de texto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSize {
    pub rows: u16,
    pub columns: u16,
}

/// Una celda concreta de la pantalla visible. Se ordena primero por fila
/// y después por columna, que es el orden de lectura del texto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GridPosition {
    pub row: u16,
    pub column: u16,
}

/// Texto seleccionado con el ratón: desde donde se empezó a arrastrar
/// (`anchor`) hasta donde está ahora el ratón (`focus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSelection {
    anchor: GridPosition,
    focus: GridPosition,
}

impl TextSelection {
    pub fn starting_at(position: GridPosition) -> Self {
        Self { anchor: position, focus: position }
    }

    pub fn extend_to(&mut self, position: GridPosition) {
        self.focus = position;
    }

    /// La primera celda en orden de lectura, da igual hacia dónde se arrastró.
    pub fn first_position(&self) -> GridPosition {
        self.anchor.min(self.focus)
    }

    pub fn last_position(&self) -> GridPosition {
        self.anchor.max(self.focus)
    }

    pub fn contains(&self, position: GridPosition) -> bool {
        self.first_position() <= position && position <= self.last_position()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(row: u16, column: u16) -> GridPosition {
        GridPosition { row, column }
    }

    #[test]
    fn selection_dragged_backwards_is_read_in_normal_order() {
        let mut selection = TextSelection::starting_at(at(5, 10));
        selection.extend_to(at(2, 3));

        assert_eq!(selection.first_position(), at(2, 3));
        assert_eq!(selection.last_position(), at(5, 10));
    }

    #[test]
    fn selection_covers_whole_middle_rows() {
        let mut selection = TextSelection::starting_at(at(1, 70));
        selection.extend_to(at(3, 2));

        assert!(selection.contains(at(2, 0)));
        assert!(selection.contains(at(2, 79)));
        assert!(!selection.contains(at(1, 69)));
        assert!(!selection.contains(at(3, 3)));
    }
}
