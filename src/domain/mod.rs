//! Dominio: las reglas de la aplicación, sin saber nada de ventanas,
//! ficheros ni procesos. Todo lo de aquí se puede probar con tests simples.

pub mod app_settings;
pub mod command_history;
pub mod command_library;
pub mod local_time;
#[cfg_attr(not(feature = "portable"), allow(dead_code))]
pub mod master_password;
pub mod screen_snapshot;
pub mod shell_profile;
pub mod shell_prompt;
pub mod ssh;
pub mod tab_journal;
pub mod terminal_grid;
pub mod terminal_keys;
pub mod updates;
pub mod web_links;
