//! Guarda la biblioteca de comandos en `commands.json`, en la carpeta de datos
//! (JSON cifrado; ver `data_folder`).

use std::fs;

use serde::{Deserialize, Serialize};

use super::data_folder::{DataFile, DataFolder};
use crate::application::ports::CommandLibraryStore;
use crate::domain::command_library::{CommandLibrary, LibraryError, LibraryNode, TreeLocation};
use crate::i18n::tr_args;

pub struct JsonCommandLibraryStore {
    file: DataFile,
}

impl JsonCommandLibraryStore {
    pub fn in_folder(data: &DataFolder) -> Self {
        Self { file: data.file("commands.json") }
    }

    /// Si el fichero no se puede leer, se aparta para no sobrescribirlo
    /// y que el usuario pueda recuperarlo.
    fn set_aside_unreadable_file(&self) -> String {
        let backup_path = self.file.path().with_extension("json.unreadable");
        match fs::rename(self.file.path(), &backup_path) {
            Ok(()) => tr_args("storage.copy_saved_at", &[("path", &backup_path.display())]),
            Err(_) => String::new(),
        }
    }
}

/// Formato del fichero. Va separado del dominio para que el dominio pueda
/// cambiar sin romper los ficheros ya guardados.
#[derive(Serialize, Deserialize)]
struct StoredLibrary {
    version: u32,
    nodes: Vec<StoredNode>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StoredNode {
    Folder {
        name: String,
        /// Los ficheros de antes no lo tienen: entonces se abre desplegada.
        #[serde(default = "folder_is_open_by_default")]
        is_open: bool,
        children: Vec<StoredNode>,
    },
    Command { name: String, command: String },
}

impl CommandLibraryStore for JsonCommandLibraryStore {
    fn load_library(&self) -> Result<Option<CommandLibrary>, String> {
        let Some(json) = self.file.read_if_exists()? else { return Ok(None) };

        let library = serde_json::from_str::<StoredLibrary>(&json)
            .map_err(|error| error.to_string())
            .and_then(|stored| library_from_stored(stored.nodes).map_err(|error| error.to_string()));

        match library {
            Ok(library) => Ok(Some(library)),
            Err(error) => {
                let backup_note = self.set_aside_unreadable_file();
                Err(tr_args("storage.error.commands_damaged", &[("error", &error), ("backup_note", &backup_note)]))
            }
        }
    }

    fn save_library(&self, library: &CommandLibrary) -> Result<(), String> {
        let stored = StoredLibrary { version: 1, nodes: library.root_nodes().iter().map(stored_node_from).collect() };
        let json = serde_json::to_string_pretty(&stored).map_err(|error| error.to_string())?;
        self.file.write_atomically(&json)
    }
}

fn folder_is_open_by_default() -> bool {
    true
}

fn stored_node_from(node: &LibraryNode) -> StoredNode {
    match node {
        LibraryNode::Folder(folder) => StoredNode::Folder {
            name: folder.name.clone(),
            is_open: folder.is_open,
            children: folder.children.iter().map(stored_node_from).collect(),
        },
        LibraryNode::Command(command) => {
            StoredNode::Command { name: command.name.clone(), command: command.command_text.clone() }
        }
    }
}

fn library_from_stored(nodes: Vec<StoredNode>) -> Result<CommandLibrary, LibraryError> {
    let mut library = CommandLibrary::empty();
    add_stored_nodes(&mut library, TreeLocation::Root, nodes)?;
    Ok(library)
}

fn add_stored_nodes(library: &mut CommandLibrary, location: TreeLocation, nodes: Vec<StoredNode>) -> Result<(), LibraryError> {
    for node in nodes {
        match node {
            StoredNode::Folder { name, is_open, children } => {
                let folder_id = library.create_folder(location, &name)?;
                library.set_folder_open(folder_id, is_open)?;
                add_stored_nodes(library, TreeLocation::InsideFolder(folder_id), children)?;
            }
            StoredNode::Command { name, command } => {
                library.create_command(location, &name, &command)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folded_folders_stay_folded_after_saving_and_loading() {
        let mut library = CommandLibrary::empty();
        let folder = library.create_folder(TreeLocation::Root, "Servidores").unwrap();
        library.set_folder_open(folder, false).unwrap();

        let stored = StoredLibrary { version: 1, nodes: library.root_nodes().iter().map(stored_node_from).collect() };
        let json = serde_json::to_string(&stored).unwrap();
        let loaded = library_from_stored(serde_json::from_str::<StoredLibrary>(&json).unwrap().nodes).unwrap();

        assert!(matches!(&loaded.root_nodes()[0], LibraryNode::Folder(folder) if !folder.is_open));
    }

    #[test]
    fn folders_saved_before_this_existed_open_unfolded() {
        let json = r#"{"version":1,"nodes":[{"type":"folder","name":"Ejemplos","children":[]}]}"#;
        let loaded = library_from_stored(serde_json::from_str::<StoredLibrary>(json).unwrap().nodes).unwrap();
        assert!(matches!(&loaded.root_nodes()[0], LibraryNode::Folder(folder) if folder.is_open));
    }
}