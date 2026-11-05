//! Procesos de terminal reales en una pseudoterminal, con `portable-pty`: en
//! Windows usa la pseudoconsola (ConPTY) y en Linux o macOS, un PTY normal.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::application::ports::{RunningShell, ShellLauncher};
use crate::domain::shell_profile::ShellProfile;
use crate::domain::terminal_grid::TerminalSize;
use crate::i18n::tr_args;

/// Función a la que se llama cada vez que llega salida nueva,
/// para que la interfaz sepa que tiene que redibujar.
pub type OutputArrivedNotifier = Arc<dyn Fn() + Send + Sync>;

pub struct PtyShellLauncher {
    notify_output_arrived: OutputArrivedNotifier,
}

impl PtyShellLauncher {
    pub fn new(notify_output_arrived: OutputArrivedNotifier) -> Self {
        Self { notify_output_arrived }
    }
}

impl ShellLauncher for PtyShellLauncher {
    fn launch(&self, profile: &ShellProfile, size: TerminalSize) -> Result<Box<dyn RunningShell>, String> {
        let pty_pair = native_pty_system()
            .openpty(pty_size_of(size))
            .map_err(|error| tr_args("terminal.error.pseudoconsole", &[("error", &error)]))?;

        let mut command = CommandBuilder::new(&profile.program);
        command.args(&profile.arguments);
        if let Some(home_folder) = dirs::home_dir() {
            command.cwd(home_folder);
        }

        let child = pty_pair
            .slave
            .spawn_command(command)
            .map_err(|error| tr_args("terminal.error.start", &[("program", &profile.program), ("error", &error)]))?;
        drop(pty_pair.slave);

        let reader = pty_pair.master.try_clone_reader().map_err(|error| error.to_string())?;
        let writer = pty_pair.master.take_writer().map_err(|error| error.to_string())?;
        let received_output = read_output_in_background(reader, self.notify_output_arrived.clone());

        Ok(Box::new(PtyShell { master: pty_pair.master, writer, child, received_output }))
    }
}

struct PtyShell {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    received_output: Receiver<Vec<u8>>,
}

impl RunningShell for PtyShell {
    fn send_input(&mut self, bytes: &[u8]) {
        // Si el proceso ya terminó no hay a quién escribir; la pestaña ya lo indica.
        let _ = self.writer.write_all(bytes).and_then(|_| self.writer.flush());
    }

    fn resize(&mut self, size: TerminalSize) {
        let _ = self.master.resize(pty_size_of(size));
    }

    fn take_received_output(&mut self) -> Vec<u8> {
        self.received_output.try_iter().flatten().collect()
    }

    fn has_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }
}

impl Drop for PtyShell {
    /// Cerrar la pestaña termina también el proceso.
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Leer de la terminal bloquea hasta que hay datos, así que se hace en un
/// hilo aparte que va pasando lo leído por un canal.
fn read_output_in_background(mut reader: Box<dyn Read + Send>, notify_output_arrived: OutputArrivedNotifier) -> Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(bytes_read) => {
                    if sender.send(buffer[..bytes_read].to_vec()).is_err() {
                        break;
                    }
                    notify_output_arrived();
                }
            }
        }
        notify_output_arrived();
    });
    receiver
}

fn pty_size_of(size: TerminalSize) -> PtySize {
    PtySize { rows: size.rows, cols: size.columns, pixel_width: 0, pixel_height: 0 }
}
