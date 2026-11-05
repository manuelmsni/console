//! Script de compilación: mete en el ejecutable todos los ficheros de idioma
//! de `locales/` (uno por idioma, `<código>.json`). Así, para añadir un idioma
//! basta con añadir su fichero: no hay que tocar el código.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
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
