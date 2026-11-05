//! Casos de uso de la sección "Claves SSH" de los ajustes.
//!
//! Lo que necesita una frase de paso o una contraseña (crear clave, ponerle
//! frase, cargarla en el agente, instalarla en un servidor) se devuelve como
//! un comando para ejecutar en una pestaña: así se escribe directamente en
//! las herramientas de OpenSSH y nunca pasa por la app.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use super::ports::{SshServerStore, SshTools};
use crate::i18n::tr_args;
use crate::domain::ssh::{
    add_to_agent_command, change_passphrase_command, connect_command, generate_key_command,
    install_public_key_command, LoginCheck, SshAgentStatus, SshDestination, SshKeyInfo, SshKeyName,
};

/// Comprobación de si un servidor deja entrar sin escribir nada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionCheck {
    InProgress,
    Finished(LoginCheck),
}

/// Estado del agente SSH tal como se muestra, incluida su activación en curso.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentState {
    Known(SshAgentStatus),
    Activating,
    ActivationFailed(String),
}

pub struct SshSettings {
    tools: Arc<dyn SshTools>,
    server_store: Box<dyn SshServerStore>,
    keys: Vec<SshKeyInfo>,
    servers: Vec<SshDestination>,
    /// Se rellenan desde hilos aparte, porque comprobar y activar tarda.
    connection_checks: Arc<Mutex<HashMap<SshDestination, ConnectionCheck>>>,
    agent_state: Arc<Mutex<AgentState>>,
}

impl SshSettings {
    /// Devuelve también un aviso si algo no se pudo leer.
    pub fn load(tools: Arc<dyn SshTools>, server_store: Box<dyn SshServerStore>) -> (Self, Option<String>) {
        let (servers, warning) = match server_store.load_servers() {
            Ok(servers) => (servers, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let agent_state = AgentState::Known(tools.agent_status());
        let mut settings = Self {
            tools,
            server_store,
            keys: Vec::new(),
            servers,
            connection_checks: Arc::default(),
            agent_state: Arc::new(Mutex::new(agent_state)),
        };
        let key_warning = settings.reload_keys().err();
        (settings, warning.or(key_warning))
    }

    pub fn keys(&self) -> &[SshKeyInfo] {
        &self.keys
    }

    pub fn servers(&self) -> &[SshDestination] {
        &self.servers
    }

    /// Vuelve a mirar la carpeta `.ssh` y el agente, por si algo cambió en una pestaña.
    pub fn reload_keys(&mut self) -> Result<(), String> {
        if self.agent_state() != AgentState::Activating {
            set_shared(&self.agent_state, AgentState::Known(self.tools.agent_status()));
        }
        self.keys = self.tools.list_keys()?;
        Ok(())
    }

    /// Comando que crea la clave; la frase de paso se escribe en la pestaña.
    pub fn key_creation_command(&self, name: &str) -> Result<String, String> {
        let name = SshKeyName::parse(name).map_err(|error| error.to_string())?;
        if self.keys.iter().any(|key| key.name == name) {
            return Err(tr_args("ssh.error.key_exists", &[("name", &name.as_str())]));
        }
        Ok(generate_key_command(&self.tools.private_key_file(&name), &self.tools.key_comment()))
    }

    pub fn passphrase_command(&self, key: &SshKeyName) -> String {
        change_passphrase_command(&self.tools.private_key_file(key))
    }

    pub fn add_to_agent_command(&self, key: &SshKeyName) -> String {
        add_to_agent_command(&self.tools.private_key_file(key))
    }

    /// Comando que copia la clave al servidor (hay que escribir su contraseña una vez).
    pub fn install_key_command(&self, key: &SshKeyName, destination: &SshDestination) -> String {
        install_public_key_command(&self.tools.public_key_file(key), destination)
    }

    pub fn connect_command(&self, destination: &SshDestination) -> String {
        connect_command(destination)
    }

    pub fn add_server(&mut self, text: &str) -> Result<(), String> {
        let destination = SshDestination::parse(text).map_err(|error| error.to_string())?;
        if self.servers.contains(&destination) {
            return Err(tr_args("ssh.error.server_already_listed", &[("server", &destination.as_str())]));
        }
        self.servers.push(destination);
        self.server_store.save_servers(&self.servers)
    }

    pub fn remove_server(&mut self, destination: &SshDestination) -> Result<(), String> {
        self.servers.retain(|server| server != destination);
        self.server_store.save_servers(&self.servers)
    }

    pub fn connection_check(&self, destination: &SshDestination) -> Option<ConnectionCheck> {
        self.connection_checks.lock().ok()?.get(destination).copied()
    }

    /// Comprueba en segundo plano si se entra sin contraseña y llama a
    /// `when_finished` al acabar (para que la interfaz se redibuje).
    pub fn start_connection_check(&self, destination: &SshDestination, when_finished: impl Fn() + Send + 'static) {
        let checks = self.connection_checks.clone();
        let tools = self.tools.clone();
        let destination = destination.clone();
        set_check(&checks, &destination, ConnectionCheck::InProgress);
        thread::spawn(move || {
            let result = ConnectionCheck::Finished(tools.check_login_without_typing(&destination));
            set_check(&checks, &destination, result);
            when_finished();
        });
    }

    pub fn agent_state(&self) -> AgentState {
        self.agent_state.lock().map(|state| state.clone()).unwrap_or(AgentState::Known(SshAgentStatus::Stopped))
    }

    /// Activa el agente en segundo plano (Windows pedirá permiso de administrador).
    pub fn start_agent_activation(&self, when_finished: impl Fn() + Send + 'static) {
        let agent_state = self.agent_state.clone();
        let tools = self.tools.clone();
        set_shared(&agent_state, AgentState::Activating);
        thread::spawn(move || {
            let result = match tools.enable_agent() {
                Ok(()) => AgentState::Known(tools.agent_status()),
                Err(error) => AgentState::ActivationFailed(error),
            };
            set_shared(&agent_state, result);
            when_finished();
        });
    }
}

fn set_check(
    checks: &Mutex<HashMap<SshDestination, ConnectionCheck>>,
    destination: &SshDestination,
    check: ConnectionCheck,
) {
    if let Ok(mut checks) = checks.lock() {
        checks.insert(destination.clone(), check);
    }
}

fn set_shared<T>(shared: &Mutex<T>, value: T) {
    if let Ok(mut current) = shared.lock() {
        *current = value;
    }
}
