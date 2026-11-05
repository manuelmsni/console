//! La biblioteca de comandos guardados: un árbol de carpetas y comandos.

use std::fmt;

use crate::i18n::tr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LibraryNodeId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryNode {
    Folder(CommandFolder),
    Command(SavedCommand),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandFolder {
    pub id: LibraryNodeId,
    pub name: String,
    /// Desplegada (se ven sus hijos) o plegada. Se guarda con el árbol, para
    /// que al volver a abrir la aplicación esté como se dejó.
    pub is_open: bool,
    pub children: Vec<LibraryNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedCommand {
    pub id: LibraryNodeId,
    pub name: String,
    pub command_text: String,
}

impl LibraryNode {
    pub fn id(&self) -> LibraryNodeId {
        match self {
            LibraryNode::Folder(folder) => folder.id,
            LibraryNode::Command(command) => command.id,
        }
    }

    fn name_mut(&mut self) -> &mut String {
        match self {
            LibraryNode::Folder(folder) => &mut folder.name,
            LibraryNode::Command(command) => &mut command.name,
        }
    }
}

/// Dónde va un elemento dentro del árbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeLocation {
    Root,
    InsideFolder(LibraryNodeId),
}

/// A qué lado de otro elemento se coloca uno al reordenar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiblingSide {
    Before,
    After,
}

/// Una carpeta a la que se puede mover algo, con su ruta legible ("Trabajo / SSH").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderDestination {
    pub location: TreeLocation,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryError {
    NodeNotFound,
    NotAFolder,
    NotACommand,
    EmptyName,
    FolderCannotGoInsideItself,
}

impl fmt::Display for LibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let message = match self {
            LibraryError::NodeNotFound => tr("library.error.not_found"),
            LibraryError::NotAFolder => tr("library.error.not_a_folder"),
            LibraryError::NotACommand => tr("library.error.not_a_command"),
            LibraryError::EmptyName => tr("library.error.empty_name"),
            LibraryError::FolderCannotGoInsideItself => tr("library.error.folder_inside_itself"),
        };
        formatter.write_str(&message)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandLibrary {
    root_nodes: Vec<LibraryNode>,
    next_free_id: u64,
}

impl CommandLibrary {
    pub fn empty() -> Self {
        Self { root_nodes: Vec::new(), next_free_id: 1 }
    }

    pub fn root_nodes(&self) -> &[LibraryNode] {
        &self.root_nodes
    }

    pub fn create_folder(&mut self, location: TreeLocation, name: &str) -> Result<LibraryNodeId, LibraryError> {
        let name = required_name(name)?;
        let id = self.take_next_free_id();
        let folder = CommandFolder { id, name, is_open: true, children: Vec::new() };
        self.children_at_mut(location)?.push(LibraryNode::Folder(folder));
        Ok(id)
    }

    pub fn create_command(
        &mut self,
        location: TreeLocation,
        name: &str,
        command_text: &str,
    ) -> Result<LibraryNodeId, LibraryError> {
        let name = required_name(name)?;
        let id = self.take_next_free_id();
        let command = SavedCommand { id, name, command_text: command_text.to_string() };
        self.children_at_mut(location)?.push(LibraryNode::Command(command));
        Ok(id)
    }

    pub fn rename_node(&mut self, id: LibraryNodeId, new_name: &str) -> Result<(), LibraryError> {
        let new_name = required_name(new_name)?;
        let node = find_node_mut(&mut self.root_nodes, id).ok_or(LibraryError::NodeNotFound)?;
        *node.name_mut() = new_name;
        Ok(())
    }

    pub fn set_folder_open(&mut self, id: LibraryNodeId, is_open: bool) -> Result<(), LibraryError> {
        match find_node_mut(&mut self.root_nodes, id) {
            Some(LibraryNode::Folder(folder)) => {
                folder.is_open = is_open;
                Ok(())
            }
            Some(LibraryNode::Command(_)) => Err(LibraryError::NotAFolder),
            None => Err(LibraryError::NodeNotFound),
        }
    }

    pub fn change_command_text(&mut self, id: LibraryNodeId, command_text: &str) -> Result<(), LibraryError> {
        match find_node_mut(&mut self.root_nodes, id) {
            Some(LibraryNode::Command(command)) => {
                command.command_text = command_text.to_string();
                Ok(())
            }
            Some(LibraryNode::Folder(_)) => Err(LibraryError::NotACommand),
            None => Err(LibraryError::NodeNotFound),
        }
    }

