use super::ports::SavedPlacesStore;
use crate::domain::saved_places::{PlaceFields, PlaceId, SavedPlaces};

/// Casos de uso de las rutas guardadas de cada terminal. Cada cambio se guarda al momento.
pub struct SavedPlacesService {
    places: SavedPlaces,
    store: Box<dyn SavedPlacesStore>,
}

impl SavedPlacesService {
    /// Devuelve también un aviso si las rutas guardadas no se pudieron leer.
    pub fn load(store: Box<dyn SavedPlacesStore>) -> (Self, Option<String>) {
        let (places, warning) = match store.load_places() {
            Ok(saved) => (saved.unwrap_or_default(), None),
            Err(error) => (SavedPlaces::default(), Some(error)),
        };
        (Self { places, store }, warning)
    }

    pub fn places(&self) -> &SavedPlaces {
        &self.places
    }

    pub fn add(&mut self, profile_name: &str, fields: &PlaceFields) -> Result<(), String> {
        let details = fields.parse().map_err(|error| error.to_string())?;
        self.places.add(profile_name, details);
        self.save()
    }

    pub fn update(&mut self, id: PlaceId, fields: &PlaceFields) -> Result<(), String> {
        let details = fields.parse().map_err(|error| error.to_string())?;
        self.places.update(id, details);
        self.save()
    }

    pub fn remove(&mut self, id: PlaceId) -> Result<(), String> {
        self.places.remove(id);
        self.save()
    }

    pub fn move_by(&mut self, id: PlaceId, steps: isize) -> Result<(), String> {
        self.places.move_by(id, steps);
        self.save()
    }

    fn save(&mut self) -> Result<(), String> {
        self.store.save_places(&self.places)
    }
}
