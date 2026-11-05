//! Puertos: lo que la aplicación necesita del exterior, sin decir cómo se hace.
//! Las implementaciones concretas viven en `adapters`.

use crate::domain::app_settings::AppSettings;
use crate::domain::command_library::CommandLibrary;
use crate::domain::local_time::LocalDateTime;
use crate::domain::screen_snapshot::ScreenSnapshot;
use crate::domain::shell_profile::ShellProfile;
use crate::domain::ssh::{LoginCheck, SshAgentStatus, SshDestination, SshKeyInfo, SshKeyName};
use crate::domain::tab_journal::{JournalEntry, TabId, TabRecord};
use crate::domain::terminal_grid::{GridPosition, TerminalSize};
use crate::domain::terminal_keys::InputModes;
use crate::domain::updates::Release;

/// Arranca procesos de terminal (cmd, PowerShell...).
pub trait ShellLauncher {
    fn launch(&self, profile: &ShellProfile, size: TerminalSize) -> Result<Box<dyn RunningShell>, String>;
}

/// Un proceso de terminal en marcha con el que se habla por bytes.
pub trait RunningShell {
    fn send_input(&mut self, bytes: &[u8]);
    fn resize(&mut self, size: TerminalSize);
    /// Lo que el proceso ha escrito desde la última vez que se preguntó.
    fn take_received_output(&mut self) -> Vec<u8>;
    fn has_exited(&mut self) -> bool;
}

/// Interpreta la salida de un proceso de terminal (colores, cursor,
/// borrados de pantalla...) y recuerda cómo queda la pantalla.
pub trait TerminalEmulator {
    fn process_shell_output(&mut self, bytes: &[u8]);
    fn resize(&mut self, size: TerminalSize);
    fn snapshot(&self) -> ScreenSnapshot;
    /// Texto visible entre dos celdas, ambas incluidas.
    fn text_between(&self, first: GridPosition, last: GridPosition) -> String;
    fn input_modes(&self) -> InputModes;
    /// Positivo: hacia líneas más antiguas. Negativo: hacia las más recientes.
    fn scroll_history_by(&mut self, lines: i32);
    fn scroll_to_newest_output(&mut self);
    /// La línea donde está el cursor, entera aunque ocupe varias filas.
    fn line_at_cursor(&self) -> String;
    /// Bytes que, dados a un emulador nuevo del mismo tamaño, dejan la
    /// pantalla y el historial de scroll como están ahora.
    fn replayable_state(&mut self) -> Vec<u8>;
}

pub trait TerminalEmulatorFactory {
    fn create_emulator(&self, size: TerminalSize) -> Box<dyn TerminalEmulator>;
}

/// Guarda y recupera la biblioteca de comandos.
pub trait CommandLibraryStore {
    /// `Ok(None)` si todavía no se ha guardado nunca nada.
    fn load_library(&self) -> Result<Option<CommandLibrary>, String>;
    fn save_library(&self, library: &CommandLibrary) -> Result<(), String>;
}

/// Los diarios de las pestañas abiertas, guardados según van ocurriendo las
/// cosas, para reconstruirlas al volver a abrir la app (aunque se cerrara de golpe).
pub trait TabJournalStore {
    /// Las pestañas abiertas, en orden, cada una con las entradas de su diario.
    fn load_open_tabs(&mut self) -> Result<Vec<(TabRecord, Vec<JournalEntry>)>, String>;
    fn save_open_tabs(&mut self, tabs: &[TabRecord]) -> Result<(), String>;
    /// Añade al final del diario, sin reescribir lo anterior.
    fn append(&mut self, tab: TabId, entries: &[JournalEntry]) -> Result<(), String>;
    /// Sustituye el diario entero (al compactarlo).
    fn replace(&mut self, tab: TabId, entries: &[JournalEntry]) -> Result<(), String>;
    /// Lo que hay guardado ahora mismo en el diario de una pestaña.
    fn read(&mut self, tab: TabId) -> Result<Vec<JournalEntry>, String>;
    /// Bytes que ocupa ya el diario, para saber cuándo compactarlo.
    fn journal_size(&self, tab: TabId) -> u64;
    fn delete(&mut self, tab: TabId) -> Result<(), String>;
}

/// Las herramientas de OpenSSH del sistema (`ssh-keygen`, `ssh`, `ssh-add`),
/// la carpeta `.ssh` y el agente SSH de Windows.
pub trait SshTools: Send + Sync {
    /// Incluye, para cada clave, si tiene frase de paso y si está en el agente.
    fn list_keys(&self) -> Result<Vec<SshKeyInfo>, String>;
    fn private_key_file(&self, name: &SshKeyName) -> String;
    fn public_key_file(&self, name: &SshKeyName) -> String;
    /// `usuario@equipo`, para saber de quién es una clave al verla en un servidor.
    fn key_comment(&self) -> String;
    /// Intenta entrar sin escribir nada. Tarda unos segundos: no llamar desde la interfaz.
    fn check_login_without_typing(&self, destination: &SshDestination) -> LoginCheck;
    fn agent_status(&self) -> SshAgentStatus;
    /// Deja el agente arrancado y en inicio automático. Pide permiso de
    /// administrador y espera a que se conceda: no llamar desde la interfaz.
    fn enable_agent(&self) -> Result<(), String>;
}

/// La lista de servidores a los que uno se conecta.
pub trait SshServerStore {
    fn load_servers(&self) -> Result<Vec<SshDestination>, String>;
    fn save_servers(&self, servers: &[SshDestination]) -> Result<(), String>;
}

/// Guarda y recupera los ajustes generales (autoguardado, atajos).
pub trait AppSettingsStore {
    /// `Ok(None)` si todavía no se han cambiado nunca.
    fn load_settings(&self) -> Result<Option<AppSettings>, String>;
    fn save_settings(&self, settings: &AppSettings) -> Result<(), String>;
}

/// La hora del sistema.
pub trait Clock {
    fn now(&self) -> LocalDateTime;
}

/// Portapapeles del sistema operativo.
pub trait Clipboard {
    fn copy_text(&mut self, text: &str) -> Result<(), String>;
    fn read_text(&mut self) -> Result<String, String>;
}

/// El navegador web del usuario.
pub trait WebBrowser {
    fn open(&self, url: &str) -> Result<(), String>;
}

/// Dónde se publican las versiones nuevas (las releases de GitHub).
pub trait ReleaseSource: Send + Sync {
    /// La última release publicada; `Ok(None)` si todavía no hay ninguna.
    fn latest_release(&self) -> Result<Option<Release>, String>;
    fn download(&self, url: &str) -> Result<Vec<u8>, String>;
}

/// Cambia el programa que se está ejecutando por una versión nueva.
pub trait ProgramInstaller: Send + Sync {
    /// Sustituye el ejecutable en disco. El que está abierto sigue funcionando
    /// hasta que se cierre.
    fn replace_running_program(&self, new_program: &[u8]) -> Result<(), String>;
    /// Arranca el programa nuevo, que espera a que este se cierre.
    fn start_new_program(&self) -> Result<(), String>;
    /// Borra lo que haya quedado de la versión anterior.
    fn clean_up_previous_program(&self);
}
