use crate::application::ports::Clipboard;
use crate::i18n::tr_args;

/// Portapapeles de Windows mediante la librería `arboard`.
pub struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn copy_text(&mut self, text: &str) -> Result<(), String> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(text.to_string()))
            .map_err(|error| tr_args("clipboard.error.copy", &[("error", &error)]))
    }

    fn read_text(&mut self) -> Result<String, String> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .map_err(|error| tr_args("clipboard.error.read", &[("error", &error)]))
    }
}
