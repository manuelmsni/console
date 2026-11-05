//! Pestaña "Claves SSH" de los ajustes: el agente SSH de Windows, las claves
//! (con su frase de paso) y los servidores en los que instalarlas.
//!
//! Lo que pide una frase de paso o contraseña se hace en una pestaña nueva,
//! escribiéndola directamente en las herramientas de OpenSSH.

use eframe::egui::{self, Align, Color32, ComboBox, Key, Layout, Margin, RichText, TextEdit, Ui};
use egui_material_icons::icons::{
    ICON_CHECK_CIRCLE, ICON_CLOUD_UPLOAD, ICON_CONTENT_COPY, ICON_DELETE, ICON_DNS, ICON_ERROR, ICON_HELP,
    ICON_HOURGLASS_EMPTY, ICON_KEY, ICON_LOCK, ICON_LOCK_OPEN, ICON_LOGIN, ICON_NETWORK_CHECK, ICON_PASSWORD,
    ICON_REFRESH, ICON_SHIELD, ICON_WARNING,
};

use super::material_widgets::{filled_button, icon_button};
use super::settings_window::{card, section_header};
use super::theme::palette;
use super::ConsoleApp;
use crate::application::ssh_settings::{AgentState, ConnectionCheck, SshSettings};
use crate::domain::ssh::{LoginCheck, SshAgentStatus, SshDestination, SshKeyInfo, SshKeyName};
use crate::i18n::{tr, tr_args};

/// Lo escrito en los campos de la pestaña mientras está abierta.
pub(super) struct SshKeysTabState {
    new_key_name: String,
    new_server: String,
    key_to_install: Option<SshKeyName>,
}

/// Lo que el usuario ha pedido en la pestaña; se aplica después de dibujarla.
pub(super) enum SshKeysAction {
    ReloadKeys,
    EnableAgent,
    CreateKey(String),
    AddPassphrase(SshKeyName),
    AddToAgent(SshKeyName),
    CopyPublicKey(String),
    AddServer(String),
    RemoveServer(SshDestination),
    CheckConnection(SshDestination),
    InstallKey { key: Option<SshKeyName>, destination: SshDestination },
    Connect(SshDestination),
}

impl SshKeysTabState {
    pub fn new(keys: &[SshKeyInfo]) -> Self {
        // Si no hay ninguna clave, se propone el nombre que ssh usa por defecto.
        let suggested_name = if keys.is_empty() { "id_ed25519" } else { "" };
        Self {
            new_key_name: suggested_name.to_string(),
            new_server: String::new(),
            key_to_install: keys.first().map(|key| key.name.clone()),
        }
    }

    pub fn show(&mut self, ui: &mut Ui, settings: &SshSettings) -> Vec<SshKeysAction> {
        let mut actions = Vec::new();
        ui.label(RichText::new(tr("ssh.intro")).color(palette::ON_SURFACE_VARIANT));
        ui.add_space(16.0);
        show_agent_section(ui, &settings.agent_state(), &mut actions);
        ui.add_space(20.0);
        self.show_keys_section(ui, settings, &mut actions);
        ui.add_space(20.0);
        self.show_servers_section(ui, settings, &mut actions);
        actions
    }

    fn show_keys_section(&mut self, ui: &mut Ui, settings: &SshSettings, actions: &mut Vec<SshKeysAction>) {
        section_header(ui, &tr("ssh.my_keys"), |ui| {
            if icon_button(ui, ICON_REFRESH, &tr("ssh.reload")).clicked() {
                actions.push(SshKeysAction::ReloadKeys);
            }
        });
        let agent_is_running = settings.agent_state() == AgentState::Known(SshAgentStatus::Running);
        card(ui, |ui| {
            if settings.keys().is_empty() {
                ui.label(RichText::new(tr("ssh.no_keys")).color(palette::OUTLINE));
            }
            for key in settings.keys() {
                show_key_row(ui, key, agent_is_running, actions);
            }
        });
        ui.add_space(8.0);
        if text_field_with_button(ui, &mut self.new_key_name, &tr("ssh.key_name_hint"), &tr("ssh.create_key")) {
            actions.push(SshKeysAction::CreateKey(self.new_key_name.clone()));
        }
        ui.label(
            RichText::new(tr("ssh.create_key_explanation"))
                .small()
                .color(palette::OUTLINE),
        );
    }

