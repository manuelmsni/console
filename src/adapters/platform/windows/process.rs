use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};

use crate::domain::updates::OperatingSystem;

pub const OPERATING_SYSTEM: OperatingSystem = OperatingSystem::Windows;

/// Lo que se espera como mucho a que se cierre la versión anterior al actualizar.
const WAIT_FOR_PREVIOUS_VERSION_MILLISECONDS: u32 = 15_000;

/// Espera a que termine el proceso `process_id` (la versión anterior, tras
/// actualizar). Si ya no existe, vuelve enseguida.
pub fn wait_for_process_to_exit(process_id: u32) {
    // Seguro: solo se pide permiso para esperar al proceso, y el handle se cierra.
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, process_id);
        if process.is_null() {
            return;
        }
        WaitForSingleObject(process, WAIT_FOR_PREVIOUS_VERSION_MILLISECONDS);
        CloseHandle(process);
    }
}