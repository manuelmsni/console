use super::ports::CommandLibraryStore;
use crate::domain::command_library::{CommandLibrary, LibraryError, LibraryNodeId, SiblingSide, TreeLocation};
use crate::i18n::tr;

/// Casos de uso de la biblioteca de comandos. Cada cambio se guarda al momento.
pub struct CommandLibraryService {
    library: CommandLibrary,
    store: Box<dyn CommandLibraryStore>,
}

impl CommandLibraryService {
    /// Carga la biblioteca guardada. Devuelve también un aviso si algo fue mal,
    /// para que la interfaz pueda mostrarlo.
    pub fn load(store: Box<dyn CommandLibraryStore>) -> (Self, Option<String>) {
        let (library, warning) = match store.load_library() {
            Ok(Some(library)) => (library, None),
            Ok(None) => (example_library(), None),
            Err(error) => (CommandLibrary::empty(), Some(error)),
        };
        (Self { library, store }, warning)
    }

    pub fn library(&self) -> &CommandLibrary {
        &self.library
    }

    pub fn create_folder(&mut self, location: TreeLocation, name: &str) -> Result<(), String> {
        self.change_and_save(|library| library.create_folder(location, name).map(|_| ()))
    }

    pub fn create_command(&mut self, location: TreeLocation, name: &str, command_text: &str) -> Result<(), String> {
        self.change_and_save(|library| library.create_command(location, name, command_text).map(|_| ()))
    }

    pub fn rename_node(&mut self, id: LibraryNodeId, new_name: &str) -> Result<(), String> {
        self.change_and_save(|library| library.rename_node(id, new_name))
    }

    /// Plegar o desplegar una carpeta también se guarda: así sigue igual la
    /// próxima vez que se abra la aplicación.
    pub fn set_folder_open(&mut self, id: LibraryNodeId, is_open: bool) -> Result<(), String> {
        self.change_and_save(|library| library.set_folder_open(id, is_open))
    }

    pub fn update_command(&mut self, id: LibraryNodeId, name: &str, command_text: &str) -> Result<(), String> {
        self.change_and_save(|library| {
            library.rename_node(id, name)?;
            library.change_command_text(id, command_text)
        })
    }

    pub fn remove_node(&mut self, id: LibraryNodeId) -> Result<(), String> {
        self.change_and_save(|library| library.remove_node(id).map(|_| ()))
    }

    pub fn move_node(&mut self, id: LibraryNodeId, destination: TreeLocation) -> Result<(), String> {
        self.change_and_save(|library| library.move_node(id, destination))
    }

    pub fn move_node_beside(&mut self, id: LibraryNodeId, sibling: LibraryNodeId, side: SiblingSide) -> Result<(), String> {
        self.change_and_save(|library| library.move_node_beside(id, sibling, side))
    }

    /// Aplica el cambio sobre una copia; solo si sale bien se queda y se guarda.
    fn change_and_save(
        &mut self,
        change: impl FnOnce(&mut CommandLibrary) -> Result<(), LibraryError>,
    ) -> Result<(), String> {
        let mut changed_library = self.library.clone();
        change(&mut changed_library).map_err(|error| error.to_string())?;
        self.library = changed_library;
        self.store.save_library(&self.library)
    }
}

/// Lo que se ve la primera vez, para que se entienda cómo funciona.
fn example_library() -> CommandLibrary {
    let mut library = CommandLibrary::empty();
    let examples = TreeLocation::InsideFolder(
        library.create_folder(TreeLocation::Root, &tr("examples.folder")).expect("nombre de ejemplo válido"),
    );
    let commands = [(tr("examples.claude_code"), "claude"), (tr("examples.list_files"), "dir"), (tr("examples.clear_screen"), "cls")];
    for (name, command_text) in commands {
        library.create_command(examples, &name, command_text).expect("comando de ejemplo válido");
    }
    library
}
