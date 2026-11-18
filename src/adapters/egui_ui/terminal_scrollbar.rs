//! La barra de scroll de la terminal, en el margen derecho de la tarjeta (no
//! quita columnas al texto). Es para quien no tiene rueda o le va mal:
//!
//! - Con el historial de la pestaña es una barra normal: se arrastra el
//!   tirador, o se pulsa en la barra para saltar a ese punto.
//! - Si el historial lo lleva el programa (Claude Code a pantalla completa,
//!   less...), no se sabe cuánto tiene: la barra hace de palanca. Cuanto más
//!   se aleja el tirador del centro, más deprisa sube o baja; al soltarlo
//!   vuelve al centro. Pulsar por encima o por debajo pasa media pantalla.

use eframe::egui::{self, pos2, vec2, CursorIcon, Id, Painter, Pos2, Rect, Rounding, Sense, Shape, Stroke, Ui};

use super::theme::palette;
use crate::application::terminal_session::{ScrollControl, TerminalSession};
use crate::domain::history_scrollbar::HistoryView;
use crate::domain::terminal_grid::GridPosition;

const IDLE_WIDTH: f32 = 4.0;
const HOVERED_WIDTH: f32 = 8.0;
/// Separación entre el centro de la barra y el borde derecho de la tarjeta.
const DISTANCE_FROM_EDGE: f32 = 6.0;
const MINIMUM_THUMB_HEIGHT: f32 = 32.0;
const PROGRAM_GRIP_HEIGHT: f32 = 44.0;
/// Con la palanca al tope, líneas por segundo que se piden al programa.
const PROGRAM_LINES_PER_SECOND_AT_FULL_TILT: f32 = 90.0;
const ARROW_SIZE: f32 = 4.0;

/// Lo que la barra recuerda entre fotogramas.
#[derive(Default)]
pub struct TerminalScrollbar {
    /// Mientras se arrastra el tirador: a cuántos píxeles de su borde de arriba se agarró.
    grab_point_in_thumb: Option<f32>,
    /// Píxeles que se ha separado la palanca del centro (positivo: hacia abajo).
    program_grip_offset: f32,
    /// Líneas pedidas por la palanca que aún no llegan a una entera.
    unsent_program_lines: f32,
}

impl TerminalScrollbar {
    /// `margin` es la franja de la derecha de la tarjeta donde va la barra;
    /// `rows`, las filas de la terminal.
    pub fn show(&mut self, ui: &mut Ui, margin: Rect, rows: u16, session: &mut TerminalSession) {
        let id = Id::new("terminal_scrollbar");
        match session.scroll_control() {
            ScrollControl::TabHistory(view) => {
                self.forget_program_grip();
                if view.has_history() {
                    self.show_history_scrollbar(ui, id, margin, view, session);
                } else {
                    self.grab_point_in_thumb = None;
                }
            }
            ScrollControl::Program => {
                self.grab_point_in_thumb = None;
                self.show_program_lever(ui, id, margin, rows, session);
            }
        }
    }

    fn show_history_scrollbar(&mut self, ui: &mut Ui, id: Id, margin: Rect, view: HistoryView, session: &mut TerminalSession) {
        let response = ui.interact(margin, id, Sense::click_and_drag());
        let track_height = margin.height();
        let thumb_rect_for = |view: HistoryView| {
            let thumb = view.thumb(MINIMUM_THUMB_HEIGHT / track_height);
            (thumb.height, margin.top() + thumb.top * track_height, thumb.height * track_height)
        };

        let (thumb_fraction, thumb_top, thumb_height) = thumb_rect_for(view);
        if let Some(pointer) = response.interact_pointer_pos() {
            if response.drag_started() || response.clicked() || self.grab_point_in_thumb.is_none() {
                // Agarrado por el tirador, se mueve desde ese punto; pulsando
                // fuera, el tirador salta para quedar centrado en el ratón.
                let grabbed_the_thumb = (thumb_top..=thumb_top + thumb_height).contains(&pointer.y);
                let grab_point = if grabbed_the_thumb { pointer.y - thumb_top } else { thumb_height / 2.0 };
                self.grab_point_in_thumb = Some(grab_point);
            }
            if let Some(grab_point) = self.grab_point_in_thumb {
                let new_top = (pointer.y - grab_point - margin.top()) / track_height;
                session.scroll_history_to(view.lines_scrolled_up_for(new_top, thumb_fraction));
            }
        }
        if !response.dragged() && !response.is_pointer_button_down_on() {
            self.grab_point_in_thumb = None;
        }

        // Se dibuja ya en su sitio nuevo.
        let view_now = if let ScrollControl::TabHistory(view_now) = session.scroll_control() { view_now } else { view };
        let (_, thumb_top, thumb_height) = thumb_rect_for(view_now);
        let is_active = response.hovered() || response.dragged();
        let width = animated_width(ui, id, is_active);
        let painter = ui.painter_at(margin);
        if is_active {
            paint_track(&painter, margin, width);
        }
        let thumb = bar_rect(margin, width, thumb_top, thumb_height);
        painter.rect_filled(thumb, Rounding::same(width / 2.0), thumb_color(response.hovered(), response.dragged()));
        if is_active {
            ui.ctx().set_cursor_icon(CursorIcon::Default);
        }
    }

