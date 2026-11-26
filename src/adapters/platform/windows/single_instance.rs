//! Solo una consola abierta a la vez: lanzar el .exe otra vez muestra la que
//! ya hay (aunque esté minimizada o escondida en la bandeja) en lugar de abrir
//! otra. Si se lanza con una carpeta ("Abrir Console aquí"), la que ya hay
//! abre una pestaña en esa carpeta.
//!
//! Funciona con un "evento con nombre" de Windows, como un timbre compartido:
//! la primera instancia lo crea y se queda esperando a que suene; las
//! siguientes, al ver que ya existe, lo hacen sonar y se cierran. La carpeta
//! no cabe en un timbre: se le pasa por una tubería con nombre que la primera
//! deja escuchando.

use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_NO_DATA, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GENERIC_WRITE, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, ReadFile, WriteFile, OPEN_EXISTING, PIPE_ACCESS_INBOUND};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{CreateEventW, GetCurrentProcessId, SetEvent, WaitForSingleObject, INFINITE};
use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};

/// `Local\` lo limita a la sesión del usuario actual.
const SHOW_WINDOW_EVENT_NAME: &str = r"Local\consola-mostrar-ventana";
/// Las tuberías no tienen `Local\`: el nombre lleva la sesión de Windows.
const OPEN_FOLDER_PIPE_PREFIX: &str = r"\\.\pipe\console-open-folder";
/// Una ruta de Windows no llega a esto; más es que algo va mal.
const LONGEST_FOLDER_MESSAGE: usize = 64 * 1024;
/// Si la primera instancia acaba de arrancar, puede que aún no escuche.
const WAIT_FOR_LISTENING_INSTANCE: Duration = Duration::from_secs(3);

pub enum Startup {
    /// Somos la primera. `None` si Windows no dejó crear el evento: la app
    /// funciona igual, solo que no se avisará de nuevos lanzamientos.
    FirstInstance(Option<LaunchRequests>),
    /// Ya había una abierta y se le ha pedido que se muestre (o que abra la carpeta).
    AlreadyRunning,
}

/// Lo que piden los lanzamientos siguientes del .exe: mostrarse o, además,
/// abrir una pestaña en una carpeta.
pub struct LaunchRequests {
    event: isize,
    /// La primera instancia de la tubería, creada ya al arrancar para que
    /// nadie tenga que esperar a que exista. `None` si no se pudo.
    first_pipe: Option<isize>,
    pipe_name: Vec<u16>,
}

impl LaunchRequests {
    /// Llama (desde otros hilos) a `show_window` cada vez que se lance el
    /// programa otra vez sin carpeta, y a `open_folder` con la carpeta si la trae.
    pub fn answer_with(self, show_window: impl Fn() + Send + 'static, open_folder: impl Fn(String) + Send + 'static) {
        let event = self.event;
        thread::spawn(move || loop {
            // Seguro: el evento lo creamos nosotros y nunca se cierra mientras dura la app.
            let result = unsafe { WaitForSingleObject(event as HANDLE, INFINITE) };
            if result != WAIT_OBJECT_0 {
                break;
            }
            show_window();
        });
        if let Some(first_pipe) = self.first_pipe {
            let pipe_name = self.pipe_name;
            thread::spawn(move || {
                let mut pipe = first_pipe;
                loop {
                    if let Some(folder) = read_one_message(pipe) {
                        open_folder(folder);
                    }
                    // Seguro: la instancia de la tubería es nuestra y ya no se usa.
                    unsafe { CloseHandle(pipe as HANDLE) };
                    match create_pipe_instance(&pipe_name) {
                        Some(next) => pipe = next,
                        None => break,
                    }
                }
            });
        }
    }
}

