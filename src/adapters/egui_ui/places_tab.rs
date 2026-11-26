//! Pestaña "Rutas" de los ajustes: para cada terminal, las carpetas y los
//! servidores SSH (con una carpeta dentro, si se quiere) que salen al pasar el
//! ratón por ella en el menú ➕. Cada una puede llevar un alias.

use eframe::egui::{vec2, Align, Area, Frame, Layout, Margin, Order, Rect, Response, RichText, TextEdit, TextStyle, Ui};
use egui_material_icons::icons::{
    ICON_ADD, ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_DELETE, ICON_DNS, ICON_EDIT, ICON_FOLDER, ICON_PLAY_ARROW,
};

use super::folder_picker::pick_folder;
use super::material_widgets::{filled_button, icon_button, menu_row, text_button};
use super::settings_window::{card, section_header};
use super::theme::palette;
use super::ConsoleApp;
use crate::domain::saved_places::{PlaceFields, PlaceId, PlaceTarget, SavedPlace, SavedPlaces};
use crate::domain::shell_profile::ShellProfile;
use crate::domain::ssh::{destination_suggestions, SshDestination};
use crate::i18n::tr;

/// Que quepa entero "Puerto (opcional)" (y sus traducciones).
const PORT_FIELD_WIDTH: f32 = 170.0;

/// La ruta que se está creando o cambiando.
struct PlaceEditor {
    profile_name: String,
    /// `None`: una nueva.
    editing: Option<PlaceId>,
    fields: PlaceFields,
    /// Dónde se dibujaron las sugerencias del servidor: mientras el ratón está
    /// encima siguen ahí, aunque el campo pierda el foco al pulsar una.
    suggestions_area: Option<Rect>,
}

#[derive(Default)]
pub(super) struct PlacesTabState {
    editor: Option<PlaceEditor>,
}

pub(super) enum PlacesAction {
    Save { profile_name: String, editing: Option<PlaceId>, fields: PlaceFields },
    Remove(PlaceId),
    Move(PlaceId, isize),
}

/// Lo que la pestaña necesita leer.
pub(super) struct PlacesOverview<'a> {
    pub shell_profiles: &'a [ShellProfile],
    pub places: &'a SavedPlaces,
    /// Los de la pestaña "Claves SSH" y los de `~/.ssh/config`, para
    /// sugerirlos al escribir el servidor.
    pub ssh_servers: &'a [SshDestination],
}

impl PlacesTabState {
    /// Tras guardar bien, se cierra el formulario.
    pub fn close_editor(&mut self) {
        self.editor = None;
    }

    pub fn show(&mut self, ui: &mut Ui, overview: &PlacesOverview) -> Vec<PlacesAction> {
        let mut actions = Vec::new();
        ui.label(RichText::new(tr("places.intro")).color(palette::ON_SURFACE_VARIANT));
        for profile in overview.shell_profiles {
            ui.add_space(20.0);
            self.show_profile_section(ui, profile, overview, &mut actions);
        }
        actions
    }

    fn show_profile_section(&mut self, ui: &mut Ui, profile: &ShellProfile, overview: &PlacesOverview, actions: &mut Vec<PlacesAction>) {
        let name = &profile.display_name;
        let mut wants_to_add = false;
        section_header(ui, name, |ui| {
            if icon_button(ui, ICON_ADD, &tr("places.add")).clicked() {
                wants_to_add = true;
            }
        });
        if wants_to_add {
            self.editor = Some(PlaceEditor { profile_name: name.clone(), editing: None, fields: PlaceFields::default(), suggestions_area: None });
        }
        let places = overview.places.places_of(name);
        card(ui, |ui| {
            let adding_here = self.editor.as_ref().is_some_and(|editor| &editor.profile_name == name && editor.editing.is_none());
            if adding_here {
                self.show_editor(ui, overview.ssh_servers, actions);
            } else if places.is_empty() {
                ui.label(RichText::new(tr("places.none")).color(palette::OUTLINE));
            }
            for (index, place) in places.iter().enumerate() {
                let editing_this = self.editor.as_ref().is_some_and(|editor| editor.editing == Some(place.id));
                if editing_this {
                    self.show_editor(ui, overview.ssh_servers, actions);
                } else if let Some(action) = show_place_row(ui, place, index, places.len()) {
                    match action {
                        RowAction::Edit => {
                            let fields = PlaceFields::of(place);
                            self.editor = Some(PlaceEditor { profile_name: name.clone(), editing: Some(place.id), fields, suggestions_area: None });
                        }
                        RowAction::Other(action) => actions.push(action),
                    }
                }
            }
        });
    }

