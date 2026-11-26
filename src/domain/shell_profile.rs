/// Un tipo de terminal que se puede abrir en una pestaña (cmd, PowerShell...).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellProfile {
    pub display_name: String,
    pub program: String,
    pub arguments: Vec<String>,
}

impl ShellProfile {
    pub fn new(display_name: &str, program: &str, arguments: &[&str]) -> Self {
        Self {
            display_name: display_name.to_string(),
            program: program.to_string(),
            arguments: arguments.iter().map(|argument| argument.to_string()).collect(),
        }
    }

    /// PowerShell (5 o 7) cambia `$algo` por sus variables y tiene sus propias
    /// comillas; cmd no.
    pub fn uses_powershell_syntax(&self) -> bool {
        let program = self.program.to_ascii_lowercase();
        let file_name = program.rsplit(['\\', '/']).next().unwrap_or(&program);
        file_name.starts_with("powershell") || file_name.starts_with("pwsh")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_is_recognised_by_its_program() {
        assert!(ShellProfile::new("PowerShell", "powershell.exe", &[]).uses_powershell_syntax());
        assert!(ShellProfile::new("PowerShell 7", r"C:\Program Files\PowerShell\pwsh.exe", &[]).uses_powershell_syntax());
        assert!(!ShellProfile::new("cmd", "cmd.exe", &[]).uses_powershell_syntax());
    }
}
