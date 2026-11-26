use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use crate::application::ports::SystemCommandRunner;
use crate::i18n::tr_args;

/// Sin esto, cada macro abriría una ventana negra de cmd. Los programas que
/// lanza el comando sí se ven (y `start cmd /k ...` abre una consola aparte).
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Ejecuta las macros con `cmd /c`, como si se escribieran en Ejecutar o en un
/// `.bat`: entienden `%USERPROFILE%`, `start`, `&`...
pub struct WindowsCommandRunner;

impl SystemCommandRunner for WindowsCommandRunner {
    fn start(&self, command_line: &str) -> Result<(), String> {
        let mut command = Command::new("cmd");
        // `raw_arg`: cmd tiene sus propias reglas de comillas y la línea le
        // tiene que llegar tal cual la escribió el usuario.
        command
            .raw_arg("/d /s /c")
            .raw_arg(format!("\"{command_line}\""))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        if let Some(home) = dirs::home_dir() {
            command.current_dir(home);
        }
        // No se espera a que termine: el proceso sigue aunque se cierre la consola.
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| tr_args("macros.error.start", &[("command", &command_line), ("error", &error)]))
    }
}
