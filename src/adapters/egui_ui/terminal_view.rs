//! La terminal en sí: la dibuja, le pasa el teclado y permite seleccionar
//! texto con el ratón para copiarlo. Con Ctrl + clic se abren en el navegador
//! los enlaces web que aparecen en ella.
//!
//! La selección la hace siempre esta aplicación, aunque el programa que corre
//! dentro (por ejemplo Claude Code por SSH) pida el ratón para él. Por eso
//! aquí sí se puede copiar.

use eframe::egui::{
    self, Align2, CursorIcon, Event, EventFilter, FontId, Margin, Painter, PointerButton, Pos2, Rect, Response,
    Rounding, Sense, Stroke, Ui, Vec2,
};
use egui_material_icons::icons::{ICON_CONTENT_COPY, ICON_CONTENT_PASTE};

use super::material_widgets::menu_row;
use super::theme::palette;
use super::{colors, key_mapping, shortcuts, ConsoleApp};
use crate::domain::app_settings::{Shortcut, ShortcutAction};
use crate::i18n::{tr, tr_args};
use crate::application::ports::Clipboard;
use crate::application::terminal_session::TerminalSession;
use crate::domain::screen_snapshot::{ScreenSnapshot, TerminalColor};
use crate::domain::terminal_grid::{GridPosition, TerminalSize, TextSelection};
use crate::domain::terminal_keys::{KeyModifiers, TerminalKey};
use crate::domain::web_links::{web_link_at, WebLink};

const FONT_SIZE: f32 = 14.0;
/// Espacio entre el borde redondeado de la tarjeta y el texto.
const CARD_PADDING: f32 = 12.0;
const LINK_UNDERLINE_WIDTH: f32 = 1.5;

impl ConsoleApp {
    pub(super) fn show_active_terminal(&mut self, egui_context: &egui::Context) {
        let surrounding = egui::Frame::none()
            .fill(palette::SURFACE)
            .inner_margin(Margin { left: 8.0, right: 8.0, top: 0.0, bottom: 8.0 });

        egui::CentralPanel::default().frame(surrounding).show(egui_context, |ui| {
            if self.terminal_tabs.active_session_mut().is_some() {
                self.show_terminal_widget(ui);
            } else {
                self.show_no_tabs_message(ui);
            }
        });
    }

    fn show_terminal_widget(&mut self, ui: &mut Ui) {
        let font_id = FontId::monospace(FONT_SIZE);
        let (card, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        // Aquí se escribe y se selecciona texto: cursor de texto (con Ctrl sobre
        // un enlace pasa a ser la mano, más abajo).
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Text);
        }
        ui.painter().rect_filled(card, Rounding::same(16.0), colors::DEFAULT_BACKGROUND);
        self.terminal_card = Some(card);
        let area = card.shrink(CARD_PADDING);
        let grid = CellGrid::fitting_in(area, measure_cell_size(ui, &font_id));

        if std::mem::take(&mut self.terminal_should_take_focus) || response.clicked() || response.drag_started() {
            response.request_focus();
        }
        if response.has_focus() {
            keep_special_keys_inside_terminal(ui, &response);
        }
        // Con el panel de historial o una ventana abierta, el teclado es para ellos.
        let keyboard_goes_to_terminal = response.has_focus() && self.history_picker.is_none() && !self.a_dialog_is_open();
        let history_shortcut = self.app_settings.shortcut(ShortcutAction::OpenCommandHistory);

        let Some(session) = self.terminal_tabs.active_session_mut() else { return };
        let clipboard = self.clipboard.as_mut();
        let mut status = None;

        session.resize_to(grid.size);

        // Seleccionar no copia: se copia con Ctrl+C o con `Copiar` del clic derecho.
        update_selection_with_mouse(ui, &response, &grid, session);
        scroll_history_with_mouse_wheel(ui, &response, session, &mut self.unapplied_scroll_lines, grid.cell_size.y);

        let mut wants_history_picker = false;
        if keyboard_goes_to_terminal {
            let keyboard = forward_keyboard_to_terminal(ui, session, clipboard, &history_shortcut);
            status = keyboard.status.or(status);
            wants_history_picker = keyboard.wants_history_picker;
        }

        response.context_menu(|ui| {
            if let Some(message) = show_terminal_context_menu(ui, session, clipboard) {
                status = Some(message);
            }
        });

        let screen = session.screen_snapshot();
        let link_under_pointer = web_link_under_pointer_with_ctrl(ui, &response, &grid, &screen);
        paint_screen(&ui.painter_at(area), &grid, &screen, session.selection(), &font_id);
        let mut link_to_open = None;
        if let Some(link) = link_under_pointer {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            paint_link_underline(&ui.painter_at(area), &grid, &link);
            if response.clicked() {
                link_to_open = Some(link.url);
            }
        }
        if let Some(url) = link_to_open {
            status = Some(match self.web_browser.open(&url) {
                Ok(()) => tr_args("links.opening", &[("url", &url)]),
                Err(error) => error,
            });
        }

