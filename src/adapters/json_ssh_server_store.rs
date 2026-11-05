//! Guarda la lista de servidores SSH en `ssh_servers.json`, en la carpeta de datos.

use serde::{Deserialize, Serialize};

use super::data_folder::{DataFile, DataFolder};
use crate::application::ports::SshServerStore;
use crate::domain::ssh::SshDestination;
use crate::i18n::tr_args;

pub struct JsonSshServerStore {
    file: DataFile,
}

impl JsonSshServerStore {
    pub fn in_folder(data: &DataFolder) -> Self {
        Self { file: data.file("ssh_servers.json") }
    }
}

#[derive(Serialize, Deserialize)]
struct StoredServers {
    version: u32,
    servers: Vec<String>,
}

impl SshServerStore for JsonSshServerStore {
    fn load_servers(&self) -> Result<Vec<SshDestination>, String> {
        let Some(json) = self.file.read_if_exists()? else { return Ok(Vec::new()) };
        let stored: StoredServers = serde_json::from_str(&json)
            .map_err(|error| tr_args("storage.error.read_ssh_servers", &[("error", &error)]))?;
        // Si alguien editó el fichero a mano con algo no válido, esa línea se ignora.
        Ok(stored.servers.iter().filter_map(|server| SshDestination::parse(server).ok()).collect())
    }

    fn save_servers(&self, servers: &[SshDestination]) -> Result<(), String> {
        let stored = StoredServers { version: 1, servers: servers.iter().map(|server| server.as_str().to_string()).collect() };
        let json = serde_json::to_string_pretty(&stored).map_err(|error| error.to_string())?;
        self.file.write_atomically(&json)
    }
}