    /// Elimina el elemento y, si es una carpeta, todo lo que contiene.
    pub fn remove_node(&mut self, id: LibraryNodeId) -> Result<LibraryNode, LibraryError> {
        take_node_out(&mut self.root_nodes, id).ok_or(LibraryError::NodeNotFound)
    }

    pub fn move_node(&mut self, id: LibraryNodeId, destination: TreeLocation) -> Result<(), LibraryError> {
        if let TreeLocation::InsideFolder(destination_folder) = destination {
            if self.is_same_or_inside(destination_folder, id) {
                return Err(LibraryError::FolderCannotGoInsideItself);
            }
        }
        // Comprobamos el destino antes de sacar el elemento para no perderlo si falla.
        self.children_at_mut(destination)?;

        let node = self.remove_node(id)?;
        self.children_at_mut(destination)?.push(node);
        Ok(())
    }

    /// Coloca el elemento justo delante o detrás de otro, en la carpeta de ese otro.
    pub fn move_node_beside(&mut self, id: LibraryNodeId, sibling: LibraryNodeId, side: SiblingSide) -> Result<(), LibraryError> {
        if id == sibling {
            return Ok(());
        }
        if self.is_same_or_inside(sibling, id) {
            return Err(LibraryError::FolderCannotGoInsideItself);
        }
        find_node(&self.root_nodes, sibling).ok_or(LibraryError::NodeNotFound)?;

        let node = self.remove_node(id)?;
        let (siblings, sibling_index) =
            find_list_containing_mut(&mut self.root_nodes, sibling).expect("se comprobó que existe antes de mover");
        let insert_index = match side {
            SiblingSide::Before => sibling_index,
            SiblingSide::After => sibling_index + 1,
        };
        siblings.insert(insert_index, node);
        Ok(())
    }

    /// El árbol reducido a lo que coincide con el filtro, como un
    /// `LIKE '%filtro%'` sin distinguir mayúsculas: los comandos cuyo nombre o
    /// texto lo contienen, y las carpetas cuyo nombre lo contiene (con todo lo
    /// que tienen dentro) o que tienen dentro algo que coincide.
    pub fn nodes_matching(&self, filter: &str) -> Vec<LibraryNode> {
        nodes_matching(&self.root_nodes, &filter.trim().to_lowercase())
    }

    /// Las carpetas que hay que abrir para ver lo que coincide con el filtro:
    /// las que tienen dentro (a cualquier profundidad) un comando o una carpeta
    /// que coincide. Una carpeta que solo coincide por su propio nombre no
    /// hace falta abrirla.
    pub fn folders_leading_to_matches(&self, filter: &str) -> Vec<LibraryNodeId> {
        let mut folders = Vec::new();
        collect_folders_leading_to_matches(&self.root_nodes, &filter.trim().to_lowercase(), &mut folders);
        folders
    }

    /// Carpetas a las que se puede mover el elemento: todas menos él mismo
    /// y las que tiene dentro.
    pub fn destinations_for_moving(&self, id: LibraryNodeId) -> Vec<FolderDestination> {
        let mut destinations = vec![FolderDestination { location: TreeLocation::Root, label: tr("library.root_folder") }];
        collect_folder_destinations(&self.root_nodes, "", &mut destinations);
        destinations.retain(|destination| match destination.location {
            TreeLocation::Root => true,
            TreeLocation::InsideFolder(folder_id) => !self.is_same_or_inside(folder_id, id),
        });
        destinations
    }

    fn take_next_free_id(&mut self) -> LibraryNodeId {
        let id = LibraryNodeId(self.next_free_id);
        self.next_free_id += 1;
        id
    }

    fn children_at_mut(&mut self, location: TreeLocation) -> Result<&mut Vec<LibraryNode>, LibraryError> {
        match location {
            TreeLocation::Root => Ok(&mut self.root_nodes),
            TreeLocation::InsideFolder(folder_id) => match find_node_mut(&mut self.root_nodes, folder_id) {
                Some(LibraryNode::Folder(folder)) => Ok(&mut folder.children),
                Some(LibraryNode::Command(_)) => Err(LibraryError::NotAFolder),
                None => Err(LibraryError::NodeNotFound),
            },
        }
    }

