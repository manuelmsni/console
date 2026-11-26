//! Con qué terminal y en qué carpeta se abren las pestañas nuevas: al abrir la
//! app, con "Abrir Console aquí" desde el Explorador o con el atajo de nueva
//! pestaña. Se puede fijar o pedir que se pregunte cada vez.

use super::shell_profile::ShellProfile;

/// El argumento con el que el menú del Explorador lanza el programa:
/// `console.exe --open-folder "C:\carpeta"`.
pub const OPEN_FOLDER_ARGUMENT: &str = "--open-folder";

/// Con qué terminal se abren las pestañas nuevas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ShellChoice {
    /// La primera de las disponibles (lo de siempre).
    #[default]
    FirstAvailable,
    AskEachTime,
    /// Por su nombre (`cmd`, `PowerShell`...). Si ya no existe, la primera.
    Profile(String),
}

impl ShellChoice {
    /// La terminal elegida; `None` si hay que preguntar.
    pub fn resolve<'a>(&self, profiles: &'a [ShellProfile]) -> Option<&'a ShellProfile> {
        match self {
            ShellChoice::AskEachTime => None,
            ShellChoice::FirstAvailable => profiles.first(),
            ShellChoice::Profile(name) => profiles.iter().find(|profile| &profile.display_name == name).or(profiles.first()),
        }
    }

    /// La que se usa cuando no se puede preguntar (un `ssh` desde los ajustes)
    /// y la que sale marcada al preguntar.
    pub fn resolve_without_asking<'a>(&self, profiles: &'a [ShellProfile]) -> Option<&'a ShellProfile> {
        self.resolve(profiles).or(profiles.first())
    }
}

/// En qué carpeta empiezan las pestañas nuevas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FolderChoice {
    /// La carpeta del usuario (lo de siempre).
    #[default]
    Home,
    /// Se pregunta al abrir la app; las pestañas nuevas de esa vez empiezan
    /// en la carpeta elegida.
    AskAtStart,
    Fixed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartSettings {
    pub shell: ShellChoice,
    pub folder: FolderChoice,
}

impl StartSettings {
    /// Dónde empiezan las pestañas nuevas abiertas con ➕ o con el atajo (y
    /// las que se reabren). `None`: la carpeta del usuario.
    pub fn folder_for_new_tabs(&self, folder_chosen_at_start: Option<&str>) -> Option<String> {
        match &self.folder {
            FolderChoice::Home => None,
            FolderChoice::AskAtStart => folder_chosen_at_start.map(str::to_string),
            FolderChoice::Fixed(folder) => Some(folder.clone()),
        }
    }
}

/// Dónde va a empezar una pestaña que se va a abrir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedFolder {
    Home,
    Known(String),
    Ask,
}

/// Lo que se sabe de una pestaña que se va a abrir y lo que falta preguntar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTabPlan {
    /// `None`: hay que preguntar la terminal.
    pub shell: Option<ShellProfile>,
    pub folder: PlannedFolder,
}

impl NewTabPlan {
    /// La primera pestaña al abrir la app (cuando no se ha reabierto ninguna).
    pub fn at_app_start(settings: &StartSettings, profiles: &[ShellProfile]) -> Self {
        let folder = match &settings.folder {
            FolderChoice::Home => PlannedFolder::Home,
            FolderChoice::AskAtStart => PlannedFolder::Ask,
            FolderChoice::Fixed(folder) => PlannedFolder::Known(folder.clone()),
        };
        Self { shell: settings.shell.resolve(profiles).cloned(), folder }
    }

    /// "Abrir Console aquí": la carpeta ya se sabe; la terminal, según el ajuste.
    pub fn in_folder(folder: String, settings: &StartSettings, profiles: &[ShellProfile]) -> Self {
        Self { shell: settings.shell.resolve(profiles).cloned(), folder: PlannedFolder::Known(folder) }
    }

    /// Atajo de nueva pestaña: en la carpeta de las pestañas nuevas.
    pub fn new_tab(settings: &StartSettings, profiles: &[ShellProfile], folder_chosen_at_start: Option<&str>) -> Self {
        let folder = match settings.folder_for_new_tabs(folder_chosen_at_start) {
            Some(folder) => PlannedFolder::Known(folder),
            None => PlannedFolder::Home,
        };
        Self { shell: settings.shell.resolve(profiles).cloned(), folder }
    }

    pub fn needs_asking(&self) -> bool {
        self.shell.is_none() || self.folder == PlannedFolder::Ask
    }
}

/// La carpeta que pide abrir la línea de órdenes (`--open-folder <carpeta>`).
pub fn folder_to_open_from(arguments: &[String]) -> Option<String> {
    let position = arguments.iter().position(|argument| argument == OPEN_FOLDER_ARGUMENT)?;
    // Si la carpeta tenía espacios y llegó partida, se vuelve a juntar.
    let rest = arguments.get(position + 1..)?;
    if rest.is_empty() {
        return None;
    }
    folder_from_explorer_argument(&rest.join(" "))
}

