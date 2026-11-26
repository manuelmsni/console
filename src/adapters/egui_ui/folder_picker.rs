//! El cuadro de Windows para elegir una carpeta.

use std::path::Path;

use crate::i18n::tr;

/// Abre el cuadro empezando en `start` (si existe) y espera a que se elija
/// una carpeta. `None` si se cancela.
pub(super) fn pick_folder(start: &str) -> Option<String> {
    let mut dialog = rfd::FileDialog::new().set_title(tr("folder_picker.title"));
    let start = Path::new(start.trim());
    if start.is_dir() {
        dialog = dialog.set_directory(start);
    } else if let Some(home) = dirs::home_dir() {
        dialog = dialog.set_directory(home);
    }
    dialog.pick_folder().map(|folder| folder.display().to_string())
}
