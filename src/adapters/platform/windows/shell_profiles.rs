use crate::domain::shell_profile::ShellProfile;

/// La primera es la que se abre al arrancar, con el atajo de nueva pestaña y
/// para ejecutar comandos desde los ajustes (por ejemplo, un `ssh`).
pub fn available_shell_profiles() -> Vec<ShellProfile> {
    vec![
        ShellProfile::new("cmd", "cmd.exe", &[]),
        ShellProfile::new("PowerShell", "powershell.exe", &["-NoLogo"]),
    ]
}