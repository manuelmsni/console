//! Claves, conexiones y agente con el cliente OpenSSH que trae Windows
//! (`C:\Windows\System32\OpenSSH`).

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use windows_sys::Win32::Foundation::{GetLastError, ERROR_SERVICE_DOES_NOT_EXIST};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatus, SC_MANAGER_CONNECT, SERVICE_QUERY_STATUS,
    SERVICE_RUNNING, SERVICE_STATUS,
};

use crate::application::ports::SshTools;
use crate::domain::ssh::{LoginCheck, SshAgentStatus, SshDestination, SshKeyInfo, SshKeyName};
use crate::i18n::tr;

/// Que `ssh`, `ssh-keygen` y `ssh-add` no abran una ventana negra un instante.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Segundos que se espera a que el servidor conteste al comprobar la conexión.
const CONNECTION_CHECK_TIMEOUT_SECONDS: u32 = 6;
const AGENT_SERVICE_NAME: &str = "ssh-agent";

pub struct OpenSshTools {
    ssh_folder: PathBuf,
}

impl OpenSshTools {
    /// Usa `%USERPROFILE%\.ssh`, la carpeta en la que `ssh` busca las claves.
    pub fn for_current_user() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self { ssh_folder: home.join(".ssh") }
    }

    fn private_key_path(&self, name: &SshKeyName) -> PathBuf {
        self.ssh_folder.join(name.as_str())
    }
}

impl SshTools for OpenSshTools {
    fn list_keys(&self) -> Result<Vec<SshKeyInfo>, String> {
        let entries = match fs::read_dir(&self.ssh_folder) {
            Ok(entries) => entries,
            // Sin carpeta .ssh simplemente no hay claves todavía.
            Err(_) => return Ok(Vec::new()),
        };
        let keys_in_agent = public_keys_loaded_in_agent();
        let mut keys: Vec<SshKeyInfo> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| key_info_from_public_file(&entry.path(), &keys_in_agent))
            .collect();
        keys.sort_by(|first, second| first.name.as_str().cmp(second.name.as_str()));
        Ok(keys)
    }

    fn private_key_file(&self, name: &SshKeyName) -> String {
        self.private_key_path(name).display().to_string()
    }

    fn public_key_file(&self, name: &SshKeyName) -> String {
        format!("{}.pub", self.private_key_file(name))
    }

    fn key_comment(&self) -> String {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "usuario".into());
        let computer = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "windows".into());
        format!("{user}@{computer}")
    }

    fn check_login_without_typing(&self, destination: &SshDestination) -> LoginCheck {
        let connection_timeout = format!("ConnectTimeout={CONNECTION_CHECK_TIMEOUT_SECONDS}");
        let arguments = [
            // Con detalle (-v), para saber si el servidor llegó a aceptar la clave.
            "-v",
            // Nunca preguntar nada: si hiciera falta contraseña o frase de paso, falla.
            "-o",
            "BatchMode=yes",
            // Un servidor nuevo se acepta la primera vez (como al decir "yes").
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            &connection_timeout,
            destination.as_str(),
            "exit",
        ];
        let Some(output) = run_quietly("ssh", &arguments) else { return LoginCheck::DoesNotEnter };
        if output.status.success() {
            LoginCheck::EntersWithoutAsking
        } else if String::from_utf8_lossy(&output.stderr).contains("Server accepts key") {
            // El servidor la aceptaba, pero no se pudo usar sin escribir su frase de paso.
            LoginCheck::KeyAcceptedButLocked
        } else {
            LoginCheck::DoesNotEnter
        }
    }

    fn agent_status(&self) -> SshAgentStatus {
        agent_service_status()
    }

    fn enable_agent(&self) -> Result<(), String> {
        // Se abre un PowerShell como administrador (Windows muestra su aviso de
        // permiso) que deja el servicio en inicio automático y lo arranca.
        let as_administrator = "Set-Service ssh-agent -StartupType Automatic; Start-Service ssh-agent";
        let launcher = format!(
            "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '-NoProfile -Command {as_administrator}'"
        );
        run_quietly("powershell", &["-NoProfile", "-Command", &launcher]);

        match agent_service_status() {
            SshAgentStatus::Running => Ok(()),
            SshAgentStatus::NotInstalled => Err(tr("ssh.error.agent_not_installed")),
            SshAgentStatus::Stopped => Err(tr("ssh.error.agent_not_enabled")),
        }
    }
}

