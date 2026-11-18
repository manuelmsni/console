//! La ConPTY moderna de Microsoft (`conpty.dll` + `OpenConsole.exe`, de
//! `vendor/conpty/`), metida dentro del programa.
//!
//! La ConPTY que trae Windows 10 no reenvía lo que escribe el programa: mira
//! cómo queda su pantalla interna y manda una foto. Si entre dos fotos el
//! programa escribe más líneas de las que caben (Claude Code lo hace a menudo),
//! las de en medio no llegan nunca y no pueden ir al historial. La moderna pasa
//! el texto tal cual.
//!
//! `portable-pty` usa una `conpty.dll` si ya hay una con ese nombre cargada (o
//! junto al programa); si no, la del sistema. Aquí se extraen los dos ficheros
//! a una carpeta y se carga la DLL antes de abrir ninguna pestaña. Si algo
//! falla, se sigue con la del sistema, como antes.

use std::fs;
use std::path::Path;

/// Versión del paquete `Microsoft.Windows.Console.ConPTY`. Cada versión se
/// extrae en su propia subcarpeta: una versión nueva nunca tiene que
/// sobrescribir ficheros que otra consola pueda estar usando.
const BUNDLED_CONPTY_VERSION: &str = "1.25.260930003";

#[cfg(target_arch = "x86_64")]
const BUNDLED_FILES: &[(&str, &[u8])] = &[
    // `conpty.dll` arranca el `OpenConsole.exe` que está en su misma carpeta.
    ("OpenConsole.exe", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/conpty/x64/OpenConsole.exe"))),
    ("conpty.dll", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/conpty/x64/conpty.dll"))),
];

/// Para otras arquitecturas no se incluye: se usa la ConPTY del sistema.
#[cfg(not(target_arch = "x86_64"))]
const BUNDLED_FILES: &[(&str, &[u8])] = &[];

/// Deja la ConPTY moderna en `folder` y la carga para que la usen todas las
/// pestañas. Hay que llamarla antes de abrir la primera.
pub fn use_bundled_conpty(folder: &Path) -> Result<(), String> {
    if BUNDLED_FILES.is_empty() {
        return Ok(());
    }
    let version_folder = folder.join(BUNDLED_CONPTY_VERSION);
    fs::create_dir_all(&version_folder).map_err(|error| error.to_string())?;
    for (name, contents) in BUNDLED_FILES {
        write_if_different(&version_folder.join(name), contents)?;
    }
    remove_other_versions(folder);
    load_library(&version_folder.join("conpty.dll"))
}

/// Si ya está igual (lo normal a partir de la segunda vez), no se toca: puede
/// estar en uso por otra consola abierta.
fn write_if_different(path: &Path, contents: &[u8]) -> Result<(), String> {
    if fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }
    // Se escribe aparte y se renombra, para no dejar nunca un fichero a medias.
    let partial = path.with_extension("partial");
    fs::write(&partial, contents).map_err(|error| error.to_string())?;
    fs::rename(&partial, path).map_err(|error| error.to_string())
}

/// Las carpetas de versiones anteriores ya no se usan. Si alguna está en uso
/// (otra consola abierta con la versión vieja), se borrará otro día.
fn remove_other_versions(folder: &Path) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    for entry in entries.filter_map(Result::ok) {
        if entry.file_name() != BUNDLED_CONPTY_VERSION && entry.path().is_dir() {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn load_library(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::LibraryLoader::LoadLibraryW;

    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // Seguro: la ruta acaba en 0. La DLL se queda cargada mientras dure el programa.
    let module = unsafe { LoadLibraryW(wide_path.as_ptr()) };
    if module.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}
