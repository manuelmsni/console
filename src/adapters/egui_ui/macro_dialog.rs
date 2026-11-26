//! Ventana para crear, editar y eliminar macros y carpetas de la barra de
//! macros: nombre, icono, comando y en qué carpeta va.

use eframe::egui::{
    self, vec2, Align, Align2, Button, ComboBox, Key, Layout, Margin, RichText, Rounding, ScrollArea, TextEdit, TextStyle, Ui,
};

use egui_material_icons::icons::ICON_BLOCK;

use super::dialog_backdrop::{backdrop_was_clicked, keep_dialog_on_top};
use super::macro_icons::{icon_glyph, MACRO_ICONS};
use super::material_widgets::{filled_button, text_button};
use super::theme::palette;
use super::ConsoleApp;
use crate::domain::macro_bar::{FolderFields, MacroBar, MacroBarItem, MacroFields, MacroId, MacroLocation};
use crate::i18n::{tr, tr_args};

const FIELD_WIDTH: f32 = 420.0;
const ICON_CELL_SIZE: f32 = 34.0;
const ICON_PICKER_HEIGHT: f32 = 118.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditedKind {
    Macro,
    Folder,
}

pub(super) struct MacroDialog {
    kind: EditedKind,
    /// `None` al crear uno nuevo.
    editing: Option<MacroId>,
    location: MacroLocation,
    name: String,
    icon: Option<String>,
    command: String,
    /// Se ha pulsado "Eliminar": se pide confirmación antes de borrar.
    confirming_removal: bool,
    /// Se abrió solo para eliminar (desde el menú): cancelar cierra la ventana
    /// en vez de volver a los campos.
    opened_for_removal: bool,
}

enum DialogOutcome {
    StillOpen,
    Cancelled,
    Save,
    Remove,
    TryCommand,
}

impl MacroDialog {
    pub fn new_macro(location: MacroLocation) -> Self {
        Self::new(EditedKind::Macro, location)
    }

    pub fn new_folder(location: MacroLocation) -> Self {
        Self::new(EditedKind::Folder, location)
    }

    fn new(kind: EditedKind, location: MacroLocation) -> Self {
        Self {
            kind,
            editing: None,
            location,
            name: String::new(),
            icon: None,
            command: String::new(),
            confirming_removal: false,
            opened_for_removal: false,
        }
    }

    /// `None` si el elemento ya no existe.
    pub fn edit(bar: &MacroBar, id: MacroId) -> Option<Self> {
        let location = bar.location_of(id)?;
        let dialog = match bar.find(id)? {
            MacroBarItem::Macro(item) => Self {
                kind: EditedKind::Macro,
                editing: Some(id),
                location,
                name: item.name.clone(),
                icon: item.icon.clone(),
                command: item.command.clone(),
                confirming_removal: false,
                opened_for_removal: false,
            },
            MacroBarItem::Folder(folder) => Self {
                kind: EditedKind::Folder,
                editing: Some(id),
                location,
                name: folder.name.clone(),
                icon: folder.icon.clone(),
                command: String::new(),
                confirming_removal: false,
                opened_for_removal: false,
            },
        };
        Some(dialog)
    }

    /// Para eliminar directamente desde el menú: se abre ya preguntando.
    pub fn confirm_removal(bar: &MacroBar, id: MacroId) -> Option<Self> {
        Self::edit(bar, id).map(|dialog| Self { confirming_removal: true, opened_for_removal: true, ..dialog })
    }

    fn title(&self) -> String {
        match (self.kind, self.editing.is_some(), self.confirming_removal) {
            (_, _, true) => tr("macros.dialog.delete"),
            (EditedKind::Macro, false, _) => tr("macros.dialog.new_macro"),
            (EditedKind::Macro, true, _) => tr("macros.dialog.edit_macro"),
            (EditedKind::Folder, false, _) => tr("macros.dialog.new_folder"),
            (EditedKind::Folder, true, _) => tr("macros.dialog.edit_folder"),
        }
    }

