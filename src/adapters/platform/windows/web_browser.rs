use std::ptr::null_mut;

use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::application::ports::WebBrowser;
use crate::i18n::tr_args;

/// Abre las direcciones con el navegador predeterminado de Windows.
pub struct WindowsWebBrowser;

impl WebBrowser for WindowsWebBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        // Por seguridad solo se abren direcciones web, nunca programas ni ficheros.
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(tr_args("links.error.open", &[("url", &url), ("error", &"not a web address")]));
        }
        let wide = |text: &str| text.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (operation, address) = (wide("open"), wide(url));
        // SAFETY: las dos cadenas acaban en 0 y viven hasta que vuelve la llamada.
        let result = unsafe {
            ShellExecuteW(null_mut(), operation.as_ptr(), address.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL)
        };
        // Según la documentación, un valor de 32 o menos es un código de error.
        if result as isize <= 32 {
            return Err(tr_args("links.error.open", &[("url", &url), ("error", &(result as isize))]));
        }
        Ok(())
    }
}