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
}