    fn shown_name(&self) -> String {
        if self.name.trim().is_empty() {
            self.command.trim().to_string()
        } else {
            self.name.trim().to_string()
        }
    }
}

impl ConsoleApp {
    pub(super) fn show_macro_dialog(&mut self, egui_context: &egui::Context) {
        let Some(dialog) = self.macro_dialog.as_mut() else { return };
        match show_dialog_window(egui_context, dialog, self.macro_bar.bar()) {
            DialogOutcome::StillOpen => {}
            DialogOutcome::Cancelled => self.macro_dialog = None,
            DialogOutcome::TryCommand => {
                let result = self.macro_bar.try_command(&dialog.command);
                self.show_status(result.unwrap_or_else(|error| error));
            }
            DialogOutcome::Save | DialogOutcome::Remove => {
                let Some(dialog) = self.macro_dialog.take() else { return };
                if let Err(error) = self.save_macro_dialog_changes(&dialog) {
                    // Se deja abierta para que se pueda corregir.
                    self.show_status(error);
                    self.macro_dialog = Some(MacroDialog { confirming_removal: false, ..dialog });
                }
            }
        }
    }

    fn save_macro_dialog_changes(&mut self, dialog: &MacroDialog) -> Result<(), String> {
        let bar = &mut self.macro_bar;
        if dialog.confirming_removal {
            return dialog.editing.map_or(Ok(()), |id| bar.remove(id));
        }
        let (name, icon) = (dialog.name.clone(), dialog.icon.clone());
        match (dialog.kind, dialog.editing) {
            (EditedKind::Macro, None) => bar.create_macro(dialog.location, MacroFields { name, icon, command: dialog.command.clone() }),
            (EditedKind::Macro, Some(id)) => {
                bar.update_macro(id, MacroFields { name, icon, command: dialog.command.clone() }, dialog.location)
            }
            (EditedKind::Folder, None) => bar.create_folder(dialog.location, FolderFields { name, icon }),
            (EditedKind::Folder, Some(id)) => bar.update_folder(id, FolderFields { name, icon }, dialog.location),
        }
    }
}

fn show_dialog_window(egui_context: &egui::Context, dialog: &mut MacroDialog, bar: &MacroBar) -> DialogOutcome {
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
            if dialog.confirming_removal {
                show_removal_question(ui, dialog, &mut outcome);
            } else {
                show_fields(ui, dialog, bar);
                ui.add_space(24.0);
                show_buttons(ui, dialog, &mut outcome);
            }
        });

    keep_dialog_on_top(egui_context, &window);
    if clicked_outside || egui_context.input(|input| input.key_pressed(Key::Escape)) {
        outcome = DialogOutcome::Cancelled;
    }
    outcome
}

fn show_removal_question(ui: &mut Ui, dialog: &mut MacroDialog, outcome: &mut DialogOutcome) {
    let question = match dialog.kind {
        EditedKind::Macro => tr_args("macros.dialog.confirm_delete_macro", &[("name", &dialog.shown_name())]),
        EditedKind::Folder => tr_args("macros.dialog.confirm_delete_folder", &[("name", &dialog.shown_name())]),
    };
    ui.label(RichText::new(question).color(palette::ON_SURFACE_VARIANT));
    ui.add_space(24.0);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if filled_button(ui, &tr("macros.dialog.delete")).clicked() {
            *outcome = DialogOutcome::Remove;
        }
        if text_button(ui, &tr("common.cancel")).clicked() {
            if dialog.opened_for_removal {
                *outcome = DialogOutcome::Cancelled;
            } else {
                dialog.confirming_removal = false;
            }
        }
    });
}

