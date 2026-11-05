//! Colores de pantalla, con la paleta "Campbell" de Windows Terminal.

use eframe::egui::Color32;

use super::theme::palette;
use crate::domain::screen_snapshot::TerminalColor;

pub const DEFAULT_FOREGROUND: Color32 = Color32::from_rgb(204, 204, 204);
/// El fondo de la terminal es el más oscuro del tema, para que encaje con el resto.
pub const DEFAULT_BACKGROUND: Color32 = palette::SURFACE_CONTAINER_LOWEST;
/// Color primario del tema (D0BCFF) al 35 % de opacidad.
pub const SELECTION_HIGHLIGHT: Color32 = Color32::from_rgba_premultiplied(73, 66, 89, 89);
pub const CURSOR: Color32 = Color32::from_rgba_premultiplied(160, 150, 190, 170);

const CAMPBELL_PALETTE: [Color32; 16] = [
    Color32::from_rgb(12, 12, 12),
    Color32::from_rgb(197, 15, 31),
    Color32::from_rgb(19, 161, 14),
    Color32::from_rgb(193, 156, 0),
    Color32::from_rgb(0, 55, 218),
    Color32::from_rgb(136, 23, 152),
    Color32::from_rgb(58, 150, 221),
    Color32::from_rgb(204, 204, 204),
    Color32::from_rgb(118, 118, 118),
    Color32::from_rgb(231, 72, 86),
    Color32::from_rgb(22, 198, 12),
    Color32::from_rgb(249, 241, 165),
    Color32::from_rgb(59, 120, 255),
    Color32::from_rgb(180, 0, 158),
    Color32::from_rgb(97, 214, 214),
    Color32::from_rgb(242, 242, 242),
];

/// El texto en negrita usa la versión brillante de los 8 colores básicos,
/// como hacen cmd y PowerShell.
pub fn screen_color_of(color: TerminalColor, is_bold: bool) -> Color32 {
    match color {
        TerminalColor::DefaultForeground => DEFAULT_FOREGROUND,
        TerminalColor::DefaultBackground => DEFAULT_BACKGROUND,
        TerminalColor::Indexed(index) if is_bold && index < 8 => CAMPBELL_PALETTE[index as usize + 8],
        TerminalColor::Indexed(index) => indexed_color(index),
        TerminalColor::Rgb(red, green, blue) => Color32::from_rgb(red, green, blue),
    }
}

/// Los 256 colores clásicos: 16 básicos, un cubo de 6×6×6 y 24 grises.
fn indexed_color(index: u8) -> Color32 {
    match index {
        0..=15 => CAMPBELL_PALETTE[index as usize],
        16..=231 => {
            let cube_index = index - 16;
            let channel_level = |step: u8| if step == 0 { 0 } else { 55 + step * 40 };
            Color32::from_rgb(
                channel_level(cube_index / 36),
                channel_level((cube_index / 6) % 6),
                channel_level(cube_index % 6),
            )
        }
        232..=255 => {
            let gray = 8 + (index - 232) * 10;
            Color32::from_rgb(gray, gray, gray)
        }
    }
}