    fn show_editor(&mut self, ui: &mut Ui, ssh_servers: &[SshDestination], actions: &mut Vec<PlacesAction>) {
        let Some(editor) = &mut self.editor else { return };
        let mut finished = None;
        let fields = &mut editor.fields;
        let field_width = ui.available_width();

        ui.horizontal(|ui| {
            ui.selectable_value(&mut fields.is_ssh, false, format!("{ICON_FOLDER}  {}", tr("places.kind_folder")));
            ui.selectable_value(&mut fields.is_ssh, true, format!("{ICON_DNS}  {}", tr("places.kind_ssh")));
        });
        ui.add_space(8.0);
        ui.add(
            TextEdit::singleline(&mut fields.alias)
                .hint_text(tr("places.alias_hint"))
                .desired_width(field_width)
                .margin(Margin::symmetric(12.0, 10.0)),
        );
        ui.add_space(8.0);
        if fields.is_ssh {
            let destination_field = ui.horizontal(|ui| {
                let destination_width = ui.available_width() - PORT_FIELD_WIDTH - 16.0;
                let destination_field = ui.add(
                    TextEdit::singleline(&mut fields.destination)
                        .hint_text(tr("places.destination_hint"))
                        .desired_width(destination_width)
                        .margin(Margin::symmetric(12.0, 10.0)),
                );
                ui.add(
                    TextEdit::singleline(&mut fields.port)
                        .hint_text(tr("places.port_hint"))
                        .desired_width(PORT_FIELD_WIDTH)
                        .margin(Margin::symmetric(12.0, 10.0)),
                );
                destination_field
            });
            let suggestions = destination_suggestions(ssh_servers, &fields.destination);
            let picked = show_destination_suggestions(ui, &destination_field.inner, &suggestions, &mut editor.suggestions_area);
            if let Some(picked) = picked {
                fields.destination = picked;
            }
            ui.add_space(8.0);
            ui.add(
                TextEdit::singleline(&mut fields.remote_folder)
                    .hint_text(tr("places.remote_folder_hint"))
                    .desired_width(field_width)
                    .margin(Margin::symmetric(12.0, 10.0)),
            );
        } else {
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut fields.folder)
                        .hint_text(tr("places.folder_hint"))
                        .desired_width(ui.available_width() - 120.0)
                        .margin(Margin::symmetric(12.0, 10.0)),
                );
                if text_button(ui, &tr("new_tab.browse")).clicked() {
                    if let Some(folder) = pick_folder(&fields.folder) {
                        fields.folder = folder;
                    }
                }
            });
        }
        ui.add_space(8.0);
        let command_hint = if fields.is_ssh { tr("places.remote_command_hint") } else { tr("places.command_hint") };
        ui.add(
            TextEdit::singleline(&mut fields.command)
                .hint_text(command_hint)
                .font(TextStyle::Monospace)
                .desired_width(field_width)
                .margin(Margin::symmetric(12.0, 10.0)),
        );
        ui.add_space(8.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if filled_button(ui, &tr("common.save")).clicked() {
                finished = Some(true);
            }
            if text_button(ui, &tr("common.cancel")).clicked() {
                finished = Some(false);
            }
        });
        ui.add_space(4.0);

        match finished {
            // El formulario se cierra cuando se haya guardado bien.
            Some(true) => actions.push(PlacesAction::Save {
                profile_name: editor.profile_name.clone(),
                editing: editor.editing,
                fields: editor.fields.clone(),
            }),
            Some(false) => self.editor = None,
            None => {}
        }
    }
}

