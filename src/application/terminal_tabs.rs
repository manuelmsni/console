use std::time::{Duration, Instant};

use super::ports::{Clock, ShellLauncher, TabJournalStore, TerminalEmulatorFactory};
use super::terminal_session::{replay_journal, TerminalSession};
use crate::domain::app_settings::AutosaveSettings;
use crate::i18n::{tr, tr_args};
use crate::domain::shell_profile::ShellProfile;
use crate::domain::tab_journal::{JournalEntry, TabId, TabRecord};
use crate::domain::terminal_grid::TerminalSize;

/// Tamaño provisional de una pestaña nueva; la interfaz lo ajusta al dibujarla.
const SIZE_BEFORE_FIRST_LAYOUT: TerminalSize = TerminalSize { rows: 24, columns: 80 };
/// Al pasar de este tamaño, el diario de una pestaña se resume en una foto
/// de su estado actual, para que no crezca sin límite.
const JOURNAL_SIZE_BEFORE_COMPACTING: u64 = 4 * 1024 * 1024;

/// Si las pestañas se guardan solas y cada cuánto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutosavePolicy {
    pub enabled: bool,
    pub interval: Duration,
}

impl AutosavePolicy {
    pub fn from_settings(settings: &AutosaveSettings) -> Self {
        Self { enabled: settings.enabled, interval: Duration::from_secs(settings.interval_seconds as u64) }
    }
}

/// Lo que la barra de pestañas necesita saber de cada una.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabOverview {
    pub label: String,
    /// Tiene cambios que se perderían al salir sin guardar.
    pub has_unsaved_changes: bool,
    pub autosaves: bool,
}

/// El conjunto de pestañas abiertas, cuál se está viendo y sus diarios.
///
/// Una pestaña se guarda (se escribe su diario en disco) cuando el usuario lo
/// pide o, si hay autoguardado, cada cierto tiempo. Las guardadas se vuelven
/// a abrir la próxima vez, aunque la app se cerrara de golpe; cerrar una
/// pestaña borra su diario.
pub struct TerminalTabs {
    sessions: Vec<TerminalSession>,
    active_index: Option<usize>,
    shell_launcher: Box<dyn ShellLauncher>,
    emulator_factory: Box<dyn TerminalEmulatorFactory>,
    journal_store: Box<dyn TabJournalStore>,
    /// Para la línea "Sesión abierta / restaurada el …" de cada pestaña.
    clock: Box<dyn Clock>,
    autosave: AutosavePolicy,
    next_tab_id: u64,
    /// El último fallo al guardar los diarios, para avisar una vez y no en cada fotograma.
    last_journal_problem: Option<String>,
    unreported_journal_problem: Option<String>,
}

impl TerminalTabs {
    pub fn new(
        shell_launcher: Box<dyn ShellLauncher>,
        emulator_factory: Box<dyn TerminalEmulatorFactory>,
        journal_store: Box<dyn TabJournalStore>,
        clock: Box<dyn Clock>,
        autosave: AutosavePolicy,
    ) -> Self {
        Self {
            sessions: Vec::new(),
            active_index: None,
            shell_launcher,
            emulator_factory,
            journal_store,
            clock,
            autosave,
            next_tab_id: 1,
            last_journal_problem: None,
            unreported_journal_problem: None,
        }
    }

    pub fn set_autosave_policy(&mut self, autosave: AutosavePolicy) {
        self.autosave = autosave;
    }

    pub fn open_tab(&mut self, profile: &ShellProfile) -> Result<(), String> {
        let size = self.active_session().map_or(SIZE_BEFORE_FIRST_LAYOUT, TerminalSession::size);
        let shell = self.shell_launcher.launch(profile, size)?;
        let emulator = self.emulator_factory.create_emulator(size);

        let record = TabRecord {
            id: self.take_next_tab_id(),
            profile_name: profile.display_name.clone(),
            title: first_free_title(&profile.display_name, self.sessions.iter().map(TerminalSession::title)),
        };
        self.add_and_activate(TerminalSession::new(record, shell, emulator, size, self.clock.now()));
        Ok(())
    }

