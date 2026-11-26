//! "Abrir Console aquí" en el menú del botón derecho del Explorador, como el
//! "Git Bash Here": en el fondo de una carpeta y sobre una carpeta. Se apunta
//! en `HKEY_CURRENT_USER\Software\Classes`, así que es solo para este usuario
//! y no pide permiso de administrador. En Windows 11 sale en "Mostrar más opciones".

use std::path::PathBuf;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegGetValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
    REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
};

use crate::application::ports::FolderContextMenu;
use crate::domain::startup::OPEN_FOLDER_ARGUMENT;
use crate::i18n::tr_args;

/// Fondo de una carpeta abierta (también la raíz de una unidad) y una carpeta
/// vista desde fuera. `%V` es la carpeta en los dos casos.
const MENU_LOCATIONS: [&str; 2] = [r"Software\Classes\Directory\Background\shell", r"Software\Classes\Directory\shell"];

pub struct ExplorerFolderMenu {
    /// Nombre de la entrada en el registro: la instalada y la portable, por separado.
    entry_name: &'static str,
    program: PathBuf,
}

impl ExplorerFolderMenu {
    /// `None` si no se sabe dónde está el programa.
    pub fn for_this_program() -> Option<Self> {
        let entry_name = if cfg!(feature = "portable") { "ConsolePortableHere" } else { "ConsoleHere" };
        Some(Self { entry_name, program: std::env::current_exe().ok()? })
    }

    fn command_line(&self) -> String {
        format!("\"{}\" {OPEN_FOLDER_ARGUMENT} \"%V\"", self.program.display())
    }

    fn entry_key(&self, location: &str) -> String {
        format!(r"{location}\{}", self.entry_name)
    }
}

impl FolderContextMenu for ExplorerFolderMenu {
    fn is_enabled(&self) -> bool {
        MENU_LOCATIONS.iter().all(|location| {
            read_default_value(&format!(r"{}\command", self.entry_key(location))).is_some_and(|command| command == self.command_line())
        })
    }

    fn enable(&self, label: &str) -> Result<(), String> {
        let icon = format!("\"{}\",0", self.program.display());
        for location in MENU_LOCATIONS {
            let entry = self.entry_key(location);
            write_value(&entry, None, label)?;
            write_value(&entry, Some("Icon"), &icon)?;
            write_value(&format!(r"{entry}\command"), None, &self.command_line())?;
        }
        Ok(())
    }

    fn disable(&self) -> Result<(), String> {
        for location in MENU_LOCATIONS {
            delete_key_and_contents(&self.entry_key(location))?;
        }
        Ok(())
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn registry_error(code: u32) -> String {
    tr_args("explorer_menu.error", &[("error", &std::io::Error::from_raw_os_error(code as i32))])
}

/// El valor (de serie o con nombre) de una clave de `HKEY_CURRENT_USER`; la crea si no existe.
fn write_value(key_path: &str, value_name: Option<&str>, value: &str) -> Result<(), String> {
    let key_path = wide(key_path);
    let value_name = value_name.map(wide);
    let value = wide(value);
    // Seguro: las cadenas terminan en 0 y viven durante las llamadas; la clave se cierra.
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let result = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key_path.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if result != ERROR_SUCCESS {
            return Err(registry_error(result));
        }
        let name = value_name.as_ref().map_or(std::ptr::null(), |name| name.as_ptr());
        let result = RegSetValueExW(key, name, 0, REG_SZ, value.as_ptr().cast(), (value.len() * 2) as u32);
        RegCloseKey(key);
        if result != ERROR_SUCCESS {
            return Err(registry_error(result));
        }
    }
    Ok(())
}

fn read_default_value(key_path: &str) -> Option<String> {
    let key_path = wide(key_path);
    let mut buffer = vec![0u16; 4096];
    let mut size_in_bytes = (buffer.len() * 2) as u32;
    // Seguro: el búfer mide lo que se le dice a Windows.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key_path.as_ptr(),
            std::ptr::null(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size_in_bytes,
        )
    };
    if result != ERROR_SUCCESS {
        return None;
    }
    let length = (size_in_bytes as usize / 2).min(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..length]);
    Some(text.trim_end_matches('\0').to_string())
}

fn delete_key_and_contents(key_path: &str) -> Result<(), String> {
    let key_path = wide(key_path);
    // Seguro: la ruta termina en 0.
    let result = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key_path.as_ptr()) };
    match result {
        ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
        other => Err(registry_error(other)),
    }
}
