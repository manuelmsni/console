//! Script de compilación:
//! - Mete en el ejecutable todos los ficheros de idioma de `locales/` (uno por
//!   idioma, `<código>.json`). Así, para añadir un idioma basta con añadir su
//!   fichero: no hay que tocar el código.
//! - En Windows, le pone al `.exe` el mismo icono que la bandeja, para que se
//!   vea en el Explorador y en los accesos directos.

use std::env;
use std::fs;
use std::path::Path;

#[allow(dead_code)]
#[path = "src/adapters/egui_ui/app_icon.rs"]
mod app_icon;

fn main() {
    embed_locales();
    embed_windows_icon();
}

fn embed_locales() {
    let locales_folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    // Recompilar si se añade, quita o cambia cualquier fichero de idioma.
    println!("cargo:rerun-if-changed={}", locales_folder.display());

    let mut locale_files: Vec<_> = fs::read_dir(&locales_folder)
        .expect("falta la carpeta locales/ con los ficheros de idioma")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
        .collect();
    locale_files.sort();

    let entries: String = locale_files
        .iter()
        .map(|path| {
            let code = path.file_stem().and_then(|stem| stem.to_str()).expect("nombre de fichero de idioma");
            format!("    ({code:?}, include_str!({:?})),\n", path.display().to_string())
        })
        .collect();
    let generated = format!(
        "/// Cada idioma disponible: su código y el contenido de su fichero.\n\
         pub const EMBEDDED_LOCALES: &[(&str, &str)] = &[\n{entries}];\n"
    );

    let output = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("locales.rs");
    fs::write(output, generated).expect("no se pudo escribir la lista de idiomas");
}

/// Lados del icono del `.exe`: Windows elige el que mejor encaja en cada sitio.
const EXECUTABLE_ICON_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

/// Escribe un fichero de recursos (`.res`) con el icono y se lo pasa al
/// enlazador de Microsoft, que los acepta tal cual: no hace falta `rc.exe`.
fn embed_windows_icon() {
    let icon_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/adapters/egui_ui/app_icon.rs");
    println!("cargo:rerun-if-changed={}", icon_source.display());
    let builds_for_windows = env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let links_with_msvc = env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|target_env| target_env == "msvc");
    if !builds_for_windows || !links_with_msvc {
        return;
    }
    let resources = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("app_icon.res");
    fs::write(&resources, icon_resources()).expect("no se pudo escribir el icono del ejecutable");
    println!("cargo:rustc-link-arg-bins={}", resources.display());
}

/// Tipos de recurso de Windows.
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
/// Inglés (EE. UU.), el idioma que usa `rc.exe` por defecto.
const LANGUAGE_EN_US: u16 = 0x0409;
/// Identificador del grupo de iconos: el Explorador usa el primero.
const ICON_GROUP_ID: u16 = 1;

/// Un `.res`: una entrada vacía (marca del formato), una por imagen y el grupo
/// que las reúne.
fn icon_resources() -> Vec<u8> {
    let mut resources = resource_entry(0, 0, 0, 0, &[]);
    let mut group = Vec::new();
    group.extend(0u16.to_le_bytes()); // reservado
    group.extend(1u16.to_le_bytes()); // 1 = icono
    group.extend((EXECUTABLE_ICON_SIZES.len() as u16).to_le_bytes());
    for (index, &size) in EXECUTABLE_ICON_SIZES.iter().enumerate() {
        let image_id = index as u16 + 1;
        let image = icon_image(size);
        resources.extend(resource_entry(RT_ICON, image_id, 0x1010, LANGUAGE_EN_US, &image));
        let side_byte = if size >= 256 { 0 } else { size as u8 }; // 0 significa 256
        group.extend([side_byte, side_byte, 0, 0]); // ancho, alto, colores, reservado
        group.extend(1u16.to_le_bytes()); // planos
        group.extend(32u16.to_le_bytes()); // bits por píxel
        group.extend((image.len() as u32).to_le_bytes());
        group.extend(image_id.to_le_bytes());
    }
    resources.extend(resource_entry(RT_GROUP_ICON, ICON_GROUP_ID, 0x1030, LANGUAGE_EN_US, &group));
    resources
}

/// Cabecera de un recurso con tipo y nombre numéricos, seguida de sus datos
/// (rellenados hasta múltiplo de 4 bytes).
fn resource_entry(resource_type: u16, name: u16, memory_flags: u16, language: u16, data: &[u8]) -> Vec<u8> {
    const HEADER_SIZE: u32 = 32;
    let mut entry = Vec::with_capacity(HEADER_SIZE as usize + data.len() + 3);
    entry.extend((data.len() as u32).to_le_bytes());
    entry.extend(HEADER_SIZE.to_le_bytes());
    entry.extend([0xFFFF, resource_type, 0xFFFF, name].iter().flat_map(|word: &u16| word.to_le_bytes()));
    entry.extend(0u32.to_le_bytes()); // versión de los datos
    entry.extend(memory_flags.to_le_bytes());
    entry.extend(language.to_le_bytes());
    entry.extend(0u32.to_le_bytes()); // versión
    entry.extend(0u32.to_le_bytes()); // características
    entry.extend(data);
    entry.resize(entry.len().next_multiple_of(4), 0);
    entry
}

/// Una imagen del icono en formato DIB de 32 bits: cabecera, píxeles BGRA de
/// abajo arriba y la máscara de 1 bit (a cero: la transparencia va en el alfa).
fn icon_image(size: u32) -> Vec<u8> {
    let rgba = app_icon::app_icon_rgba(size);
    let mask_row_bytes = (size as usize).div_ceil(32) * 4;
    let mut image = Vec::new();
    image.extend(40u32.to_le_bytes()); // tamaño de la cabecera
    image.extend((size as i32).to_le_bytes());
    image.extend((size as i32 * 2).to_le_bytes()); // alto de los píxeles más la máscara
    image.extend(1u16.to_le_bytes()); // planos
    image.extend(32u16.to_le_bytes()); // bits por píxel
    image.extend([0u8; 24]); // sin compresión; el resto de campos, a cero
    for row in rgba.chunks_exact(size as usize * 4).rev() {
        for pixel in row.chunks_exact(4) {
            image.extend([pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    image.resize(image.len() + mask_row_bytes * size as usize, 0);
    image
}
