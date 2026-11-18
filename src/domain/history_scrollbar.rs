//! La barra de scroll del historial de una pestaña: dónde va el tirador y a
//! qué parte del historial lleva al arrastrarlo. Las posiciones son
//! fracciones de la altura de la barra (0 arriba, 1 abajo), así que no
//! dependen de píxeles.

/// Qué parte del historial se está viendo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HistoryView {
    /// Líneas que ya salieron por arriba de la pantalla.
    pub lines_above_screen: usize,
    /// Cuántas de ellas se han subido para verlas (0: lo más reciente).
    pub lines_scrolled_up: usize,
    pub visible_rows: usize,
}

/// El tirador, en fracciones de la altura de la barra.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thumb {
    pub top: f32,
    pub height: f32,
}

impl HistoryView {
    pub fn has_history(&self) -> bool {
        self.lines_above_screen > 0
    }

    /// Proporcional a lo que se ve del total, pero nunca más pequeño que
    /// `minimum_height`, para poder agarrarlo aunque el historial sea enorme.
    pub fn thumb(&self, minimum_height: f32) -> Thumb {
        let total_lines = (self.lines_above_screen + self.visible_rows).max(1) as f32;
        let height = (self.visible_rows as f32 / total_lines).clamp(minimum_height.min(1.0), 1.0);
        let top = if self.has_history() { (1.0 - height) * self.first_visible_line() as f32 / self.lines_above_screen as f32 } else { 0.0 };
        Thumb { top, height }
    }

    /// Cuántas líneas hay que subir para que el tirador quede con su borde
    /// de arriba en `thumb_top` (lo contrario de `thumb`).
    pub fn lines_scrolled_up_for(&self, thumb_top: f32, thumb_height: f32) -> usize {
        let free_space = 1.0 - thumb_height;
        if !self.has_history() || free_space <= 0.0 {
            return 0;
        }
        let fraction_from_top = (thumb_top / free_space).clamp(0.0, 1.0);
        let first_visible_line = (fraction_from_top * self.lines_above_screen as f32).round() as usize;
        self.lines_above_screen - first_visible_line.min(self.lines_above_screen)
    }

    /// Contando desde la línea más antigua del historial.
    fn first_visible_line(&self) -> usize {
        self.lines_above_screen - self.lines_scrolled_up.min(self.lines_above_screen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(lines_above_screen: usize, lines_scrolled_up: usize) -> HistoryView {
        HistoryView { lines_above_screen, lines_scrolled_up, visible_rows: 20 }
    }

    #[test]
    fn at_the_newest_output_the_thumb_is_at_the_bottom() {
        let thumb = view(60, 0).thumb(0.05);
        assert_eq!(thumb.height, 0.25);
        assert_eq!(thumb.top + thumb.height, 1.0);
    }

    #[test]
    fn at_the_oldest_line_the_thumb_is_at_the_top() {
        assert_eq!(view(60, 60).thumb(0.05).top, 0.0);
    }

    #[test]
    fn a_huge_history_still_has_a_thumb_big_enough_to_grab() {
        let thumb = view(10_000, 5_000).thumb(0.1);
        assert_eq!(thumb.height, 0.1);
        assert!((thumb.top - 0.45).abs() < 0.001);
    }

    #[test]
    fn dragging_the_thumb_leads_back_to_the_same_lines() {
        for view in [view(60, 0), view(60, 25), view(60, 60), view(10_000, 1234)] {
            let thumb = view.thumb(0.1);
            assert_eq!(view.lines_scrolled_up_for(thumb.top, thumb.height), view.lines_scrolled_up, "{view:?}");
        }
    }

    #[test]
    fn dragging_beyond_the_ends_stops_at_them() {
        let view = view(60, 10);
        let thumb = view.thumb(0.1);
        assert_eq!(view.lines_scrolled_up_for(-0.5, thumb.height), 60);
        assert_eq!(view.lines_scrolled_up_for(2.0, thumb.height), 0);
    }

    #[test]
    fn without_history_there_is_nothing_to_scroll() {
        let empty = view(0, 0);
        assert!(!empty.has_history());
        assert_eq!(empty.thumb(0.1), Thumb { top: 0.0, height: 1.0 });
        assert_eq!(empty.lines_scrolled_up_for(0.3, 1.0), 0);
    }
}
