//! Rutas guardadas de cada terminal: carpetas del equipo o servidores SSH
//! (con una carpeta dentro, si se quiere), con un alias y un comando al abrir
//! opcionales. Salen al pasar el ratón por la terminal en el menú ➕, para
//! abrir una pestaña ya en ese sitio (y con ese comando lanzado).

use std::collections::BTreeMap;
use std::fmt;

use super::shell_profile::ShellProfile;
use super::ssh::SshDestination;
use crate::i18n::tr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaceId(pub u64);

/// A dónde lleva una ruta guardada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceTarget {
    LocalFolder(String),
    Ssh {
        destination: SshDestination,
        /// `None`: el 22 (o lo que diga `~/.ssh/config`).
        port: Option<u16>,
        /// Carpeta en el servidor. `None`: donde entre `ssh` (su carpeta personal).
        remote_folder: Option<RemoteFolder>,
    },
}

/// Una carpeta en un servidor SSH: `/var/www`, `~/proyectos`...
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteFolder(String);

impl RemoteFolder {
    pub fn parse(text: &str) -> Result<Option<Self>, PlaceError> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(None);
        }
        if !reaches_ssh_untouched(text) {
            return Err(PlaceError::UnsafeRemoteFolder);
        }
        Ok(Some(Self(text.to_string())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Para `cd` en la shell del servidor: entre comillas simples, salvo el
    /// `~` del principio, que si no la shell no lo cambiaría por la carpeta personal.
    fn quoted_for_posix_shell(&self) -> String {
        let quote = |text: &str| format!("'{}'", text.replace('\'', r"'\''"));
        match self.0.as_str() {
            "~" => "~".to_string(),
            folder => match folder.strip_prefix("~/") {
                Some(rest) => format!("~/{}", quote(rest)),
                None => quote(folder),
            },
        }
    }
}

/// Lo que va entre comillas dobles en la línea de `ssh`: ni comillas dobles,
/// ni `%`, ni saltos de línea, que cmd y PowerShell interpretarían antes de que
/// lleguen al servidor.
fn reaches_ssh_untouched(text: &str) -> bool {
    !text.chars().any(|character| character == '"' || character == '%' || character.is_control())
}

/// Lo que se puede cambiar de una ruta guardada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceDetails {
    pub alias: Option<String>,
    pub target: PlaceTarget,
    /// Se lanza al abrir: en la pestaña si es una carpeta, en el servidor si es SSH.
    pub command: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedPlace {
    pub id: PlaceId,
    pub alias: Option<String>,
    pub target: PlaceTarget,
    pub command: Option<String>,
}

impl SavedPlace {
    /// Lo que se ve en el menú: el alias o, si no tiene, a dónde lleva.
    pub fn label(&self) -> String {
        self.alias.clone().unwrap_or_else(|| self.target.to_string())
    }

    /// El nombre de la pestaña que se abre: el alias, el nombre de la carpeta
    /// o el servidor.
    pub fn tab_title(&self) -> String {
        if let Some(alias) = &self.alias {
            return alias.clone();
        }
        match &self.target {
            PlaceTarget::LocalFolder(folder) => last_folder_name(folder),
            PlaceTarget::Ssh { destination, .. } => destination.as_str().to_string(),
        }
    }
}

impl fmt::Display for PlaceTarget {
    /// `C:\código`, `usuario@servidor`, `usuario@servidor:2222:/var/www`.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PlaceTarget::LocalFolder(folder) => formatter.write_str(folder),
            PlaceTarget::Ssh { destination, port, remote_folder } => {
                formatter.write_str(destination.as_str())?;
                if let Some(port) = port {
                    write!(formatter, ":{port}")?;
                }
                if let Some(folder) = remote_folder {
                    write!(formatter, ":{}", folder.as_str())?;
                }
                Ok(())
            }
        }
    }
}

fn last_folder_name(folder: &str) -> String {
    let trimmed = folder.trim_end_matches(['\\', '/']);
    match trimmed.rsplit(['\\', '/']).next() {
        Some(name) if !name.is_empty() && !name.ends_with(':') => name.to_string(),
        _ => folder.to_string(),
    }
}