    /// Vuelve a abrir las pestañas guardadas, cada una con su pantalla e
    /// historial reconstruidos desde su diario. Devuelve cuántas.
    pub fn reopen_tabs_from_journals(&mut self, profiles: &[ShellProfile]) -> Result<usize, String> {
        let tabs = self.journal_store.load_open_tabs()?;
        let mut reopened = 0;
        for (record, journal) in tabs {
            // Si ese tipo de terminal ya no existe, se reabre con el primero disponible.
            let Some(profile) = profiles.iter().find(|profile| profile.display_name == record.profile_name).or(profiles.first())
            else {
                return Err(tr("tabs.error.no_terminal_types"));
            };
            let initial_size = first_size_in(&journal).unwrap_or(SIZE_BEFORE_FIRST_LAYOUT);
            let mut emulator = self.emulator_factory.create_emulator(initial_size);
            let (command_history, size) = replay_journal(&journal, emulator.as_mut(), initial_size);
            let shell = self.shell_launcher.launch(profile, size)?;

            self.next_tab_id = self.next_tab_id.max(record.id.0 + 1);
            let restored_at = self.clock.now();
            self.add_and_activate(TerminalSession::restored(record, command_history, shell, emulator, size, restored_at));
            reopened += 1;
        }
        Ok(reopened)
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Un fallo nuevo al guardar los diarios, si lo hay, para mostrarlo.
    pub fn take_new_journal_problem(&mut self) -> Option<String> {
        self.unreported_journal_problem.take()
    }

    /// Lleva a cada pantalla lo que haya escrito su consola y guarda las
    /// pestañas con autoguardado a las que ya les toca.
    pub fn forward_all_shell_output(&mut self) {
        for session in &mut self.sessions {
            session.forward_shell_output_to_screen();
        }
        self.autosave_due_tabs(Instant::now());
    }

    /// Guarda la pestaña activa ahora mismo (Ctrl+S).
    pub fn save_active_tab(&mut self) -> Result<(), String> {
        let Some(index) = self.active_index else { return Ok(()) };
        self.save_tab(index, Instant::now())
    }

    /// Guarda todas las pestañas con cambios (al salir).
    pub fn save_all_tabs_with_changes(&mut self) -> Result<(), String> {
        let now = Instant::now();
        for index in 0..self.sessions.len() {
            if self.sessions[index].has_unsaved_changes() {
                self.save_tab(index, now)?;
            }
        }
        Ok(())
    }

    /// Las pestañas con autoguardado se guardan ya, sin esperar a su turno
    /// (al salir); las demás no se tocan.
    pub fn save_tabs_with_autosave_now(&mut self) {
        let now = Instant::now();
        for index in 0..self.sessions.len() {
            if self.autosaves(&self.sessions[index]) && self.sessions[index].has_unsaved_changes() {
                if let Err(error) = self.write_journal(index, now) {
                    self.note_journal_problem(error);
                }
            }
        }
    }

    /// Cuánto falta para que una pestaña con autoguardado y cambios pendientes
    /// deba guardarse, para despertar a la app en ese momento aunque no pase
    /// nada más. `None` si no hay ninguna esperando.
    pub fn time_until_next_autosave(&self) -> Option<Duration> {
        let now = Instant::now();
        self.sessions
            .iter()
            .filter(|session| self.autosaves(session) && session.has_unsaved_changes())
            .map(|session| match session.last_saved_at() {
                Some(saved_at) => self.autosave.interval.saturating_sub(now.duration_since(saved_at)),
                None => Duration::ZERO,
            })
            .min()
    }

    /// Cuántas pestañas perderían cambios si se saliera ahora.
    pub fn count_of_tabs_with_unsaved_changes(&self) -> usize {
        self.sessions.iter().filter(|session| session.has_unsaved_changes()).count()
    }

    /// Activa o desactiva el autoguardado solo de la pestaña activa y solo
    /// durante esta ejecución. Devuelve cómo queda.
    pub fn toggle_autosave_of_active_tab(&mut self) -> Option<bool> {
        let index = self.active_index?;
        let autosaves_now = !self.autosaves(&self.sessions[index]);
        self.sessions[index].set_autosave_override(autosaves_now);
        Some(autosaves_now)
    }

    /// Borra un comando del historial de la pestaña activa. Si la pestaña
    /// tiene diario en disco, se quita también de ahí en este momento (sin
    /// guardar nada más), por si era un dato delicado.
    pub fn forget_command_in_active_tab(&mut self, command: &str) -> Result<(), String> {
        let Some(session) = self.active_index.and_then(|index| self.sessions.get_mut(index)) else { return Ok(()) };
        session.forget_command(command);
        if !session.is_saved_on_disk() {
            return Ok(());
        }
        let id = session.id();
        let saved: Vec<JournalEntry> = self
            .journal_store
            .read(id)?
            .into_iter()
            .filter(|entry| !matches!(entry, JournalEntry::CommandRecorded(remembered) if remembered == command))
            .collect();
        self.journal_store.replace(id, &saved)
    }

    /// Cambia el nombre de la pestaña. Si está guardada, el nombre nuevo se
    /// guarda ya (es lo que se verá al reabrirla).
    pub fn rename_tab(&mut self, id: TabId, title: &str) -> Result<(), String> {
        let title = title.trim();
        if title.is_empty() {
            return Err(tr("library.error.empty_name"));
        }
        let Some(session) = self.sessions.iter_mut().find(|session| session.id() == id) else { return Ok(()) };
        session.rename(title.to_string());
        if session.is_saved_on_disk() {
            self.save_list_of_saved_tabs();
        }
        Ok(())
    }

    /// `true` si la pestaña tiene diario en disco: al cerrarla se borrará.
    pub fn is_saved_on_disk(&self, index: usize) -> bool {
        self.sessions.get(index).is_some_and(TerminalSession::is_saved_on_disk)
    }

    pub fn tab_id(&self, index: usize) -> Option<TabId> {
        self.sessions.get(index).map(TerminalSession::id)
    }

    pub fn index_of(&self, id: TabId) -> Option<usize> {
        self.sessions.iter().position(|session| session.id() == id)
    }

    pub fn title_of(&self, index: usize) -> Option<&str> {
        self.sessions.get(index).map(TerminalSession::title)
    }

    /// Cierra la pestaña (y su proceso), borra su diario y deja activa la de al lado.
    pub fn close_tab(&mut self, index: usize) {
        if index >= self.sessions.len() {
            return;
        }
        let closed = self.sessions.remove(index);
        if closed.is_saved_on_disk() {
            if let Err(error) = self.journal_store.delete(closed.id()) {
                self.note_journal_problem(error);
            }
            self.save_list_of_saved_tabs();
        }

        let last_index = self.sessions.len().checked_sub(1);
        self.active_index = match (self.active_index, last_index) {
            (_, None) => None,
            (Some(active), Some(_)) if active > index => Some(active - 1),
            (Some(active), Some(last)) => Some(active.min(last)),
            (None, Some(_)) => None,
        };
    }

    pub fn activate_tab(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active_index = Some(index);
        }
    }