/// El Explorador pasa la raíz de una unidad como `"C:\"`, y la regla de
/// Windows de "barra y comilla" hace que llegue `C:"`. Se arregla aquí.
pub fn folder_from_explorer_argument(argument: &str) -> Option<String> {
    let mut folder = argument.trim().trim_matches('"').trim().to_string();
    if folder.is_empty() {
        return None;
    }
    if folder.ends_with(':') {
        folder.push('\\');
    }
    Some(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profiles() -> Vec<ShellProfile> {
        vec![ShellProfile::new("cmd", "cmd.exe", &[]), ShellProfile::new("PowerShell", "powershell.exe", &[])]
    }

    fn arguments(list: &[&str]) -> Vec<String> {
        list.iter().map(|argument| argument.to_string()).collect()
    }

    #[test]
    fn by_default_the_first_terminal_opens_in_the_home_folder_without_asking() {
        let plan = NewTabPlan::at_app_start(&StartSettings::default(), &profiles());
        assert_eq!(plan.shell.map(|profile| profile.display_name), Some("cmd".to_string()));
        assert_eq!(plan.folder, PlannedFolder::Home);
    }

    #[test]
    fn a_chosen_terminal_that_no_longer_exists_falls_back_to_the_first() {
        let gone = ShellChoice::Profile("WSL".into());
        assert_eq!(gone.resolve(&profiles()).map(|profile| profile.display_name.as_str()), Some("cmd"));
        let powershell = ShellChoice::Profile("PowerShell".into());
        assert_eq!(powershell.resolve(&profiles()).map(|profile| profile.display_name.as_str()), Some("PowerShell"));
    }

    #[test]
    fn asking_for_the_terminal_still_has_one_to_use_without_asking() {
        let ask = ShellChoice::AskEachTime;
        assert_eq!(ask.resolve(&profiles()), None);
        assert_eq!(ask.resolve_without_asking(&profiles()).map(|profile| profile.display_name.as_str()), Some("cmd"));
    }

    #[test]
    fn the_folder_is_asked_only_at_start_and_then_reused_for_new_tabs() {
        let settings = StartSettings { shell: ShellChoice::FirstAvailable, folder: FolderChoice::AskAtStart };
        assert!(NewTabPlan::at_app_start(&settings, &profiles()).needs_asking());

        let before_choosing = NewTabPlan::new_tab(&settings, &profiles(), None);
        assert_eq!(before_choosing.folder, PlannedFolder::Home);
        assert!(!before_choosing.needs_asking());
        let after_choosing = NewTabPlan::new_tab(&settings, &profiles(), Some(r"D:\work"));
        assert_eq!(after_choosing.folder, PlannedFolder::Known(r"D:\work".into()));
    }

    #[test]
    fn a_folder_from_the_explorer_only_asks_for_the_terminal_if_set_to_ask() {
        let fixed_folder = StartSettings { shell: ShellChoice::FirstAvailable, folder: FolderChoice::AskAtStart };
        let plan = NewTabPlan::in_folder(r"C:\repo".into(), &fixed_folder, &profiles());
        assert_eq!(plan.folder, PlannedFolder::Known(r"C:\repo".into()));
        assert!(!plan.needs_asking());

        let ask_shell = StartSettings { shell: ShellChoice::AskEachTime, folder: FolderChoice::Home };
        assert!(NewTabPlan::in_folder(r"C:\repo".into(), &ask_shell, &profiles()).needs_asking());
    }

    #[test]
    fn a_fixed_folder_is_used_for_every_new_tab() {
        let settings = StartSettings { shell: ShellChoice::FirstAvailable, folder: FolderChoice::Fixed(r"C:\code".into()) };
        assert_eq!(settings.folder_for_new_tabs(Some(r"D:\other")), Some(r"C:\code".into()));
        assert_eq!(NewTabPlan::at_app_start(&settings, &profiles()).folder, PlannedFolder::Known(r"C:\code".into()));
    }

    #[test]
    fn the_folder_comes_after_its_argument() {
        assert_eq!(folder_to_open_from(&arguments(&["console.exe", "--open-folder", r"C:\My files"])), Some(r"C:\My files".into()));
        assert_eq!(folder_to_open_from(&arguments(&["console.exe", "--open-folder", r"C:\My", "files"])), Some(r"C:\My files".into()));
        assert_eq!(folder_to_open_from(&arguments(&["console.exe"])), None);
        assert_eq!(folder_to_open_from(&arguments(&["console.exe", "--open-folder"])), None);
    }

    #[test]
    fn the_root_of_a_drive_survives_the_quotes_of_the_explorer() {
        assert_eq!(folder_from_explorer_argument("C:\""), Some(r"C:\".into()));
        assert_eq!(folder_from_explorer_argument(r"D:\"), Some(r"D:\".into()));
        assert_eq!(folder_from_explorer_argument("\"\""), None);
    }
}
