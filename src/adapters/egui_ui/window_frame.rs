//! La ventana no usa la barra de título de Windows: la barra de pestañas hace
//! de barra de título (arrastrar para mover, doble clic para maximizar) y los
//! bordes se pueden estirar para cambiar el tamaño.

use eframe::egui::{self, CursorIcon, Id, PointerButton, Pos2, Rect, Response, Sense, Ui, ViewportCommand};
use eframe::egui::viewport::ResizeDirection;

/// Grosor de la franja del borde desde la que se puede estirar la ventana.
const RESIZE_BORDER: f32 = 6.0;

pub fn is_maximized(egui_context: &egui::Context) -> bool {
    egui_context.input(|input| input.viewport().maximized.unwrap_or(false))
}

pub fn is_minimized(egui_context: &egui::Context) -> bool {
    egui_context.input(|input| input.viewport().minimized.unwrap_or(false))
}

pub fn minimize(egui_context: &egui::Context) {
    egui_context.send_viewport_cmd(ViewportCommand::Minimized(true));
}

pub fn toggle_maximized(egui_context: &egui::Context) {
    egui_context.send_viewport_cmd(ViewportCommand::Maximized(!is_maximized(egui_context)));
}

/// Hace que el fondo de la barra (lo que no son botones ni pestañas) mueva la
/// ventana. Hay que llamarlo antes de dibujar el contenido de la barra: lo que
/// se dibuje después queda encima y recibe sus propios clics.
pub fn title_bar_background(ui: &mut Ui) -> Response {
    let background = ui.interact(ui.max_rect(), Id::new("title_bar_background"), Sense::click_and_drag());
    // Se puede pulsar (para arrastrar la ventana), pero no es un botón: flecha normal.
    super::pointer_cursor::keep_arrow_over(ui.ctx(), background.id);
    background
}

pub fn move_or_maximize_from_title_bar(egui_context: &egui::Context, background: &Response) {
    if background.double_clicked() {
        toggle_maximized(egui_context);
    } else if background.drag_started_by(PointerButton::Primary) {
        egui_context.send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

/// Cambia el cursor al acercarse a un borde y, al pulsar, deja que Windows
/// estire la ventana desde ahí.
pub fn allow_resizing_from_edges(egui_context: &egui::Context) {
    if is_maximized(egui_context) {
        return;
    }
    let Some(pointer) = egui_context.input(|input| input.pointer.hover_pos()) else { return };
    let Some(direction) = resize_direction_at(pointer, egui_context.screen_rect()) else { return };

    egui_context.set_cursor_icon(cursor_for(direction));
    if egui_context.input(|input| input.pointer.primary_pressed()) {
        egui_context.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}

fn resize_direction_at(pointer: Pos2, window: Rect) -> Option<ResizeDirection> {
    let near_left = pointer.x - window.left() < RESIZE_BORDER;
    let near_right = window.right() - pointer.x < RESIZE_BORDER;
    let near_top = pointer.y - window.top() < RESIZE_BORDER;
    let near_bottom = window.bottom() - pointer.y < RESIZE_BORDER;

    match (near_top, near_bottom, near_left, near_right) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (true, _, _, true) => Some(ResizeDirection::NorthEast),
        (_, true, true, _) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::North),
        (_, true, _, _) => Some(ResizeDirection::South),
        (_, _, true, _) => Some(ResizeDirection::West),
        (_, _, _, true) => Some(ResizeDirection::East),
        _ => None,
    }
}

fn cursor_for(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}