    fn show_program_lever(&mut self, ui: &mut Ui, id: Id, margin: Rect, rows: u16, session: &mut TerminalSession) {
        let response = ui.interact(margin, id, Sense::click_and_drag());
        let center_y = margin.center().y;
        let travel = ((margin.height() - PROGRAM_GRIP_HEIGHT) / 2.0).max(1.0);
        // Al programa se le pide desde el centro de la pantalla: es donde suele
        // estar lo que se desplaza (en Claude Code, la conversación).
        let middle_of_screen = GridPosition { row: rows / 2, column: 0 };

        if response.dragged() {
            self.program_grip_offset = (self.program_grip_offset + response.drag_delta().y).clamp(-travel, travel);
            let tilt = self.program_grip_offset / travel;
            let seconds = ui.input(|input| input.stable_dt).min(0.1);
            // Hacia arriba (offset negativo) son líneas más antiguas (positivas).
            self.unsent_program_lines -= tilt * PROGRAM_LINES_PER_SECOND_AT_FULL_TILT * seconds;
            let whole_lines = self.unsent_program_lines.trunc();
            if whole_lines != 0.0 {
                session.turn_mouse_wheel(whole_lines as i32, middle_of_screen);
                self.unsent_program_lines -= whole_lines;
            }
            // Mientras se mantiene, sigue desplazando aunque el ratón no se mueva.
            ui.ctx().request_repaint();
        } else {
            self.forget_program_grip();
            let grip_range = center_y - PROGRAM_GRIP_HEIGHT / 2.0..=center_y + PROGRAM_GRIP_HEIGHT / 2.0;
            if response.clicked() {
                if let Some(pointer) = response.interact_pointer_pos().filter(|pointer| !grip_range.contains(&pointer.y)) {
                    let half_screen = i32::from(rows / 2).max(1);
                    let lines = if pointer.y < center_y { half_screen } else { -half_screen };
                    session.turn_mouse_wheel(lines, middle_of_screen);
                }
            }
        }

        let is_active = response.hovered() || response.dragged();
        let width = animated_width(ui, id, is_active);
        let grip_offset = ui.ctx().animate_value_with_time(id.with("grip"), self.program_grip_offset, 0.12);
        let painter = ui.painter_at(margin);
        if is_active {
            paint_track(&painter, margin, width);
        }
        let grip_top = center_y + grip_offset - PROGRAM_GRIP_HEIGHT / 2.0;
        let grip = bar_rect(margin, width, grip_top, PROGRAM_GRIP_HEIGHT);
        let color = thumb_color(response.hovered(), response.dragged());
        painter.rect_filled(grip, Rounding::same(width / 2.0), color);
        if is_active {
            // Flechas encima y debajo, para que se entienda que es una palanca.
            let gap = ARROW_SIZE + 4.0;
            paint_arrow(&painter, pos2(grip.center().x, grip.top() - gap), true, color);
            paint_arrow(&painter, pos2(grip.center().x, grip.bottom() + gap), false, color);
            ui.ctx().set_cursor_icon(if response.dragged() { CursorIcon::Grabbing } else { CursorIcon::Grab });
        }
    }

    fn forget_program_grip(&mut self) {
        self.program_grip_offset = 0.0;
        self.unsent_program_lines = 0.0;
    }
}

/// Se ensancha con suavidad al pasar el ratón, como en Windows 11.
fn animated_width(ui: &Ui, id: Id, is_active: bool) -> f32 {
    let expansion = ui.ctx().animate_bool_with_time(id.with("width"), is_active, 0.12);
    egui::lerp(IDLE_WIDTH..=HOVERED_WIDTH, expansion)
}

fn bar_rect(margin: Rect, width: f32, top: f32, height: f32) -> Rect {
    let center_x = margin.right() - DISTANCE_FROM_EDGE;
    Rect::from_min_size(pos2(center_x - width / 2.0, top), vec2(width, height))
}

fn paint_track(painter: &Painter, margin: Rect, width: f32) {
    let track = bar_rect(margin, width, margin.top(), margin.height());
    painter.rect_filled(track, Rounding::same(width / 2.0), palette::ON_SURFACE.gamma_multiply(0.06));
}

/// Gris discreto en reposo, más claro al pasar el ratón y el color principal al arrastrar.
fn thumb_color(is_hovered: bool, is_dragged: bool) -> egui::Color32 {
    if is_dragged {
        palette::PRIMARY
    } else if is_hovered {
        palette::ON_SURFACE_VARIANT.gamma_multiply(0.8)
    } else {
        palette::OUTLINE.gamma_multiply(0.5)
    }
}

fn paint_arrow(painter: &Painter, tip_center: Pos2, points_up: bool, color: egui::Color32) {
    let direction = if points_up { -1.0 } else { 1.0 };
    let tip = tip_center + vec2(0.0, direction * ARROW_SIZE / 2.0);
    let base_y = tip_center.y - direction * ARROW_SIZE / 2.0;
    let corners = vec![tip, pos2(tip_center.x - ARROW_SIZE, base_y), pos2(tip_center.x + ARROW_SIZE, base_y)];
    painter.add(Shape::convex_polygon(corners, color, Stroke::NONE));
}
