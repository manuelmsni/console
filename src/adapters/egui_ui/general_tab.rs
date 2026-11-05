//! Pestaña "General" de los ajustes: el idioma, el guardado de las pestañas y
//! las actualizaciones.

use eframe::egui::{Align, Checkbox, ComboBox, DragValue, Layout, RichText, Ui};

use super::material_widgets::text_button;
use super::settings_window::{card, section_header};
use super::theme::palette;
use super::ConsoleApp;
use crate::application::terminal_tabs::AutosavePolicy;
use crate::domain::app_settings::{AppSettings, AutosaveSettings, ShortcutAction, UpdateSettings};
use crate::domain::updates::Version;
use crate::i18n::{self, tr, tr_args};

pub(super) enum GeneralAction {
    /// `None`: el idioma de Windows.
    SetLanguage(Option<String>),
    SetAutosave(AutosaveSettings),
    SetUpdateSettings(UpdateSettings),
    CheckForUpdatesNow,
}

/// Lo que la sección de actualizaciones necesita saber del estado actual.
pub(super) struct UpdatesOverview {
    /// `None` si este programa no se puede actualizar solo.
    pub current_version: Option<Version>,
    pub is_checking: bool,
}

pub(super) fn show_general_tab(ui: &mut Ui, settings: &AppSettings, updates: &UpdatesOverview) -> Vec<GeneralAction> {
    let mut actions = Vec::new();
    show_language_section(ui, settings, &mut actions);
    ui.add_space(20.0);
    show_saving_section(ui, settings, &mut actions);
    ui.add_space(20.0);
    show_updates_section(ui, settings, updates, &mut actions);
    actions
}

fn show_updates_section(ui: &mut Ui, settings: &AppSettings, updates: &UpdatesOverview, actions: &mut Vec<GeneralAction>) {
    let Some(current_version) = updates.current_version else { return };
    let mut update_settings = settings.updates;
    section_header(ui, &tr("settings.general.updates"), |_| {});
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(tr_args("settings.general.current_version", &[("version", &current_version)]));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if updates.is_checking {
                    ui.spinner();
                } else if text_button(ui, &tr("settings.general.check_now")).clicked() {
                    actions.push(GeneralAction::CheckForUpdatesNow);
                }
            });
        });
        ui.add(Checkbox::new(&mut update_settings.check_automatically, tr("settings.general.check_automatically")));
        ui.label(RichText::new(tr("settings.general.updates_explanation")).small().color(palette::ON_SURFACE_VARIANT));
    });
    if update_settings != settings.updates {
        actions.push(GeneralAction::SetUpdateSettings(update_settings));
    }
}

fn show_language_section(ui: &mut Ui, settings: &AppSettings, actions: &mut Vec<GeneralAction>) {
    section_header(ui, &tr("settings.general.language"), |_| {});
    card(ui, |ui| {
        let languages = i18n::available_languages();
        let name_of = |code: &Option<String>| match code {
            None => tr("settings.general.language_automatic"),
            Some(code) => languages.iter().find(|(known, _)| known == code).map_or(code.clone(), |(_, name)| name.clone()),
        };
        let mut chosen = settings.language.clone();
        ComboBox::from_id_salt("language").selected_text(name_of(&chosen)).width(260.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut chosen, None, tr("settings.general.language_automatic"));
            for (code, name) in &languages {
                ui.selectable_value(&mut chosen, Some(code.clone()), name);
            }
        });
        if chosen != settings.language {
            actions.push(GeneralAction::SetLanguage(chosen));
        }
    });
}

fn show_saving_section(ui: &mut Ui, settings: &AppSettings, actions: &mut Vec<GeneralAction>) {
    let mut autosave = settings.autosave;
    section_header(ui, &tr("settings.general.saving"), |_| {});
    card(ui, |ui| {
        ui.add(Checkbox::new(&mut autosave.enabled, tr("settings.general.autosave")));
        ui.add_enabled_ui(autosave.enabled, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(28.0);
                ui.label(tr("settings.general.every"));
                ui.add(
                    DragValue::new(&mut autosave.interval_seconds)
                        .range(AutosaveSettings::MIN_INTERVAL_SECONDS..=AutosaveSettings::MAX_INTERVAL_SECONDS)
                        .suffix(" s"),
                );
            });
        });
        ui.add_space(8.0);
        let save = settings.shortcuts.get(ShortcutAction::SaveTab);
        let toggle = settings.shortcuts.get(ShortcutAction::ToggleTabAutosave);
        ui.label(
            RichText::new(tr_args("settings.general.saving_explanation", &[("save", &save), ("toggle", &toggle)]))
                .small()
                .color(palette::ON_SURFACE_VARIANT),
        );
    });
    if autosave != settings.autosave {
        actions.push(GeneralAction::SetAutosave(autosave));
    }
}

impl ConsoleApp {
    pub(super) fn apply_general_actions(&mut self, actions: Vec<GeneralAction>) {
        for action in actions {
            match action {
                GeneralAction::SetLanguage(language) => match self.app_settings.set_language(language) {
                    // Se aplica al momento: el siguiente fotograma ya sale en el idioma nuevo.
                    Ok(()) => i18n::set_language(self.app_settings.settings().language.as_deref(), &self.system_language),
                    Err(error) => self.show_status(error),
                },
                GeneralAction::SetAutosave(autosave) => match self.app_settings.set_autosave(autosave) {
                    Ok(()) => {
                        let policy = AutosavePolicy::from_settings(&self.app_settings.settings().autosave);
                        self.terminal_tabs.set_autosave_policy(policy);
                    }
                    Err(error) => self.show_status(error),
                },
                GeneralAction::SetUpdateSettings(updates) => {
                    if let Err(error) = self.app_settings.set_update_settings(updates) {
                        self.show_status(error);
                    }
                }
                GeneralAction::CheckForUpdatesNow => self.check_for_updates_now(),
            }
        }
    }
}