    /// Pasa a la pestaña de al lado: `steps` = 1 es la siguiente y -1 la
    /// anterior. Desde la última se vuelve a la primera y al revés.
    pub fn activate_neighbour_tab(&mut self, steps: isize) {
        if let Some(active) = self.active_index {
            self.active_index = Some(index_after_wrapping(active, steps, self.sessions.len()));
        }
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active_index
    }

    pub fn tabs_overview(&self) -> Vec<TabOverview> {
        self.sessions
            .iter()
            .map(|session| {
                let autosaves = self.autosaves(session);
                let label = if session.shell_has_exited() {
                    tr_args("tabs.finished", &[("title", &session.title())])
                } else {
                    session.title().to_string()
                };
                // Con autoguardado los cambios se guardan solos: no se marcan.
                TabOverview { label, has_unsaved_changes: session.has_unsaved_changes() && !autosaves, autosaves }
            })
            .collect()
    }

    pub fn active_session_mut(&mut self) -> Option<&mut TerminalSession> {
        self.active_index.and_then(|index| self.sessions.get_mut(index))
    }

    pub fn has_scheduled_input(&self) -> bool {
        self.sessions.iter().any(TerminalSession::has_scheduled_input)
    }

    fn autosaves(&self, session: &TerminalSession) -> bool {
        session.autosave_override().unwrap_or(self.autosave.enabled)
    }

