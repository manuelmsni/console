use super::ports::{MacroBarStore, SystemCommandRunner};
use crate::domain::macro_bar::{
    command_line_of, FolderFields, MacroBar, MacroBarItem, MacroError, MacroFields, MacroId, MacroLocation, MacroSide,
};
use crate::i18n::{tr, tr_args};

/// Casos de uso de la barra de macros. Cada cambio se guarda al momento.
pub struct MacroBarService {
    bar: MacroBar,
    store: Box<dyn MacroBarStore>,
    runner: Box<dyn SystemCommandRunner>,
}

impl MacroBarService {
    /// Carga la barra guardada. Devuelve también un aviso si algo fue mal.
    pub fn load(store: Box<dyn MacroBarStore>, runner: Box<dyn SystemCommandRunner>) -> (Self, Option<String>) {
        let (bar, warning) = match store.load_macro_bar() {
            Ok(Some(bar)) => (bar, None),
            Ok(None) => (example_bar(), None),
            Err(error) => (MacroBar::empty(), Some(error)),
        };
        (Self { bar, store, runner }, warning)
    }

    pub fn bar(&self) -> &MacroBar {
        &self.bar
    }

    /// Ejecuta la macro en el equipo. Devuelve el aviso para la barra de estado.
    pub fn run(&self, id: MacroId) -> Result<String, String> {
        match self.bar.find(id) {
            Some(MacroBarItem::Macro(item)) => {
                self.runner.start(&item.command_line())?;
                let shown_name = if item.name.is_empty() { item.command_line() } else { item.name.clone() };
                Ok(tr_args("macros.started", &[("name", &shown_name)]))
            }
            Some(MacroBarItem::Folder(_)) => Err(MacroError::NotAMacro.to_string()),
            None => Err(MacroError::NotFound.to_string()),
        }
    }

    /// Prueba un comando sin guardarlo (desde el diálogo de edición).
    pub fn try_command(&self, command: &str) -> Result<String, String> {
        let command_line = command_line_of(command);
        if command_line.is_empty() {
            return Err(MacroError::EmptyCommand.to_string());
        }
        self.runner.start(&command_line)?;
        Ok(tr_args("macros.started", &[("name", &command_line)]))
    }

    pub fn create_macro(&mut self, location: MacroLocation, fields: MacroFields) -> Result<(), String> {
        self.change_and_save(|bar| bar.create_macro(location, fields).map(|_| ()))
    }

    pub fn create_folder(&mut self, location: MacroLocation, fields: FolderFields) -> Result<(), String> {
        self.change_and_save(|bar| bar.create_folder(location, fields).map(|_| ()))
    }

    /// Cambia lo escrito y, si se ha elegido otra carpeta, la lleva allí.
    pub fn update_macro(&mut self, id: MacroId, fields: MacroFields, location: MacroLocation) -> Result<(), String> {
        self.change_and_save(|bar| {
            bar.update_macro(id, fields)?;
            bar.move_into(id, location)
        })
    }

    pub fn update_folder(&mut self, id: MacroId, fields: FolderFields, location: MacroLocation) -> Result<(), String> {
        self.change_and_save(|bar| {
            bar.update_folder(id, fields)?;
            bar.move_into(id, location)
        })
    }

    pub fn remove(&mut self, id: MacroId) -> Result<(), String> {
        self.change_and_save(|bar| bar.remove(id).map(|_| ()))
    }

    pub fn move_into(&mut self, id: MacroId, destination: MacroLocation) -> Result<(), String> {
        self.change_and_save(|bar| bar.move_into(id, destination))
    }

    pub fn move_beside(&mut self, id: MacroId, sibling: MacroId, side: MacroSide) -> Result<(), String> {
        self.change_and_save(|bar| bar.move_beside(id, sibling, side))
    }

