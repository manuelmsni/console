//! Preguntas antes de perder algo: cerrar una pestaña guardada (se borra su
//! diario) o salir con pestañas sin guardar.

use eframe::egui::{self, Align, Align2, Key, Layout, RichText, ViewportCommand};

use super::dialog_backdrop::{backdrop_was_clicked, keep_dialog_on_top};
use super::material_widgets::{filled_button, text_button};
use super::theme::palette;
use super::ConsoleApp;
use crate::domain::tab_journal::TabId;
use crate::i18n::{tr, tr_args};

pub(super) enum Confirmation {
    CloseSavedTab { tab: TabId, title: String },
    ExitWithUnsavedTabs { count: usize },
}

enum Answer {
    /// Cerrar y borrar / Guardar y salir.
    Main,
    /// Salir sin guardar.
    Secondary,
    Cancel,
}

impl ConsoleApp {
    /// Cierra la pestaña; si estaba guardada, antes pregunta, porque se borra su diario.
    pub(super) fn close_tab_asking_if_saved(&mut self, index: usize) {
        self.close_history_picker();
        let (Some(tab), Some(title)) = (self.terminal_tabs.tab_id(index), self.terminal_tabs.title_of(index)) else { return };
        if self.terminal_tabs.is_saved_on_disk(index) {
            self.confirmation = Some(Confirmation::CloseSavedTab { tab, title: title.to_string() });
        } else {
            self.terminal_tabs.close_tab(index);
        }
    }

    /// Sale de la app; si hay pestañas con cambios sin guardar, antes pregunta.
    /// Las pestañas con autoguardado se guardan ya, sin esperar a su turno.
    pub(super) fn exit_app(&mut self) {
        self.terminal_tabs.save_tabs_with_autosave_now();
        match self.terminal_tabs.count_of_tabs_with_unsaved_changes() {
            0 => self.exit_now(),
            count => self.confirmation = Some(Confirmation::ExitWithUnsavedTabs { count }),
        }
    }

    fn exit_now(&mut self) {
        self.start_new_version_if_installed();
        self.exit_is_confirmed = true;
        self.egui_context.send_viewport_cmd(ViewportCommand::Close);
    }

    pub(super) fn show_confirmation(&mut self, egui_context: &egui::Context) {
        let Some(confirmation) = &self.confirmation else { return };
        let (title, explanation, main_button, secondary_button) = match confirmation {
            Confirmation::CloseSavedTab { title, .. } => (
                tr_args("confirm.close_saved_tab.title", &[("title", title)]),
                tr("confirm.close_saved_tab.body"),
                tr("confirm.close_saved_tab.confirm"),
                None,
            ),
            Confirmation::ExitWithUnsavedTabs { count } => (
                if *count == 1 { tr("confirm.exit.title_one") } else { tr_args("confirm.exit.title_many", &[("count", count)]) },
                tr("confirm.exit.body"),
                tr("confirm.exit.save_and_exit"),
                Some(tr("confirm.exit.exit_without_saving")),
            ),
        };

        let mut answer = None;
        let clicked_outside = backdrop_was_clicked(egui_context);
        let window = egui::Window::new("Confirmar")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(egui_context, |ui| {
                ui.set_width(420.0);
                ui.label(RichText::new(title).size(22.0));
                ui.add_space(12.0);
                ui.label(RichText::new(explanation).color(palette::ON_SURFACE_VARIANT));
                ui.add_space(24.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if filled_button(ui, &main_button).clicked() {
                        answer = Some(Answer::Main);
                    }
                    if let Some(secondary_button) = &secondary_button {
                        if text_button(ui, secondary_button).clicked() {
                            answer = Some(Answer::Secondary);
                        }
                    }
                    if text_button(ui, &tr("common.cancel")).clicked() {
                        answer = Some(Answer::Cancel);
                    }
                });
            });
        keep_dialog_on_top(egui_context, &window);
        if clicked_outside || egui_context.input(|input| input.key_pressed(Key::Escape)) {
            answer = Some(Answer::Cancel);
        }

        let Some(answer) = answer else { return };
        let Some(confirmation) = self.confirmation.take() else { return };
        match (confirmation, answer) {
            // Si se salía para reiniciar con la versión nueva, ya no: queda el
            // botón de reiniciar en la barra para cuando se quiera.
            (_, Answer::Cancel) => self.updates.restart_after_exit = false,
            (Confirmation::CloseSavedTab { tab, .. }, _) => {
                if let Some(index) = self.terminal_tabs.index_of(tab) {
                    self.terminal_tabs.close_tab(index);
                }
            }
            (Confirmation::ExitWithUnsavedTabs { .. }, Answer::Main) => match self.terminal_tabs.save_all_tabs_with_changes() {
                Ok(()) => self.exit_now(),
                Err(error) => self.show_status(error),
            },
            (Confirmation::ExitWithUnsavedTabs { .. }, Answer::Secondary) => self.exit_now(),
        }
    }
}