    fn is_same_or_inside(&self, candidate: LibraryNodeId, ancestor: LibraryNodeId) -> bool {
        find_node(&self.root_nodes, ancestor)
            .is_some_and(|ancestor_node| find_node(std::slice::from_ref(ancestor_node), candidate).is_some())
    }
}

fn required_name(name: &str) -> Result<String, LibraryError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        Err(LibraryError::EmptyName)
    } else {
        Ok(trimmed.to_string())
    }
}

fn find_node(nodes: &[LibraryNode], id: LibraryNodeId) -> Option<&LibraryNode> {
    nodes.iter().find_map(|node| {
        if node.id() == id {
            return Some(node);
        }
        match node {
            LibraryNode::Folder(folder) => find_node(&folder.children, id),
            LibraryNode::Command(_) => None,
        }
    })
}

fn find_node_mut(nodes: &mut [LibraryNode], id: LibraryNodeId) -> Option<&mut LibraryNode> {
    for node in nodes.iter_mut() {
        if node.id() == id {
            return Some(node);
        }
        if let LibraryNode::Folder(folder) = node {
            if let Some(found) = find_node_mut(&mut folder.children, id) {
                return Some(found);
            }
        }
    }
    None
}

/// La lista (raíz o hijos de una carpeta) donde está el elemento, y su posición en ella.
fn find_list_containing_mut(nodes: &mut Vec<LibraryNode>, id: LibraryNodeId) -> Option<(&mut Vec<LibraryNode>, usize)> {
    if let Some(index) = nodes.iter().position(|node| node.id() == id) {
        return Some((nodes, index));
    }
    for node in nodes.iter_mut() {
        if let LibraryNode::Folder(folder) = node {
            if let Some(found) = find_list_containing_mut(&mut folder.children, id) {
                return Some(found);
            }
        }
    }
    None
}

fn take_node_out(nodes: &mut Vec<LibraryNode>, id: LibraryNodeId) -> Option<LibraryNode> {
    if let Some(index) = nodes.iter().position(|node| node.id() == id) {
        return Some(nodes.remove(index));
    }
    nodes.iter_mut().find_map(|node| match node {
        LibraryNode::Folder(folder) => take_node_out(&mut folder.children, id),
        LibraryNode::Command(_) => None,
    })
}

/// `lowercase_filter` ya viene en minúsculas.
fn nodes_matching(nodes: &[LibraryNode], lowercase_filter: &str) -> Vec<LibraryNode> {
    let contains_filter = |text: &str| text.to_lowercase().contains(lowercase_filter);
    nodes
        .iter()
        .filter_map(|node| match node {
            LibraryNode::Command(command) => {
                (contains_filter(&command.name) || contains_filter(&command.command_text)).then(|| node.clone())
            }
            LibraryNode::Folder(folder) if contains_filter(&folder.name) => Some(node.clone()),
            LibraryNode::Folder(folder) => {
                let matching_children = nodes_matching(&folder.children, lowercase_filter);
                (!matching_children.is_empty())
                    .then(|| LibraryNode::Folder(CommandFolder { children: matching_children, ..folder.clone() }))
            }
        })
        .collect()
}

/// Devuelve si algo de `nodes` (o de dentro) coincide, y va apuntando las
/// carpetas que contienen coincidencias.
fn collect_folders_leading_to_matches(nodes: &[LibraryNode], lowercase_filter: &str, folders: &mut Vec<LibraryNodeId>) -> bool {
    let contains_filter = |text: &str| text.to_lowercase().contains(lowercase_filter);
    let mut anything_matches = false;
    for node in nodes {
        match node {
            LibraryNode::Command(command) => {
                anything_matches |= contains_filter(&command.name) || contains_filter(&command.command_text);
            }
            LibraryNode::Folder(folder) => {
                let has_matches_inside = collect_folders_leading_to_matches(&folder.children, lowercase_filter, folders);
                if has_matches_inside {
                    folders.push(folder.id);
                }
                anything_matches |= has_matches_inside || contains_filter(&folder.name);
            }
        }
    }
    anything_matches
}