/// La línea que se escribe en la terminal para entrar en el servidor. Si hay
/// carpeta, va a ella; si hay comando, lo lanza allí. En los dos casos se
/// queda después con una shell abierta en el servidor.
pub fn ssh_command_line(
    profile: &ShellProfile,
    destination: &SshDestination,
    port: Option<u16>,
    remote_folder: Option<&RemoteFolder>,
    command: Option<&str>,
) -> String {
    let port = port.map(|port| format!(" -p {port}")).unwrap_or_default();
    // Si no se puede entrar en la carpeta, el comando no se lanza (`&&`), pero
    // la shell se abre igual (`;`).
    let remote_command = match (remote_folder, command) {
        (None, None) => return format!("ssh{port} {}", destination.as_str()),
        (Some(folder), None) => format!("cd {} && exec $SHELL -l", folder.quoted_for_posix_shell()),
        (None, Some(command)) => format!("{command}; exec $SHELL -l"),
        (Some(folder), Some(command)) => format!("cd {} && {command}; exec $SHELL -l", folder.quoted_for_posix_shell()),
    };
    // En PowerShell, `--%` hace que el resto llegue a `ssh` tal cual (sin
    // cambiar `$SHELL` por una variable de PowerShell).
    let stop_parsing = if profile.uses_powershell_syntax() { " --%" } else { "" };
    format!("ssh{stop_parsing}{port} -t {} \"{remote_command}\"", destination.as_str())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceError {
    EmptyFolder,
    InvalidDestination,
    InvalidPort,
    UnsafeRemoteFolder,
    UnsafeRemoteCommand,
}

impl fmt::Display for PlaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&match self {
            PlaceError::EmptyFolder => tr("places.error.empty_folder"),
            PlaceError::InvalidDestination => tr("places.error.invalid_destination"),
            PlaceError::InvalidPort => tr("places.error.invalid_port"),
            PlaceError::UnsafeRemoteFolder => tr("places.error.unsafe_remote_folder"),
            PlaceError::UnsafeRemoteCommand => tr("places.error.unsafe_remote_command"),
        })
    }
}

/// Lo escrito en el formulario de una ruta, tal cual.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceFields {
    pub alias: String,
    pub is_ssh: bool,
    pub folder: String,
    pub destination: String,
    pub port: String,
    pub remote_folder: String,
    pub command: String,
}

impl PlaceFields {
    pub fn of(place: &SavedPlace) -> Self {
        let alias = place.alias.clone().unwrap_or_default();
        let command = place.command.clone().unwrap_or_default();
        match &place.target {
            PlaceTarget::LocalFolder(folder) => Self { alias, command, folder: folder.clone(), ..Self::default() },
            PlaceTarget::Ssh { destination, port, remote_folder } => Self {
                alias,
                command,
                is_ssh: true,
                destination: destination.as_str().to_string(),
                port: port.map(|port| port.to_string()).unwrap_or_default(),
                remote_folder: remote_folder.as_ref().map(|folder| folder.as_str().to_string()).unwrap_or_default(),
                ..Self::default()
            },
        }
    }

    /// Los campos vacíos opcionales (alias, comando) quedan en `None`.
    pub fn parse(&self) -> Result<PlaceDetails, PlaceError> {
        let alias = Some(self.alias.trim().to_string()).filter(|alias| !alias.is_empty());
        let command = Some(self.command.trim().to_string()).filter(|command| !command.is_empty());
        if !self.is_ssh {
            let folder = self.folder.trim();
            if folder.is_empty() {
                return Err(PlaceError::EmptyFolder);
            }
            return Ok(PlaceDetails { alias, target: PlaceTarget::LocalFolder(folder.to_string()), command });
        }
        if command.as_deref().is_some_and(|command| !reaches_ssh_untouched(command)) {
            return Err(PlaceError::UnsafeRemoteCommand);
        }
        let destination = SshDestination::parse(&self.destination).map_err(|_| PlaceError::InvalidDestination)?;
        let port = match self.port.trim() {
            "" => None,
            port => Some(port.parse::<u16>().ok().filter(|port| *port > 0).ok_or(PlaceError::InvalidPort)?),
        };
        let remote_folder = RemoteFolder::parse(&self.remote_folder)?;
        Ok(PlaceDetails { alias, target: PlaceTarget::Ssh { destination, port, remote_folder }, command })
    }
}

/// Las rutas de cada terminal, por su nombre (`cmd`, `PowerShell`...), en el
/// orden en que se muestran.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedPlaces {
    by_profile: BTreeMap<String, Vec<SavedPlace>>,
    next_id: u64,
}

