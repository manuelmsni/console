//! Diarios de las pestañas en la subcarpeta `tabs` de la carpeta de datos:
//!
//! - `index.json`: qué pestañas hay abiertas y en qué orden (cifrado).
//! - `<id>.journal`: lo ocurrido en cada pestaña, añadido al final según pasa.
//!   Cada entrada se cifra por separado y va precedida de su longitud, así que
//!   si la app se cierra de golpe a mitad de escribir una, al leer se descarta
//!   solo esa entrada incompleta.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::data_folder::{remove_if_exists, DataFile, DataFolder};
use super::file_cipher::FileCipher;
use crate::application::ports::TabJournalStore;
use crate::domain::tab_journal::{JournalEntry, TabId, TabRecord};
use crate::i18n::tr_args;

const LENGTH_PREFIX_SIZE: usize = 4;
const JOURNAL_EXTENSION: &str = "journal";

pub struct EncryptedTabJournalStore {
    data: DataFolder,
    folder: PathBuf,
    /// Ficheros de diario abiertos para añadir, para no abrirlos en cada fotograma.
    open_journals: HashMap<TabId, File>,
    journal_sizes: HashMap<TabId, u64>,
}

impl EncryptedTabJournalStore {
    pub fn in_folder(data: &DataFolder) -> Self {
        let data = data.subfolder("tabs");
        Self {
            folder: data.path().to_path_buf(),
            data,
            open_journals: HashMap::new(),
            journal_sizes: HashMap::new(),
        }
    }

    fn index_file(&self) -> DataFile {
        self.data.file("index.json")
    }

    fn journal_path(&self, tab: TabId) -> PathBuf {
        self.folder.join(format!("{}.{JOURNAL_EXTENSION}", tab.0))
    }

