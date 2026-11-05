//! La carpeta donde la aplicación guarda todo (comandos, ajustes, pestañas...)
//! y el cifrado con que lo guarda:
//!
//! - Instalada: la carpeta de configuración del usuario (`%APPDATA%\console`
//!   en Windows), cifrado con la cuenta del sistema.
//! - Portable: la carpeta `data` junto al programa, cifrado con contraseña.
//!
//! Los nombres de ficheros y carpetas van en inglés, para que no dependan del
//! idioma. Las versiones anteriores los tenían en español: al arrancar se
//! renombran (ver `rename_files_of_older_versions`).

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::file_cipher::FileCipher;
use crate::i18n::tr_args;

/// Nombre de la carpeta de datos de la versión portable, junto al programa.
#[cfg_attr(not(feature = "portable"), allow(dead_code))]
const PORTABLE_DATA_FOLDER: &str = "data";
/// Nombre de la carpeta de datos de la versión instalada, en la del usuario.
const USER_DATA_FOLDER: &str = "console";
/// Cómo se llamaba en versiones anteriores, cuando el programa era `consola`.
#[cfg_attr(feature = "portable", allow(dead_code))]
const OLDER_USER_DATA_FOLDER: &str = "consola";
/// Cómo se llamaba en versiones anteriores.
#[cfg_attr(not(feature = "portable"), allow(dead_code))]
const OLDER_PORTABLE_DATA_FOLDER: &str = "datos";

/// Nombres de versiones anteriores (en español) y los de ahora, dentro de la carpeta de datos.
const OLDER_NAMES: [(&str, &str); 5] = [
    ("comandos.json", "commands.json"),
    ("ajustes.json", "settings.json"),
    ("servidores_ssh.json", "ssh_servers.json"),
    ("clave.json", "key.json"),
    ("pestanas", "tabs"),
];
/// Lo mismo dentro de la carpeta de pestañas.
const OLDER_TAB_INDEX: (&str, &str) = ("indice.json", "index.json");
const OLDER_JOURNAL_EXTENSION: (&str, &str) = ("diario", "journal");

#[derive(Clone)]
pub struct DataFolder {
    path: PathBuf,
    cipher: Arc<dyn FileCipher>,
}

impl DataFolder {
    pub fn new(path: PathBuf, cipher: Arc<dyn FileCipher>) -> Self {
        Self { path, cipher }
    }

    /// `%APPDATA%\console` en Windows; `~/.config/console` en Linux.
    #[cfg_attr(feature = "portable", allow(dead_code))]
    pub fn user_config_location() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join(USER_DATA_FOLDER)
    }

    /// La carpeta `data` junto al ejecutable.
    #[cfg_attr(not(feature = "portable"), allow(dead_code))]
    pub fn next_to_program_location() -> PathBuf {
        let program_folder = std::env::current_exe().ok().and_then(|program| program.parent().map(Path::to_path_buf));
        program_folder.unwrap_or_else(|| PathBuf::from(".")).join(PORTABLE_DATA_FOLDER)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn cipher(&self) -> &dyn FileCipher {
        self.cipher.as_ref()
    }

    pub fn subfolder(&self, name: &str) -> DataFolder {
        Self { path: self.path.join(name), cipher: self.cipher.clone() }
    }

    pub fn file(&self, name: &str) -> DataFile {
        DataFile { path: self.path.join(name), cipher: self.cipher.clone() }
    }
}

/// Un fichero de texto (JSON) cifrado dentro de la carpeta de datos.
pub struct DataFile {
    path: PathBuf,
    cipher: Arc<dyn FileCipher>,
}

