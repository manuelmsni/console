//! Aplicación: los casos de uso (abrir pestañas, ejecutar comandos guardados...)
//! y los puertos, es decir, los traits que el mundo exterior debe implementar.

pub mod app_settings_service;
pub mod command_library_service;
pub mod ports;
pub mod ssh_settings;
pub mod terminal_session;
pub mod terminal_tabs;
pub mod update_service;
