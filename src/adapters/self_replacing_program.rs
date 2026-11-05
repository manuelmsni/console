//! Cambia el ejecutable de la app por una versión nueva.
//!
//! Un programa abierto no se puede borrar en Windows, pero sí renombrar. Así
//! que el nuevo se escribe al lado (`console.exe.new`), el actual pasa a
//! llamarse `console.exe.old` y el nuevo ocupa su nombre. El que está abierto
//! sigue funcionando hasta que se cierra; el `.old` se borra al arrancar el
//! nuevo. En Linux funciona igual.

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::application::ports::ProgramInstaller;
use crate::i18n::tr_args;

/// El programa nuevo recibe esto y el número de proceso del anterior, y
/// espera a que se cierre antes de abrir su ventana.
pub const AFTER_UPDATE_ARGUMENT: &str = "--after-update";

pub struct SelfReplacingProgram {
    program: PathBuf,
}

impl SelfReplacingProgram {
    /// `None` si no se sabe dónde está el ejecutable (no debería pasar).
    pub fn current() -> Option<Self> {
        std::env::current_exe().ok().map(|program| Self { program })
    }

    /// `console.exe` → `console.exe.old`, etc.
    fn next_to_program(&self, suffix: &str) -> PathBuf {
        let mut name = OsString::from(self.program.as_os_str());
        name.push(suffix);
        PathBuf::from(name)
    }
}

impl ProgramInstaller for SelfReplacingProgram {
    fn replace_running_program(&self, new_program: &[u8]) -> Result<(), String> {
        let (new, old) = (self.next_to_program(".new"), self.next_to_program(".old"));
        let failed = |error: std::io::Error| tr_args("updates.error.install", &[("path", &self.program.display()), ("error", &error)]);

        fs::write(&new, new_program).map_err(failed)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&new, fs::Permissions::from_mode(0o755)).map_err(failed)?;
        }
        let _ = fs::remove_file(&old);
        fs::rename(&self.program, &old).map_err(failed)?;
        if let Err(error) = fs::rename(&new, &self.program) {
            // Se deja todo como estaba.
            let _ = fs::rename(&old, &self.program);
            return Err(failed(error));
        }
        Ok(())
    }

    fn start_new_program(&self) -> Result<(), String> {
        Command::new(&self.program)
            .arg(AFTER_UPDATE_ARGUMENT)
            .arg(std::process::id().to_string())
            .spawn()
            .map(|_| ())
            .map_err(|error| tr_args("updates.error.restart", &[("error", &error)]))
    }

    fn clean_up_previous_program(&self) {
        let _ = fs::remove_file(self.next_to_program(".old"));
        let _ = fs::remove_file(self.next_to_program(".new"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_new_program_takes_the_place_of_the_old_one() {
        let folder = std::env::temp_dir().join(format!("console-tests-actualizar-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        let program = SelfReplacingProgram { program: folder.join("console.exe") };
        fs::write(&program.program, "versión 1").unwrap();

        program.replace_running_program(b"version 2").unwrap();

        assert_eq!(fs::read(&program.program).unwrap(), b"version 2");
        assert_eq!(fs::read_to_string(folder.join("console.exe.old")).unwrap(), "versión 1");
        program.clean_up_previous_program();
        assert!(!folder.join("console.exe.old").exists());
        let _ = fs::remove_dir_all(folder);
    }
}