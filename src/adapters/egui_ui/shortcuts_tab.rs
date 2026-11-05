//! Pestaña "Atajos" de los ajustes: cambiar la combinación de cada acción
//! pulsándola.

use eframe::egui::{vec2, Align, Button, Event, Key, Layout, RichText, Ui};

use super::material_widgets::text_button;
use super::settings_window::{card, section_header};
use super::shortcuts::shortcut_from;
use super::theme::palette;
use super::ConsoleApp;
use crate::domain::app_settings::{AppSettings, Shortcut, ShortcutAction};
use crate::i18n::tr;

#[derive(Default)]
pub(super) struct ShortcutsTabState {
    /// La acción cuya combinación se está grabando: la próxima pulsación será la nueva.
    recording: Option<ShortcutAction>,
    problem: Option<String>,
}

pub(super) enum ShortcutsAction {
    Set(ShortcutAction, Shortcut),
    RestoreDefaults,
}

enum Captured {
    Cancelled,
    Combination(Shortcut),
}

impl ShortcutsTabState {
    /// Mientras se graba, Esc cancela la grabación en vez de cerrar los ajustes.
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    pub fn show(&mut self, ui: &mut Ui, settings: &AppSettings) -> Vec<ShortcutsAction> {
        let mut actions = Vec::new();
        if let Some(action) = self.recording {
            match capture_next_key_press(ui) {
                Some(Captured::Cancelled) => self.recording = None,
                Some(Captured::Combination(shortcut)) => {
                    actions.push(ShortcutsAction::Set(action, shortcut));
                    self.recording = None;
                }
                None => {}
            }
        }

        section_header(ui, &tr("settings.shortcuts.section"), |_| {});
        ui.label(
            RichText::new(tr("settings.shortcuts.explanation"))
                .color(palette::ON_SURFACE_VARIANT),
        );
        ui.add_space(8.0);
        card(ui, |ui| {
            for action in ShortcutAction::ALL {
                ui.horizontal(|ui| {
                    ui.label(action.description());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let is_being_recorded = self.recording == Some(action);
                        let text = if is_being_recorded {
                            RichText::new(tr("settings.shortcuts.press_keys")).color(palette::PRIMARY)
                        } else {
                            RichText::new(settings.shortcuts.get(action).to_string()).monospace()
                        };
                        if ui.add(Button::new(text).min_size(vec2(170.0, 32.0))).clicked() {
                            self.recording = Some(action);
                            self.problem = None;
                        }
                    });
                });
            }
        });
        if let Some(problem) = &self.problem {
            ui.label(RichText::new(problem).color(palette::ERROR));
        }
        ui.add_space(8.0);
        if text_button(ui, &tr("settings.shortcuts.restore_defaults")).clicked() {
            actions.push(ShortcutsAction::RestoreDefaults);
        }
        actions
    }
}

/// La siguiente tecla pulsada (con sus modificadores), retirada para que no
/// la use nadie más. Esc sola cancela.
fn capture_next_key_press(ui: &mut Ui) -> Option<Captured> {
    ui.input_mut(|input| {
        let index = input.events.iter().position(|event| matches!(event, Event::Key { pressed: true, .. }))?;
        let Event::Key { key, modifiers, .. } = input.events.remove(index) else { return None };
        if key == Key::Escape && modifiers.is_none() {
            Some(Captured::Cancelled)
        } else {
            Some(Captured::Combination(shortcut_from(key, modifiers)))
        }
    })
}

impl ConsoleApp {
    pub(super) fn apply_shortcuts_actions(&mut self, actions: Vec<ShortcutsAction>) {
        for action in actions {
            let result = match action {
                ShortcutsAction::Set(action, shortcut) => self.app_settings.set_shortcut(action, shortcut),
                ShortcutsAction::RestoreDefaults => self.app_settings.restore_default_shortcuts(),
            };
            if let Some(settings) = &mut self.settings_window {
                settings.shortcuts_mut().problem = result.err();
            }
        }
    }
}