    /// Las pestañas con autoguardado se guardan cuando ha pasado su intervalo.
    /// En las demás no se acumula lo ocurrido: al guardarlas se escribirá una
    /// foto del estado completo.
    fn autosave_due_tabs(&mut self, now: Instant) {
        for index in 0..self.sessions.len() {
            let session = &self.sessions[index];
            if !self.autosaves(session) {
                self.sessions[index].discard_unsaved_journal_entries();
                continue;
            }
            let interval_passed = session.last_saved_at().map_or(true, |saved_at| now.duration_since(saved_at) >= self.autosave.interval);
            if interval_passed && session.has_unsaved_changes() {
                if let Err(error) = self.write_journal(index, now) {
                    self.note_journal_problem(error);
                }
            }
        }
    }

    /// Guardar a petición del usuario: siempre una foto completa del estado.
    fn save_tab(&mut self, index: usize, now: Instant) -> Result<(), String> {
        let was_saved_before = self.sessions[index].is_saved_on_disk();
        let id = self.sessions[index].id();
        let compacted = self.sessions[index].compacted_journal();
        self.journal_store.replace(id, &compacted)?;
        self.sessions[index].mark_saved(now);
        if !was_saved_before {
            self.save_list_of_saved_tabs();
        }
        Ok(())
    }

    /// Autoguardado: se añade lo nuevo al diario, salvo que toque reescribirlo
    /// entero (primera vez, demasiado grande o se había dejado de apuntar).
    fn write_journal(&mut self, index: usize, now: Instant) -> Result<(), String> {
        let session = &mut self.sessions[index];
        let id = session.id();
        let was_saved_before = session.is_saved_on_disk();
        let too_big = self.journal_store.journal_size(id) > JOURNAL_SIZE_BEFORE_COMPACTING;
        if !was_saved_before || session.journal_must_be_compacted() || too_big {
            self.journal_store.replace(id, &session.compacted_journal())?;
        } else {
            let entries = session.take_unsaved_journal_entries();
            self.journal_store.append(id, &entries)?;
        }
        session.mark_saved(now);
        if !was_saved_before {
            self.save_list_of_saved_tabs();
        }
        Ok(())
    }

    fn add_and_activate(&mut self, session: TerminalSession) {
        self.sessions.push(session);
        self.active_index = Some(self.sessions.len() - 1);
    }

    fn take_next_tab_id(&mut self) -> TabId {
        let id = TabId(self.next_tab_id);
        self.next_tab_id += 1;
        id
    }

    /// El índice solo lista las pestañas con diario en disco: son las que se reabrirán.
    fn save_list_of_saved_tabs(&mut self) {
        let records: Vec<TabRecord> =
            self.sessions.iter().filter(|session| session.is_saved_on_disk()).map(TerminalSession::record).collect();
        if let Err(error) = self.journal_store.save_open_tabs(&records) {
            self.note_journal_problem(error);
        }
    }

    fn note_journal_problem(&mut self, error: String) {
        if self.last_journal_problem.as_ref() != Some(&error) {
            self.unreported_journal_problem = Some(tr_args("tabs.error.save_failed", &[("error", &error)]));
            self.last_journal_problem = Some(error);
        }
    }

    fn active_session(&self) -> Option<&TerminalSession> {
        self.active_index.and_then(|index| self.sessions.get(index))
    }
}

/// El tamaño con el que empezó la pestaña, para crear su pantalla al reconstruirla.
fn first_size_in(journal: &[JournalEntry]) -> Option<TerminalSize> {
    journal.iter().find_map(|entry| match entry {
        JournalEntry::Resized(size) => Some(*size),
        _ => None,
    })
}

fn index_after_wrapping(current: usize, steps: isize, tab_count: usize) -> usize {
    (current as isize + steps).rem_euclid(tab_count as isize) as usize
}

