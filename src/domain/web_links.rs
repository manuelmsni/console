//! Enlaces web en la pantalla de la terminal: encontrar la dirección que hay
//! bajo una celda para abrirla con Ctrl + clic.
//!
//! Una dirección larga ocupa varias filas, así que una fila que llega llena
//! hasta la última columna se considera continuada en la siguiente.

use super::screen_snapshot::ScreenSnapshot;
use super::terminal_grid::GridPosition;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebLink {
    pub url: String,
    /// Primera y última celda del enlace en pantalla (pueden estar en filas distintas).
    pub first: GridPosition,
    pub last: GridPosition,
}

impl WebLink {
    /// Las celdas del enlace, fila por fila: (fila, primera columna, última columna).
    pub fn row_spans(&self, columns: u16) -> Vec<(u16, u16, u16)> {
        (self.first.row..=self.last.row)
            .map(|row| {
                let from = if row == self.first.row { self.first.column } else { 0 };
                let to = if row == self.last.row { self.last.column } else { columns.saturating_sub(1) };
                (row, from, to)
            })
            .collect()
    }
}

const LINK_STARTS: [&str; 2] = ["https://", "http://"];

/// El enlace que pasa por `position`, si hay alguno.
pub fn web_link_at(screen: &ScreenSnapshot, position: GridPosition) -> Option<WebLink> {
    let rows: Vec<Vec<char>> = screen.rows.iter().map(|row| row.iter().map(|cell| cell.text.chars().next().unwrap_or(' ')).collect()).collect();
    let pointed_row = usize::from(position.row);
    let columns = rows.get(pointed_row)?.len();
    if columns == 0 {
        return None;
    }
    let row_is_full = |row: usize| rows[row].last().is_some_and(|character| !character.is_whitespace());

    // Las filas que forman una sola línea larga con la señalada.
    let mut first_row = pointed_row;
    while first_row > 0 && row_is_full(first_row - 1) && rows[first_row - 1].len() == columns {
        first_row -= 1;
    }
    let mut last_row = pointed_row;
    while last_row + 1 < rows.len() && row_is_full(last_row) && rows[last_row + 1].len() == columns {
        last_row += 1;
    }
    let line: Vec<char> = rows[first_row..=last_row].concat();
    let pointed_index = (pointed_row - first_row) * columns + usize::from(position.column);

    let (start, end) = link_ranges_in(&line).into_iter().find(|(start, end)| (*start..*end).contains(&pointed_index))?;
    let position_of = |index: usize| GridPosition { row: (first_row + index / columns) as u16, column: (index % columns) as u16 };
    Some(WebLink { url: line[start..end].iter().collect(), first: position_of(start), last: position_of(end - 1) })
}

/// Dónde empieza y acaba (sin incluir) cada enlace de la línea.
fn link_ranges_in(line: &[char]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < line.len() {
        let starts_a_link = LINK_STARTS.iter().any(|start| line[index..].iter().take(start.len()).copied().eq(start.chars()));
        if !starts_a_link {
            index += 1;
            continue;
        }
        let mut end = index;
        while end < line.len() && is_part_of_a_link(line[end]) {
            end += 1;
        }
        end = without_trailing_punctuation(line, index, end);
        if end - index > "https://".len() - 1 {
            ranges.push((index, end));
        }
        index = end.max(index + 1);
    }
    ranges
}

fn is_part_of_a_link(character: char) -> bool {
    !character.is_whitespace() && !matches!(character, '"' | '\'' | '<' | '>' | '`' | '│' | '|')
}

/// El punto o la coma tras una dirección son de la frase, no de la dirección;
/// un paréntesis de cierre solo es de ella si también abrió uno.
fn without_trailing_punctuation(line: &[char], start: usize, mut end: usize) -> usize {
    while end > start {
        let last = line[end - 1];
        let count = |wanted: char| line[start..end].iter().filter(|character| **character == wanted).count();
        let is_unbalanced_closing = match last {
            ')' => count(')') > count('('),
            ']' => count(']') > count('['),
            '}' => count('}') > count('{'),
            _ => false,
        };
        if matches!(last, '.' | ',' | ';' | ':' | '!' | '?') || is_unbalanced_closing {
            end -= 1;
        } else {
            break;
        }
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::screen_snapshot::ScreenCell;

    fn screen(lines: &[&str]) -> ScreenSnapshot {
        let rows = lines
            .iter()
            .map(|line| line.chars().map(|character| ScreenCell { text: character.to_string(), ..ScreenCell::blank() }).collect())
            .collect();
        ScreenSnapshot { rows, visible_cursor: None }
    }

    fn at(row: u16, column: u16) -> GridPosition {
        GridPosition { row, column }
    }

    #[test]
    fn finds_the_link_under_the_pointer() {
        let screen = screen(&["Abre https://example.com/docs para leer   "]);
        let link = web_link_at(&screen, at(0, 10)).unwrap();
        assert_eq!(link.url, "https://example.com/docs");
        assert_eq!((link.first, link.last), (at(0, 5), at(0, 28)));
    }

    #[test]
    fn outside_a_link_there_is_nothing() {
        let screen = screen(&["Abre https://example.com para leer"]);
        assert_eq!(web_link_at(&screen, at(0, 1)), None);
        assert_eq!(web_link_at(&screen, at(0, 26)), None);
    }

    #[test]
    fn the_full_stop_after_a_link_belongs_to_the_sentence() {
        let screen = screen(&["Ver https://example.com/a. Y (https://example.com/b), fin"]);
        assert_eq!(web_link_at(&screen, at(0, 6)).unwrap().url, "https://example.com/a");
        assert_eq!(web_link_at(&screen, at(0, 32)).unwrap().url, "https://example.com/b");
    }

    #[test]
    fn brackets_that_are_part_of_the_link_are_kept() {
        let screen = screen(&["https://es.wikipedia.org/wiki/Rust_(lenguaje) "]);
        assert_eq!(web_link_at(&screen, at(0, 3)).unwrap().url, "https://es.wikipedia.org/wiki/Rust_(lenguaje)");
    }

    #[test]
    fn a_long_link_continues_on_the_next_rows() {
        let screen = screen(&["login: https://cla", "ude.ai/oauth?code=", "abc123            "]);
        let link = web_link_at(&screen, at(2, 1)).unwrap();
        assert_eq!(link.url, "https://claude.ai/oauth?code=abc123");
        assert_eq!((link.first, link.last), (at(0, 7), at(2, 5)));
        assert_eq!(link.row_spans(18), vec![(0, 7, 17), (1, 0, 17), (2, 0, 5)]);
    }

    #[test]
    fn plain_text_is_not_a_link() {
        let screen = screen(&["http:// a medias y www.example.com sin protocolo"]);
        assert_eq!(web_link_at(&screen, at(0, 2)), None);
        assert_eq!(web_link_at(&screen, at(0, 22)), None);
    }
}
