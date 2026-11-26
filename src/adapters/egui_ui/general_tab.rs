//! Pestaña "General" de los ajustes: el idioma, las pestañas nuevas (terminal,
//! carpeta y "Abrir Console aquí"), el guardado de las pestañas y las
//! actualizaciones.

use std::path::Path;

use eframe::egui::{Align, Checkbox, ComboBox, DragValue, Layout, RichText, Ui};

use super::folder_picker::pick_folder;
use super::material_widgets::text_button;
use super::settings_window::{card, section_header};
use super::theme::palette;
use super::ConsoleApp;
use crate::application::terminal_tabs::AutosavePolicy;
use crate::domain::app_settings::{AppSettings, AutosaveSettings, ShortcutAction, UpdateSettings};
use crate::domain::shell_profile::ShellProfile;
use crate::domain::startup::{FolderChoice, ShellChoice, StartSettings};
use crate::domain::updates::Version;
use crate::i18n::{self, tr, tr_args};

pub(super) enum GeneralAction {
    /// `None`: el idioma de Windows.
    SetLanguage(Option<String>),
    SetAutosave(AutosaveSettings),
    SetUpdateSettings(UpdateSettings),
    CheckForUpdatesNow,
    SetStartSettings(StartSettings),
    SetFolderContextMenu(bool),
}

/// Lo que la sección de pestañas nuevas necesita saber.
pub(super) struct NewTabsOverview<'a> {
    pub shell_profiles: &'a [ShellProfile],
    /// `None` si el menú del Explorador no se puede poner desde aquí.
    pub folder_context_menu_is_enabled: Option<bool>,
}

/// Lo que la sección de actualizaciones necesita saber del estado actual.
pub(super) struct UpdatesOverview {
    /// `None` si este programa no se puede actualizar solo.
    pub current_version: Option<Version>,
    pub is_checking: bool,
}

