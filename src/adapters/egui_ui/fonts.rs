use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

/// Fuentes de Windows preferidas para la terminal, en orden.
const MONOSPACE_FONT_CANDIDATES: [&str; 2] = [r"C:\Windows\Fonts\CascadiaMono.ttf", r"C:\Windows\Fonts\consola.ttf"];
/// Fuente para menús y pestañas.
const INTERFACE_FONT_CANDIDATES: [&str; 1] = [r"C:\Windows\Fonts\segoeui.ttf"];

/// Usa las fuentes de Windows si están instaladas (Cascadia Mono o Consolas
/// para la terminal, Segoe UI para la interfaz) y añade los iconos de Material.
/// Los símbolos que les falten se siguen dibujando con las fuentes que trae egui.
pub fn install_fonts(egui_context: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    if let Some(font_bytes) = first_readable_font(&MONOSPACE_FONT_CANDIDATES) {
        fonts.font_data.insert("windows_monospace".to_string(), FontData::from_owned(font_bytes));
        fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "windows_monospace".to_string());
    }
    if let Some(font_bytes) = first_readable_font(&INTERFACE_FONT_CANDIDATES) {
        fonts.font_data.insert("windows_interface".to_string(), FontData::from_owned(font_bytes));
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "windows_interface".to_string());
    }

    let mut material_icons = FontData::from_static(egui_material_icons::FONT_DATA);
    material_icons.tweak.y_offset_factor = 0.05;
    fonts.font_data.insert("material_icons".to_string(), material_icons);
    fonts.families.entry(FontFamily::Proportional).or_default().push("material_icons".to_string());

    egui_context.set_fonts(fonts);
}

fn first_readable_font(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| std::fs::read(path).ok())
}
