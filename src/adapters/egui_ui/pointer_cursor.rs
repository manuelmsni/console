//! La mano sobre todo lo que se puede pulsar. egui solo la pone en sus botones
//! estándar, así que aquí se mira, al final de cada fotograma, qué hay bajo el
//! ratón: si se puede pulsar y nadie ha pedido otro cursor (el de texto de un
//! campo o de la consola, el de estirar...), se pone la mano.

use eframe::egui::{self, CursorIcon, Id};

/// Dónde se apuntan las zonas que se pueden pulsar pero son fondo (la barra
/// para arrastrar la ventana, la capa de detrás de una ventana emergente).
const BACKGROUND_AREAS: &str = "clickable_background_areas";

/// Esta zona se puede pulsar, pero es fondo: sigue con la flecha normal.
pub(super) fn keep_arrow_over(egui_context: &egui::Context, area: Id) {
    egui_context.data_mut(|data| data.get_temp_mut_or_default::<Vec<Id>>(Id::new(BACKGROUND_AREAS)).push(area));
}

pub(super) fn show_hand_over_clickable_things(egui_context: &egui::Context) {
    let background_areas =
        egui_context.data_mut(|data| data.remove_temp::<Vec<Id>>(Id::new(BACKGROUND_AREAS))).unwrap_or_default();
    if egui_context.output(|output| output.cursor_icon) != CursorIcon::Default {
        return;
    }
    let hovered = egui_context.interaction_snapshot(|interaction| interaction.hovered.clone());
    let over_something_clickable = hovered.into_iter().filter(|id| !background_areas.contains(id)).any(|id| {
        egui_context.read_response(id).is_some_and(|widget| widget.sense.click && widget.enabled())
    });
    if over_something_clickable {
        egui_context.set_cursor_icon(CursorIcon::PointingHand);
    }
}
