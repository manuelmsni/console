//! Guarda la barra de macros en `macros.json`, en la carpeta de datos (JSON
//! cifrado; ver `data_folder`).

use std::fs;

use serde::{Deserialize, Serialize};

use super::data_folder::{DataFile, DataFolder};
use crate::application::ports::MacroBarStore;
use crate::domain::macro_bar::{FolderFields, MacroBar, MacroBarItem, MacroError, MacroFields, MacroLocation};
use crate::i18n::tr_args;

pub struct JsonMacroBarStore {
    file: DataFile,
}

impl JsonMacroBarStore {
    pub fn in_folder(data: &DataFolder) -> Self {
        Self { file: data.file("macros.json") }
    }

    /// Si el fichero no se puede leer, se aparta para no sobrescribirlo y que
    /// el usuario pueda recuperarlo.
    fn set_aside_unreadable_file(&self) -> String {
        let backup_path = self.file.path().with_extension("json.unreadable");
        match fs::rename(self.file.path(), &backup_path) {
            Ok(()) => tr_args("storage.copy_saved_at", &[("path", &backup_path.display())]),
            Err(_) => String::new(),
        }
    }
}

/// Formato del fichero, separado del dominio para que este pueda cambiar sin
/// romper los ficheros ya guardados.
#[derive(Serialize, Deserialize)]
struct StoredMacroBar {
    version: u32,
    items: Vec<StoredItem>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StoredItem {
    Macro {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        command: String,
    },
    Folder {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        children: Vec<StoredItem>,
    },
}

impl MacroBarStore for JsonMacroBarStore {
    fn load_macro_bar(&self) -> Result<Option<MacroBar>, String> {
        let Some(json) = self.file.read_if_exists()? else { return Ok(None) };
        let bar = serde_json::from_str::<StoredMacroBar>(&json)
            .map_err(|error| error.to_string())
            .and_then(|stored| bar_from_stored(stored.items).map_err(|error| error.to_string()));
        match bar {
            Ok(bar) => Ok(Some(bar)),
            Err(error) => {
                let backup_note = self.set_aside_unreadable_file();
                Err(tr_args("storage.error.macros_damaged", &[("error", &error), ("backup_note", &backup_note)]))
            }
        }
    }

    fn save_macro_bar(&self, bar: &MacroBar) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&stored_from(bar)).map_err(|error| error.to_string())?;
        self.file.write_atomically(&json)
    }
}

fn stored_from(bar: &MacroBar) -> StoredMacroBar {
    StoredMacroBar { version: 1, items: bar.items().iter().map(stored_item_from).collect() }
}

fn stored_item_from(item: &MacroBarItem) -> StoredItem {
    match item {
        MacroBarItem::Macro(item) => {
            StoredItem::Macro { name: item.name.clone(), icon: item.icon.clone(), command: item.command.clone() }
        }
        MacroBarItem::Folder(folder) => StoredItem::Folder {
            name: folder.name.clone(),
            icon: folder.icon.clone(),
            children: folder.children.iter().map(stored_item_from).collect(),
        },
    }
}

fn bar_from_stored(items: Vec<StoredItem>) -> Result<MacroBar, MacroError> {
    let mut bar = MacroBar::empty();
    add_stored_items(&mut bar, MacroLocation::Bar, items)?;
    Ok(bar)
}

fn add_stored_items(bar: &mut MacroBar, location: MacroLocation, items: Vec<StoredItem>) -> Result<(), MacroError> {
    for item in items {
        match item {
            StoredItem::Macro { name, icon, command } => {
                bar.create_macro(location, MacroFields { name, icon, command })?;
            }
            StoredItem::Folder { name, icon, children } => {
                let folder = bar.create_folder(location, FolderFields { name, icon })?;
                add_stored_items(bar, MacroLocation::InsideFolder(folder), children)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_icons_and_order_survive_saving_and_loading() {
        let mut bar = MacroBar::empty();
        let folder = bar.create_folder(MacroLocation::Bar, FolderFields { name: "Webs".into(), icon: Some("public".into()) }).unwrap();
        let inside = MacroFields { name: String::new(), icon: Some("code".into()), command: "start https://docs.rs".into() };
        bar.create_macro(MacroLocation::InsideFolder(folder), inside).unwrap();
        let outside = MacroFields { name: "Notas".into(), icon: None, command: "notepad".into() };
        bar.create_macro(MacroLocation::Bar, outside).unwrap();

        let json = serde_json::to_string(&stored_from(&bar)).unwrap();
        let loaded = bar_from_stored(serde_json::from_str::<StoredMacroBar>(&json).unwrap().items).unwrap();

        assert_eq!(loaded, bar);
    }
}
