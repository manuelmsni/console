//! Atajos de teclado configurables: reconocerlos entre lo que se pulsa y
//! hacer lo que corresponde.

use eframe::egui::{self, Event, Key, Modifiers};

use super::ConsoleApp;
use crate::domain::app_settings::{Shortcut, ShortcutAction};
use crate::i18n::tr;

/// `true` (y se retira la pulsación, para que nadie más la use) si se ha
/// pulsado exactamente esa combinación. Se compara entera a propósito: egui
/// ignora el Shift si no se pide, y Ctrl+S saltaría también con Ctrl+Shift+S.
pub fn consume_shortcut(input: &mut egui::InputState, shortcut: &Shortcut) -> bool {
    let Some(shortcut_key) = Key::from_name(&shortcut.key) else { return false };
    let position = input.events.iter().position(|event| {
        matches!(event, Event::Key { key, pressed: true, modifiers, .. } if *key == shortcut_key && same_modifiers(*modifiers, shortcut))
    });
    match position {
        Some(index) => {
            input.events.remove(index);
            true
        }
        None => false,
    }
}

pub fn is_pressing(key: Key, modifiers: Modifiers, shortcut: &Shortcut) -> bool {
    Key::from_name(&shortcut.key) == Some(key) && same_modifiers(modifiers, shortcut)
}

/// La combinación que corresponde a una pulsación (para grabar un atajo nuevo).
pub fn shortcut_from(key: Key, modifiers: Modifiers) -> Shortcut {
    Shortcut { ctrl: modifiers.ctrl, shift: modifiers.shift, alt: modifiers.alt, key: key.name().to_string() }
}

fn same_modifiers(modifiers: Modifiers, shortcut: &Shortcut) -> bool {
    modifiers.ctrl == shortcut.ctrl && modifiers.shift == shortcut.shift && modifiers.alt == shortcut.alt
}

impl ConsoleApp {
    /// Los atajos que valen en toda la ventana. Con una ventana de ajustes o
    /// un diálogo abiertos no se atienden: ahí las teclas son para ellos.
    /// (El del historial de comandos lo atiende la terminal, porque solo
    /// funciona en el prompt.)
    pub(super) fn handle_window_shortcuts(&mut self, egui_context: &egui::Context) {
        if self.a_dialog_is_open() {
            return;
        }
        let window_actions = [
            ShortcutAction::NewTab,
            ShortcutAction::PreviousTab,
            ShortcutAction::NextTab,
            ShortcutAction::SaveTab,
            ShortcutAction::ToggleTabAutosave,
        ];
        let pressed: Vec<ShortcutAction> = egui_context.input_mut(|input| {
            window_actions.into_iter().filter(|action| consume_shortcut(input, &self.app_settings.shortcut(*action))).collect()
        });
        for action in pressed {
            self.run_shortcut_action(action);
        }
    }

    fn run_shortcut_action(&mut self, action: ShortcutAction) {
        match action {
            ShortcutAction::NewTab => {
                if let Some(profile) = self.shell_profiles.first().cloned() {
                    self.open_tab_with(&profile);
                }
            }
            ShortcutAction::PreviousTab | ShortcutAction::NextTab => {
                self.close_history_picker();
                self.terminal_tabs.activate_neighbour_tab(if action == ShortcutAction::NextTab { 1 } else { -1 });
                self.terminal_should_take_focus = true;
            }
            ShortcutAction::SaveTab => match self.terminal_tabs.save_active_tab() {
                Ok(()) => self.show_status(tr("tabs.saved")),
                Err(error) => self.show_status(error),
            },
            ShortcutAction::ToggleTabAutosave => match self.terminal_tabs.toggle_autosave_of_active_tab() {
                Some(true) => self.show_status(tr("tabs.autosave_on")),
                Some(false) => self.show_status(tr("tabs.autosave_off")),
                None => {}
            },
            ShortcutAction::OpenCommandHistory => self.open_history_picker(),
        }
    }
}