    fn journal_for_appending(&mut self, tab: TabId) -> Result<&mut File, String> {
        if !self.open_journals.contains_key(&tab) {
            fs::create_dir_all(&self.folder).map_err(|error| error.to_string())?;
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.journal_path(tab))
                .map_err(|error| tr_args("storage.error.open_journal", &[("error", &error)]))?;
            let current_size = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
            self.journal_sizes.insert(tab, current_size);
            self.open_journals.insert(tab, file);
        }
        Ok(self.open_journals.get_mut(&tab).expect("se acaba de abrir"))
    }

    /// Borra los diarios que ya no están en el índice (por ejemplo, de una
    /// pestaña que se cerró justo cuando la app se colgaba).
    fn remove_orphan_journals(&self, open_tabs: &[TabRecord]) {
        let Ok(entries) = fs::read_dir(&self.folder) else { return };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let is_journal = path.extension().is_some_and(|extension| extension == JOURNAL_EXTENSION);
            let belongs_to_open_tab = path
                .file_stem()
                .and_then(|stem| stem.to_str()?.parse::<u64>().ok())
                .is_some_and(|id| open_tabs.iter().any(|tab| tab.id.0 == id));
            if is_journal && !belongs_to_open_tab {
                let _ = fs::remove_file(path);
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct StoredIndex {
    version: u32,
    tabs: Vec<StoredTab>,
}

#[derive(Serialize, Deserialize)]
struct StoredTab {
    id: u64,
    terminal: String,
    title: String,
}

impl TabJournalStore for EncryptedTabJournalStore {
    fn load_open_tabs(&mut self) -> Result<Vec<(TabRecord, Vec<JournalEntry>)>, String> {
        let Some(json) = self.index_file().read_if_exists()? else { return Ok(Vec::new()) };
        let index: StoredIndex =
            serde_json::from_str(&json).map_err(|error| tr_args("storage.error.read_tab_index", &[("error", &error)]))?;
        let records: Vec<TabRecord> = index
            .tabs
            .into_iter()
            .map(|tab| TabRecord { id: TabId(tab.id), profile_name: tab.terminal, title: tab.title })
            .collect();
        self.remove_orphan_journals(&records);

        Ok(records
            .into_iter()
            .map(|record| {
                let journal = fs::read(self.journal_path(record.id)).map(|bytes| entries_from(self.data.cipher(), &bytes)).unwrap_or_default();
                (record, journal)
            })
            .collect())
    }

    fn save_open_tabs(&mut self, tabs: &[TabRecord]) -> Result<(), String> {
        let index = StoredIndex {
            version: 1,
            tabs: tabs
                .iter()
                .map(|tab| StoredTab { id: tab.id.0, terminal: tab.profile_name.clone(), title: tab.title.clone() })
                .collect(),
        };
        let json = serde_json::to_string_pretty(&index).map_err(|error| error.to_string())?;
        self.index_file().write_atomically(&json)
    }

    fn append(&mut self, tab: TabId, entries: &[JournalEntry]) -> Result<(), String> {
        let bytes = encrypted_records(self.data.cipher(), entries)?;
        let file = self.journal_for_appending(tab)?;
        file.write_all(&bytes).map_err(|error| tr_args("storage.error.write_journal", &[("error", &error)]))?;
        *self.journal_sizes.entry(tab).or_default() += bytes.len() as u64;
        Ok(())
    }

    fn replace(&mut self, tab: TabId, entries: &[JournalEntry]) -> Result<(), String> {
        // Windows no deja sustituir un fichero que está abierto.
        self.open_journals.remove(&tab);
        fs::create_dir_all(&self.folder).map_err(|error| error.to_string())?;
        let bytes = encrypted_records(self.data.cipher(), entries)?;
        let path = self.journal_path(tab);
        let temporary_path = path.with_extension("tmp");
        fs::write(&temporary_path, &bytes)
            .and_then(|_| fs::rename(&temporary_path, &path))
            .map_err(|error| tr_args("storage.error.write_journal", &[("error", &error)]))?;
        self.journal_sizes.insert(tab, bytes.len() as u64);
        Ok(())
    }

    fn read(&mut self, tab: TabId) -> Result<Vec<JournalEntry>, String> {
        match fs::read(self.journal_path(tab)) {
            Ok(bytes) => Ok(entries_from(self.data.cipher(), &bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(tr_args("storage.error.read_journal", &[("error", &error)])),
        }
    }

    fn journal_size(&self, tab: TabId) -> u64 {
        self.journal_sizes.get(&tab).copied().unwrap_or(0)
    }

    fn delete(&mut self, tab: TabId) -> Result<(), String> {
        self.open_journals.remove(&tab);
        self.journal_sizes.remove(&tab);
        remove_if_exists(&self.journal_path(tab))
    }
}

/// Cada entrada: 4 bytes con la longitud y después la entrada cifrada.
fn encrypted_records(cipher: &dyn FileCipher, entries: &[JournalEntry]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    for entry in entries {
        let encrypted = cipher.encrypt(&entry.to_bytes())?;
        bytes.extend((encrypted.len() as u32).to_le_bytes());
        bytes.extend(encrypted);
    }
    Ok(bytes)
}

/// Lee entradas hasta el final o hasta la primera incompleta o ilegible.
fn entries_from(cipher: &dyn FileCipher, bytes: &[u8]) -> Vec<JournalEntry> {
    let mut entries = Vec::new();
    let mut rest = bytes;
    while rest.len() >= LENGTH_PREFIX_SIZE {
        let (length, after_length) = rest.split_at(LENGTH_PREFIX_SIZE);
        let length = u32::from_le_bytes(length.try_into().expect("son 4 bytes")) as usize;
        if after_length.len() < length {
            break;
        }
        let (encrypted, after_record) = after_length.split_at(length);
        let Some(entry) = cipher.decrypt(encrypted).ok().and_then(|plain| JournalEntry::from_bytes(&plain)) else {
            break;
        };
        entries.push(entry);
        rest = after_record;
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::terminal_grid::TerminalSize;

    /// Para los tests basta con un "cifrado" que invierte los bytes.
    struct ReversingCipher;

    impl FileCipher for ReversingCipher {
        fn file_mark(&self) -> &'static [u8] {
            b"PRUEBA"
        }
        fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, String> {
            Ok(plain.iter().rev().copied().collect())
        }
        fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, String> {
            Ok(encrypted.iter().rev().copied().collect())
        }
    }

    #[test]
    fn entries_written_one_after_another_are_read_back_in_order() {
        let first = vec![JournalEntry::Resized(TerminalSize { rows: 30, columns: 100 })];
        let second = vec![JournalEntry::Output(b"hola\r\n".to_vec()), JournalEntry::CommandRecorded("dir".into())];
        let mut bytes = encrypted_records(&ReversingCipher, &first).unwrap();
        bytes.extend(encrypted_records(&ReversingCipher, &second).unwrap());

        assert_eq!(entries_from(&ReversingCipher, &bytes), [first, second].concat());
    }

    #[test]
    fn an_entry_cut_off_by_a_crash_is_ignored() {
        let entries = vec![JournalEntry::CommandRecorded("dir".into()), JournalEntry::CommandRecorded("cls".into())];
        let bytes = encrypted_records(&ReversingCipher, &entries).unwrap();
        let cut_off = &bytes[..bytes.len() - 5];

        assert_eq!(entries_from(&ReversingCipher, cut_off), [JournalEntry::CommandRecorded("dir".into())]);
    }
}