    fn show_servers_section(&mut self, ui: &mut Ui, settings: &SshSettings, actions: &mut Vec<SshKeysAction>) {
        section_header(ui, &tr("ssh.servers"), |_| {});
        if !settings.keys().is_empty() {
            ui.horizontal(|ui| {
                ui.label(tr("ssh.key_to_install"));
                let selected = self.key_to_install.as_ref().map_or("—", SshKeyName::as_str).to_string();
                ComboBox::from_id_salt("key_to_install").selected_text(selected).show_ui(ui, |ui| {
                    for key in settings.keys() {
                        ui.selectable_value(&mut self.key_to_install, Some(key.name.clone()), key.name.as_str());
                    }
                });
            });
            ui.add_space(4.0);
        }
        card(ui, |ui| {
            if settings.servers().is_empty() {
                ui.label(RichText::new(tr("ssh.no_servers")).color(palette::OUTLINE));
            }
            for destination in settings.servers() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(ICON_DNS).size(20.0).color(palette::PRIMARY));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(destination.as_str()).monospace());
                        show_connection_check(ui, settings.connection_check(destination));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, ICON_DELETE, &tr("ssh.remove_server")).clicked() {
                            actions.push(SshKeysAction::RemoveServer(destination.clone()));
                        }
                        if icon_button(ui, ICON_LOGIN, &tr("ssh.connect")).clicked() {
                            actions.push(SshKeysAction::Connect(destination.clone()));
                        }
                        if icon_button(ui, ICON_CLOUD_UPLOAD, &tr("ssh.install_key")).clicked() {
                            let key = self.key_to_install.clone();
                            actions.push(SshKeysAction::InstallKey { key, destination: destination.clone() });
                        }
                        if icon_button(ui, ICON_NETWORK_CHECK, &tr("ssh.check")).clicked() {
                            actions.push(SshKeysAction::CheckConnection(destination.clone()));
                        }
                    });
                });
            }
        });
        ui.add_space(8.0);
        if text_field_with_button(ui, &mut self.new_server, &tr("ssh.server_hint"), &tr("common.add")) {
            actions.push(SshKeysAction::AddServer(self.new_server.clone()));
        }
    }
}

fn show_agent_section(ui: &mut Ui, agent_state: &AgentState, actions: &mut Vec<SshKeysAction>) {
    section_header(ui, &tr("ssh.agent.section"), |_| {});
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(ICON_SHIELD).size(20.0).color(palette::PRIMARY));
            ui.vertical(|ui| {
                ui.label(tr("ssh.agent.explanation"));
                let (icon, text, color) = match agent_state {
                    AgentState::Known(SshAgentStatus::Running) => (ICON_CHECK_CIRCLE, tr("ssh.agent.running"), palette::SUCCESS),
                    AgentState::Known(SshAgentStatus::Stopped) => (ICON_WARNING, tr("ssh.agent.stopped"), palette::ERROR),
                    AgentState::Known(SshAgentStatus::NotInstalled) => {
                        (ICON_ERROR, tr("ssh.agent.not_installed"), palette::ERROR)
                    }
                    AgentState::Activating => (ICON_HOURGLASS_EMPTY, tr("ssh.agent.activating"), palette::ON_SURFACE_VARIANT),
                    AgentState::ActivationFailed(error) => (ICON_ERROR, error.clone(), palette::ERROR),
                };
                status_line(ui, icon, &text, color);
            });
        });
        let can_be_activated = matches!(agent_state, AgentState::Known(SshAgentStatus::Stopped) | AgentState::ActivationFailed(_));
        if can_be_activated {
            ui.add_space(8.0);
            if filled_button(ui, &tr("ssh.agent.enable")).clicked() {
                actions.push(SshKeysAction::EnableAgent);
            }
        }
    });
}

fn show_key_row(ui: &mut Ui, key: &SshKeyInfo, agent_is_running: bool, actions: &mut Vec<SshKeysAction>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(ICON_KEY).size(20.0).color(palette::PRIMARY));
        ui.vertical(|ui| {
            ui.label(RichText::new(key.name.as_str()).strong());
            ui.label(RichText::new(format!("{} · {}", key.key_type, key.comment)).small().color(palette::ON_SURFACE_VARIANT));
            ui.horizontal(|ui| {
                if key.has_passphrase {
                    status_line(ui, ICON_LOCK, &tr("ssh.key.has_passphrase"), palette::SUCCESS);
                } else {
                    status_line(ui, ICON_LOCK_OPEN, &tr("ssh.key.no_passphrase"), palette::ERROR);
                }
                if key.loaded_in_agent {
                    status_line(ui, ICON_CHECK_CIRCLE, &tr("ssh.key.in_agent"), palette::SUCCESS);
                }
            });
            // Con frase de paso pero fuera del agente, ssh la pide en cada conexión:
            // es lo más fácil de pasar por alto, así que se avisa con un botón con texto.
            if key.has_passphrase && !key.loaded_in_agent {
                status_line(ui, ICON_WARNING, &tr("ssh.key.not_in_agent"), palette::ERROR);
                if agent_is_running && filled_button(ui, &tr("ssh.key.add_to_agent")).clicked() {
                    actions.push(SshKeysAction::AddToAgent(key.name.clone()));
                }
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, ICON_CONTENT_COPY, &tr("ssh.key.copy_public")).clicked() {
                actions.push(SshKeysAction::CopyPublicKey(key.public_key.clone()));
            }
            if !key.has_passphrase && icon_button(ui, ICON_PASSWORD, &tr("ssh.key.set_passphrase")).clicked() {
                actions.push(SshKeysAction::AddPassphrase(key.name.clone()));
            }
        });
    });
}