fn show_fields(ui: &mut Ui, dialog: &mut MacroDialog, bar: &MacroBar) {
    show_field_label(ui, &tr("macros.dialog.name"));
    ui.add(TextEdit::singleline(&mut dialog.name).desired_width(FIELD_WIDTH).margin(Margin::symmetric(12.0, 10.0)));
    ui.add_space(12.0);

    show_field_label(ui, &tr("macros.dialog.icon"));
    show_icon_picker(ui, &mut dialog.icon);
    ui.add_space(12.0);

    if dialog.kind == EditedKind::Macro {
        show_field_label(ui, &tr("macros.dialog.command"));
        ui.add(
            TextEdit::multiline(&mut dialog.command)
                .font(TextStyle::Monospace)
                .desired_width(FIELD_WIDTH)
                .desired_rows(3)
                .hint_text(RichText::new("explorer \"C:\\Proyectos\"").color(palette::OUTLINE))
                .margin(Margin::symmetric(12.0, 10.0)),
        );
        ui.label(RichText::new(tr("macros.dialog.command_explanation")).small().color(palette::ON_SURFACE_VARIANT));
        ui.add_space(12.0);
    }

    show_field_label(ui, &tr("macros.dialog.location"));
    let destinations = bar.destinations_for(dialog.editing);
    let selected_label = destinations
        .iter()
        .find(|destination| destination.location == dialog.location)
        .map_or_else(|| tr("macros.location.bar"), |destination| destination.label.clone());
    ComboBox::from_id_salt("macro_location").selected_text(selected_label).width(FIELD_WIDTH).show_ui(ui, |ui| {
        for destination in destinations {
            ui.selectable_value(&mut dialog.location, destination.location, destination.label);
        }
    });
}

/// Una cuadrícula de iconos; el primero es "sin icono".
fn show_icon_picker(ui: &mut Ui, chosen: &mut Option<String>) {
    ScrollArea::vertical().id_salt("macro_icon_picker").max_height(ICON_PICKER_HEIGHT).show(ui, |ui| {
        ui.set_width(FIELD_WIDTH);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            if icon_cell(ui, ICON_BLOCK, chosen.is_none() || icon_glyph(chosen.as_deref()).is_none())
                .on_hover_text(tr("macros.dialog.no_icon"))
                .clicked()
            {
                *chosen = None;
            }
            for (name, glyph) in MACRO_ICONS {
                if icon_cell(ui, glyph, chosen.as_deref() == Some(*name)).clicked() {
                    *chosen = Some(name.to_string());
                }
            }
        });
    });
}

fn icon_cell(ui: &mut Ui, glyph: &str, is_chosen: bool) -> egui::Response {
    let (fill, color) = if is_chosen {
        (palette::PRIMARY, palette::ON_PRIMARY)
    } else {
        (palette::SURFACE_CONTAINER_HIGHEST, palette::ON_SURFACE_VARIANT)
    };
    ui.add(
        Button::new(RichText::new(glyph).size(18.0).color(color))
            .fill(fill)
            .rounding(Rounding::same(8.0))
            .min_size(vec2(ICON_CELL_SIZE, ICON_CELL_SIZE)),
    )
}

/// A la izquierda, eliminar y probar; a la derecha, guardar y cancelar.
fn show_buttons(ui: &mut Ui, dialog: &mut MacroDialog, outcome: &mut DialogOutcome) {
    ui.horizontal(|ui| {
        if dialog.editing.is_some()
            && ui.add(Button::new(RichText::new(tr("macros.dialog.delete")).color(palette::ERROR)).frame(false)).clicked()
        {
            dialog.confirming_removal = true;
        }
        if dialog.kind == EditedKind::Macro
            && ui
                .add(Button::new(RichText::new(tr("macros.dialog.try")).color(palette::PRIMARY)).frame(false))
                .on_hover_text(tr("macros.dialog.try_tooltip"))
                .clicked()
        {
            *outcome = DialogOutcome::TryCommand;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if filled_button(ui, &tr("common.save")).clicked() {
                *outcome = DialogOutcome::Save;
            }
            if text_button(ui, &tr("common.cancel")).clicked() {
                *outcome = DialogOutcome::Cancelled;
            }
        });
    });
}

fn show_field_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().color(palette::ON_SURFACE_VARIANT));
}
