//! Ajustes generales de la aplicación: el idioma, el autoguardado de las
//! pestañas, los atajos de teclado y las actualizaciones.

use std::collections::BTreeMap;
use std::fmt;

use super::updates::Version;
use crate::i18n::{tr, tr_args};

/// Cada cuánto se guardan solas las pestañas, si se quiere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutosaveSettings {
    pub enabled: bool,
    pub interval_seconds: u32,
}

impl AutosaveSettings {
    pub const MIN_INTERVAL_SECONDS: u32 = 1;
    pub const MAX_INTERVAL_SECONDS: u32 = 600;
}

impl Default for AutosaveSettings {
    /// Desactivado: las pestañas se guardan cuando el usuario lo pide.
    fn default() -> Self {
        Self { enabled: false, interval_seconds: 10 }
    }
}

/// Buscar versiones nuevas al abrir (y una vez al día con la app abierta).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateSettings {
    pub check_automatically: bool,
    /// La versión que el usuario dijo "saltar": no se vuelve a ofrecer.
    pub skipped_version: Option<Version>,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self { check_automatically: true, skipped_version: None }
    }
}

/// Lo que se puede hacer con un atajo de teclado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShortcutAction {
    NewTab,
    PreviousTab,
    NextTab,
    OpenCommandHistory,
    SaveTab,
    ToggleTabAutosave,
}

impl ShortcutAction {
    pub const ALL: [ShortcutAction; 6] = [
        ShortcutAction::NewTab,
        ShortcutAction::PreviousTab,
        ShortcutAction::NextTab,
        ShortcutAction::OpenCommandHistory,
        ShortcutAction::SaveTab,
        ShortcutAction::ToggleTabAutosave,
    ];

    pub fn description(self) -> String {
        match self {
            ShortcutAction::NewTab => tr("shortcuts.action.new_tab"),
            ShortcutAction::PreviousTab => tr("shortcuts.action.previous_tab"),
            ShortcutAction::NextTab => tr("shortcuts.action.next_tab"),
            ShortcutAction::OpenCommandHistory => tr("shortcuts.action.command_history"),
            ShortcutAction::SaveTab => tr("shortcuts.action.save_tab"),
            ShortcutAction::ToggleTabAutosave => tr("shortcuts.action.toggle_tab_autosave"),
        }
    }

    fn default_shortcut(self) -> Shortcut {
        match self {
            ShortcutAction::NewTab => Shortcut::ctrl_shift("T"),
            ShortcutAction::PreviousTab => Shortcut::ctrl("ArrowLeft"),
            ShortcutAction::NextTab => Shortcut::ctrl("ArrowRight"),
            ShortcutAction::OpenCommandHistory => Shortcut::ctrl("ArrowUp"),
            ShortcutAction::SaveTab => Shortcut::ctrl("S"),
            ShortcutAction::ToggleTabAutosave => Shortcut::ctrl_shift("S"),
        }
    }
}

/// Una combinación de teclas. `key` es el nombre de la tecla tal como lo da
/// la interfaz (`T`, `ArrowLeft`, `F5`...).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: String,
}

impl Shortcut {
    fn ctrl(key: &str) -> Self {
        Self { ctrl: true, shift: false, alt: false, key: key.to_string() }
    }

    fn ctrl_shift(key: &str) -> Self {
        Self { ctrl: true, shift: true, alt: false, key: key.to_string() }
    }

    /// Sin Ctrl ni Alt, la combinación se comería letras al escribir en la
    /// consola; solo se permiten así las teclas de función (F1-F12).
    pub fn is_usable_while_typing(&self) -> bool {
        self.ctrl || self.alt || is_function_key(&self.key)
    }
}

