//! Guarda las rutas de cada terminal en `places.json`, en la carpeta de datos (cifrado).

use serde::{Deserialize, Serialize};

use super::data_folder::{DataFile, DataFolder};
use crate::application::ports::SavedPlacesStore;
use crate::domain::saved_places::{PlaceDetails, PlaceTarget, RemoteFolder, SavedPlaces};
use crate::domain::ssh::SshDestination;
use crate::i18n::tr_args;

pub struct JsonSavedPlacesStore {
    file: DataFile,
}

impl JsonSavedPlacesStore {
    pub fn in_folder(data: &DataFolder) -> Self {
        Self { file: data.file("places.json") }
    }
}

/// Formato del fichero, separado del dominio igual que en los demás. Los
/// identificadores no se guardan: se dan de nuevo al leer.
#[derive(Serialize, Deserialize)]
struct StoredPlaces {
    version: u32,
    places: Vec<StoredPlace>,
}

#[derive(Serialize, Deserialize)]
struct StoredPlace {
    /// El nombre de la terminal (`cmd`, `PowerShell`...).
    terminal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    alias: Option<String>,
    target: StoredTarget,
    /// Falta en ficheros de versiones anteriores: sin comando.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    command: Option<String>,
}

/// `{"local_folder": "C:\código"}` o
/// `{"ssh": {"destination": "ana@web", "port": 2222, "remote_folder": "/srv"}}`.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredTarget {
    LocalFolder(String),
    Ssh {
        destination: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port: Option<u16>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remote_folder: Option<String>,
    },
}

fn stored_from(places: &SavedPlaces) -> StoredPlaces {
    let places = places
        .all()
        .map(|(terminal, place)| StoredPlace {
            terminal: terminal.to_string(),
            alias: place.alias.clone(),
            command: place.command.clone(),
            target: match &place.target {
                PlaceTarget::LocalFolder(folder) => StoredTarget::LocalFolder(folder.clone()),
                PlaceTarget::Ssh { destination, port, remote_folder } => StoredTarget::Ssh {
                    destination: destination.as_str().to_string(),
                    port: *port,
                    remote_folder: remote_folder.as_ref().map(|folder| folder.as_str().to_string()),
                },
            },
        })
        .collect();
    StoredPlaces { version: 1, places }
}

fn places_from(stored: StoredPlaces) -> SavedPlaces {
    let mut places = SavedPlaces::default();
    for place in stored.places {
        let target = match place.target {
            StoredTarget::LocalFolder(folder) => PlaceTarget::LocalFolder(folder),
            StoredTarget::Ssh { destination, port, remote_folder } => {
                // Si alguien editó el fichero a mano con algo no válido, esa ruta se ignora.
                let Ok(destination) = SshDestination::parse(&destination) else { continue };
                let Ok(remote_folder) = RemoteFolder::parse(remote_folder.as_deref().unwrap_or_default()) else { continue };
                PlaceTarget::Ssh { destination, port, remote_folder }
            }
        };
        places.add(&place.terminal, PlaceDetails { alias: place.alias, target, command: place.command });
    }
    places
}

impl SavedPlacesStore for JsonSavedPlacesStore {
    fn load_places(&self) -> Result<Option<SavedPlaces>, String> {
        let Some(json) = self.file.read_if_exists()? else { return Ok(None) };
        let stored: StoredPlaces =
            serde_json::from_str(&json).map_err(|error| tr_args("storage.error.read_places", &[("error", &error)]))?;
        Ok(Some(places_from(stored)))
    }

    fn save_places(&self, places: &SavedPlaces) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&stored_from(places)).map_err(|error| error.to_string())?;
        self.file.write_atomically(&json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_aliases_and_order_survive_saving_and_loading() {
        let mut places = SavedPlaces::default();
        let details = |alias: Option<&str>, target, command: Option<&str>| PlaceDetails {
            alias: alias.map(str::to_string),
            target,
            command: command.map(str::to_string),
        };
        places.add("cmd", details(Some("Código"), PlaceTarget::LocalFolder(r"C:\code".into()), Some("code .")));
        places.add("cmd", details(None, PlaceTarget::LocalFolder(r"D:\".into()), None));
        let ssh = PlaceTarget::Ssh {
            destination: SshDestination::parse("ana@web").unwrap(),
            port: Some(2222),
            remote_folder: RemoteFolder::parse("~/app").unwrap(),
        };
        places.add("PowerShell", details(None, ssh, Some("git status")));

        let json = serde_json::to_string(&stored_from(&places)).unwrap();
        let loaded = places_from(serde_json::from_str(&json).unwrap());

        // Los identificadores se dan de nuevo al leer: se compara lo demás.
        let contents = |places: &SavedPlaces, terminal: &str| {
            places
                .places_of(terminal)
                .iter()
                .map(|place| (place.alias.clone(), place.target.clone(), place.command.clone()))
                .collect::<Vec<_>>()
        };
        for terminal in ["cmd", "PowerShell"] {
            assert_eq!(contents(&loaded, terminal), contents(&places, terminal));
        }
    }
}
