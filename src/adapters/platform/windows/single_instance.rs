//! Solo una consola abierta a la vez: lanzar el .exe otra vez muestra la que
//! ya hay (aunque esté minimizada o escondida en la bandeja) en lugar de abrir
//! otra.
//!
//! Funciona con un "evento con nombre" de Windows, como un timbre compartido:
//! la primera instancia lo crea y se queda esperando a que suene; las
//! siguientes, al ver que ya existe, lo hacen sonar y se cierran.

use std::thread;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject, INFINITE};
use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};

/// `Local\` lo limita a la sesión del usuario actual.
const SHOW_WINDOW_EVENT_NAME: &str = r"Local\consola-mostrar-ventana";

pub enum Startup {
    /// Somos la primera. `None` si Windows no dejó crear el evento: la app
    /// funciona igual, solo que no se avisará de nuevos lanzamientos.
    FirstInstance(Option<ShowWindowRequests>),
    /// Ya había una abierta y se le ha pedido que se muestre.
    AlreadyRunning,
}

/// Los avisos de "muéstrate" que llegan cuando alguien lanza el .exe otra vez.
pub struct ShowWindowRequests {
    event: isize,
}

impl ShowWindowRequests {
    /// Llama a `show_window` (desde otro hilo) cada vez que llegue un aviso.
    pub fn answer_with(self, show_window: impl Fn() + Send + 'static) {
        thread::spawn(move || loop {
            // Seguro: el evento lo creamos nosotros y nunca se cierra mientras dura la app.
            let result = unsafe { WaitForSingleObject(self.event as HANDLE, INFINITE) };
            if result != WAIT_OBJECT_0 {
                break;
            }
            show_window();
        });
    }
}

/// `scope` separa consolas que pueden estar abiertas a la vez: vacío para la
/// instalada, y uno distinto para cada carpeta portable (así una portable no
/// impide abrir la instalada, ni otra copia portable en otro USB).
pub fn claim_single_instance(scope: &str) -> Startup {
    let full_name = if scope.is_empty() { SHOW_WINDOW_EVENT_NAME.to_string() } else { format!("{SHOW_WINDOW_EVENT_NAME}-{scope}") };
    let event_name: Vec<u16> = full_name.encode_utf16().chain(Some(0)).collect();

    // Seguro: el nombre termina en 0 como pide Windows; el resto de parámetros son nulos/constantes.
    unsafe {
        let event = CreateEventW(std::ptr::null(), 0, 0, event_name.as_ptr());
        if event.is_null() {
            return Startup::FirstInstance(None);
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            // Deja que la otra instancia se ponga delante: Windows solo se lo
            // permite al programa que el usuario acaba de lanzar (este).
            AllowSetForegroundWindow(ASFW_ANY);
            SetEvent(event);
            CloseHandle(event);
            return Startup::AlreadyRunning;
        }
        Startup::FirstInstance(Some(ShowWindowRequests { event: event as isize }))
    }
}
