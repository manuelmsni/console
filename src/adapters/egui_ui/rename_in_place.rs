//! Renombrar en su sitio: doble clic en el nombre de un comando, una carpeta
//! o una pestaña lo convierte en un campo de texto. Enter (o clic fuera)
//! guarda y Esc cancela.

use std::time::{Duration, Instant};

use eframe::egui;

use super::material_widgets::{RenameField, RenameOutcome};
use super::ConsoleApp;
use crate::domain::command_library::LibraryNodeId;
use crate::domain::tab_journal::TabId;

/// Lo que tarda en considerarse que un clic no va a ser un doble clic (el
/// mismo margen que usa la interfaz para reconocer el doble clic).
const DOUBLE_CLICK_WAIT: Duration = Duration::from_millis(320);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RenameTarget {
    LibraryNode(LibraryNodeId),
    Tab(TabId),
}

pub(super) struct RenameInPlace {
    target: RenameTarget,
    field: RenameField,
}

/// El campo de texto si `target` es lo que se está renombrando.
pub(super) fn rename_field_for(renaming: &mut Option<RenameInPlace>, target: RenameTarget) -> Option<&mut RenameField> {
    renaming.as_mut().filter(|renaming| renaming.target == target).map(|renaming| &mut renaming.field)
}

/// Un clic simple en un comando lo escribe en la consola, pero no enseguida:
/// si llega un segundo clic es un doble clic (renombrar) y no se escribe nada.
pub(super) struct PendingCommandClick {
    clicked_at: Instant,
    command_text: String,
}

impl ConsoleApp {
    pub(super) fn start_renaming(&mut self, target: RenameTarget, current_name: &str) {
        self.pending_command_click = None;
        self.renaming = Some(RenameInPlace { target, field: RenameField::starting_with(current_name) });
    }

    pub(super) fn finish_renaming(&mut self, outcome: RenameOutcome) {
        if outcome == RenameOutcome::StillEditing {
            return;
        }
        let Some(renaming) = self.renaming.take() else { return };
        if outcome == RenameOutcome::Cancelled {
            return;
        }
        let result = match renaming.target {
            RenameTarget::LibraryNode(node) => self.command_library.rename_node(node, &renaming.field.text),
            RenameTarget::Tab(tab) => self.terminal_tabs.rename_tab(tab, &renaming.field.text),
        };
        if let Err(error) = result {
            self.show_status(error);
        }
    }

    pub(super) fn type_command_after_double_click_wait(&mut self, command_text: String) {
        self.pending_command_click = Some(PendingCommandClick { clicked_at: Instant::now(), command_text });
    }

    /// Escribe el comando pendiente si ya no puede llegar un doble clic; si no,
    /// pide que se vuelva a dibujar justo cuando toque.
    pub(super) fn type_pending_command_when_due(&mut self, egui_context: &egui::Context) {
        let Some(pending) = &self.pending_command_click else { return };
        let waited = pending.clicked_at.elapsed();
        if waited < DOUBLE_CLICK_WAIT {
            egui_context.request_repaint_after(DOUBLE_CLICK_WAIT - waited);
            return;
        }
        if let Some(pending) = self.pending_command_click.take() {
            self.type_into_active_terminal(&pending.command_text);
        }
    }
}
