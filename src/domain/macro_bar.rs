//! La barra de macros: botones que ejecutan comandos en el equipo (abrir el
//! explorador en una carpeta, lanzar un programa...), no en las pestañas.
//! Como los marcadores de un navegador, se pueden agrupar en carpetas
//! desplegables, unas dentro de otras.

use std::fmt;

use crate::i18n::tr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MacroId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MacroBarItem {
    Macro(Macro),
    Folder(MacroFolder),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Macro {
    pub id: MacroId,
    /// Puede ir vacío si lleva icono: entonces el botón es solo el icono.
    pub name: String,
    /// Nombre del icono (`rocket_launch`...); qué dibujo es lo decide la interfaz.
    pub icon: Option<String>,
    /// Lo que se ejecuta; si tiene varias líneas, una detrás de otra.
    pub command: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroFolder {
    pub id: MacroId,
    pub name: String,
    pub icon: Option<String>,
    pub children: Vec<MacroBarItem>,
}

/// Lo que se escribe al crear o editar una macro.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MacroFields {
    pub name: String,
    pub icon: Option<String>,
    pub command: String,
}

/// Lo que se escribe al crear o editar una carpeta.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderFields {
    pub name: String,
    pub icon: Option<String>,
}

/// Dónde va un elemento: en la propia barra o dentro de una carpeta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroLocation {
    Bar,
    InsideFolder(MacroId),
}

/// A qué lado de otro elemento se coloca uno al reordenar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroSide {
    Before,
    After,
}

/// Una carpeta a la que se puede mover algo, con su ruta legible ("Trabajo / Webs").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroDestination {
    pub location: MacroLocation,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroError {
    NotFound,
    NotAFolder,
    NotAMacro,
    NeedsNameOrIcon,
    EmptyCommand,
    FolderCannotGoInsideItself,
}

impl fmt::Display for MacroError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let message = match self {
            MacroError::NotFound => tr("macros.error.not_found"),
            MacroError::NotAFolder => tr("macros.error.not_a_folder"),
            MacroError::NotAMacro => tr("macros.error.not_a_macro"),
            MacroError::NeedsNameOrIcon => tr("macros.error.needs_name_or_icon"),
            MacroError::EmptyCommand => tr("macros.error.empty_command"),
            MacroError::FolderCannotGoInsideItself => tr("macros.error.folder_inside_itself"),
        };
        formatter.write_str(&message)
    }
}

impl MacroBarItem {
    pub fn id(&self) -> MacroId {
        match self {
            MacroBarItem::Macro(item) => item.id,
            MacroBarItem::Folder(folder) => folder.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            MacroBarItem::Macro(item) => &item.name,
            MacroBarItem::Folder(folder) => &folder.name,
        }
    }

    pub fn icon(&self) -> Option<&str> {
        match self {
            MacroBarItem::Macro(item) => item.icon.as_deref(),
            MacroBarItem::Folder(folder) => folder.icon.as_deref(),
        }
    }

    pub fn is_folder(&self) -> bool {
        matches!(self, MacroBarItem::Folder(_))
    }
}

impl Macro {
    /// Una sola línea para la shell del sistema: las líneas no vacías, una
    /// detrás de otra (aunque alguna falle, como en un `.bat`).
    pub fn command_line(&self) -> String {
        command_line_of(&self.command)
    }
}

