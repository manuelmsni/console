//! El diario de una pestaña: todo lo que pasa en ella, apuntado en orden,
//! para poder reconstruirla tal cual al volver a abrir la aplicación (aunque
//! se cerrara de golpe). Solo se borra al cerrar la pestaña.

use super::terminal_grid::TerminalSize;

/// Identifica una pestaña a lo largo de distintas ejecuciones de la app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(pub u64);

/// Lo que hay que saber de una pestaña para volver a abrirla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabRecord {
    pub id: TabId,
    /// Nombre del tipo de terminal (cmd, PowerShell...) con el que reabrirla.
    pub profile_name: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalEntry {
    /// Bytes tal cual los escribió la consola (texto, colores, movimientos del cursor).
    Output(Vec<u8>),
    Resized(TerminalSize),
    /// Bytes que reproducen el estado completo de la pantalla en un momento
    /// dado; sustituye a todo lo anterior cuando el diario se compacta.
    Snapshot(Vec<u8>),
    CommandRecorded(String),
    CommandForgotten(String),
}

const OUTPUT: u8 = 1;
const RESIZED: u8 = 2;
const SNAPSHOT: u8 = 3;
const COMMAND_RECORDED: u8 = 4;
const COMMAND_FORGOTTEN: u8 = 5;

impl JournalEntry {
    /// Una marca de tipo seguida de su contenido.
    pub fn to_bytes(&self) -> Vec<u8> {
        let (kind, content): (u8, Vec<u8>) = match self {
            JournalEntry::Output(bytes) => (OUTPUT, bytes.clone()),
            JournalEntry::Resized(size) => {
                let mut content = size.rows.to_le_bytes().to_vec();
                content.extend(size.columns.to_le_bytes());
                (RESIZED, content)
            }
            JournalEntry::Snapshot(bytes) => (SNAPSHOT, bytes.clone()),
            JournalEntry::CommandRecorded(command) => (COMMAND_RECORDED, command.as_bytes().to_vec()),
            JournalEntry::CommandForgotten(command) => (COMMAND_FORGOTTEN, command.as_bytes().to_vec()),
        };
        let mut bytes = vec![kind];
        bytes.extend(content);
        bytes
    }

    /// `None` si los bytes no son una entrada válida.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (&kind, content) = bytes.split_first()?;
        let text = || String::from_utf8(content.to_vec()).ok();
        match kind {
            OUTPUT => Some(JournalEntry::Output(content.to_vec())),
            RESIZED => {
                let [row_low, row_high, column_low, column_high] = content.try_into().ok()?;
                let size = TerminalSize {
                    rows: u16::from_le_bytes([row_low, row_high]),
                    columns: u16::from_le_bytes([column_low, column_high]),
                };
                (size.rows > 0 && size.columns > 0).then_some(JournalEntry::Resized(size))
            }
            SNAPSHOT => Some(JournalEntry::Snapshot(content.to_vec())),
            COMMAND_RECORDED => Some(JournalEntry::CommandRecorded(text()?)),
            COMMAND_FORGOTTEN => Some(JournalEntry::CommandForgotten(text()?)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_of_entry_survives_being_stored() {
        let entries = [
            JournalEntry::Output(b"\x1b[31mhola\x1b[0m\r\n".to_vec()),
            JournalEntry::Resized(TerminalSize { rows: 40, columns: 120 }),
            JournalEntry::Snapshot(b"\x1b[H\x1b[Jpantalla".to_vec()),
            JournalEntry::CommandRecorded("git status".into()),
            JournalEntry::CommandForgotten("login secreto".into()),
        ];
        for entry in entries {
            assert_eq!(JournalEntry::from_bytes(&entry.to_bytes()), Some(entry));
        }
    }

    #[test]
    fn damaged_entries_are_rejected() {
        assert_eq!(JournalEntry::from_bytes(&[]), None);
        assert_eq!(JournalEntry::from_bytes(&[99, 1, 2]), None);
        assert_eq!(JournalEntry::from_bytes(&[RESIZED, 1]), None);
    }
}
