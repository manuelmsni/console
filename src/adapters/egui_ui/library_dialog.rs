//! Ventanitas para crear, editar y eliminar elementos de la biblioteca.

use eframe::egui::{self, Align, Align2, Key, Layout, Margin, RichText, TextEdit, TextStyle, Ui};

use super::dialog_backdrop::{backdrop_was_clicked, keep_dialog_on_top};
use super::material_widgets::{filled_button, text_button};
use super::theme::palette;
use super::ConsoleApp;
use crate::i18n::{tr, tr_args};

const FIELD_WIDTH: f32 = 360.0;
use crate::domain::command_library::{LibraryNodeId, SavedCommand, TreeLocation};

pub(super) enum LibraryDialog {
    CreateFolder { location: TreeLocation, name: String },
    CreateCommand { location: TreeLocation, name: String, command_text: String },
    EditCommand { command_id: LibraryNodeId, name: String, command_text: String },
    ConfirmRemoval { node_id: LibraryNodeId, name: String },
}

enum DialogOutcome {
    StillOpen,
    Cancelled,
    Confirmed,
}

impl LibraryDialog {
    pub fn create_folder_at(location: TreeLocation) -> Self {
        Self::CreateFolder { location, name: String::new() }
    }

    pub fn create_command_at(location: TreeLocation, command_text: String) -> Self {
        Self::CreateCommand { location, name: String::new(), command_text }
    }

    pub fn edit_command(command: &SavedCommand) -> Self {
        Self::EditCommand { command_id: command.id, name: command.name.clone(), command_text: command.command_text.clone() }
    }

    pub fn confirm_removal(node_id: LibraryNodeId, name: &str) -> Self {
        Self::ConfirmRemoval { node_id, name: name.to_string() }
    }

    fn title(&self) -> String {
        match self {
            Self::CreateFolder { .. } => tr("library.dialog.new_folder"),
            Self::CreateCommand { .. } => tr("library.dialog.new_command"),
            Self::EditCommand { .. } => tr("library.dialog.edit_command"),
            Self::ConfirmRemoval { .. } => tr("library.dialog.delete"),
        }
    }

    fn confirm_button_label(&self) -> String {
        match self {
            Self::ConfirmRemoval { .. } => tr("library.dialog.delete"),
            _ => tr("common.save"),
        }
    }
}

impl ConsoleApp {
    pub(super) fn show_library_dialog(&mut self, egui_context: &egui::Context) {
        let Some(dialog) = self.library_dialog.as_mut() else { return };

        match show_dialog_window(egui_context, dialog) {
            DialogOutcome::StillOpen => {}
            DialogOutcome::Cancelled => self.library_dialog = None,
            DialogOutcome::Confirmed => {
                let Some(dialog) = self.library_dialog.take() else { return };
                if let Err(error) = self.save_dialog_changes(&dialog) {
                    // Se deja abierta para que el usuario pueda corregir el problema.
                    self.show_status(error);
                    self.library_dialog = Some(dialog);
                }
            }
        }
    }

    fn save_dialog_changes(&mut self, dialog: &LibraryDialog) -> Result<(), String> {
        let library = &mut self.command_library;
        match dialog {
            LibraryDialog::CreateFolder { location, name } => library.create_folder(*location, name),
            LibraryDialog::CreateCommand { location, name, command_text } => {
                library.create_command(*location, name, command_text)
            }
            LibraryDialog::EditCommand { command_id, name, command_text } => {
                library.update_command(*command_id, name, command_text)
            }
            LibraryDialog::ConfirmRemoval { node_id, .. } => library.remove_node(*node_id),
        }
    }
}

fn show_dialog_window(egui_context: &egui::Context, dialog: &mut LibraryDialog) -> DialogOutcome {
    let mut outcome = DialogOutcome::StillOpen;
    let clicked_outside = backdrop_was_clicked(egui_context);

    let window = egui::Window::new(dialog.title())
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(egui_context, |ui| {
            ui.set_width(FIELD_WIDTH);
            ui.label(RichText::new(dialog.title()).size(22.0).color(palette::ON_SURFACE));
            ui.add_space(16.0);

            match dialog {
                LibraryDialog::CreateFolder { name, .. } => {
                    show_name_field(ui, name);
                }
                LibraryDialog::CreateCommand { name, command_text, .. }
                | LibraryDialog::EditCommand { name, command_text, .. } => {
                    show_name_field(ui, name);
                    show_command_field(ui, command_text);
                }
                LibraryDialog::ConfirmRemoval { name, .. } => {
                    ui.label(
                        RichText::new(tr_args("library.dialog.confirm_delete", &[("name", name)]))
                            .color(palette::ON_SURFACE_VARIANT),
                    );
                }
            }

            ui.add_space(24.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if filled_button(ui, &dialog.confirm_button_label()).clicked() {
                    outcome = DialogOutcome::Confirmed;
                }
                if text_button(ui, &tr("common.cancel")).clicked() {
                    outcome = DialogOutcome::Cancelled;
                }
            });
        });

    keep_dialog_on_top(egui_context, &window);
    if clicked_outside || egui_context.input(|input| input.key_pressed(Key::Escape)) {
        outcome = DialogOutcome::Cancelled;
    }
    outcome
}

fn show_name_field(ui: &mut Ui, name: &mut String) {
    show_field_label(ui, &tr("library.dialog.name"));
    ui.add(TextEdit::singleline(name).desired_width(FIELD_WIDTH).margin(Margin::symmetric(12.0, 10.0)));
    ui.add_space(12.0);
}

fn show_command_field(ui: &mut Ui, command_text: &mut String) {
    show_field_label(ui, &tr("library.dialog.command"));
    ui.add(
        TextEdit::multiline(command_text)
            .font(TextStyle::Monospace)
            .desired_width(FIELD_WIDTH)
            .desired_rows(3)
            .margin(Margin::symmetric(12.0, 10.0)),
    );
}

fn show_field_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().color(palette::ON_SURFACE_VARIANT));
}
