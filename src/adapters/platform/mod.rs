//! Lo que depende del sistema operativo, en un solo sitio. Cada sistema tiene
//! su carpeta (de momento solo `windows/`) y todas ofrecen lo mismo, con los
//! mismos nombres, para que el resto del programa no sepa en cuál está:
//!
//! | Nombre                                      | Para qué                                                    |
//! |---------------------------------------------|-------------------------------------------------------------|
//! | `SystemClock`                               | La hora local (puerto `Clock`).                              |
//! | `system_display_language()`                 | Código del idioma del sistema, p. ej. `"es-ES"`.             |
//! | `SystemWebBrowser`                          | Abrir enlaces en el navegador (puerto `WebBrowser`).         |
//! | `SystemUserCipher`                          | Cifrar los ficheros con la cuenta del usuario (sin contraseña). |
//! | `claim_single_instance(scope)`, `InstanceStartup`, `ShowWindowRequests` | Una sola consola abierta a la vez (por `scope`). |
//! | `SystemSshTools`                            | Claves, agente y conexiones SSH (puerto `SshTools`).         |
//! | `MainWindow`                                | Esconder y volver a mostrar la ventana (bandeja del sistema). |
//! | `available_shell_profiles()`                | Terminales que se pueden abrir; la primera es la principal.  |
//! | `OPERATING_SYSTEM`                          | Qué sistema es, para elegir su ejecutable al actualizar.     |
//! | `wait_for_process_to_exit(pid)`             | Esperar a que se cierre la versión anterior tras actualizar. |
//!
//! Para llevar la aplicación a otro sistema basta con añadir su carpeta con
//! todo lo de la tabla y elegirla abajo con `#[cfg(...)]`.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(not(windows))]
compile_error!(
    "De momento la consola solo funciona en Windows. Para otro sistema, añade su carpeta en \
     src/adapters/platform/ con lo que lista la tabla de src/adapters/platform/mod.rs."
);