impl SavedPlaces {
    pub fn places_of(&self, profile_name: &str) -> &[SavedPlace] {
        self.by_profile.get(profile_name).map_or(&[], Vec::as_slice)
    }

    pub fn find(&self, id: PlaceId) -> Option<&SavedPlace> {
        self.by_profile.values().flatten().find(|place| place.id == id)
    }

    /// Todas, con el nombre de su terminal (para guardarlas).
    pub fn all(&self) -> impl Iterator<Item = (&str, &SavedPlace)> {
        self.by_profile.iter().flat_map(|(profile, places)| places.iter().map(move |place| (profile.as_str(), place)))
    }

    pub fn add(&mut self, profile_name: &str, details: PlaceDetails) -> PlaceId {
        self.next_id += 1;
        let id = PlaceId(self.next_id);
        let PlaceDetails { alias, target, command } = details;
        self.by_profile.entry(profile_name.to_string()).or_default().push(SavedPlace { id, alias, target, command });
        id
    }

    pub fn update(&mut self, id: PlaceId, details: PlaceDetails) {
        if let Some(place) = self.by_profile.values_mut().flatten().find(|place| place.id == id) {
            place.alias = details.alias;
            place.target = details.target;
            place.command = details.command;
        }
    }

    pub fn remove(&mut self, id: PlaceId) {
        for places in self.by_profile.values_mut() {
            places.retain(|place| place.id != id);
        }
        self.by_profile.retain(|_, places| !places.is_empty());
    }

