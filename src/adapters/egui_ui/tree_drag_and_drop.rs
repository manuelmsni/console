//! Arrastrar y soltar en el árbol de comandos guardados.
//!
//! Al soltar sobre una fila, la posición vertical del puntero decide qué pasa:
//!
//! ```text
//!  ┌──────────────────────┐ ← cuarto de arriba: delante de esta fila
//!  │  📁 Carpeta          │ ← centro (solo carpetas): dentro de la carpeta
//!  └──────────────────────┘ ← cuarto de abajo: detrás de esta fila
//! ```
//!
//! En los comandos no hay "dentro": la mitad de arriba es delante y la de abajo, detrás.

use eframe::egui::{vec2, Align2, Context, CursorIcon, DragAndDrop, FontId, Rect, Response, Sense, Ui};
use egui_material_icons::icons::{ICON_FOLDER, ICON_TERMINAL};

use super::material_widgets::{paint_drag_ghost, paint_drop_into, paint_drop_line};
use super::theme::palette;
use crate::domain::command_library::{LibraryNodeId, SiblingSide, TreeLocation};
use crate::i18n::tr;

/// Lo que viaja con el puntero mientras se arrastra.
#[derive(Clone)]
pub(super) struct DraggedNode {
    pub id: LibraryNodeId,
    pub name: String,
    pub is_folder: bool,
}

/// Dónde ha pedido el usuario dejar lo que arrastraba.
pub(super) enum DropRequest {
    Beside { node_id: LibraryNodeId, sibling: LibraryNodeId, side: SiblingSide },
    Into { node_id: LibraryNodeId, destination: TreeLocation },
}

/// Fila sobre la que está el puntero al soltar.
pub(super) struct DropTargetRow<'a> {
    pub response: &'a Response,
    pub node_id: LibraryNodeId,
    pub is_folder: bool,
    pub depth: usize,
}

#[derive(Clone, Copy)]
enum DropZone {
    Before,
    Inside,
    After,
}

pub(super) fn make_row_draggable(row: &Response, dragged: DraggedNode) {
    if row.drag_started() {
        row.dnd_set_drag_payload(dragged);
    }
}

/// Si se está arrastrando algo sobre la fila, dibuja dónde caerá y, al
/// soltar, devuelve la petición de mover.
pub(super) fn accept_drop_on_row(ui: &Ui, target: DropTargetRow) -> Option<DropRequest> {
    let dragged = target.response.dnd_hover_payload::<DraggedNode>()?;
    if dragged.id == target.node_id {
        return None;
    }
    let pointer = ui.ctx().pointer_latest_pos()?;
    let row_rect = target.response.rect;
    let zone = drop_zone_at(row_rect, pointer.y, target.is_folder);

    match zone {
        DropZone::Before => paint_drop_line(ui, row_rect, target.depth, true),
        DropZone::After => paint_drop_line(ui, row_rect, target.depth, false),
        DropZone::Inside => paint_drop_into(ui, row_rect),
    }

    target.response.dnd_release_payload::<DraggedNode>()?;
    let request = match zone {
        DropZone::Before => DropRequest::Beside { node_id: dragged.id, sibling: target.node_id, side: SiblingSide::Before },
        DropZone::After => DropRequest::Beside { node_id: dragged.id, sibling: target.node_id, side: SiblingSide::After },
        DropZone::Inside => DropRequest::Into { node_id: dragged.id, destination: TreeLocation::InsideFolder(target.node_id) },
    };
    Some(request)
}

/// Hueco al final del árbol para sacar algo de todas las carpetas.
pub(super) fn root_drop_area(ui: &mut Ui) -> Option<DropRequest> {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::hover());
    let dragged = response.dnd_hover_payload::<DraggedNode>();
    let something_is_being_dragged = DragAndDrop::has_payload_of_type::<DraggedNode>(ui.ctx());

    if something_is_being_dragged {
        let highlighted = dragged.is_some();
        paint_root_drop_hint(ui, rect, highlighted);
    }

    let dragged = response.dnd_release_payload::<DraggedNode>()?;
    Some(DropRequest::Into { node_id: dragged.id, destination: TreeLocation::Root })
}

/// Mientras se arrastra, una etiqueta sigue al puntero y este cambia a "mano cerrada".
pub(super) fn show_dragged_node_under_pointer(egui_context: &Context) {
    let Some(dragged) = DragAndDrop::payload::<DraggedNode>(egui_context) else { return };
    egui_context.set_cursor_icon(CursorIcon::Grabbing);
    let icon = if dragged.is_folder { ICON_FOLDER } else { ICON_TERMINAL };
    paint_drag_ghost(egui_context, icon, &dragged.name);
}

fn drop_zone_at(row_rect: Rect, pointer_y: f32, row_is_folder: bool) -> DropZone {
    let fraction_from_top = (pointer_y - row_rect.top()) / row_rect.height();
    if row_is_folder {
        match fraction_from_top {
            f if f < 0.25 => DropZone::Before,
            f if f > 0.75 => DropZone::After,
            _ => DropZone::Inside,
        }
    } else if fraction_from_top < 0.5 {
        DropZone::Before
    } else {
        DropZone::After
    }
}

fn paint_root_drop_hint(ui: &Ui, rect: Rect, highlighted: bool) {
    let color = if highlighted { palette::PRIMARY } else { palette::OUTLINE };
    let hint_rect = rect.shrink(4.0);
    if highlighted {
        ui.painter().rect_filled(hint_rect, 17.0, palette::PRIMARY.gamma_multiply(0.12));
    }
    ui.painter().text(
        hint_rect.center(),
        Align2::CENTER_CENTER,
        tr("sidebar.drop_to_top_level"),
        FontId::proportional(12.0),
        color,
    );
}