pub(super) fn show_general_tab(
    ui: &mut Ui,
    settings: &AppSettings,
    new_tabs: &NewTabsOverview,
    updates: &UpdatesOverview,
) -> Vec<GeneralAction> {
    let mut actions = Vec::new();
    show_language_section(ui, settings, &mut actions);
    ui.add_space(20.0);
    show_new_tabs_section(ui, settings, new_tabs, &mut actions);
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

/// Con qué terminal y en qué carpeta empiezan las pestañas nuevas, y el menú
/// del Explorador.
fn show_new_tabs_section(ui: &mut Ui, settings: &AppSettings, new_tabs: &NewTabsOverview, actions: &mut Vec<GeneralAction>) {
    let mut start = settings.start.clone();
    section_header(ui, &tr("settings.general.new_tabs"), |_| {});
    card(ui, |ui| {
        show_shell_choice(ui, &mut start.shell, new_tabs.shell_profiles);
        ui.add_space(8.0);
        show_folder_choice(ui, &mut start.folder);
        ui.add_space(8.0);
        ui.label(RichText::new(tr("settings.general.new_tabs_explanation")).small().color(palette::ON_SURFACE_VARIANT));

        if let Some(is_enabled) = new_tabs.folder_context_menu_is_enabled {
            ui.add_space(12.0);
            let mut wants_menu = is_enabled;
            ui.add(Checkbox::new(&mut wants_menu, tr("settings.general.folder_context_menu")));
            ui.label(RichText::new(tr("settings.general.folder_context_menu_explanation")).small().color(palette::ON_SURFACE_VARIANT));
            if wants_menu != is_enabled {
                actions.push(GeneralAction::SetFolderContextMenu(wants_menu));
            }
        }
    });
    if start != settings.start {
        actions.push(GeneralAction::SetStartSettings(start));
    }
}

/// Ancho de las etiquetas de la izquierda ("Terminal", "Carpeta"), para que
/// las listas queden alineadas.
const CHOICE_LABEL_WIDTH: f32 = 90.0;
const CHOICE_WIDTH: f32 = 260.0;

fn show_choice_label(ui: &mut Ui, text: &str) {
    ui.add_sized([CHOICE_LABEL_WIDTH, ui.spacing().interact_size.y], eframe::egui::Label::new(text));
}

fn show_shell_choice(ui: &mut Ui, shell: &mut ShellChoice, profiles: &[ShellProfile]) {
    // `None`: preguntar. Lo de serie (la primera) se ve como la primera.
    let mut chosen: Option<usize> = shell
        .resolve(profiles)
        .and_then(|resolved| profiles.iter().position(|profile| profile == resolved));
    let name_of = |chosen: Option<usize>| match chosen.and_then(|index| profiles.get(index)) {
        Some(profile) => profile.display_name.clone(),
        None => tr("settings.general.ask_each_time"),
    };
    let before = chosen;
    ui.horizontal(|ui| {
        show_choice_label(ui, &tr("settings.general.terminal"));
        ComboBox::from_id_salt("start_shell").selected_text(name_of(chosen)).width(CHOICE_WIDTH).show_ui(ui, |ui| {
            for (index, profile) in profiles.iter().enumerate() {
                ui.selectable_value(&mut chosen, Some(index), &profile.display_name);
            }
            ui.selectable_value(&mut chosen, None, tr("settings.general.ask_each_time"));
        });
    });
    if chosen != before {
        *shell = match chosen.and_then(|index| profiles.get(index)) {
            Some(profile) => ShellChoice::Profile(profile.display_name.clone()),
            None => ShellChoice::AskEachTime,
        };
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FolderKind {
    Home,
    AskAtStart,
    Fixed,
}

fn show_folder_choice(ui: &mut Ui, folder: &mut FolderChoice) {
    let current = match folder {
        FolderChoice::Home => FolderKind::Home,
        FolderChoice::AskAtStart => FolderKind::AskAtStart,
        FolderChoice::Fixed(_) => FolderKind::Fixed,
    };
    let name_of = |kind: FolderKind| match kind {
        FolderKind::Home => tr("settings.general.folder_home"),
        FolderKind::AskAtStart => tr("settings.general.folder_ask_at_start"),
        FolderKind::Fixed => tr("settings.general.folder_fixed"),
    };
    let mut chosen = current;
    let mut wants_to_pick = false;
    ui.horizontal(|ui| {
        show_choice_label(ui, &tr("settings.general.folder"));
        ComboBox::from_id_salt("start_folder").selected_text(name_of(chosen)).width(CHOICE_WIDTH).show_ui(ui, |ui| {
            for kind in [FolderKind::Home, FolderKind::AskAtStart, FolderKind::Fixed] {
                ui.selectable_value(&mut chosen, kind, name_of(kind));
            }
        });
    });
    if let FolderChoice::Fixed(fixed) = folder {
        ui.horizontal(|ui| {
            ui.add_space(CHOICE_LABEL_WIDTH + ui.spacing().item_spacing.x);
            ui.label(RichText::new(fixed.as_str()).monospace());
            if text_button(ui, &tr("settings.general.change_folder")).clicked() {
                wants_to_pick = true;
            }
        });
        if !Path::new(fixed.as_str()).is_dir() {
            ui.horizontal(|ui| {
                ui.add_space(CHOICE_LABEL_WIDTH + ui.spacing().item_spacing.x);
                ui.label(RichText::new(tr("settings.general.folder_missing")).small().color(palette::ERROR));
            });
        }
    }

    match chosen {
        _ if chosen == current && !wants_to_pick => {}
        FolderKind::Home => *folder = FolderChoice::Home,
        FolderKind::AskAtStart => *folder = FolderChoice::AskAtStart,
        // Para fijar una carpeta hay que elegirla; si se cancela, todo sigue igual.
        FolderKind::Fixed => {
            let start = match folder {
                FolderChoice::Fixed(fixed) => fixed.clone(),
                _ => String::new(),
            };
            if let Some(picked) = pick_folder(&start) {
                *folder = FolderChoice::Fixed(picked);
            }
        }
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
                    Ok(()) => {
                        i18n::set_language(self.app_settings.settings().language.as_deref(), &self.system_language);
                        // El texto del menú del Explorador, también en el idioma nuevo.
                        if self.folder_context_menu_is_enabled {
                            self.set_folder_context_menu(true);
                        }
                    }
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
                GeneralAction::SetStartSettings(start) => {
                    if let Err(error) = self.app_settings.set_start_settings(start) {
                        self.show_status(error);
                    }
                }
                GeneralAction::SetFolderContextMenu(enabled) => self.set_folder_context_menu(enabled),
            }
        }
    }

    fn set_folder_context_menu(&mut self, enabled: bool) {
        let Some(menu) = &self.folder_context_menu else { return };
        let result = if enabled { menu.enable(&folder_context_menu_label()) } else { menu.disable() };
        self.folder_context_menu_is_enabled = menu.is_enabled();
        if let Err(error) = result {
            self.show_status(error);
        }
    }
}

/// El texto del menú del Explorador. La portable se distingue de la instalada.
fn folder_context_menu_label() -> String {
    if cfg!(feature = "portable") {
        tr("explorer_menu.open_here_portable")
    } else {
        tr("explorer_menu.open_here")
    }
}
