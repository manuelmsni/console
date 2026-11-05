//! Adaptadores: implementaciones concretas de los puertos (ConPTY, vt100,
//! ficheros cifrados con la cuenta o con contraseña, OpenSSH, portapapeles) y la interfaz gráfica que lo maneja todo.

pub mod egui_ui;
pub mod encrypted_tab_journal_store;
pub mod json_app_settings_store;
pub mod json_command_library_store;
pub mod data_folder;
pub mod file_cipher;
pub mod github_releases;
pub mod json_ssh_server_store;
#[cfg_attr(not(feature = "portable"), allow(dead_code))]
pub mod password_cipher;
pub mod platform;
pub mod pty_shell;
pub mod self_replacing_program;
pub mod system_clipboard;
pub mod vt100_emulator;