/// Debajo del campo del servidor, mientras se escribe en él: los conocidos
/// que encajan con lo escrito. Devuelve el que se pulse.
fn show_destination_suggestions(
    ui: &Ui,
    field: &Response,
    suggestions: &[&SshDestination],
    shown_area: &mut Option<Rect>,
) -> Option<String> {
    let pointer_is_over_them = shown_area
        .zip(ui.ctx().pointer_hover_pos())
        .is_some_and(|(area, pointer)| area.contains(pointer));
    if suggestions.is_empty() || !(field.has_focus() || pointer_is_over_them) {
        *shown_area = None;
        return None;
    }
    let mut picked = None;
    let area = Area::new(field.id.with("suggestions"))
        // Por encima de la ventana de ajustes.
        .order(Order::Foreground)
        .fixed_pos(field.rect.left_bottom() + vec2(0.0, 4.0))
        .show(ui.ctx(), |ui| {
            Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_width(field.rect.width() - 16.0);
                for destination in suggestions {
                    if menu_row(ui, ICON_DNS, destination.as_str()).clicked() {
                        picked = Some(destination.as_str().to_string());
                    }
                }
            });
        });
    *shown_area = if picked.is_some() { None } else { Some(area.response.rect) };
    picked
}

enum RowAction {
    Edit,
    Other(PlacesAction),
}

fn show_place_row(ui: &mut Ui, place: &SavedPlace, index: usize, count: usize) -> Option<RowAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        let icon = match place.target {
            PlaceTarget::LocalFolder(_) => ICON_FOLDER,
            PlaceTarget::Ssh { .. } => ICON_DNS,
        };
        ui.label(RichText::new(icon).size(20.0).color(palette::ON_SURFACE_VARIANT));
        ui.vertical(|ui| {
            ui.label(RichText::new(place.label()).color(palette::ON_SURFACE));
            if place.alias.is_some() {
                ui.label(RichText::new(place.target.to_string()).small().color(palette::ON_SURFACE_VARIANT));
            }
            if let Some(command) = &place.command {
                ui.label(RichText::new(format!("{ICON_PLAY_ARROW} {command}")).small().monospace().color(palette::ON_SURFACE_VARIANT));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, ICON_DELETE, &tr("places.remove")).clicked() {
                action = Some(RowAction::Other(PlacesAction::Remove(place.id)));
            }
            if index + 1 < count && icon_button(ui, ICON_ARROW_DOWNWARD, &tr("places.move_down")).clicked() {
                action = Some(RowAction::Other(PlacesAction::Move(place.id, 1)));
            }
            if index > 0 && icon_button(ui, ICON_ARROW_UPWARD, &tr("places.move_up")).clicked() {
                action = Some(RowAction::Other(PlacesAction::Move(place.id, -1)));
            }
            if icon_button(ui, ICON_EDIT, &tr("places.edit")).clicked() {
                action = Some(RowAction::Edit);
            }
        });
    });
    action
}

impl ConsoleApp {
    pub(super) fn apply_places_actions(&mut self, actions: Vec<PlacesAction>) {
        for action in actions {
            let result = match action {
                PlacesAction::Save { profile_name, editing, fields } => {
                    let saved = match editing {
                        Some(id) => self.saved_places.update(id, &fields),
                        None => self.saved_places.add(&profile_name, &fields),
                    };
                    if saved.is_ok() {
                        if let Some(settings) = &mut self.settings_window {
                            settings.places_mut().close_editor();
                        }
                    }
                    saved
                }
                PlacesAction::Remove(id) => self.saved_places.remove(id),
                PlacesAction::Move(id, steps) => self.saved_places.move_by(id, steps),
            };
            if let Err(error) = result {
                self.show_status(error);
            }
        }
    }
}