impl fmt::Display for Shortcut {
    /// `Ctrl + Shift + T`, `Ctrl + ←`...
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let modifiers = [(self.ctrl, "Ctrl + "), (self.shift, "Shift + "), (self.alt, "Alt + ")];
        for (is_pressed, name) in modifiers {
            if is_pressed {
                formatter.write_str(name)?;
            }
        }
        let key = match self.key.as_str() {
            "ArrowLeft" => "←",
            "ArrowRight" => "→",
            "ArrowUp" => "↑",
            "ArrowDown" => "↓",
            other => other,
        };
        formatter.write_str(key)
    }
}

fn is_function_key(key: &str) -> bool {
    key.strip_prefix('F').is_some_and(|number| number.parse::<u8>().is_ok_and(|number| (1..=12).contains(&number)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutError {
    /// La combinación ya la usa otra acción.
    AlreadyUsedBy(ShortcutAction),
    NeedsCtrlOrAlt,
}

impl fmt::Display for ShortcutError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShortcutError::AlreadyUsedBy(action) => {
                formatter.write_str(&tr_args("shortcuts.error.already_used", &[("action", &action.description())]))
            }
            ShortcutError::NeedsCtrlOrAlt => formatter.write_str(&tr("shortcuts.error.needs_ctrl_or_alt")),
        }
    }
}

/// El atajo de cada acción.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcuts(BTreeMap<ShortcutAction, Shortcut>);

impl Default for Shortcuts {
    fn default() -> Self {
        Self(ShortcutAction::ALL.iter().map(|action| (*action, action.default_shortcut())).collect())
    }
}

impl Shortcuts {
    pub fn get(&self, action: ShortcutAction) -> Shortcut {
        self.0.get(&action).cloned().unwrap_or_else(|| action.default_shortcut())
    }

    pub fn set(&mut self, action: ShortcutAction, shortcut: Shortcut) -> Result<(), ShortcutError> {
        if !shortcut.is_usable_while_typing() {
            return Err(ShortcutError::NeedsCtrlOrAlt);
        }
        if let Some(other) = ShortcutAction::ALL.into_iter().find(|other| *other != action && self.get(*other) == shortcut) {
            return Err(ShortcutError::AlreadyUsedBy(other));
        }
        self.0.insert(action, shortcut);
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppSettings {
    /// Código del idioma de la interfaz (`en`, `es`...). `None`: el de Windows.
    pub language: Option<String>,
    pub autosave: AutosaveSettings,
    pub shortcuts: Shortcuts,
    pub updates: UpdateSettings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autosave_is_off_by_default() {
        assert!(!AppSettings::default().autosave.enabled);
    }

    #[test]
    fn shortcuts_read_like_on_the_keyboard() {
        let shortcuts = Shortcuts::default();
        assert_eq!(shortcuts.get(ShortcutAction::NewTab).to_string(), "Ctrl + Shift + T");
        assert_eq!(shortcuts.get(ShortcutAction::NextTab).to_string(), "Ctrl + →");
    }

    #[test]
    fn two_actions_cannot_share_a_shortcut() {
        let mut shortcuts = Shortcuts::default();
        let save = shortcuts.get(ShortcutAction::SaveTab);
        assert_eq!(shortcuts.set(ShortcutAction::NewTab, save), Err(ShortcutError::AlreadyUsedBy(ShortcutAction::SaveTab)));
    }

    #[test]
    fn plain_letters_are_not_allowed_but_function_keys_are() {
        let mut shortcuts = Shortcuts::default();
        let plain_letter = Shortcut { ctrl: false, shift: false, alt: false, key: "S".into() };
        let function_key = Shortcut { ctrl: false, shift: false, alt: false, key: "F5".into() };
        assert_eq!(shortcuts.set(ShortcutAction::SaveTab, plain_letter), Err(ShortcutError::NeedsCtrlOrAlt));
        assert_eq!(shortcuts.set(ShortcutAction::SaveTab, function_key.clone()), Ok(()));
        assert_eq!(shortcuts.get(ShortcutAction::SaveTab), function_key);
    }
}