fn collect_folder_destinations(nodes: &[LibraryNode], parent_label: &str, destinations: &mut Vec<FolderDestination>) {
    for node in nodes {
        if let LibraryNode::Folder(folder) = node {
            let label = if parent_label.is_empty() {
                folder.name.clone()
            } else {
                format!("{parent_label} / {}", folder.name)
            };
            destinations.push(FolderDestination { location: TreeLocation::InsideFolder(folder.id), label: label.clone() });
            collect_folder_destinations(&folder.children, &label, destinations);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_can_be_created_inside_folders() {
        let mut library = CommandLibrary::empty();
        let folder = library.create_folder(TreeLocation::Root, "SSH").unwrap();
        library.create_command(TreeLocation::InsideFolder(folder), "Servidor", "ssh yo@servidor").unwrap();

        let LibraryNode::Folder(created_folder) = &library.root_nodes()[0] else { panic!("se esperaba una carpeta") };
        assert_eq!(created_folder.children.len(), 1);
    }

    #[test]
    fn folder_cannot_be_moved_inside_its_own_subfolder() {
        let mut library = CommandLibrary::empty();
        let parent = library.create_folder(TreeLocation::Root, "Padre").unwrap();
        let child = library.create_folder(TreeLocation::InsideFolder(parent), "Hija").unwrap();

        let result = library.move_node(parent, TreeLocation::InsideFolder(child));

        assert_eq!(result, Err(LibraryError::FolderCannotGoInsideItself));
        assert_eq!(library.root_nodes().len(), 1, "la carpeta no debe perderse");
    }

    #[test]
    fn moving_offers_only_valid_destinations() {
        let mut library = CommandLibrary::empty();
        let parent = library.create_folder(TreeLocation::Root, "Padre").unwrap();
        library.create_folder(TreeLocation::InsideFolder(parent), "Hija").unwrap();
        library.create_folder(TreeLocation::Root, "Otra").unwrap();

        let labels: Vec<String> =
            library.destinations_for_moving(parent).into_iter().map(|destination| destination.label).collect();

        assert_eq!(labels, vec![tr("library.root_folder"), "Otra".to_string()]);
    }

    fn root_names(library: &CommandLibrary) -> Vec<String> {
        library
            .root_nodes()
            .iter()
            .map(|node| match node {
                LibraryNode::Folder(folder) => folder.name.clone(),
                LibraryNode::Command(command) => command.name.clone(),
            })
            .collect()
    }

    #[test]
    fn commands_can_be_reordered() {
        let mut library = CommandLibrary::empty();
        let first = library.create_command(TreeLocation::Root, "uno", "1").unwrap();
        library.create_command(TreeLocation::Root, "dos", "2").unwrap();
        let third = library.create_command(TreeLocation::Root, "tres", "3").unwrap();

        library.move_node_beside(third, first, SiblingSide::Before).unwrap();
        assert_eq!(root_names(&library), vec!["tres", "uno", "dos"]);

        library.move_node_beside(third, first, SiblingSide::After).unwrap();
        assert_eq!(root_names(&library), vec!["uno", "tres", "dos"]);
    }

    #[test]
    fn placing_beside_a_nested_item_moves_into_its_folder() {
        let mut library = CommandLibrary::empty();
        let folder = library.create_folder(TreeLocation::Root, "Carpeta").unwrap();
        let nested = library.create_command(TreeLocation::InsideFolder(folder), "dentro", "x").unwrap();
        let outside = library.create_command(TreeLocation::Root, "fuera", "y").unwrap();

        library.move_node_beside(outside, nested, SiblingSide::After).unwrap();

        let LibraryNode::Folder(folder) = &library.root_nodes()[0] else { panic!("se esperaba una carpeta") };
        assert_eq!(folder.children.len(), 2);
        assert_eq!(library.root_nodes().len(), 1);
    }

    #[test]
    fn folder_cannot_be_placed_beside_its_own_content() {
        let mut library = CommandLibrary::empty();
        let folder = library.create_folder(TreeLocation::Root, "Carpeta").unwrap();
        let nested = library.create_command(TreeLocation::InsideFolder(folder), "dentro", "x").unwrap();

        let result = library.move_node_beside(folder, nested, SiblingSide::Before);

        assert_eq!(result, Err(LibraryError::FolderCannotGoInsideItself));
        assert_eq!(library.root_nodes().len(), 1, "la carpeta no debe perderse");
    }

    /// Nombres de lo que queda tras filtrar, con su ruta, para comparar fácilmente.
    fn names_after_filtering(library: &CommandLibrary, filter: &str) -> Vec<String> {
        fn collect(nodes: &[LibraryNode], parent: &str, names: &mut Vec<String>) {
            for node in nodes {
                match node {
                    LibraryNode::Command(command) => names.push(format!("{parent}{}", command.name)),
                    LibraryNode::Folder(folder) => {
                        names.push(format!("{parent}{}/", folder.name));
                        collect(&folder.children, &format!("{parent}{}/", folder.name), names);
                    }
                }
            }
        }
        let mut names = Vec::new();
        collect(&library.nodes_matching(filter), "", &mut names);
        names
    }

    fn library_for_filtering() -> CommandLibrary {
        let mut library = CommandLibrary::empty();
        let pi = library.create_folder(TreeLocation::Root, "PI").unwrap();
        library.create_command(TreeLocation::InsideFolder(pi), "Entrar", "ssh usuario@servidor.local").unwrap();
        library.create_command(TreeLocation::InsideFolder(pi), "Reiniciar", "sudo reboot").unwrap();
        let tools = library.create_folder(TreeLocation::Root, "Útil").unwrap();
        library.create_command(TreeLocation::InsideFolder(tools), "Borrar pantalla", "cls").unwrap();
        library
    }

    #[test]
    fn filter_finds_commands_by_name_or_by_command_ignoring_case() {
        let library = library_for_filtering();
        assert_eq!(names_after_filtering(&library, "REINI"), ["PI/", "PI/Reiniciar"]);
        assert_eq!(names_after_filtering(&library, "servidor.local"), ["PI/", "PI/Entrar"]);
    }

    #[test]
    fn a_folder_whose_name_matches_keeps_all_its_content() {
        let library = library_for_filtering();
        assert_eq!(names_after_filtering(&library, "pi"), ["PI/", "PI/Entrar", "PI/Reiniciar"]);
    }

    #[test]
    fn a_folder_starts_open_and_can_be_folded() {
        let mut library = CommandLibrary::empty();
        let folder = library.create_folder(TreeLocation::Root, "Servidores").unwrap();
        let command = library.create_command(TreeLocation::Root, "Listar", "dir").unwrap();
        let is_open = |library: &CommandLibrary| match &library.root_nodes()[0] {
            LibraryNode::Folder(folder) => folder.is_open,
            LibraryNode::Command(_) => unreachable!(),
        };
        assert!(is_open(&library));

        library.set_folder_open(folder, false).unwrap();

        assert!(!is_open(&library));
        assert_eq!(library.set_folder_open(command, false), Err(LibraryError::NotAFolder));
    }

    #[test]
    fn only_folders_with_matches_inside_need_opening() {
        let mut library = CommandLibrary::empty();
        let servers = library.create_folder(TreeLocation::Root, "Servidores").unwrap();
        let pi = library.create_folder(TreeLocation::InsideFolder(servers), "PI").unwrap();
        library.create_command(TreeLocation::InsideFolder(pi), "Reiniciar", "sudo reboot").unwrap();
        let backups = library.create_folder(TreeLocation::Root, "Copias reboot").unwrap();
        library.create_command(TreeLocation::InsideFolder(backups), "Copiar", "robocopy").unwrap();

        let to_open = library.folders_leading_to_matches("reboot");

        assert!(to_open.contains(&servers) && to_open.contains(&pi), "hay que abrir el camino hasta el comando");
        assert!(!to_open.contains(&backups), "esta carpeta solo coincide por su nombre: no se despliega");
    }

    #[test]
    fn an_empty_filter_keeps_everything_and_no_match_keeps_nothing() {
        let library = library_for_filtering();
        assert_eq!(names_after_filtering(&library, "  ").len(), 5);
        assert!(names_after_filtering(&library, "no existe").is_empty());
    }

    #[test]
    fn names_are_required() {
        let mut library = CommandLibrary::empty();
        assert_eq!(library.create_folder(TreeLocation::Root, "   "), Err(LibraryError::EmptyName));
    }
}