/// "cmd 1", "cmd 2"…: el primer número que no usa ninguna pestaña abierta. Si
/// solo queda "cmd 8", la siguiente es "cmd 1"; si no queda ninguna, también.
fn first_free_title<'a>(base_name: &str, open_titles: impl Iterator<Item = &'a str>) -> String {
    let prefix = format!("{base_name} ");
    let used_numbers: Vec<usize> =
        open_titles.filter_map(|title| title.strip_prefix(prefix.as_str())?.parse().ok()).collect();
    let free_number = (1..).find(|number| !used_numbers.contains(number)).unwrap_or(1);
    format!("{prefix}{free_number}")
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use super::*;
    use crate::application::ports::{RunningShell, TerminalEmulator};
    use crate::domain::local_time::LocalDateTime;
    use crate::domain::screen_snapshot::ScreenSnapshot;
    use crate::domain::terminal_grid::GridPosition;
    use crate::domain::terminal_keys::InputModes;

    #[test]
    fn a_new_tab_takes_the_first_free_number() {
        assert_eq!(first_free_title("cmd", [].into_iter()), "cmd 1");
        assert_eq!(first_free_title("cmd", ["cmd 8"].into_iter()), "cmd 1");
        assert_eq!(first_free_title("cmd", ["cmd 1", "cmd 3"].into_iter()), "cmd 2");
        assert_eq!(first_free_title("cmd", ["cmd 1", "cmd 2"].into_iter()), "cmd 3");
    }

    #[test]
    fn renamed_tabs_and_other_terminal_types_do_not_take_numbers() {
        let open = ["Servidor", "PowerShell 1", "cmd 2"];
        assert_eq!(first_free_title("cmd", open.into_iter()), "cmd 1");
    }

    #[test]
    fn moving_past_the_last_tab_goes_back_to_the_first() {
        assert_eq!(index_after_wrapping(2, 1, 3), 0);
    }

    #[test]
    fn moving_before_the_first_tab_goes_to_the_last() {
        assert_eq!(index_after_wrapping(0, -1, 3), 2);
    }

    #[test]
    fn moving_inside_the_tabs_just_steps() {
        assert_eq!(index_after_wrapping(1, 1, 3), 2);
        assert_eq!(index_after_wrapping(1, -1, 3), 0);
    }

    /// Diarios en memoria, para ver qué se habría escrito en disco.
    #[derive(Default)]
    struct JournalsInMemory {
        journals: HashMap<TabId, Vec<JournalEntry>>,
        index: Vec<TabRecord>,
    }

    struct InMemoryJournalStore(Rc<RefCell<JournalsInMemory>>);

    impl TabJournalStore for InMemoryJournalStore {
        fn load_open_tabs(&mut self) -> Result<Vec<(TabRecord, Vec<JournalEntry>)>, String> {
            let disk = self.0.borrow();
            Ok(disk.index.iter().map(|record| (record.clone(), disk.journals.get(&record.id).cloned().unwrap_or_default())).collect())
        }
        fn save_open_tabs(&mut self, tabs: &[TabRecord]) -> Result<(), String> {
            self.0.borrow_mut().index = tabs.to_vec();
            Ok(())
        }
        fn append(&mut self, tab: TabId, entries: &[JournalEntry]) -> Result<(), String> {
            self.0.borrow_mut().journals.entry(tab).or_default().extend_from_slice(entries);
            Ok(())
        }
        fn replace(&mut self, tab: TabId, entries: &[JournalEntry]) -> Result<(), String> {
            self.0.borrow_mut().journals.insert(tab, entries.to_vec());
            Ok(())
        }
        fn read(&mut self, tab: TabId) -> Result<Vec<JournalEntry>, String> {
            Ok(self.0.borrow().journals.get(&tab).cloned().unwrap_or_default())
        }
        fn journal_size(&self, _tab: TabId) -> u64 {
            0
        }
        fn delete(&mut self, tab: TabId) -> Result<(), String> {
            self.0.borrow_mut().journals.remove(&tab);
            Ok(())
        }
    }

    struct SilentShell;

    impl RunningShell for SilentShell {
        fn send_input(&mut self, _bytes: &[u8]) {}
        fn resize(&mut self, _size: TerminalSize) {}
        fn take_received_output(&mut self) -> Vec<u8> {
            Vec::new()
        }
        fn has_exited(&mut self) -> bool {
            false
        }
    }

    struct SilentShellLauncher;

    impl ShellLauncher for SilentShellLauncher {
        fn launch(&self, _profile: &ShellProfile, _size: TerminalSize) -> Result<Box<dyn RunningShell>, String> {
            Ok(Box::new(SilentShell))
        }
    }

    /// Pantalla falsa cuyo cursor está siempre en el texto que se le indique.
    struct ScreenShowingPrompt(Rc<RefCell<String>>);

    impl TerminalEmulator for ScreenShowingPrompt {
        fn process_shell_output(&mut self, _bytes: &[u8]) {}
        fn resize(&mut self, _size: TerminalSize) {}
        fn snapshot(&self) -> ScreenSnapshot {
            ScreenSnapshot { rows: Vec::new(), visible_cursor: None }
        }
        fn text_between(&self, _first: GridPosition, _last: GridPosition) -> String {
            String::new()
        }
        fn input_modes(&self) -> InputModes {
            InputModes::default()
        }
        fn scroll_history_by(&mut self, _lines: i32) {}
        fn scroll_to_newest_output(&mut self) {}
        fn line_at_cursor(&self) -> String {
            self.0.borrow().clone()
        }
        fn replayable_state(&mut self) -> Vec<u8> {
            b"foto".to_vec()
        }
    }

    struct ScreensShowingPrompt(Rc<RefCell<String>>);

    impl TerminalEmulatorFactory for ScreensShowingPrompt {
        fn create_emulator(&self, _size: TerminalSize) -> Box<dyn TerminalEmulator> {
            Box::new(ScreenShowingPrompt(self.0.clone()))
        }
    }

    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> LocalDateTime {
            LocalDateTime { year: 2026, month: 11, day: 5, hour: 18, minute: 30 }
        }
    }

    const MANUAL_SAVING: AutosavePolicy = AutosavePolicy { enabled: false, interval: Duration::from_secs(10) };
    const AUTOSAVING_ALWAYS: AutosavePolicy = AutosavePolicy { enabled: true, interval: Duration::ZERO };

    struct TabsUnderTest {
        tabs: TerminalTabs,
        disk: Rc<RefCell<JournalsInMemory>>,
        line_at_cursor: Rc<RefCell<String>>,
    }

    impl TabsUnderTest {
        fn with(autosave: AutosavePolicy, disk: Rc<RefCell<JournalsInMemory>>) -> Self {
            let line_at_cursor = Rc::new(RefCell::new(String::new()));
            let tabs = TerminalTabs::new(
                Box::new(SilentShellLauncher),
                Box::new(ScreensShowingPrompt(line_at_cursor.clone())),
                Box::new(InMemoryJournalStore(disk.clone())),
                Box::new(FixedClock),
                autosave,
            );
            Self { tabs, disk, line_at_cursor }
        }

        fn new(autosave: AutosavePolicy) -> Self {
            Self::with(autosave, Rc::default())
        }

        fn open_cmd_tab(&mut self) {
            self.tabs.open_tab(&ShellProfile::new("cmd", "cmd.exe", &[])).unwrap();
            self.tabs.forward_all_shell_output();
        }

        fn run_at_prompt(&mut self, command: &str) {
            *self.line_at_cursor.borrow_mut() = format!(r"C:\>{command}");
            let session = self.tabs.active_session_mut().unwrap();
            session.send_key(crate::domain::terminal_keys::TerminalKey::Enter, crate::domain::terminal_keys::KeyModifiers::NONE);
            self.tabs.forward_all_shell_output();
        }

        fn journals_on_disk(&self) -> usize {
            self.disk.borrow().journals.len()
        }
    }

    #[test]
    fn without_autosave_nothing_is_written_until_saving() {
        let mut test = TabsUnderTest::new(MANUAL_SAVING);
        test.open_cmd_tab();
        test.run_at_prompt("dir");

        assert_eq!(test.journals_on_disk(), 0);
        assert!(test.tabs.tabs_overview()[0].has_unsaved_changes, "la pestaña muestra el punto");

        test.tabs.save_active_tab().unwrap();
        assert_eq!(test.journals_on_disk(), 1);
        assert!(!test.tabs.tabs_overview()[0].has_unsaved_changes, "el punto desaparece al guardar");

        test.run_at_prompt("cls");
        assert!(test.tabs.tabs_overview()[0].has_unsaved_changes, "el punto vuelve al cambiar algo");
    }

    #[test]
    fn only_saved_tabs_come_back_and_with_their_history() {
        let disk: Rc<RefCell<JournalsInMemory>> = Rc::default();
        let mut first_run = TabsUnderTest::with(MANUAL_SAVING, disk.clone());
        first_run.open_cmd_tab();
        first_run.run_at_prompt("git status");
        first_run.tabs.save_active_tab().unwrap();
        first_run.open_cmd_tab(); // nunca se guarda

        let mut second_run = TabsUnderTest::with(MANUAL_SAVING, disk);
        let reopened = second_run.tabs.reopen_tabs_from_journals(&[ShellProfile::new("cmd", "cmd.exe", &[])]).unwrap();

        assert_eq!(reopened, 1);
        assert_eq!(second_run.tabs.active_session_mut().unwrap().command_history(), ["git status"]);
    }

    #[test]
    fn toggling_autosave_of_one_tab_saves_it_without_changing_the_others() {
        let mut test = TabsUnderTest::new(AutosavePolicy { interval: Duration::ZERO, ..MANUAL_SAVING });
        test.open_cmd_tab();
        test.open_cmd_tab();

        assert_eq!(test.tabs.toggle_autosave_of_active_tab(), Some(true));
        test.run_at_prompt("dir");

        assert_eq!(test.journals_on_disk(), 1, "solo se guarda la pestaña con autoguardado");
        let overview = test.tabs.tabs_overview();
        assert!(overview[0].has_unsaved_changes && !overview[0].autosaves);
        assert!(!overview[1].has_unsaved_changes && overview[1].autosaves);
        assert_eq!(test.tabs.toggle_autosave_of_active_tab(), Some(false), "la misma tecla lo desactiva");
    }

    #[test]
    fn with_global_autosave_tabs_are_saved_as_things_happen() {
        let mut test = TabsUnderTest::new(AUTOSAVING_ALWAYS);
        test.open_cmd_tab();
        test.run_at_prompt("dir");

        assert_eq!(test.journals_on_disk(), 1);
        assert_eq!(test.tabs.count_of_tabs_with_unsaved_changes(), 0);
    }

    #[test]
    fn renaming_a_saved_tab_keeps_the_new_name_for_next_time() {
        let mut test = TabsUnderTest::new(MANUAL_SAVING);
        test.open_cmd_tab();
        test.tabs.save_active_tab().unwrap();
        let id = test.tabs.tab_id(0).unwrap();

        test.tabs.rename_tab(id, "  Servidor  ").unwrap();

        assert_eq!(test.tabs.title_of(0), Some("Servidor"));
        assert_eq!(test.disk.borrow().index[0].title, "Servidor");
        assert!(test.tabs.rename_tab(id, "   ").is_err(), "un nombre vacío no vale");
        assert_eq!(test.tabs.title_of(0), Some("Servidor"));
    }

    #[test]
    fn closing_a_saved_tab_deletes_its_journal() {
        let mut test = TabsUnderTest::new(MANUAL_SAVING);
        test.open_cmd_tab();
        test.tabs.save_active_tab().unwrap();
        assert!(test.tabs.is_saved_on_disk(0));

        test.tabs.close_tab(0);
        assert_eq!(test.journals_on_disk(), 0);
        assert!(test.disk.borrow().index.is_empty());
    }

    #[test]
    fn forgetting_a_command_removes_it_from_the_saved_journal_right_away() {
        let mut test = TabsUnderTest::new(MANUAL_SAVING);
        test.open_cmd_tab();
        test.run_at_prompt("login secreto");
        test.run_at_prompt("dir");
        test.tabs.save_active_tab().unwrap();

        test.tabs.forget_command_in_active_tab("login secreto").unwrap();

        let saved = test.disk.borrow().journals.values().next().cloned().unwrap();
        assert!(!saved.contains(&JournalEntry::CommandRecorded("login secreto".into())));
        assert!(saved.contains(&JournalEntry::CommandRecorded("dir".into())));
    }
}