/// Lee un `.pub` (formato `tipo clave comentario`) si tiene al lado su clave privada.
fn key_info_from_public_file(public_file: &Path, keys_in_agent: &[String]) -> Option<SshKeyInfo> {
    if public_file.extension()? != "pub" {
        return None;
    }
    let private_file = public_file.with_extension("");
    if !private_file.exists() {
        return None;
    }
    let name = SshKeyName::parse(public_file.file_stem()?.to_str()?).ok()?;
    let public_key = fs::read_to_string(public_file).ok()?.trim().to_string();
    let mut parts = public_key.splitn(3, ' ');
    let key_type = parts.next()?.to_string();
    let key_body = parts.next()?.to_string();
    let comment = parts.next().unwrap_or_default().to_string();

    Some(SshKeyInfo {
        loaded_in_agent: keys_in_agent.iter().any(|loaded| loaded == &format!("{key_type} {key_body}")),
        has_passphrase: is_protected_with_passphrase(&private_file),
        name,
        key_type,
        comment,
        public_key,
    })
}

/// `ssh-keygen -y` solo puede sacar la clave pública con la frase de paso
/// vacía si la clave no tiene frase de paso.
fn is_protected_with_passphrase(private_file: &Path) -> bool {
    let private_file = private_file.display().to_string();
    let opens_with_empty_passphrase = run_quietly("ssh-keygen", &["-y", "-P", "", "-f", &private_file])
        .is_some_and(|output| output.status.success());
    !opens_with_empty_passphrase
}

/// `tipo clave` de cada clave cargada en el agente (vacío si no está en marcha).
fn public_keys_loaded_in_agent() -> Vec<String> {
    let Some(output) = run_quietly("ssh-add", &["-L"]) else { return Vec::new() };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(' ');
            Some(format!("{} {}", parts.next()?, parts.next()?))
        })
        .collect()
}

/// Se pregunta a Windows por el servicio con su API: devuelve números, no
/// texto, así que funciona igual con Windows en cualquier idioma.
fn agent_service_status() -> SshAgentStatus {
    let service_name: Vec<u16> = AGENT_SERVICE_NAME.encode_utf16().chain(Some(0)).collect();
    // Seguro: se abren y cierran los handles en este mismo bloque; el nombre termina en 0.
    unsafe {
        let service_manager = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if service_manager.is_null() {
            return SshAgentStatus::Stopped;
        }
        let service = OpenServiceW(service_manager, service_name.as_ptr(), SERVICE_QUERY_STATUS);
        let status = if service.is_null() {
            if GetLastError() == ERROR_SERVICE_DOES_NOT_EXIST {
                SshAgentStatus::NotInstalled
            } else {
                SshAgentStatus::Stopped
            }
        } else {
            let mut service_status: SERVICE_STATUS = std::mem::zeroed();
            let is_running =
                QueryServiceStatus(service, &mut service_status) != 0 && service_status.dwCurrentState == SERVICE_RUNNING;
            CloseServiceHandle(service);
            if is_running {
                SshAgentStatus::Running
            } else {
                SshAgentStatus::Stopped
            }
        };
        CloseServiceHandle(service_manager);
        status
    }
}

/// Ejecuta un programa sin ventana ni entrada; `None` si no se pudo lanzar.
fn run_quietly(program: &str, arguments: &[&str]) -> Option<std::process::Output> {
    Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
}
