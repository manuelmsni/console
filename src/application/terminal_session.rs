use std::time::{Duration, Instant};

use super::ports::{RunningShell, TerminalEmulator};
use crate::domain::command_history::CommandHistory;
use crate::i18n::tr_args;
use crate::domain::local_time::LocalDateTime;
use crate::domain::history_scrollbar::HistoryView;
use crate::domain::hover_feedback::HoverFeedback;
use crate::domain::terminal_mouse::{
    encode_click, encode_pointer_motion, program_handles_scrolling, MouseWheel, WheelEffect,
};
use crate::domain::screen_snapshot::ScreenSnapshot;
use crate::domain::shell_prompt::{is_powershell_input_line, text_after_local_prompt};
use crate::domain::tab_journal::{JournalEntry, TabId, TabRecord};
use crate::domain::terminal_grid::{GridPosition, TerminalSize, TextSelection};
use crate::domain::terminal_keys::{
    encode_key_press, encode_new_line_without_running, encode_paste, encode_prompt_line_replacement, KeyModifiers, NewLineReader,
    TerminalKey,
};

/// Algunos programas (Claude Code, por ejemplo) toman texto + Enter llegados
/// de golpe como si fuera un pegado, y el Enter no ejecuta. Esperar un poco
/// antes de enviar el Enter hace que se comporte como si se tecleara.
const PAUSE_BEFORE_ENTER: Duration = Duration::from_millis(80);

/// Las líneas de "Sesión abierta / restaurada el …" van en gris (color 90 de la paleta).
const GRAY: &str = "\x1b[90m";
const NORMAL_COLOR: &str = "\x1b[0m";

/// Lo que haya dejado activado el programa anterior (por ejemplo, Claude Code
/// a pantalla completa si la app se cerró con él abierto), desactivado: la
/// consola nueva empieza en la pantalla normal, sin ratón y con el cursor visible.
const MODES_OF_A_NEW_CONSOLE: &str =
    "\x1b[?1049l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l\x1b[?2004l\x1b[?1l\x1b[r\x1b[?25h";

/// Quién se encarga de desplazarse, para saber qué hace la barra de scroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollControl {
    /// El historial de la pestaña.
    TabHistory(HistoryView),
    /// El programa de dentro (Claude Code a pantalla completa, less...): no
    /// se sabe cuánto tiene, solo se le puede pedir que suba o baje.
    Program,
}

/// Una pestaña: un proceso de terminal, su pantalla, lo seleccionado en ella,
/// los comandos que se han ejecutado y lo que falta apuntar en su diario.
pub struct TerminalSession {
    id: TabId,
    title: String,
    profile_name: String,
    shell: Box<dyn RunningShell>,
    emulator: Box<dyn TerminalEmulator>,
    size: TerminalSize,
    selection: Option<TextSelection>,
    command_history: CommandHistory,
    shell_has_exited: bool,
    enter_scheduled_at: Option<Instant>,
    mouse_wheel: MouseWheel,
    /// La celda sobre la que está el ratón (`None`: fuera de la terminal).
    pointer_cell: Option<GridPosition>,
    hover_feedback: HoverFeedback,
    /// Lo ocurrido desde la última vez que se guardó el diario.
    unsaved_journal_entries: Vec<JournalEntry>,
    /// El diario debe reescribirse entero (por ejemplo, tras borrar un comando
    /// del historial, para que desaparezca también del disco).
    journal_must_be_compacted: bool,
    /// Ha cambiado algo desde la última vez que se guardó.
    has_unsaved_changes: bool,
    /// Tiene un diario en disco, así que se reabrirá la próxima vez.
    is_saved_on_disk: bool,
    /// Autoguardado solo para esta pestaña y esta ejecución; `None` sigue el ajuste general.
    autosave_override: Option<bool>,
    last_saved_at: Option<Instant>,
}