/// `scope` separa consolas que pueden estar abiertas a la vez: vacío para la
/// instalada, y uno distinto para cada carpeta portable (así una portable no
/// impide abrir la instalada, ni otra copia portable en otro USB).
/// `folder_to_open`: la carpeta que se ha pedido al lanzar el programa.
pub fn claim_single_instance(scope: &str, folder_to_open: Option<&str>) -> Startup {
    let full_name = if scope.is_empty() { SHOW_WINDOW_EVENT_NAME.to_string() } else { format!("{SHOW_WINDOW_EVENT_NAME}-{scope}") };
    let event_name = wide(&full_name);
    let pipe_name = wide(&open_folder_pipe_name(scope));

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
            // Sin carpeta, o si no se pudo pasar, al menos que se muestre.
            let folder_was_sent = folder_to_open.is_some_and(|folder| send_folder(&pipe_name, folder));
            if !folder_was_sent {
                SetEvent(event);
            }
            CloseHandle(event);
            return Startup::AlreadyRunning;
        }
        let first_pipe = create_pipe_instance(&pipe_name);
        Startup::FirstInstance(Some(LaunchRequests { event: event as isize, first_pipe, pipe_name }))
    }
}

fn open_folder_pipe_name(scope: &str) -> String {
    let mut session_id = 0;
    // Seguro: solo escribe en `session_id`. Si falla, se queda en 0.
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session_id) };
    if scope.is_empty() {
        format!("{OPEN_FOLDER_PIPE_PREFIX}-{session_id}")
    } else {
        format!("{OPEN_FOLDER_PIPE_PREFIX}-{session_id}-{scope}")
    }
}

/// Una instancia de la tubería esperando a quien escriba. Con los permisos de
/// serie, solo puede escribir este mismo usuario (o un administrador).
fn create_pipe_instance(pipe_name: &[u16]) -> Option<isize> {
    // Seguro: el nombre termina en 0; el resto son constantes.
    let pipe = unsafe {
        CreateNamedPipeW(
            pipe_name.as_ptr(),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            0,
            LONGEST_FOLDER_MESSAGE as u32,
            0,
            std::ptr::null(),
        )
    };
    (pipe != INVALID_HANDLE_VALUE).then_some(pipe as isize)
}

/// Espera a que alguien se conecte y lee lo que mande hasta que cierre.
fn read_one_message(pipe: isize) -> Option<String> {
    let pipe = pipe as HANDLE;
    // Seguro: la tubería es nuestra; los búferes viven durante cada llamada.
    unsafe {
        // Si se conectó antes de empezar a esperar (o ya escribió y cerró), lo
        // que mandó sigue ahí para leerlo.
        let connected = ConnectNamedPipe(pipe, std::ptr::null_mut()) != 0
            || matches!(GetLastError(), ERROR_PIPE_CONNECTED | ERROR_NO_DATA);
        if !connected {
            return None;
        }
        let mut message = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let mut bytes_read = 0;
            let ok = ReadFile(pipe, buffer.as_mut_ptr(), buffer.len() as u32, &mut bytes_read, std::ptr::null_mut()) != 0;
            if !ok || bytes_read == 0 {
                break;
            }
            message.extend_from_slice(&buffer[..bytes_read as usize]);
            if message.len() > LONGEST_FOLDER_MESSAGE {
                DisconnectNamedPipe(pipe);
                return None;
            }
        }
        DisconnectNamedPipe(pipe);
        String::from_utf8(message).ok().filter(|folder| !folder.trim().is_empty())
    }
}

/// Pasa la carpeta a la consola que ya está abierta. `false` si no escucha.
fn send_folder(pipe_name: &[u16], folder: &str) -> bool {
    let started = Instant::now();
    loop {
        // Seguro: el nombre termina en 0; el handle se cierra antes de salir.
        unsafe {
            let pipe = CreateFileW(pipe_name.as_ptr(), GENERIC_WRITE, 0, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut());
            if pipe != INVALID_HANDLE_VALUE {
                let bytes = folder.as_bytes();
                let mut written = 0;
                let ok = WriteFile(pipe, bytes.as_ptr(), bytes.len() as u32, &mut written, std::ptr::null_mut()) != 0;
                CloseHandle(pipe);
                return ok && written as usize == bytes.len();
            }
            if started.elapsed() > WAIT_FOR_LISTENING_INSTANCE {
                return false;
            }
            if GetLastError() == ERROR_PIPE_BUSY {
                WaitNamedPipeW(pipe_name.as_ptr(), 500);
            } else {
                // Aún no existe: la otra consola está arrancando.
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
