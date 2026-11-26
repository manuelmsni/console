//! La ventanita que pregunta con qué terminal y en qué carpeta abrir una
//! pestaña, cuando en los ajustes se ha pedido "preguntar": al abrir la app,
//! con "Abrir Console aquí" o con el atajo de nueva pestaña.

use std::path::Path;

use eframe::egui::{self, Align, Align2, Key, Layout, Margin, RichText, TextEdit, Ui};

use super::dialog_backdrop::{backdrop_was_clicked, keep_dialog_on_top};
use super::folder_picker::pick_folder;
use super::material_widgets::{filled_button, text_button};
use super::theme::palette;
use super::ConsoleApp;
use crate::domain::shell_profile::ShellProfile;
use crate::domain::startup::{NewTabPlan, PlannedFolder};
use crate::i18n::{tr, tr_args};

const DIALOG_WIDTH: f32 = 420.0;
/// Lo que se deja a la derecha del campo de la carpeta para el botón "Examinar".
const BROWSE_BUTTON_ROOM: f32 = 120.0;

pub(super) struct NewTabDialog {
    asks_shell: bool,
    /// La terminal marcada (o la ya decidida), entre las disponibles.
    profile_index: usize,
    asks_folder: bool,
    /// Lo escrito, si se pregunta la carpeta; si no, la ya decidida
    /// (vacía: la del usuario).
    folder: String,
    /// Al abrir la app: la carpeta elegida vale también para las pestañas
    /// nuevas de esta vez.
    remembers_folder: bool,
}

enum DialogOutcome {
    StillOpen,
    Cancelled,
    Confirmed,
}

impl NewTabDialog {
    /// `marked_profile`: la que sale marcada si hay que preguntar la terminal.
    pub fn from_plan(plan: NewTabPlan, profiles: &[ShellProfile], marked_profile: Option<&ShellProfile>, remembers_folder: bool) -> Self {
        let index_of = |wanted: Option<&ShellProfile>| {
            wanted.and_then(|wanted| profiles.iter().position(|profile| profile == wanted)).unwrap_or(0)
        };
        let profile_index = match &plan.shell {
            Some(profile) => index_of(Some(profile)),
            None => index_of(marked_profile),
        };
        let home = || dirs::home_dir().map(|home| home.display().to_string()).unwrap_or_default();
        let (asks_folder, folder) = match plan.folder {
            PlannedFolder::Home => (false, String::new()),
            PlannedFolder::Known(folder) => (false, folder),
            PlannedFolder::Ask => (true, home()),
        };
        Self { asks_shell: plan.shell.is_none(), profile_index, asks_folder, folder, remembers_folder }
    }

    fn folder_to_open(&self) -> Option<&str> {
        let folder = self.folder.trim();
        (!folder.is_empty()).then_some(folder)
    }
}

impl ConsoleApp {
    pub(super) fn show_new_tab_dialog(&mut self, egui_context: &egui::Context) {
        let Some(dialog) = self.new_tab_dialog.as_mut() else { return };
        match show_dialog_window(egui_context, dialog, &self.shell_profiles) {
            DialogOutcome::StillOpen => {}
            DialogOutcome::Cancelled => self.new_tab_dialog = None,
            DialogOutcome::Confirmed => {
                let Some(dialog) = self.new_tab_dialog.take() else { return };
                if dialog.asks_folder && !dialog.folder_to_open().is_some_and(|folder| Path::new(folder).is_dir()) {
                    // Se deja abierta para que se pueda corregir.
                    self.show_status(tr_args("new_tab.error.no_such_folder", &[("folder", &dialog.folder.trim())]));
                    self.new_tab_dialog = Some(dialog);
                    return;
                }
                let Some(profile) = self.shell_profiles.get(dialog.profile_index).cloned() else { return };
                if dialog.remembers_folder && dialog.asks_folder {
                    self.folder_chosen_at_start = dialog.folder_to_open().map(str::to_string);
                }
                self.open_tab_in(&profile, dialog.folder_to_open());
            }
        }
    }
}

fn show_dialog_window(egui_context: &egui::Context, dialog: &mut NewTabDialog, profiles: &[ShellProfile]) -> DialogOutcome {
    let mut outcome = DialogOutcome::StillOpen;
    let clicked_outside = backdrop_was_clicked(egui_context);

    let window = egui::Window::new("new_tab_dialog")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(egui_context, |ui| {
            ui.set_width(DIALOG_WIDTH);
            ui.label(RichText::new(tr("new_tab.title")).size(22.0).color(palette::ON_SURFACE));
            ui.add_space(16.0);

            if dialog.asks_shell {
                show_field_label(ui, &tr("new_tab.terminal"));
                ui.horizontal_wrapped(|ui| {
                    for (index, profile) in profiles.iter().enumerate() {
                        ui.selectable_value(&mut dialog.profile_index, index, &profile.display_name);
                    }
                });
                ui.add_space(12.0);
            }

            if dialog.asks_folder {
                show_field_label(ui, &tr("new_tab.folder"));
                ui.horizontal(|ui| {
                    let field_width = DIALOG_WIDTH - BROWSE_BUTTON_ROOM;
                    ui.add(TextEdit::singleline(&mut dialog.folder).desired_width(field_width).margin(Margin::symmetric(12.0, 10.0)));
                    if text_button(ui, &tr("new_tab.browse")).clicked() {
                        if let Some(folder) = pick_folder(&dialog.folder) {
                            dialog.folder = folder;
                        }
                    }
                });
            } else if let Some(folder) = dialog.folder_to_open() {
                ui.label(RichText::new(tr_args("new_tab.in_folder", &[("folder", &folder)])).color(palette::ON_SURFACE_VARIANT));
            }

            ui.add_space(24.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if filled_button(ui, &tr("new_tab.open")).clicked() {
                    outcome = DialogOutcome::Confirmed;
                }
                if text_button(ui, &tr("common.cancel")).clicked() {
                    outcome = DialogOutcome::Cancelled;
                }
            });
        });

    keep_dialog_on_top(egui_context, &window);
    if egui_context.input(|input| input.key_pressed(Key::Enter)) {
        outcome = DialogOutcome::Confirmed;
    }
    if clicked_outside || egui_context.input(|input| input.key_pressed(Key::Escape)) {
        outcome = DialogOutcome::Cancelled;
    }
    outcome
}

fn show_field_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().color(palette::ON_SURFACE_VARIANT));
}