        if let Some(message) = status {
            self.show_status(message);
        }
        if wants_history_picker {
            self.open_history_picker();
        }
    }

    fn show_no_tabs_message(&mut self, ui: &mut Ui) {
        let mut profile_to_open = None;
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.label(tr("tabs.none_open"));
                for profile in &self.shell_profiles {
                    if ui.button(tr_args("tabs.open_profile", &[("name", &profile.display_name)])).clicked() {
                        profile_to_open = Some(profile.clone());
                    }
                }
            });
        });
        if let Some(profile) = profile_to_open {
            self.open_tab_with(&profile);
        }
    }
}

/// Correspondencia entre píxeles de la ventana y celdas de la terminal.
struct CellGrid {
    origin: Pos2,
    cell_size: Vec2,
    size: TerminalSize,
}

impl CellGrid {
    fn fitting_in(area: Rect, cell_size: Vec2) -> Self {
        let rows = (area.height() / cell_size.y).floor().max(1.0) as u16;
        let columns = (area.width() / cell_size.x).floor().max(1.0) as u16;
        Self { origin: area.min, cell_size, size: TerminalSize { rows, columns } }
    }

    fn rect_of(&self, position: GridPosition) -> Rect {
        let top_left = self.origin
            + Vec2::new(position.column as f32 * self.cell_size.x, position.row as f32 * self.cell_size.y);
        Rect::from_min_size(top_left, self.cell_size)
    }

    fn contains(&self, point: Pos2) -> bool {
        let whole_size = Vec2::new(self.size.columns as f32 * self.cell_size.x, self.size.rows as f32 * self.cell_size.y);
        Rect::from_min_size(self.origin, whole_size).contains(point)
    }

    /// La celda bajo un punto; si el punto está fuera, la celda más cercana.
    fn position_at(&self, point: Pos2) -> GridPosition {
        let offset = point - self.origin;
        let column = (offset.x / self.cell_size.x).floor().clamp(0.0, (self.size.columns - 1) as f32);
        let row = (offset.y / self.cell_size.y).floor().clamp(0.0, (self.size.rows - 1) as f32);
        GridPosition { row: row as u16, column: column as u16 }
    }
}

/// El enlace bajo el ratón, solo mientras se mantiene Ctrl pulsado (como en
/// otros editores y terminales: sin Ctrl, un clic sigue siendo un clic normal).
fn web_link_under_pointer_with_ctrl(ui: &Ui, response: &Response, grid: &CellGrid, screen: &ScreenSnapshot) -> Option<WebLink> {
    if !ui.input(|input| input.modifiers.ctrl) {
        return None;
    }
    let pointer = response.hover_pos().filter(|point| grid.contains(*point))?;
    web_link_at(screen, grid.position_at(pointer))
}

fn paint_link_underline(painter: &Painter, grid: &CellGrid, link: &WebLink) {
    for (row, first_column, last_column) in link.row_spans(grid.size.columns) {
        let left = grid.rect_of(GridPosition { row, column: first_column }).left_bottom();
        let right = grid.rect_of(GridPosition { row, column: last_column }).right_bottom();
        painter.line_segment([left, right], Stroke::new(LINK_UNDERLINE_WIDTH, palette::PRIMARY));
    }
}

fn measure_cell_size(ui: &Ui, font_id: &FontId) -> Vec2 {
    ui.fonts(|fonts| Vec2::new(fonts.glyph_width(font_id, 'M'), fonts.row_height(font_id)))
}

fn update_selection_with_mouse(
    ui: &Ui,
    response: &Response,
    grid: &CellGrid,
    session: &mut TerminalSession,
) {
    if response.drag_started_by(PointerButton::Primary) {
        // Se empieza donde se pulsó, no donde está el ratón ahora (ya se ha movido un poco).
        if let Some(press_point) = ui.input(|input| input.pointer.press_origin()) {
            session.start_selection_at(grid.position_at(press_point));
        }
    } else if response.dragged_by(PointerButton::Primary) {
        if let Some(pointer) = response.interact_pointer_pos() {
            session.extend_selection_to(grid.position_at(pointer));
        }
    } else if response.clicked() {
        session.clear_selection();
    }
}

fn scroll_history_with_mouse_wheel(
    ui: &Ui,
    response: &Response,
    session: &mut TerminalSession,
    unapplied_scroll_lines: &mut f32,
    line_height: f32,
) {
    if !response.hovered() {
        return;
    }
    let wheel_movement = ui.input(|input| input.raw_scroll_delta.y);
    if wheel_movement == 0.0 {
        return;
    }
    *unapplied_scroll_lines += wheel_movement / line_height;
    let whole_lines = unapplied_scroll_lines.trunc();
    if whole_lines != 0.0 {
        session.scroll_history_by(whole_lines as i32);
        *unapplied_scroll_lines -= whole_lines;
    }
}