    /// Sube (`-1`) o baja (`1`) una ruta dentro de las de su terminal.
    pub fn move_by(&mut self, id: PlaceId, steps: isize) {
        for places in self.by_profile.values_mut() {
            if let Some(index) = places.iter().position(|place| place.id == id) {
                let target = index as isize + steps;
                if target >= 0 && (target as usize) < places.len() {
                    places.swap(index, target as usize);
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd() -> ShellProfile {
        ShellProfile::new("cmd", "cmd.exe", &[])
    }

    fn powershell() -> ShellProfile {
        ShellProfile::new("PowerShell", "powershell.exe", &["-NoLogo"])
    }

    fn server() -> SshDestination {
        SshDestination::parse("ana@web.example.com").unwrap()
    }

    fn folder(text: &str) -> RemoteFolder {
        RemoteFolder::parse(text).unwrap().unwrap()
    }

    fn details(alias: Option<&str>, target: PlaceTarget) -> PlaceDetails {
        PlaceDetails { alias: alias.map(str::to_string), target, command: None }
    }

    fn local(folder: &str) -> PlaceTarget {
        PlaceTarget::LocalFolder(folder.into())
    }

    #[test]
    fn without_a_folder_it_is_a_plain_ssh() {
        assert_eq!(ssh_command_line(&cmd(), &server(), None, None, None), "ssh ana@web.example.com");
        assert_eq!(ssh_command_line(&cmd(), &server(), Some(2222), None, None), "ssh -p 2222 ana@web.example.com");
    }

    #[test]
    fn with_a_folder_it_goes_there_and_stays_in_a_shell() {
        assert_eq!(
            ssh_command_line(&cmd(), &server(), None, Some(&folder("/var/www/my site")), None),
            r#"ssh -t ana@web.example.com "cd '/var/www/my site' && exec $SHELL -l""#
        );
    }

    #[test]
    fn powershell_passes_the_remote_command_untouched() {
        assert_eq!(
            ssh_command_line(&powershell(), &server(), Some(22), Some(&folder("/srv")), None),
            r#"ssh --% -p 22 -t ana@web.example.com "cd '/srv' && exec $SHELL -l""#
        );
    }

    #[test]
    fn a_command_runs_on_the_server_and_then_a_shell_stays_open() {
        assert_eq!(
            ssh_command_line(&cmd(), &server(), None, Some(&folder("/srv/app")), Some("git status")),
            r#"ssh -t ana@web.example.com "cd '/srv/app' && git status; exec $SHELL -l""#
        );
        assert_eq!(
            ssh_command_line(&cmd(), &server(), None, None, Some("htop")),
            r#"ssh -t ana@web.example.com "htop; exec $SHELL -l""#
        );
    }

    #[test]
    fn a_remote_command_cannot_break_the_ssh_line_but_a_local_one_can_be_anything() {
        let remote = PlaceFields { is_ssh: true, destination: "ana@web".into(), command: "echo \"hi\"".into(), ..PlaceFields::default() };
        assert_eq!(remote.parse(), Err(PlaceError::UnsafeRemoteCommand));
        let local = PlaceFields { folder: r"C:\code".into(), command: "echo \"%PATH%\"".into(), ..PlaceFields::default() };
        assert_eq!(local.parse().unwrap().command.as_deref(), Some("echo \"%PATH%\""));
    }

    #[test]
    fn the_home_tilde_still_works_and_single_quotes_are_escaped() {
        assert_eq!(folder("~/projects/app").quoted_for_posix_shell(), "~/'projects/app'");
        assert_eq!(folder("~").quoted_for_posix_shell(), "~");
        assert_eq!(folder("/tmp/it's").quoted_for_posix_shell(), r"'/tmp/it'\''s'");
    }

    #[test]
    fn remote_folders_that_cmd_or_powershell_would_change_are_rejected() {
        assert_eq!(RemoteFolder::parse("/tmp/\"x\""), Err(PlaceError::UnsafeRemoteFolder));
        assert_eq!(RemoteFolder::parse("/tmp/%PATH%"), Err(PlaceError::UnsafeRemoteFolder));
        assert_eq!(RemoteFolder::parse("   "), Ok(None));
    }

    #[test]
    fn the_form_checks_what_each_kind_needs() {
        let empty_folder = PlaceFields::default();
        assert_eq!(empty_folder.parse(), Err(PlaceError::EmptyFolder));

        let bad_port = PlaceFields { is_ssh: true, destination: "ana@web".into(), port: "70000".into(), ..PlaceFields::default() };
        assert_eq!(bad_port.parse(), Err(PlaceError::InvalidPort));

        let bad_destination = PlaceFields { is_ssh: true, destination: "ana@web; rm -rf".into(), ..PlaceFields::default() };
        assert_eq!(bad_destination.parse(), Err(PlaceError::InvalidDestination));

        let good = PlaceFields { alias: "  ".into(), folder: r"C:\code".into(), ..PlaceFields::default() };
        assert_eq!(good.parse(), Ok(details(None, local(r"C:\code"))));
    }

    #[test]
    fn the_form_round_trips_a_saved_place() {
        let mut places = SavedPlaces::default();
        let saved = PlaceDetails {
            alias: Some("Web".into()),
            target: PlaceTarget::Ssh { destination: server(), port: Some(2222), remote_folder: Some(folder("/srv")) },
            command: Some("git pull".into()),
        };
        let id = places.add("cmd", saved.clone());
        assert_eq!(PlaceFields::of(places.find(id).unwrap()).parse(), Ok(saved));
    }

    #[test]
    fn labels_prefer_the_alias() {
        let mut places = SavedPlaces::default();
        let plain = places.add("cmd", details(None, local(r"C:\Users\ana\code\")));
        let named = places.add(
            "cmd",
            details(Some("Producción"), PlaceTarget::Ssh { destination: server(), port: None, remote_folder: Some(folder("/srv")) }),
        );
        let drive = places.add("cmd", details(None, local(r"D:\")));

        assert_eq!(places.find(plain).unwrap().label(), r"C:\Users\ana\code\");
        assert_eq!(places.find(plain).unwrap().tab_title(), "code");
        assert_eq!(places.find(named).unwrap().label(), "Producción");
        assert_eq!(places.find(drive).unwrap().tab_title(), r"D:\");
        let unnamed_ssh = SavedPlace {
            id: PlaceId(9),
            alias: None,
            target: PlaceTarget::Ssh { destination: server(), port: Some(2222), remote_folder: Some(folder("/srv")) },
            command: None,
        };
        assert_eq!(unnamed_ssh.label(), "ana@web.example.com:2222:/srv");
    }

    #[test]
    fn each_terminal_keeps_its_own_places_in_order() {
        let mut places = SavedPlaces::default();
        let first = places.add("cmd", details(None, local("A")));
        let second = places.add("cmd", details(None, local("B")));
        places.add("PowerShell", details(None, local("C")));

        places.move_by(second, -1);
        let order: Vec<PlaceId> = places.places_of("cmd").iter().map(|place| place.id).collect();
        assert_eq!(order, vec![second, first]);
        places.move_by(second, -1);
        assert_eq!(places.places_of("cmd")[0].id, second);

        places.remove(first);
        places.remove(second);
        assert!(places.places_of("cmd").is_empty());
        assert_eq!(places.places_of("PowerShell").len(), 1);
    }
}