impl DataFile {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `Ok(None)` si el fichero todavía no existe. Un fichero antiguo sin cifrar
    /// se lee igual y se vuelve a guardar cifrado en ese mismo momento.
    pub fn read_if_exists(&self) -> Result<Option<String>, String> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(tr_args("storage.error.read", &[("path", &self.path.display()), ("error", &error)])),
        };

        let contents = match bytes.strip_prefix(self.cipher.file_mark()) {
            Some(encrypted) => self.text_from(self.cipher.decrypt(encrypted)?)?,
            None => {
                let contents = self.text_from(bytes)?;
                self.write_atomically(&contents)?;
                contents
            }
        };
        Ok(Some(contents))
    }

    /// Cifra y escribe primero en un temporal que luego se renombra: así, si algo
    /// falla, el fichero anterior queda intacto en vez de a medias.
    pub fn write_atomically(&self, contents: &str) -> Result<(), String> {
        if let Some(folder) = self.path.parent() {
            fs::create_dir_all(folder)
                .map_err(|error| tr_args("storage.error.create_folder", &[("path", &folder.display()), ("error", &error)]))?;
        }
        let mut file_bytes = self.cipher.file_mark().to_vec();
        file_bytes.extend(self.cipher.encrypt(contents.as_bytes())?);

        let temporary_path = self.path.with_extension("tmp");
        fs::write(&temporary_path, file_bytes)
            .and_then(|_| fs::rename(&temporary_path, &self.path))
            .map_err(|error| tr_args("storage.error.save", &[("path", &self.path.display()), ("error", &error)]))
    }

    fn text_from(&self, bytes: Vec<u8>) -> Result<String, String> {
        String::from_utf8(bytes).map_err(|_| tr_args("storage.error.not_text", &[("path", &self.path.display())]))
    }
}

pub fn remove_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(tr_args("storage.error.delete", &[("path", &path.display()), ("error", &error)])),
    }
}

/// La carpeta de la versión instalada se llamaba `consola`: si la nueva aún
/// no existe, se renombra la antigua.
#[cfg_attr(feature = "portable", allow(dead_code))]
pub fn rename_user_folder_of_older_versions(data_location: &Path) {
    rename_if_only_old_exists(&data_location.with_file_name(OLDER_USER_DATA_FOLDER), data_location);
}

/// La carpeta portable se llamaba `datos`: si la nueva aún no existe, se
/// renombra la antigua.
#[cfg_attr(not(feature = "portable"), allow(dead_code))]
pub fn rename_portable_folder_of_older_versions(data_location: &Path) {
    rename_if_only_old_exists(&data_location.with_file_name(OLDER_PORTABLE_DATA_FOLDER), data_location);
}

/// Renombra a los nombres de ahora lo que se guardó con los de versiones
/// anteriores. Solo si el nombre nuevo no existe todavía: nunca se pisa nada.
pub fn rename_files_of_older_versions(data_location: &Path) {
    for (old_name, new_name) in OLDER_NAMES {
        rename_if_only_old_exists(&data_location.join(old_name), &data_location.join(new_name));
    }
    let tabs_folder = data_location.join("tabs");
    let (old_index, new_index) = OLDER_TAB_INDEX;
    rename_if_only_old_exists(&tabs_folder.join(old_index), &tabs_folder.join(new_index));
    let Ok(entries) = fs::read_dir(&tabs_folder) else { return };
    let (old_extension, new_extension) = OLDER_JOURNAL_EXTENSION;
    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        if path.extension().is_some_and(|extension| extension == old_extension) {
            rename_if_only_old_exists(&path, &path.with_extension(new_extension));
        }
    }
}

fn rename_if_only_old_exists(old: &Path, new: &Path) {
    if old.exists() && !new.exists() {
        // Si falla, todo sigue con el nombre antiguo y no se pierde nada.
        let _ = fs::rename(old, new);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_test_folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("console-tests-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn files_saved_by_older_versions_get_the_new_names() {
        let folder = empty_test_folder("nombres");
        fs::write(folder.join("comandos.json"), "mis comandos").unwrap();
        fs::create_dir_all(folder.join("pestanas")).unwrap();
        fs::write(folder.join("pestanas").join("indice.json"), "índice").unwrap();
        fs::write(folder.join("pestanas").join("7.diario"), "diario").unwrap();

        rename_files_of_older_versions(&folder);

        assert_eq!(fs::read_to_string(folder.join("commands.json")).unwrap(), "mis comandos");
        assert_eq!(fs::read_to_string(folder.join("tabs").join("index.json")).unwrap(), "índice");
        assert_eq!(fs::read_to_string(folder.join("tabs").join("7.journal")).unwrap(), "diario");
        assert!(!folder.join("comandos.json").exists() && !folder.join("pestanas").exists());
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn an_existing_new_file_is_never_overwritten() {
        let folder = empty_test_folder("sin-pisar");
        fs::write(folder.join("ajustes.json"), "antiguo").unwrap();
        fs::write(folder.join("settings.json"), "nuevo").unwrap();

        rename_files_of_older_versions(&folder);

        assert_eq!(fs::read_to_string(folder.join("settings.json")).unwrap(), "nuevo");
        assert!(folder.join("ajustes.json").exists());
        let _ = fs::remove_dir_all(folder);
    }
}
