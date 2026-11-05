//! Comandos ejecutados en una pestaña, para poder elegirlos de nuevo.

/// Cuántos comandos se recuerdan como mucho por pestaña.
const MAX_REMEMBERED_COMMANDS: usize = 1000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandHistory {
    /// Del más antiguo al más reciente.
    commands: Vec<String>,
}

impl CommandHistory {
    pub fn commands(&self) -> &[String] {
        &self.commands
    }

    /// Apunta un comando recién ejecutado. No repite el último si es igual.
    pub fn record(&mut self, command: &str) {
        let command = command.trim();
        if command.is_empty() || self.commands.last().is_some_and(|last| last == command) {
            return;
        }
        self.commands.push(command.to_string());
        if self.commands.len() > MAX_REMEMBERED_COMMANDS {
            self.commands.remove(0);
        }
    }

    /// Quita el comando del historial, todas las veces que aparezca (por
    /// ejemplo, si llevaba una contraseña).
    pub fn forget(&mut self, command: &str) {
        self.commands.retain(|remembered| remembered != command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_kept_from_oldest_to_newest() {
        let mut history = CommandHistory::default();
        history.record("dir");
        history.record("cd proyectos");
        assert_eq!(history.commands(), ["dir", "cd proyectos"]);
    }

    #[test]
    fn empty_and_repeated_commands_are_not_recorded() {
        let mut history = CommandHistory::default();
        for command in ["dir", "dir", "   ", "cls"] {
            history.record(command);
        }
        assert_eq!(history.commands(), ["dir", "cls"]);
    }

    #[test]
    fn forgetting_a_command_removes_every_copy() {
        let mut history = CommandHistory::default();
        for command in ["login secreto", "dir", "login secreto"] {
            history.record(command);
        }
        history.forget("login secreto");
        assert_eq!(history.commands(), ["dir"]);
    }

    #[test]
    fn only_the_most_recent_commands_are_kept() {
        let mut history = CommandHistory::default();
        for number in 0..=MAX_REMEMBERED_COMMANDS {
            history.record(&format!("comando {number}"));
        }
        assert_eq!(history.commands().len(), MAX_REMEMBERED_COMMANDS);
        assert_eq!(history.commands()[0], "comando 1");
    }
}