/// Sin esto, Tab, Escape y las flechas moverían el foco a otros botones
/// en lugar de llegar al programa de la terminal.
fn keep_special_keys_inside_terminal(ui: &Ui, response: &Response) {
    let all_keys_go_to_terminal = EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
    ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, all_keys_go_to_terminal));
}

#[derive(Default)]
struct KeyboardOutcome {
    /// Mensaje para la barra de estado, por ejemplo al copiar.
    status: Option<String>,
    /// Se pulsó Ctrl+↑ en el prompt y hay historial: hay que abrir el panel.
    wants_history_picker: bool,
}

fn forward_keyboard_to_terminal(
    ui: &Ui,
    session: &mut TerminalSession,
    clipboard: &mut dyn Clipboard,
    history_shortcut: &Shortcut,
) -> KeyboardOutcome {
    let mut outcome = KeyboardOutcome::default();
    for event in ui.input(|input| input.events.clone()) {
        match event {
            Event::Key { key, pressed: true, modifiers, .. }
                if shortcuts::is_pressing(key, modifiers, history_shortcut)
                    && session.is_at_local_prompt()
                    && !session.command_history().is_empty() =>
            {
                outcome.wants_history_picker = true;
            }
            Event::Text(text) => session.send_typed_text(&text),
            // Ctrl+V y Shift+Insert.
            Event::Paste(text) => session.paste_text(&text),
            // Ctrl+C copia si hay algo seleccionado; si no, interrumpe como siempre.
            Event::Copy => match session.selected_text() {
                Some(selected_text) => {
                    outcome.status = Some(copy_text(clipboard, &selected_text));
                    session.clear_selection();
                }
                None => session.send_key(TerminalKey::ControlLetter('C'), KeyModifiers::NONE),
            },
            Event::Cut => session.send_key(TerminalKey::ControlLetter('X'), KeyModifiers::NONE),
            Event::Key { key, pressed: true, modifiers, .. } => {
                if let Some(terminal_key) = key_mapping::terminal_key_for(key, modifiers) {
                    session.send_key(terminal_key, key_mapping::terminal_modifiers_of(modifiers));
                }
            }
            _ => {}
        }
    }
    outcome
}

fn show_terminal_context_menu(ui: &mut Ui, session: &mut TerminalSession, clipboard: &mut dyn Clipboard) -> Option<String> {
    let mut status = None;
    let selected_text = session.selected_text();

    if let Some(text) = &selected_text {
        if menu_row(ui, ICON_CONTENT_COPY, &tr("common.copy")).clicked() {
            status = Some(copy_text(clipboard, text));
            ui.close_menu();
        }
    }
    if menu_row(ui, ICON_CONTENT_PASTE, &tr("terminal.paste")).clicked() {
        match clipboard.read_text() {
            Ok(text) => session.paste_text(&text),
            Err(error) => status = Some(error),
        }
        ui.close_menu();
    }
    status
}

fn copy_text(clipboard: &mut dyn Clipboard, text: &str) -> String {
    match clipboard.copy_text(text) {
        Ok(()) => tr_args("clipboard.copied_characters", &[("count", &text.chars().count())]),
        Err(error) => error,
    }
}

fn paint_screen(
    painter: &Painter,
    grid: &CellGrid,
    snapshot: &ScreenSnapshot,
    selection: Option<&TextSelection>,
    font_id: &FontId,
) {
    let cells_with_positions = || {
        snapshot.rows.iter().enumerate().flat_map(|(row, cells)| {
            cells.iter().enumerate().map(move |(column, cell)| {
                (GridPosition { row: row as u16, column: column as u16 }, cell)
            })
        })
    };

    // Primero todos los fondos y después todo el texto, para que ningún
    // fondo tape una letra ancha que sobresale de su celda.
    for (position, cell) in cells_with_positions() {
        let cell_rect = grid.rect_of(position);
        if cell.background != TerminalColor::DefaultBackground {
            painter.rect_filled(cell_rect, 0.0, colors::screen_color_of(cell.background, false));
        }
        if selection.is_some_and(|selection| selection.contains(position)) {
            painter.rect_filled(cell_rect, 0.0, colors::SELECTION_HIGHLIGHT);
        }
    }

    if let Some(cursor) = snapshot.visible_cursor {
        painter.rect_filled(grid.rect_of(cursor), 0.0, colors::CURSOR);
    }

    for (position, cell) in cells_with_positions() {
        if !cell.text.trim().is_empty() {
            let text_color = colors::screen_color_of(cell.foreground, cell.is_bold);
            painter.text(grid.rect_of(position).min, Align2::LEFT_TOP, &cell.text, font_id.clone(), text_color);
        }
    }
}