impl ConsoleApp {
    pub(super) fn apply_ssh_keys_actions(&mut self, actions: Vec<SshKeysAction>, egui_context: &egui::Context) {
        for action in actions {
            match action {
                SshKeysAction::ReloadKeys => match self.ssh_settings.reload_keys() {
                    Ok(()) => self.show_status(tr("ssh.keys_reloaded")),
                    Err(error) => self.show_status(error),
                },
                SshKeysAction::EnableAgent => {
                    let context_to_wake_up = egui_context.clone();
                    self.ssh_settings.start_agent_activation(move || context_to_wake_up.request_repaint());
                }
                SshKeysAction::CreateKey(name) => match self.ssh_settings.key_creation_command(&name) {
                    Ok(command) => self.run_in_new_tab_and_explain(&command, &tr("ssh.explain.create_key")),
                    Err(error) => self.show_status(error),
                },
                SshKeysAction::AddPassphrase(key) => {
                    let command = self.ssh_settings.passphrase_command(&key);
                    self.run_in_new_tab_and_explain(&command, &tr("ssh.explain.change_passphrase"));
                }
                SshKeysAction::AddToAgent(key) => {
                    let command = self.ssh_settings.add_to_agent_command(&key);
                    self.run_in_new_tab_and_explain(&command, &tr("ssh.explain.add_to_agent"));
                }
                SshKeysAction::CopyPublicKey(public_key) => self.copy_to_clipboard(&public_key),
                SshKeysAction::AddServer(text) => match self.ssh_settings.add_server(&text) {
                    Ok(()) => {
                        if let Some(settings) = &mut self.settings_window {
                            settings.ssh_keys_mut().new_server.clear();
                        }
                    }
                    Err(error) => self.show_status(error),
                },
                SshKeysAction::RemoveServer(destination) => {
                    if let Err(error) = self.ssh_settings.remove_server(&destination) {
                        self.show_status(error);
                    }
                }
                SshKeysAction::CheckConnection(destination) => {
                    let context_to_wake_up = egui_context.clone();
                    self.ssh_settings.start_connection_check(&destination, move || context_to_wake_up.request_repaint());
                }
                SshKeysAction::InstallKey { key: None, .. } => self.show_status(tr("ssh.create_key_first")),
                SshKeysAction::InstallKey { key: Some(key), destination } => {
                    let command = self.ssh_settings.install_key_command(&key, &destination);
                    let explanation = tr_args("ssh.explain.install_key", &[("server", &destination.as_str())]);
                    self.run_in_new_tab_and_explain(&command, &explanation);
                }
                SshKeysAction::Connect(destination) => {
                    let command = self.ssh_settings.connect_command(&destination);
                    self.close_settings();
                    self.open_tab_running(&command);
                }
            }
        }
    }

    /// Cierra los ajustes para que se vea la pestaña donde hay que escribir.
    fn run_in_new_tab_and_explain(&mut self, command: &str, explanation: &str) {
        self.close_settings();
        self.open_tab_running(command);
        self.show_status(explanation);
    }
}

fn show_connection_check(ui: &mut Ui, check: Option<ConnectionCheck>) {
    let (icon, text, color) = match check {
        None => (ICON_HELP, tr("ssh.check.not_checked"), palette::OUTLINE),
        Some(ConnectionCheck::InProgress) => (ICON_HOURGLASS_EMPTY, tr("ssh.check.in_progress"), palette::ON_SURFACE_VARIANT),
        Some(ConnectionCheck::Finished(LoginCheck::EntersWithoutAsking)) => {
            (ICON_CHECK_CIRCLE, tr("ssh.check.ok"), palette::SUCCESS)
        }
        Some(ConnectionCheck::Finished(LoginCheck::KeyAcceptedButLocked)) => (
            ICON_WARNING,
            tr("ssh.check.locked"),
            palette::ERROR,
        ),
        Some(ConnectionCheck::Finished(LoginCheck::DoesNotEnter)) => {
            (ICON_ERROR, tr("ssh.check.fails"), palette::ERROR)
        }
    };
    status_line(ui, icon, &text, color);
}

fn status_line(ui: &mut Ui, icon: &str, text: &str, color: Color32) {
    ui.label(RichText::new(format!("{icon} {text}")).small().color(color));
}

/// Campo de texto con un botón al lado. Devuelve `true` al pulsar el botón
/// o Enter dentro del campo.
fn text_field_with_button(ui: &mut Ui, text: &mut String, hint: &str, button_label: &str) -> bool {
    ui.horizontal(|ui| {
        let field = ui.add(TextEdit::singleline(text).hint_text(hint).desired_width(280.0).margin(Margin::symmetric(12.0, 10.0)));
        let enter_pressed = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        filled_button(ui, button_label).clicked() || enter_pressed
    })
    .inner
}