    /// Aplica el cambio sobre una copia; solo si sale bien se queda y se guarda.
    fn change_and_save(&mut self, change: impl FnOnce(&mut MacroBar) -> Result<(), MacroError>) -> Result<(), String> {
        let mut changed = self.bar.clone();
        change(&mut changed).map_err(|error| error.to_string())?;
        self.bar = changed;
        self.store.save_macro_bar(&self.bar)
    }
}

/// Lo que se ve la primera vez, para que se entienda para qué sirve.
fn example_bar() -> MacroBar {
    let mut bar = MacroBar::empty();
    let home_folder = MacroFields {
        name: tr("examples.macro_home_folder"),
        icon: Some("home".to_string()),
        command: "explorer \"%USERPROFILE%\"".to_string(),
    };
    bar.create_macro(MacroLocation::Bar, home_folder).expect("macro de ejemplo válida");
    bar
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    #[derive(Default)]
    struct MemoryStore {
        saved: Rc<RefCell<Option<MacroBar>>>,
    }

    impl MacroBarStore for MemoryStore {
        fn load_macro_bar(&self) -> Result<Option<MacroBar>, String> {
            Ok(self.saved.borrow().clone())
        }

        fn save_macro_bar(&self, bar: &MacroBar) -> Result<(), String> {
            *self.saved.borrow_mut() = Some(bar.clone());
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingRunner {
        started: Rc<RefCell<Vec<String>>>,
    }

    impl SystemCommandRunner for RecordingRunner {
        fn start(&self, command_line: &str) -> Result<(), String> {
            self.started.borrow_mut().push(command_line.to_string());
            Ok(())
        }
    }

    fn empty_service() -> (MacroBarService, Rc<RefCell<Option<MacroBar>>>, Rc<RefCell<Vec<String>>>) {
        let store = MemoryStore { saved: Rc::new(RefCell::new(Some(MacroBar::empty()))) };
        let runner = RecordingRunner::default();
        let (saved, started) = (store.saved.clone(), runner.started.clone());
        let (service, warning) = MacroBarService::load(Box::new(store), Box::new(runner));
        assert_eq!(warning, None);
        (service, saved, started)
    }

    #[test]
    fn the_first_time_there_is_an_example() {
        let (service, _) = MacroBarService::load(Box::new(MemoryStore::default()), Box::new(RecordingRunner::default()));
        assert_eq!(service.bar().items().len(), 1);
    }

    #[test]
    fn running_a_macro_starts_its_command_line_on_the_system() {
        let (mut service, saved, started) = empty_service();
        let fields = MacroFields { name: "Proyecto".into(), icon: None, command: "cd /d C:\\p\ncode .".into() };
        service.create_macro(MacroLocation::Bar, fields).unwrap();
        let id = service.bar().items()[0].id();

        service.run(id).unwrap();

        assert_eq!(*started.borrow(), ["cd /d C:\\p & code ."]);
        assert_eq!(saved.borrow().as_ref().map(|bar| bar.items().len()), Some(1), "se guarda al crear");
    }

    #[test]
    fn a_failed_change_is_not_kept_nor_saved() {
        let (mut service, saved, _) = empty_service();
        let without_command = MacroFields { name: "x".into(), icon: None, command: String::new() };

        assert!(service.create_macro(MacroLocation::Bar, without_command).is_err());

        assert!(service.bar().items().is_empty());
        assert_eq!(saved.borrow().as_ref().map(|bar| bar.items().len()), Some(0));
    }

    #[test]
    fn editing_can_also_move_it_to_another_folder() {
        let (mut service, _, _) = empty_service();
        service.create_folder(MacroLocation::Bar, FolderFields { name: "Webs".into(), icon: None }).unwrap();
        let fields = MacroFields { name: "Docs".into(), icon: None, command: "start https://docs.rs".into() };
        service.create_macro(MacroLocation::Bar, fields.clone()).unwrap();
        let (folder, item) = (service.bar().items()[0].id(), service.bar().items()[1].id());

        service.update_macro(item, fields, MacroLocation::InsideFolder(folder)).unwrap();

        assert_eq!(service.bar().location_of(item), Some(MacroLocation::InsideFolder(folder)));
    }
}
