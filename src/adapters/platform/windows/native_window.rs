use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_HIDE, SW_RESTORE};

/// La ventana principal, manejada con la API de Windows directamente: así se
/// puede volver a mostrar desde el icono de la bandeja aunque egui no esté
/// dibujando (con la ventana escondida, egui no dibuja).
#[derive(Clone, Copy)]
pub struct MainWindow {
    handle: isize,
}

impl MainWindow {
    pub fn of(creation_context: &eframe::CreationContext) -> Option<Self> {
        match creation_context.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(win32) => Some(Self { handle: win32.hwnd.get() }),
            _ => None,
        }
    }

    pub fn hide(self) {
        // Seguro: el handle es el de nuestra propia ventana, que vive mientras la aplicación.
        unsafe {
            ShowWindow(self.handle as _, SW_HIDE);
        }
    }

    pub fn show(self) {
        // Seguro: igual que en `hide`.
        unsafe {
            ShowWindow(self.handle as _, SW_RESTORE);
            SetForegroundWindow(self.handle as _);
        }
    }
}