/// Ver `Macro::command_line`; también sirve para probar un comando sin guardarlo.
pub fn command_line_of(command: &str) -> String {
    command.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(" & ")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroBar {
    items: Vec<MacroBarItem>,
    next_free_id: u64,
}

impl MacroBar {
    pub fn empty() -> Self {
        Self { items: Vec::new(), next_free_id: 1 }
    }

    pub fn items(&self) -> &[MacroBarItem] {
        &self.items
    }

    pub fn find(&self, id: MacroId) -> Option<&MacroBarItem> {
        find_item(&self.items, id)
    }

    pub fn create_macro(&mut self, location: MacroLocation, fields: MacroFields) -> Result<MacroId, MacroError> {
        let fields = checked_macro_fields(fields)?;
        let id = self.take_next_free_id();
        let item = Macro { id, name: fields.name, icon: fields.icon, command: fields.command };
        self.children_at_mut(location)?.push(MacroBarItem::Macro(item));
        Ok(id)
    }

    pub fn create_folder(&mut self, location: MacroLocation, fields: FolderFields) -> Result<MacroId, MacroError> {
        let fields = checked_folder_fields(fields)?;
        let id = self.take_next_free_id();
        let folder = MacroFolder { id, name: fields.name, icon: fields.icon, children: Vec::new() };
        self.children_at_mut(location)?.push(MacroBarItem::Folder(folder));
        Ok(id)
    }

    pub fn update_macro(&mut self, id: MacroId, fields: MacroFields) -> Result<(), MacroError> {
        let fields = checked_macro_fields(fields)?;
        match find_item_mut(&mut self.items, id) {
            Some(MacroBarItem::Macro(item)) => {
                item.name = fields.name;
                item.icon = fields.icon;
                item.command = fields.command;
                Ok(())
            }
            Some(MacroBarItem::Folder(_)) => Err(MacroError::NotAMacro),
            None => Err(MacroError::NotFound),
        }
    }

    pub fn update_folder(&mut self, id: MacroId, fields: FolderFields) -> Result<(), MacroError> {
        let fields = checked_folder_fields(fields)?;
        match find_item_mut(&mut self.items, id) {
            Some(MacroBarItem::Folder(folder)) => {
                folder.name = fields.name;
                folder.icon = fields.icon;
                Ok(())
            }
            Some(MacroBarItem::Macro(_)) => Err(MacroError::NotAFolder),
            None => Err(MacroError::NotFound),
        }
    }

    /// Elimina el elemento y, si es una carpeta, todo lo que contiene.
    pub fn remove(&mut self, id: MacroId) -> Result<MacroBarItem, MacroError> {
        take_item_out(&mut self.items, id).ok_or(MacroError::NotFound)
    }

    /// Dónde está el elemento ahora: en la barra o en qué carpeta.
    pub fn location_of(&self, id: MacroId) -> Option<MacroLocation> {
        if self.items.iter().any(|item| item.id() == id) {
            return Some(MacroLocation::Bar);
        }
        location_inside_folders(&self.items, id)
    }

    /// Lo lleva al final de otra carpeta (o de la barra). Si ya estaba ahí, no se mueve.
    pub fn move_into(&mut self, id: MacroId, destination: MacroLocation) -> Result<(), MacroError> {
        if self.location_of(id).ok_or(MacroError::NotFound)? == destination {
            return Ok(());
        }
        if let MacroLocation::InsideFolder(folder) = destination {
            if self.is_same_or_inside(folder, id) {
                return Err(MacroError::FolderCannotGoInsideItself);
            }
        }
        // Se comprueba el destino antes de sacar el elemento, para no perderlo si falla.
        self.children_at_mut(destination)?;
        let item = self.remove(id)?;
        self.children_at_mut(destination)?.push(item);
        Ok(())
    }

    /// Lo coloca justo delante o detrás de otro, en la carpeta de ese otro.
    pub fn move_beside(&mut self, id: MacroId, sibling: MacroId, side: MacroSide) -> Result<(), MacroError> {
        if id == sibling {
            return Ok(());
        }
        if self.is_same_or_inside(sibling, id) {
            return Err(MacroError::FolderCannotGoInsideItself);
        }
        self.find(sibling).ok_or(MacroError::NotFound)?;
        let item = self.remove(id)?;
        let (siblings, index) = find_list_containing_mut(&mut self.items, sibling).expect("se comprobó que existe antes de mover");
        let insert_at = match side {
            MacroSide::Before => index,
            MacroSide::After => index + 1,
        };
        siblings.insert(insert_at, item);
        Ok(())
    }

    /// Sitios a los que se puede llevar el elemento: la barra y todas las
    /// carpetas menos él mismo y las que tiene dentro.
    pub fn destinations_for(&self, id: Option<MacroId>) -> Vec<MacroDestination> {
        let mut destinations = vec![MacroDestination { location: MacroLocation::Bar, label: tr("macros.location.bar") }];
        collect_destinations(&self.items, "", &mut destinations);
        if let Some(id) = id {
            destinations.retain(|destination| match destination.location {
                MacroLocation::Bar => true,
                MacroLocation::InsideFolder(folder) => !self.is_same_or_inside(folder, id),
            });
        }
        destinations
    }

    fn take_next_free_id(&mut self) -> MacroId {
        let id = MacroId(self.next_free_id);
        self.next_free_id += 1;
        id
    }

    fn children_at_mut(&mut self, location: MacroLocation) -> Result<&mut Vec<MacroBarItem>, MacroError> {
        match location {
            MacroLocation::Bar => Ok(&mut self.items),
            MacroLocation::InsideFolder(folder) => match find_item_mut(&mut self.items, folder) {
                Some(MacroBarItem::Folder(folder)) => Ok(&mut folder.children),
                Some(MacroBarItem::Macro(_)) => Err(MacroError::NotAFolder),
                None => Err(MacroError::NotFound),
            },
        }
    }

    fn is_same_or_inside(&self, candidate: MacroId, ancestor: MacroId) -> bool {
        self.find(ancestor).is_some_and(|ancestor| find_item(std::slice::from_ref(ancestor), candidate).is_some())
    }
}

fn checked_macro_fields(fields: MacroFields) -> Result<MacroFields, MacroError> {
    let (name, icon) = checked_name_and_icon(&fields.name, fields.icon)?;
    let command = fields.command.trim().to_string();
    if command.is_empty() {
        return Err(MacroError::EmptyCommand);
    }
    Ok(MacroFields { name, icon, command })
}

fn checked_folder_fields(fields: FolderFields) -> Result<FolderFields, MacroError> {
    let (name, icon) = checked_name_and_icon(&fields.name, fields.icon)?;
    Ok(FolderFields { name, icon })
}

/// Sin nombre ni icono, el botón no se vería.
fn checked_name_and_icon(name: &str, icon: Option<String>) -> Result<(String, Option<String>), MacroError> {
    let name = name.trim().to_string();
    let icon = icon.filter(|icon| !icon.trim().is_empty());
    if name.is_empty() && icon.is_none() {
        return Err(MacroError::NeedsNameOrIcon);
    }
    Ok((name, icon))
}

fn find_item(items: &[MacroBarItem], id: MacroId) -> Option<&MacroBarItem> {
    items.iter().find_map(|item| {
        if item.id() == id {
            return Some(item);
        }
        match item {
            MacroBarItem::Folder(folder) => find_item(&folder.children, id),
            MacroBarItem::Macro(_) => None,
        }
    })
}

fn find_item_mut(items: &mut [MacroBarItem], id: MacroId) -> Option<&mut MacroBarItem> {
    for item in items.iter_mut() {
        if item.id() == id {
            return Some(item);
        }
        if let MacroBarItem::Folder(folder) = item {
            if let Some(found) = find_item_mut(&mut folder.children, id) {
                return Some(found);
            }
        }
    }
    None
}

fn location_inside_folders(items: &[MacroBarItem], id: MacroId) -> Option<MacroLocation> {
    items.iter().find_map(|item| match item {
        MacroBarItem::Folder(folder) if folder.children.iter().any(|child| child.id() == id) => {
            Some(MacroLocation::InsideFolder(folder.id))
        }
        MacroBarItem::Folder(folder) => location_inside_folders(&folder.children, id),
        MacroBarItem::Macro(_) => None,
    })
}

/// La lista (la barra o los hijos de una carpeta) donde está el elemento, y su posición.
fn find_list_containing_mut(items: &mut Vec<MacroBarItem>, id: MacroId) -> Option<(&mut Vec<MacroBarItem>, usize)> {
    if let Some(index) = items.iter().position(|item| item.id() == id) {
        return Some((items, index));
    }
    for item in items.iter_mut() {
        if let MacroBarItem::Folder(folder) = item {
            if let Some(found) = find_list_containing_mut(&mut folder.children, id) {
                return Some(found);
            }
        }
    }
    None
}

fn take_item_out(items: &mut Vec<MacroBarItem>, id: MacroId) -> Option<MacroBarItem> {
    if let Some(index) = items.iter().position(|item| item.id() == id) {
        return Some(items.remove(index));
    }
    items.iter_mut().find_map(|item| match item {
        MacroBarItem::Folder(folder) => take_item_out(&mut folder.children, id),
        MacroBarItem::Macro(_) => None,
    })
}

fn collect_destinations(items: &[MacroBarItem], parent_label: &str, destinations: &mut Vec<MacroDestination>) {
    for item in items {
        if let MacroBarItem::Folder(folder) = item {
            let name = if folder.name.is_empty() { tr("macros.unnamed_folder") } else { folder.name.clone() };
            let label = if parent_label.is_empty() { name } else { format!("{parent_label} / {name}") };
            destinations.push(MacroDestination { location: MacroLocation::InsideFolder(folder.id), label: label.clone() });
            collect_destinations(&folder.children, &label, destinations);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn macro_fields(name: &str, command: &str) -> MacroFields {
        MacroFields { name: name.into(), icon: None, command: command.into() }
    }

    fn folder_fields(name: &str) -> FolderFields {
        FolderFields { name: name.into(), icon: None }
    }

    fn names(items: &[MacroBarItem]) -> Vec<&str> {
        items.iter().map(MacroBarItem::name).collect()
    }

    #[test]
    fn macros_can_go_inside_folders_inside_folders() {
        let mut bar = MacroBar::empty();
        let work = bar.create_folder(MacroLocation::Bar, folder_fields("Trabajo")).unwrap();
        let webs = bar.create_folder(MacroLocation::InsideFolder(work), folder_fields("Webs")).unwrap();
        let docs = bar.create_macro(MacroLocation::InsideFolder(webs), macro_fields("Docs", "start https://docs.rs")).unwrap();

        assert_eq!(bar.location_of(docs), Some(MacroLocation::InsideFolder(webs)));
        assert_eq!(bar.location_of(webs), Some(MacroLocation::InsideFolder(work)));
        assert_eq!(bar.location_of(work), Some(MacroLocation::Bar));
    }

    #[test]
    fn a_button_needs_a_name_or_an_icon() {
        let mut bar = MacroBar::empty();
        let nothing = MacroFields { name: "  ".into(), icon: None, command: "explorer .".into() };
        let only_icon = MacroFields { name: String::new(), icon: Some("folder".into()), command: "explorer .".into() };

        assert_eq!(bar.create_macro(MacroLocation::Bar, nothing), Err(MacroError::NeedsNameOrIcon));
        assert!(bar.create_macro(MacroLocation::Bar, only_icon).is_ok());
        assert_eq!(bar.create_folder(MacroLocation::Bar, folder_fields("")), Err(MacroError::NeedsNameOrIcon));
    }

    #[test]
    fn a_macro_needs_a_command() {
        let mut bar = MacroBar::empty();
        assert_eq!(bar.create_macro(MacroLocation::Bar, macro_fields("Nada", " \n ")), Err(MacroError::EmptyCommand));
    }

    #[test]
    fn several_lines_run_one_after_another() {
        let item = Macro {
            id: MacroId(1),
            name: "Abrir".into(),
            icon: None,
            command: "cd /d C:\\proyecto\n\n  code .  \r\nexplorer .".into(),
        };
        assert_eq!(item.command_line(), "cd /d C:\\proyecto & code . & explorer .");
    }

    #[test]
    fn editing_keeps_the_place_and_changes_the_rest() {
        let mut bar = MacroBar::empty();
        let first = bar.create_macro(MacroLocation::Bar, macro_fields("Uno", "1")).unwrap();
        bar.create_macro(MacroLocation::Bar, macro_fields("Dos", "2")).unwrap();

        let changed = MacroFields { name: "Explorador".into(), icon: Some("folder".into()), command: "explorer .".into() };
        bar.update_macro(first, changed).unwrap();

        let MacroBarItem::Macro(edited) = &bar.items()[0] else { panic!("se esperaba una macro") };
        assert_eq!((edited.name.as_str(), edited.icon.as_deref(), edited.command.as_str()), ("Explorador", Some("folder"), "explorer ."));
        assert_eq!(bar.update_folder(first, folder_fields("x")), Err(MacroError::NotAFolder));
    }

    #[test]
    fn items_can_be_reordered_and_moved_between_folders() {
        let mut bar = MacroBar::empty();
        let first = bar.create_macro(MacroLocation::Bar, macro_fields("uno", "1")).unwrap();
        let second = bar.create_macro(MacroLocation::Bar, macro_fields("dos", "2")).unwrap();
        let folder = bar.create_folder(MacroLocation::Bar, folder_fields("carpeta")).unwrap();

        bar.move_beside(folder, first, MacroSide::Before).unwrap();
        assert_eq!(names(bar.items()), ["carpeta", "uno", "dos"]);

        bar.move_into(second, MacroLocation::InsideFolder(folder)).unwrap();
        assert_eq!(names(bar.items()), ["carpeta", "uno"]);

        // Ponerlo al lado de algo que está en una carpeta lo mete en ella.
        bar.move_beside(first, second, MacroSide::Before).unwrap();
        let MacroBarItem::Folder(folder) = &bar.items()[0] else { panic!("se esperaba una carpeta") };
        assert_eq!(names(&folder.children), ["uno", "dos"]);
    }

    #[test]
    fn moving_to_where_it_already_is_keeps_its_position() {
        let mut bar = MacroBar::empty();
        let first = bar.create_macro(MacroLocation::Bar, macro_fields("uno", "1")).unwrap();
        bar.create_macro(MacroLocation::Bar, macro_fields("dos", "2")).unwrap();

        bar.move_into(first, MacroLocation::Bar).unwrap();

        assert_eq!(names(bar.items()), ["uno", "dos"]);
    }

    #[test]
    fn a_folder_cannot_go_inside_itself() {
        let mut bar = MacroBar::empty();
        let parent = bar.create_folder(MacroLocation::Bar, folder_fields("Padre")).unwrap();
        let child = bar.create_folder(MacroLocation::InsideFolder(parent), folder_fields("Hija")).unwrap();
        let nested = bar.create_macro(MacroLocation::InsideFolder(child), macro_fields("m", "x")).unwrap();

        assert_eq!(bar.move_into(parent, MacroLocation::InsideFolder(child)), Err(MacroError::FolderCannotGoInsideItself));
        assert_eq!(bar.move_beside(parent, nested, MacroSide::After), Err(MacroError::FolderCannotGoInsideItself));
        assert_eq!(names(bar.items()), ["Padre"], "la carpeta no debe perderse");
    }

    #[test]
    fn destinations_leave_out_the_folder_itself_and_its_content() {
        let mut bar = MacroBar::empty();
        let parent = bar.create_folder(MacroLocation::Bar, folder_fields("Padre")).unwrap();
        bar.create_folder(MacroLocation::InsideFolder(parent), folder_fields("Hija")).unwrap();
        bar.create_folder(MacroLocation::Bar, folder_fields("Otra")).unwrap();

        let all: Vec<String> = bar.destinations_for(None).into_iter().map(|destination| destination.label).collect();
        let for_parent: Vec<String> = bar.destinations_for(Some(parent)).into_iter().map(|destination| destination.label).collect();

        assert_eq!(all, [tr("macros.location.bar"), "Padre".into(), "Padre / Hija".into(), "Otra".into()]);
        assert_eq!(for_parent, [tr("macros.location.bar"), "Otra".to_string()]);
    }

    #[test]
    fn removing_a_folder_removes_what_it_holds() {
        let mut bar = MacroBar::empty();
        let folder = bar.create_folder(MacroLocation::Bar, folder_fields("Carpeta")).unwrap();
        let inside = bar.create_macro(MacroLocation::InsideFolder(folder), macro_fields("m", "x")).unwrap();

        bar.remove(folder).unwrap();

        assert!(bar.items().is_empty());
        assert_eq!(bar.find(inside), None);
    }
}
