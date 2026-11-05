//! Lo que tienen en común las ventanas emergentes (crear, editar, borrar,
//! confirmar, ajustes): detrás llevan una capa invisible que tapa el resto de
//! la aplicación. Un clic fuera de la ventana cae en esa capa, cierra la
//! ventana cancelando lo que se hacía, y no llega a lo que hay debajo (no
//! cambia de pestaña ni pulsa ningún botón sin querer).

use eframe::egui::{self, Area, Id, InnerResponse, Order, Sense};

/// Dibuja la capa de detrás de una ventana emergente. Devuelve si se ha hecho
/// clic en ella, es decir, fuera de la ventana. Hay que llamarla antes de
/// dibujar la ventana y, después, pasar la ventana a `keep_dialog_on_top`.
pub(super) fn backdrop_was_clicked(egui_context: &egui::Context) -> bool {
    let whole_window = egui_context.screen_rect();
    let backdrop = Area::new(Id::new("dialog_backdrop"))
        .order(Order::Middle)
        .fixed_pos(whole_window.min)
        .show(egui_context, |ui| ui.allocate_rect(whole_window, Sense::click_and_drag()));
    super::pointer_cursor::keep_arrow_over(egui_context, backdrop.inner.id);
    backdrop.inner.clicked() || backdrop.inner.secondary_clicked()
}

/// Deja la ventana por encima de su capa de detrás (las dos están en el mismo
/// nivel, y egui sube al frente lo último en lo que se pulsa).
pub(super) fn keep_dialog_on_top<R>(egui_context: &egui::Context, dialog: &Option<InnerResponse<R>>) {
    if let Some(dialog) = dialog {
        egui_context.move_to_top(dialog.response.layer_id);
    }
}
