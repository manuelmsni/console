//! Windows: ConPTY la pone `portable-pty`; aquí va lo demás que solo existe en
//! Windows (DPAPI, eventos con nombre, servicio ssh-agent, API Win32...).

mod clock;
mod language;
mod native_window;
mod openssh_tools;
mod process;
mod shell_profiles;
mod single_instance;
// La portable cifra con contraseña, no con la cuenta de Windows.
#[cfg(not(feature = "portable"))]
mod user_encryption;
mod web_browser;

pub use clock::WindowsClock as SystemClock;
pub use language::windows_display_language as system_display_language;
pub use native_window::MainWindow;
pub use openssh_tools::OpenSshTools as SystemSshTools;
pub use process::{wait_for_process_to_exit, OPERATING_SYSTEM};
pub use shell_profiles::available_shell_profiles;
pub use single_instance::{claim_single_instance, ShowWindowRequests, Startup as InstanceStartup};
#[cfg(not(feature = "portable"))]
pub use user_encryption::WindowsUserCipher as SystemUserCipher;
pub use web_browser::WindowsWebBrowser as SystemWebBrowser;