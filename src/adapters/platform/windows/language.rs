use windows_sys::Win32::Globalization::{GetUserDefaultUILanguage, LCIDToLocaleName};

/// El idioma de la interfaz de Windows, como código de dos letras (`es`, `en`...).
pub fn windows_display_language() -> String {
    let mut name = [0u16; 85];
    // Seguro: Windows escribe como mucho `name.len()` caracteres en el búfer.
    let length = unsafe { LCIDToLocaleName(GetUserDefaultUILanguage() as u32, name.as_mut_ptr(), name.len() as i32, 0) };
    if length <= 0 {
        return String::new();
    }
    // "es-ES" → "es"
    let locale_name = String::from_utf16_lossy(&name[..(length as usize - 1)]);
    locale_name.split('-').next().unwrap_or_default().to_lowercase()
}
