//! Guarda los ajustes generales en `settings.json`, en la carpeta de datos (cifrado).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::data_folder::{DataFile, DataFolder};
use crate::application::ports::AppSettingsStore;
use crate::domain::app_settings::{AppSettings, AutosaveSettings, Shortcut, ShortcutAction, Shortcuts, UpdateSettings};
use crate::domain::updates::Version;
use crate::i18n::tr_args;

pub struct JsonAppSettingsStore {
    file: DataFile,
}

impl JsonAppSettingsStore {
    pub fn in_folder(data: &DataFolder) -> Self {
        Self { file: data.file("settings.json") }
    }
}

/// Formato del fichero, separado del dominio igual que en los demás.
#[derive(Serialize, Deserialize)]
struct StoredSettings {
    version: u32,
    /// `null`: el idioma de Windows. Puede faltar en ficheros de versiones anteriores.
    #[serde(default)]
    language: Option<String>,
    autosave: StoredAutosave,
    /// Por nombre de acción; las que falten usan su atajo de serie.
    shortcuts: BTreeMap<String, StoredShortcut>,
    /// Falta en ficheros de versiones anteriores: entonces, lo de serie.
    #[serde(default)]
    updates: StoredUpdates,
}

#[derive(Serialize, Deserialize)]
struct StoredUpdates {
    check_automatically: bool,
    /// `1.4.2`.
    skipped_version: Option<String>,
}

impl Default for StoredUpdates {
    fn default() -> Self {
        let defaults = UpdateSettings::default();
        Self { check_automatically: defaults.check_automatically, skipped_version: None }
    }
}

#[derive(Serialize, Deserialize)]
struct StoredAutosave {
    enabled: bool,
    interval_seconds: u32,
}

#[derive(Serialize, Deserialize)]
struct StoredShortcut {
    ctrl: bool,
    shift: bool,
    alt: bool,
    key: String,
}

impl AppSettingsStore for JsonAppSettingsStore {
    fn load_settings(&self) -> Result<Option<AppSettings>, String> {
        let Some(json) = self.file.read_if_exists()? else { return Ok(None) };
        let stored: StoredSettings =
            serde_json::from_str(&json).map_err(|error| tr_args("storage.error.read_settings", &[("error", &error)]))?;

        let mut shortcuts = Shortcuts::default();
        for action in ShortcutAction::ALL {
            if let Some(saved) = stored.shortcuts.get(stored_name_of(action)) {
                let shortcut = Shortcut { ctrl: saved.ctrl, shift: saved.shift, alt: saved.alt, key: saved.key.clone() };
                // Una combinación que ya no es válida (por editar el fichero a mano) se ignora.
                let _ = shortcuts.set(action, shortcut);
            }
        }
        let autosave = AutosaveSettings { enabled: stored.autosave.enabled, interval_seconds: stored.autosave.interval_seconds };
        let updates = UpdateSettings {
            check_automatically: stored.updates.check_automatically,
            skipped_version: stored.updates.skipped_version.as_deref().and_then(Version::parse),
        };
        Ok(Some(AppSettings { language: stored.language, autosave, shortcuts, updates }))
    }

    fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let shortcuts = ShortcutAction::ALL
            .into_iter()
            .map(|action| {
                let shortcut = settings.shortcuts.get(action);
                let stored = StoredShortcut { ctrl: shortcut.ctrl, shift: shortcut.shift, alt: shortcut.alt, key: shortcut.key };
                (stored_name_of(action).to_string(), stored)
            })
            .collect();
        let stored = StoredSettings {
            version: 1,
            language: settings.language.clone(),
            autosave: StoredAutosave { enabled: settings.autosave.enabled, interval_seconds: settings.autosave.interval_seconds },
            shortcuts,
            updates: StoredUpdates {
                check_automatically: settings.updates.check_automatically,
                skipped_version: settings.updates.skipped_version.map(|version| version.to_string()),
            },
        };
        let json = serde_json::to_string_pretty(&stored).map_err(|error| error.to_string())?;
        self.file.write_atomically(&json)
    }
}

/// Nombres fijos en el fichero, para que renombrar algo en el código no rompa los ajustes guardados.
fn stored_name_of(action: ShortcutAction) -> &'static str {
    match action {
        ShortcutAction::NewTab => "nueva_pestana",
        ShortcutAction::PreviousTab => "pestana_anterior",
        ShortcutAction::NextTab => "pestana_siguiente",
        ShortcutAction::OpenCommandHistory => "historial_de_comandos",
        ShortcutAction::SaveTab => "guardar_pestana",
        ShortcutAction::ToggleTabAutosave => "autoguardado_de_pestana",
    }
}
