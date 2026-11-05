use super::ports::AppSettingsStore;
use crate::domain::app_settings::{AppSettings, AutosaveSettings, Shortcut, ShortcutAction, Shortcuts, UpdateSettings};

/// Casos de uso de los ajustes generales. Cada cambio se guarda al momento.
pub struct AppSettingsService {
    settings: AppSettings,
    store: Box<dyn AppSettingsStore>,
}

impl AppSettingsService {
    /// Devuelve también un aviso si los ajustes guardados no se pudieron leer.
    pub fn load(store: Box<dyn AppSettingsStore>) -> (Self, Option<String>) {
        let (settings, warning) = match store.load_settings() {
            Ok(saved) => (saved.unwrap_or_default(), None),
            Err(error) => (AppSettings::default(), Some(error)),
        };
        (Self { settings, store }, warning)
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    pub fn shortcut(&self, action: ShortcutAction) -> Shortcut {
        self.settings.shortcuts.get(action)
    }

    /// `None`: el idioma de Windows.
    pub fn set_language(&mut self, language: Option<String>) -> Result<(), String> {
        self.settings.language = language;
        self.store.save_settings(&self.settings)
    }

    pub fn set_autosave(&mut self, autosave: AutosaveSettings) -> Result<(), String> {
        self.settings.autosave = AutosaveSettings {
            enabled: autosave.enabled,
            interval_seconds: autosave
                .interval_seconds
                .clamp(AutosaveSettings::MIN_INTERVAL_SECONDS, AutosaveSettings::MAX_INTERVAL_SECONDS),
        };
        self.store.save_settings(&self.settings)
    }

    pub fn set_shortcut(&mut self, action: ShortcutAction, shortcut: Shortcut) -> Result<(), String> {
        self.settings.shortcuts.set(action, shortcut).map_err(|error| error.to_string())?;
        self.store.save_settings(&self.settings)
    }

    pub fn set_update_settings(&mut self, updates: UpdateSettings) -> Result<(), String> {
        self.settings.updates = updates;
        self.store.save_settings(&self.settings)
    }

    pub fn restore_default_shortcuts(&mut self) -> Result<(), String> {
        self.settings.shortcuts = Shortcuts::default();
        self.store.save_settings(&self.settings)
    }
}