impl TerminalSession {
    /// Una pestaña nueva. Empieza con una línea que dice cuándo se abrió.
    pub fn new(
        record: TabRecord,
        shell: Box<dyn RunningShell>,
        emulator: Box<dyn TerminalEmulator>,
        size: TerminalSize,
        opened_at: LocalDateTime,
    ) -> Self {
        let mut session = Self::without_heading(record, shell, emulator, size);
        session.write_heading_line(&tr_args("session.opened", &[("moment", &opened_at.spoken())]));
        session
    }

    /// Una pestaña con consola nueva que muestra lo que había en la anterior
    /// (ya reconstruido en `emulator` con `replay_journal`) y recuerda sus
    /// comandos. Lo recuperado queda encima de una línea con la fecha.
    pub fn restored(
        record: TabRecord,
        command_history: CommandHistory,
        shell: Box<dyn RunningShell>,
        emulator: Box<dyn TerminalEmulator>,
        size: TerminalSize,
        restored_at: LocalDateTime,
    ) -> Self {
        let mut session = Self::without_heading(record, shell, emulator, size);
        session.command_history = command_history;
        session.write_heading_line(&tr_args("session.restored", &[("moment", &restored_at.spoken())]));
        session.is_saved_on_disk = true;
        // Su diario, al guardarlo, se reescribe con lo recuperado como punto de partida.
        session.journal_must_be_compacted = true;
        session
    }

    fn without_heading(
        record: TabRecord,
        shell: Box<dyn RunningShell>,
        emulator: Box<dyn TerminalEmulator>,
        size: TerminalSize,
    ) -> Self {
        Self {
            id: record.id,
            title: record.title,
            profile_name: record.profile_name,
            shell,
            emulator,
            size,
            selection: None,
            command_history: CommandHistory::default(),
            shell_has_exited: false,
            enter_scheduled_at: None,
            mouse_wheel: MouseWheel::default(),
            pointer_cell: None,
            hover_feedback: HoverFeedback::default(),
            unsaved_journal_entries: vec![JournalEntry::Resized(size)],
            journal_must_be_compacted: false,
            has_unsaved_changes: false,
            is_saved_on_disk: false,
            autosave_override: None,
            last_saved_at: None,
        }
    }

    /// Apunta algo para el diario. Todo cuenta como cambio sin guardar menos
    /// un cambio de tamaño: si no, redimensionar la ventana marcaría todas las pestañas.
    fn note(&mut self, entry: JournalEntry) {
        if !matches!(entry, JournalEntry::Resized(_)) {
            self.has_unsaved_changes = true;
        }
        self.unsaved_journal_entries.push(entry);
    }

    /// Escribe una línea gris (y la apunta en el diario). La consola escribe
    /// desde arriba de la pantalla (la del sistema, además, la borra al
    /// arrancar), así que la línea se empuja hacia el historial de scroll y el
    /// cursor vuelve arriba: queda justo encima de lo que escriba la consola,
    /// visible al subir con la rueda.
    fn write_heading_line(&mut self, text: &str) {
        let line = format!("{GRAY}── {text} ──{NORMAL_COLOR}");
        self.emulator.scroll_to_newest_output();
        // Primero la pantalla normal, y luego se mira qué hay en ella.
        let mut bytes = MODES_OF_A_NEW_CONSOLE.as_bytes().to_vec();
        self.emulator.process_shell_output(&bytes);
        let heading = line_pushed_into_history(&line, &self.emulator.snapshot());
        self.emulator.process_shell_output(&heading);
        bytes.extend(heading);
        self.note(JournalEntry::Output(bytes));
    }

    pub fn record(&self) -> TabRecord {
        TabRecord { id: self.id, profile_name: self.profile_name.clone(), title: self.title.clone() }
    }

    pub fn id(&self) -> TabId {
        self.id
    }

    /// Lo que hay que añadir al diario desde la última vez.
    pub fn take_unsaved_journal_entries(&mut self) -> Vec<JournalEntry> {
        std::mem::take(&mut self.unsaved_journal_entries)
    }

