//! Ventana de ajustes, organizada en pestañas. Para añadir una sección nueva:
//! un valor más en `SettingsTab` y su módulo con lo que se dibuja dentro.

use eframe::egui::{self, Align, Align2, Frame, Key, Layout, Margin, RichText, Rounding, ScrollArea, Ui};
use egui_material_icons::icons::{ICON_CLOSE, ICON_KEYBOARD, ICON_SETTINGS, ICON_VPN_KEY};

use super::dialog_backdrop::{backdrop_was_clicked, keep_dialog_on_top};
use super::general_tab::{show_general_tab, GeneralAction, UpdatesOverview};
use super::material_widgets::{icon_button, section_tab};
use super::shortcuts_tab::{ShortcutsAction, ShortcutsTabState};
use super::ssh_keys_tab::{SshKeysAction, SshKeysTabState};
use super::theme::palette;
use super::ConsoleApp;
use crate::i18n::tr;

const WINDOW_WIDTH: f32 = 640.0;
const CONTENT_HEIGHT: f32 = 460.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    Shortcuts,
    SshKeys,
}

impl SettingsTab {
    const ALL: [SettingsTab; 3] = [SettingsTab::General, SettingsTab::Shortcuts, SettingsTab::SshKeys];

    fn icon(self) -> &'static str {
        match self {
            SettingsTab::General => ICON_SETTINGS,
            SettingsTab::Shortcuts => ICON_KEYBOARD,
            SettingsTab::SshKeys => ICON_VPN_KEY,
        }
    }

    fn title(self) -> String {
        match self {
            SettingsTab::General => tr("settings.tab.general"),
            SettingsTab::Shortcuts => tr("settings.tab.shortcuts"),
            SettingsTab::SshKeys => tr("settings.tab.ssh_keys"),
        }
    }
}

/// Lo que se ha pedido en la pestaña visible; se aplica después de dibujarla.
enum TabActions {
    General(Vec<GeneralAction>),
    Shortcuts(Vec<ShortcutsAction>),
    SshKeys(Vec<SshKeysAction>),
}

pub(super) struct SettingsWindow {
    active_tab: SettingsTab,
    shortcuts: ShortcutsTabState,
    ssh_keys: SshKeysTabState,
}

impl SettingsWindow {
    pub fn ssh_keys_mut(&mut self) -> &mut SshKeysTabState {
        &mut self.ssh_keys
    }

    pub fn shortcuts_mut(&mut self) -> &mut ShortcutsTabState {
        &mut self.shortcuts
    }
}

impl ConsoleApp {
    pub(super) fn open_settings(&mut self) {
        // Por si se han creado claves por fuera desde la última vez.
        if let Err(error) = self.ssh_settings.reload_keys() {
            self.show_status(error);
        }
        self.settings_window = Some(SettingsWindow {
            active_tab: SettingsTab::General,
            shortcuts: ShortcutsTabState::default(),
            ssh_keys: SshKeysTabState::new(self.ssh_settings.keys()),
        });
    }

    pub(super) fn close_settings(&mut self) {
        self.settings_window = None;
    }

    pub(super) fn show_settings_window(&mut self, egui_context: &egui::Context) {
        let Some(settings) = &mut self.settings_window else { return };
        // Mientras se graba un atajo, Esc es para cancelar la grabación.
        let mut close_requested = !settings.shortcuts.is_recording()
            && egui_context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape));
        let mut active_tab = settings.active_tab;

        let mut tab_actions = None;
        let clicked_outside = backdrop_was_clicked(egui_context);
        let window = egui::Window::new("Ajustes")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(egui_context, |ui| {
                ui.set_width(WINDOW_WIDTH);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("settings.title")).size(22.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, ICON_CLOSE, &tr("settings.close")).clicked() {
                            close_requested = true;
                        }
                    });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    for tab in SettingsTab::ALL {
                        if section_tab(ui, tab.icon(), &tab.title(), tab == active_tab).clicked() {
                            active_tab = tab;
                        }
                    }
                });
                ui.add_space(12.0);
                ScrollArea::vertical().max_height(CONTENT_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
                    tab_actions = Some(match active_tab {
                        SettingsTab::General => {
                            let updates = UpdatesOverview {
                                current_version: self.updates.current_version(),
                                is_checking: self.updates.is_checking(),
                            };
                            TabActions::General(show_general_tab(ui, self.app_settings.settings(), &updates))
                        }
                        SettingsTab::Shortcuts => TabActions::Shortcuts(settings.shortcuts.show(ui, self.app_settings.settings())),
                        SettingsTab::SshKeys => TabActions::SshKeys(settings.ssh_keys.show(ui, &self.ssh_settings)),
                    });
                });
            });

        keep_dialog_on_top(egui_context, &window);
        if clicked_outside {
            close_requested = true;
        }
        settings.active_tab = active_tab;
        match tab_actions {
            Some(TabActions::General(actions)) => self.apply_general_actions(actions),
            Some(TabActions::Shortcuts(actions)) => self.apply_shortcuts_actions(actions),
            Some(TabActions::SshKeys(actions)) => self.apply_ssh_keys_actions(actions, egui_context),
            None => {}
        }
        if close_requested {
            self.close_settings();
        }
    }
}

/// Título de una sección, con sitio a la derecha para algún botón.
pub(super) fn section_header(ui: &mut Ui, title: &str, trailing: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(16.0).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), trailing);
    });
}

/// Recuadro con fondo para agrupar una lista o unas opciones.
pub(super) fn card(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    Frame::none()
        .fill(palette::SURFACE_CONTAINER_LOW)
        .rounding(Rounding::same(12.0))
        .inner_margin(Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui);
        });
}
