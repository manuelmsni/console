//! Guarda los ajustes generales en `settings.json`, en la carpeta de datos (cifrado).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::data_folder::{DataFile, DataFolder};
use crate::application::ports::AppSettingsStore;
use crate::domain::app_settings::{AppSettings, AutosaveSettings, Shortcut, ShortcutAction, Shortcuts, UpdateSettings};
use crate::domain::startup::{FolderChoice, ShellChoice, StartSettings};
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
    /// Falta en ficheros de versiones anteriores: entonces, encendido.
    #[serde(default = "new_line_shortcut_is_enabled_by_default")]
    new_line_shortcut_enabled: bool,
    /// Falta en ficheros de versiones anteriores: entonces, lo de serie.
    #[serde(default)]
    updates: StoredUpdates,
    /// Falta en ficheros de versiones anteriores: entonces, lo de siempre
    /// (la primera terminal, en la carpeta del usuario).
    #[serde(default)]
    start: StoredStart,
}

#[derive(Serialize, Deserialize, Default)]
struct StoredStart {
    shell: StoredShellChoice,
    folder: StoredFolderChoice,
}

/// `"first_available"`, `"ask_each_time"` o `{"profile": "PowerShell"}`.
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum StoredShellChoice {
    #[default]
    FirstAvailable,
    AskEachTime,
    Profile(String),
}

/// `"home"`, `"ask_at_start"` o `{"fixed": "C:\carpeta"}`.
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum StoredFolderChoice {
    #[default]
    Home,
    AskAtStart,
    Fixed(String),
}

impl StoredStart {
    fn from_domain(start: &StartSettings) -> Self {
        let shell = match &start.shell {
            ShellChoice::FirstAvailable => StoredShellChoice::FirstAvailable,
            ShellChoice::AskEachTime => StoredShellChoice::AskEachTime,
            ShellChoice::Profile(name) => StoredShellChoice::Profile(name.clone()),
        };
        let folder = match &start.folder {
            FolderChoice::Home => StoredFolderChoice::Home,
            FolderChoice::AskAtStart => StoredFolderChoice::AskAtStart,
            FolderChoice::Fixed(folder) => StoredFolderChoice::Fixed(folder.clone()),
        };
        Self { shell, folder }
    }

    fn into_domain(self) -> StartSettings {
        let shell = match self.shell {
            StoredShellChoice::FirstAvailable => ShellChoice::FirstAvailable,
            StoredShellChoice::AskEachTime => ShellChoice::AskEachTime,
            StoredShellChoice::Profile(name) => ShellChoice::Profile(name),
        };
        let folder = match self.folder {
            StoredFolderChoice::Home => FolderChoice::Home,
            StoredFolderChoice::AskAtStart => FolderChoice::AskAtStart,
            StoredFolderChoice::Fixed(folder) => FolderChoice::Fixed(folder),
        };
        StartSettings { shell, folder }
    }
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
        Ok(Some(AppSettings {
            language: stored.language,
            autosave,
            shortcuts,
            new_line_shortcut_enabled: stored.new_line_shortcut_enabled,
            updates,
            start: stored.start.into_domain(),
        }))
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
            new_line_shortcut_enabled: settings.new_line_shortcut_enabled,
            updates: StoredUpdates {
                check_automatically: settings.updates.check_automatically,
                skipped_version: settings.updates.skipped_version.map(|version| version.to_string()),
            },
            start: StoredStart::from_domain(&settings.start),
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
        ShortcutAction::InsertNewLine => "insert_new_line",
    }
}

fn new_line_shortcut_is_enabled_by_default() -> bool {
    AppSettings::default().new_line_shortcut_enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_terminal_and_folder_of_new_tabs_survive_saving_and_loading() {
        let choices = [
            StartSettings::default(),
            StartSettings { shell: ShellChoice::AskEachTime, folder: FolderChoice::AskAtStart },
            StartSettings { shell: ShellChoice::Profile("PowerShell".into()), folder: FolderChoice::Fixed(r"C:\Mis cosas".into()) },
        ];
        for start in choices {
            let json = serde_json::to_string(&StoredStart::from_domain(&start)).unwrap();
            assert_eq!(serde_json::from_str::<StoredStart>(&json).unwrap().into_domain(), start);
        }
    }

    #[test]
    fn settings_of_older_versions_open_like_before() {
        let older = r#"{ "version": 1, "autosave": { "enabled": false, "interval_seconds": 10 }, "shortcuts": {} }"#;
        let stored: StoredSettings = serde_json::from_str(older).unwrap();
        assert_eq!(stored.start.into_domain(), StartSettings::default());
    }
}