    /// Sin autoguardado no se acumula lo ocurrido (podría ser mucho): al
    /// guardar se escribirá una foto del estado completo.
    pub fn discard_unsaved_journal_entries(&mut self) {
        if !self.unsaved_journal_entries.is_empty() {
            self.unsaved_journal_entries.clear();
            self.journal_must_be_compacted = true;
        }
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.has_unsaved_changes
    }

    pub fn is_saved_on_disk(&self) -> bool {
        self.is_saved_on_disk
    }

    pub fn mark_saved(&mut self, at: Instant) {
        self.has_unsaved_changes = false;
        self.is_saved_on_disk = true;
        self.last_saved_at = Some(at);
    }

    pub fn last_saved_at(&self) -> Option<Instant> {
        self.last_saved_at
    }

    pub fn autosave_override(&self) -> Option<bool> {
        self.autosave_override
    }

    pub fn set_autosave_override(&mut self, autosaves: bool) {
        self.autosave_override = Some(autosaves);
    }

    pub fn journal_must_be_compacted(&self) -> bool {
        self.journal_must_be_compacted
    }

    /// El diario resumido en una foto del estado actual más el historial de
    /// comandos; sustituye a todo lo apuntado hasta ahora.
    pub fn compacted_journal(&mut self) -> Vec<JournalEntry> {
        self.unsaved_journal_entries.clear();
        self.journal_must_be_compacted = false;
        let mut entries = vec![JournalEntry::Resized(self.size), JournalEntry::Snapshot(self.emulator.replayable_state())];
        entries.extend(self.command_history.commands().iter().cloned().map(JournalEntry::CommandRecorded));
        entries
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn rename(&mut self, title: String) {
        self.title = title;
    }

    pub fn size(&self) -> TerminalSize {
        self.size
    }

    pub fn shell_has_exited(&self) -> bool {
        self.shell_has_exited
    }

    pub fn has_scheduled_input(&self) -> bool {
        self.enter_scheduled_at.is_some()
    }

    /// Lleva a la pantalla lo que el proceso haya escrito y envía
    /// el Enter pendiente si ya le toca.
    pub fn forward_shell_output_to_screen(&mut self) {
        let output = self.shell.take_received_output();
        if !output.is_empty() {
            self.emulator.process_shell_output(&output);
            self.note(JournalEntry::Output(output));
        }
        self.shell_has_exited = self.shell.has_exited();

        if self.enter_scheduled_at.is_some_and(|scheduled_at| scheduled_at <= Instant::now()) {
            self.enter_scheduled_at = None;
            self.send_to_shell(b"\r");
        }
    }

    pub fn resize_to(&mut self, new_size: TerminalSize) {
        if new_size == self.size {
            return;
        }
        self.size = new_size;
        self.shell.resize(new_size);
        self.emulator.resize(new_size);
        self.selection = None;
        self.note(JournalEntry::Resized(new_size));
    }

    pub fn screen_snapshot(&self) -> ScreenSnapshot {
        self.emulator.snapshot()
    }

    pub fn send_typed_text(&mut self, text: &str) {
        self.send_to_shell(text.as_bytes());
    }

    /// En el prompt de cmd o PowerShell, Enter apunta el comando en el
    /// historial de esta pestaña. Todas las teclas llegan a la consola.
    pub fn send_key(&mut self, key: TerminalKey, modifiers: KeyModifiers) {
        if key == TerminalKey::Enter && modifiers == KeyModifiers::NONE {
            self.emulator.scroll_to_newest_output();
            self.remember_command_typed_at_prompt();
        }
        let bytes = encode_key_press(key, modifiers, self.emulator.input_modes());
        self.send_to_shell(&bytes);
    }

    /// Salto de línea dentro de lo que se está escribiendo, sin ejecutarlo,
    /// de la forma que entiende quien lo lee (PowerShell u otro programa).
    pub fn insert_new_line(&mut self) {
        self.emulator.scroll_to_newest_output();
        let reader = if is_powershell_input_line(&self.emulator.line_at_cursor()) {
            NewLineReader::PowerShell
        } else {
            NewLineReader::Other
        };
        self.send_to_shell(&encode_new_line_without_running(reader));
    }

    /// `true` si el cursor está en el prompt de cmd o PowerShell (y no dentro
    /// de un programa como Claude Code, python o SSH).
    pub fn is_at_local_prompt(&mut self) -> bool {
        self.emulator.scroll_to_newest_output();
        self.cursor_is_at_local_prompt()
    }

    /// Del más antiguo al más reciente.
    pub fn command_history(&self) -> &[String] {
        self.command_history.commands()
    }

    pub fn forget_command(&mut self, command: &str) {
        self.command_history.forget(command);
        self.note(JournalEntry::CommandForgotten(command.to_string()));
    }

    /// Sustituye lo escrito en el prompt por el comando, sin ejecutarlo.
    pub fn replace_prompt_line_with(&mut self, command: &str) {
        self.send_to_shell(&encode_prompt_line_replacement(command));
    }

    pub fn paste_text(&mut self, text: &str) {
        let bytes = encode_paste(text, self.emulator.input_modes());
        self.send_to_shell(&bytes);
    }

    /// Escribe la línea y pulsa Enter, como si la hubiera tecleado el usuario.
    pub fn run_command_line(&mut self, command_line: &str) {
        if command_line.is_empty() {
            self.send_to_shell(b"\r");
            return;
        }
        if self.cursor_is_at_local_prompt() {
            self.remember_command(command_line);
        }
        self.paste_text(command_line);
        self.enter_scheduled_at = Some(Instant::now() + PAUSE_BEFORE_ENTER);
    }

    pub fn start_selection_at(&mut self, position: GridPosition) {
        self.selection = Some(TextSelection::starting_at(position));
    }

    pub fn extend_selection_to(&mut self, position: GridPosition) {
        if let Some(selection) = &mut self.selection {
            selection.extend_to(position);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn selection(&self) -> Option<&TextSelection> {
        self.selection.as_ref()
    }

    pub fn selected_text(&self) -> Option<String> {
        let selection = self.selection.as_ref()?;
        let text = self.emulator.text_between(selection.first_position(), selection.last_position());
        (!text.is_empty()).then_some(text)
    }

    pub fn scroll_history_by(&mut self, lines: i32) {
        self.emulator.scroll_history_by(lines);
        self.selection = None;
    }

    pub fn scroll_history_to(&mut self, lines_scrolled_up: usize) {
        self.emulator.scroll_history_to(lines_scrolled_up);
        self.selection = None;
    }

    pub fn scroll_control(&mut self) -> ScrollControl {
        if program_handles_scrolling(self.emulator.input_modes()) {
            ScrollControl::Program
        } else {
            ScrollControl::TabHistory(self.emulator.history_view())
        }
    }

    /// Un clic (sin arrastrar) se le pasa al programa si ha pedido el ratón,
    /// para que funcione lo que tenga para pulsar (en Claude Code, "Jump to
    /// bottom" o las opciones de una lista).
    pub fn click_at(&mut self, position: GridPosition) {
        if let Some(bytes) = encode_click(position, self.emulator.input_modes()) {
            self.send_to_shell(&bytes);
        }
    }

    /// El ratón ha pasado a otra celda (`None`: ha salido de la terminal o se
    /// está arrastrando para seleccionar). Si el programa lo ha pedido, se le
    /// cuenta, para que ilumine lo que se puede pulsar.
    pub fn pointer_moved_to(&mut self, position: Option<GridPosition>) {
        if position == self.pointer_cell {
            return;
        }
        self.pointer_cell = position;
        self.hover_feedback.pointer_moved(position, &self.emulator.snapshot());
        let motion = position.and_then(|position| encode_pointer_motion(position, self.emulator.input_modes()));
        if let Some(bytes) = motion {
            // Sin `send_to_shell`: mover el ratón no quita la selección.
            self.shell.send_input(&bytes);
        }
    }

    /// `true` si el programa ha iluminado lo que hay bajo el ratón, es decir,
    /// si ahí hay algo que se puede pulsar. `screen` es la pantalla de ahora.
    pub fn pointer_is_over_something_clickable(&mut self, screen: &ScreenSnapshot) -> bool {
        let modes = self.emulator.input_modes();
        match self.pointer_cell {
            Some(position) if modes.reports_pointer_motion => self.hover_feedback.program_highlights(position, screen),
            _ => false,
        }
    }

    /// Mueve el historial de la pestaña o, si el programa de dentro lo ha
    /// pedido (Claude Code a pantalla completa, vim...), se lo cuenta a él.
    /// `lines` positivo es hacia arriba; `pointer`, la celda bajo el ratón.
    pub fn turn_mouse_wheel(&mut self, lines: i32, pointer: GridPosition) {
        match self.mouse_wheel.turn(lines, pointer, self.emulator.input_modes()) {
            WheelEffect::ScrollHistory(lines) => self.scroll_history_by(lines),
            WheelEffect::SendToProgram(bytes) => self.send_to_shell(&bytes),
            WheelEffect::Nothing => {}
        }
    }

    fn cursor_is_at_local_prompt(&self) -> bool {
        text_after_local_prompt(&self.emulator.line_at_cursor()).is_some()
    }

    /// Se lee de la pantalla en vez de ir sumando teclas: así el comando sale
    /// bien aunque se haya editado, autocompletado con Tab o pegado.
    fn remember_command_typed_at_prompt(&mut self) {
        if let Some(command) = text_after_local_prompt(&self.emulator.line_at_cursor()) {
            let command = command.to_string();
            self.remember_command(&command);
        }
    }

    fn remember_command(&mut self, command: &str) {
        self.command_history.record(command);
        self.note(JournalEntry::CommandRecorded(command.to_string()));
    }

    /// Al escribir se vuelve a la parte más reciente de la salida y se
    /// quita la selección, como en Windows Terminal.
    fn send_to_shell(&mut self, bytes: &[u8]) {
        self.emulator.scroll_to_newest_output();
        self.selection = None;
        self.shell.send_input(bytes);
    }
}

/// Reconstruye en `emulator` lo apuntado en un diario (pantalla con colores,
/// tamaños, historial de comandos) y devuelve el historial y el último tamaño.
pub fn replay_journal(
    entries: &[JournalEntry],
    emulator: &mut dyn TerminalEmulator,
    initial_size: TerminalSize,
) -> (CommandHistory, TerminalSize) {
    let mut command_history = CommandHistory::default();
    let mut size = initial_size;
    for entry in entries {
        match entry {
            JournalEntry::Output(bytes) | JournalEntry::Snapshot(bytes) => emulator.process_shell_output(bytes),
            JournalEntry::Resized(new_size) => {
                size = *new_size;
                emulator.resize(size);
            }
            JournalEntry::CommandRecorded(command) => command_history.record(command),
            JournalEntry::CommandForgotten(command) => command_history.forget(command),
        }
    }
    (command_history, size)
}

/// La línea, en una fila propia, seguida de los saltos necesarios para que
/// la pantalla quede vacía y todo lo anterior pase al historial de scroll.
/// Se escribe debajo de la última fila con algo, no donde esté el cursor: un
/// programa puede haberlo dejado más arriba (Claude Code lo deja en su cuadro
/// de escritura, con más líneas debajo), y eso se quedaría en pantalla, con la
/// consola nueva escribiendo encima.
pub(crate) fn line_pushed_into_history(line: &str, screen: &ScreenSnapshot) -> Vec<u8> {
    let row_above_line = screen.last_row_in_use().map_or(1, |row| row + 1);
    let mut text = format!("\x1b[{row_above_line};1H\r\n{line}");
    text.push_str(&"\r\n".repeat(screen.rows.len()));
    // La consola nueva empieza a escribir arriba (la del sistema, además, borra).
    text.push_str("\x1b[H");
    text.into_bytes()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::domain::terminal_keys::InputModes;

    /// Consola falsa que apunta todo lo que se le envía.
    struct RecordingShell {
        received: Rc<RefCell<Vec<u8>>>,
    }

    impl RunningShell for RecordingShell {
        fn send_input(&mut self, bytes: &[u8]) {
            self.received.borrow_mut().extend_from_slice(bytes);
        }
        fn resize(&mut self, _size: TerminalSize) {}
        fn take_received_output(&mut self) -> Vec<u8> {
            Vec::new()
        }
        fn has_exited(&mut self) -> bool {
            false
        }
    }

    /// Pantalla falsa en la que solo importa la línea del cursor.
    struct ScreenShowingLine {
        line_at_cursor: Rc<RefCell<String>>,
    }

    impl TerminalEmulator for ScreenShowingLine {
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
        fn scroll_history_to(&mut self, _lines_scrolled_up: usize) {}
        fn history_view(&mut self) -> HistoryView {
            HistoryView::default()
        }
        fn scroll_to_newest_output(&mut self) {}
        fn line_at_cursor(&self) -> String {
            self.line_at_cursor.borrow().clone()
        }
        fn replayable_state(&mut self) -> Vec<u8> {
            b"pantalla".to_vec()
        }
    }

    const TEST_SIZE: TerminalSize = TerminalSize { rows: 24, columns: 80 };
    const TEST_MOMENT: LocalDateTime = LocalDateTime { year: 2026, month: 11, day: 5, hour: 0, minute: 12 };

    fn test_record() -> TabRecord {
        TabRecord { id: TabId(1), profile_name: "cmd".into(), title: "cmd 1".into() }
    }

    struct SessionUnderTest {
        session: TerminalSession,
        sent_to_shell: Rc<RefCell<Vec<u8>>>,
        line_at_cursor: Rc<RefCell<String>>,
    }

    impl SessionUnderTest {
        fn new() -> Self {
            Self::built_with(|shell, emulator| TerminalSession::new(test_record(), shell, emulator, TEST_SIZE, TEST_MOMENT))
        }

        fn restored_from(journal: &[JournalEntry]) -> Self {
            Self::built_with(|shell, mut emulator| {
                let (history, size) = replay_journal(journal, emulator.as_mut(), TEST_SIZE);
                TerminalSession::restored(test_record(), history, shell, emulator, size, TEST_MOMENT)
            })
        }

        fn built_with(
            build: impl FnOnce(Box<dyn RunningShell>, Box<dyn TerminalEmulator>) -> TerminalSession,
        ) -> Self {
            let sent_to_shell = Rc::new(RefCell::new(Vec::new()));
            let line_at_cursor = Rc::new(RefCell::new(String::new()));
            let session = build(
                Box::new(RecordingShell { received: sent_to_shell.clone() }),
                Box::new(ScreenShowingLine { line_at_cursor: line_at_cursor.clone() }),
            );
            Self { session, sent_to_shell, line_at_cursor }
        }

        fn type_command_and_press_enter(&mut self, line_on_screen: &str) {
            *self.line_at_cursor.borrow_mut() = line_on_screen.to_string();
            self.session.send_key(TerminalKey::Enter, KeyModifiers::NONE);
        }

        fn show_line(&mut self, line_on_screen: &str) {
            *self.line_at_cursor.borrow_mut() = line_on_screen.to_string();
        }
    }

    #[test]
    fn commands_run_at_the_prompt_are_remembered_in_order() {
        let mut terminal = SessionUnderTest::new();
        terminal.type_command_and_press_enter(r"C:\>dir");
        terminal.type_command_and_press_enter(r"C:\>cls");

        assert_eq!(terminal.session.command_history(), ["dir", "cls"]);
    }

    #[test]
    fn lines_typed_inside_a_program_are_not_remembered() {
        let mut terminal = SessionUnderTest::new();
        terminal.type_command_and_press_enter(r"C:\>claude");
        terminal.type_command_and_press_enter("> explícame este código");

        assert_eq!(terminal.session.command_history(), ["claude"]);
    }

    #[test]
    fn knows_whether_the_cursor_is_at_the_prompt() {
        let mut terminal = SessionUnderTest::new();
        terminal.show_line(r"PS C:\Users\yo> ");
        assert!(terminal.session.is_at_local_prompt());
        terminal.show_line("> Escribe tu mensaje");
        assert!(!terminal.session.is_at_local_prompt());
    }

    #[test]
    fn new_line_is_sent_the_way_each_program_understands_it() {
        let mut terminal = SessionUnderTest::new();
        terminal.show_line(r"PS C:\Users\yo> 'uno'");
        terminal.session.insert_new_line();
        assert_eq!(*terminal.sent_to_shell.borrow(), encode_new_line_without_running(NewLineReader::PowerShell));

        terminal.sent_to_shell.borrow_mut().clear();
        terminal.show_line("> primera línea del mensaje");
        terminal.session.insert_new_line();
        assert_eq!(*terminal.sent_to_shell.borrow(), b"\n");
    }

    #[test]
    fn chosen_command_replaces_the_prompt_line_without_running_it() {
        let mut terminal = SessionUnderTest::new();
        terminal.session.replace_prompt_line_with("git status");
        assert_eq!(*terminal.sent_to_shell.borrow(), encode_prompt_line_replacement("git status"));
    }

    #[test]
    fn new_tab_starts_with_the_date_it_was_opened() {
        let mut terminal = SessionUnderTest::new();
        let opening_line = match terminal.session.take_unsaved_journal_entries().get(1) {
            Some(JournalEntry::Output(bytes)) => String::from_utf8_lossy(bytes).into_owned(),
            other => panic!("se esperaba la línea de apertura, no {other:?}"),
        };
        // La frase depende del idioma; la fecha y la hora, no.
        assert!(opening_line.contains("05/11/2026") && opening_line.contains("00:12"));
    }

    #[test]
    fn resizing_is_noted_but_does_not_count_as_an_unsaved_change() {
        let mut terminal = SessionUnderTest::new();
        terminal.session.mark_saved(Instant::now());
        terminal.session.resize_to(TerminalSize { rows: 30, columns: 100 });
        assert!(!terminal.session.has_unsaved_changes());

        terminal.type_command_and_press_enter(r"C:\>dir");
        assert!(terminal.session.has_unsaved_changes());
    }

    #[test]
    fn journal_notes_size_and_commands_as_they_happen() {
        let mut terminal = SessionUnderTest::new();
        terminal.type_command_and_press_enter(r"C:\>git status");

        let entries = terminal.session.take_unsaved_journal_entries();
        assert_eq!(entries.first(), Some(&JournalEntry::Resized(TEST_SIZE)));
        assert_eq!(entries.last(), Some(&JournalEntry::CommandRecorded("git status".into())));
        assert!(terminal.session.take_unsaved_journal_entries().is_empty(), "lo ya entregado no se repite");
    }

    #[test]
    fn reopened_tab_recovers_its_history_from_the_journal() {
        let mut terminal = SessionUnderTest::new();
        terminal.type_command_and_press_enter(r"C:\>git status");
        terminal.type_command_and_press_enter(r"C:\>cargo build");
        let journal = terminal.session.take_unsaved_journal_entries();

        let reopened = SessionUnderTest::restored_from(&journal);

        assert_eq!(reopened.session.command_history(), ["git status", "cargo build"]);
        assert!(reopened.session.journal_must_be_compacted(), "el diario se reescribe al reabrir");
    }

    #[test]
    fn a_forgotten_command_is_left_out_when_the_journal_is_compacted() {
        let mut terminal = SessionUnderTest::new();
        terminal.type_command_and_press_enter(r"C:\>login secreto");
        terminal.type_command_and_press_enter(r"C:\>dir");
        terminal.session.forget_command("login secreto");

        let compacted = terminal.session.compacted_journal();
        assert!(!compacted.contains(&JournalEntry::CommandRecorded("login secreto".into())));
        assert!(compacted.contains(&JournalEntry::CommandRecorded("dir".into())));
    }
}
