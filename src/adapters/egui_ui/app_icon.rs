//! Icono de la aplicación (barra de tareas, bandeja y el propio `.exe`),
//! dibujado por código: un cuadrado redondeado con ">_" en los colores del tema.
//!
//! No depende de nada más del programa porque `build.rs` también lo usa, para
//! meter el icono en el ejecutable.

/// Fondo del icono (`palette::PRIMARY`; un test comprueba que coinciden).
const BACKGROUND_RGB: [u8; 3] = [0xD0, 0xBC, 0xFF];
/// Color de ">_" (`palette::ON_PRIMARY`).
const SYMBOL_RGB: [u8; 3] = [0x38, 0x1E, 0x72];

/// Radio de las esquinas, como fracción del lado.
const CORNER_RADIUS: f32 = 0.22;
/// Medio grosor de los trazos de ">_".
const STROKE_HALF_WIDTH: f32 = 0.055;
/// Muestras por lado de cada píxel, para suavizar los bordes.
const SAMPLES_PER_SIDE: u32 = 4;

/// Píxeles RGBA de un icono cuadrado de `size` × `size`.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (shape_coverage, symbol_coverage) = coverage_of_pixel(x, y, size);
            let [red, green, blue] = mix(BACKGROUND_RGB, SYMBOL_RGB, symbol_coverage);
            rgba.extend([red, green, blue, (shape_coverage * 255.0) as u8]);
        }
    }
    rgba
}

/// Qué parte del píxel cae dentro del cuadrado y qué parte (de esa) sobre ">_".
fn coverage_of_pixel(x: u32, y: u32, size: u32) -> (f32, f32) {
    let mut samples_in_shape = 0;
    let mut samples_on_symbol = 0;
    for sample_y in 0..SAMPLES_PER_SIDE {
        for sample_x in 0..SAMPLES_PER_SIDE {
            let point = (
                (x as f32 + (sample_x as f32 + 0.5) / SAMPLES_PER_SIDE as f32) / size as f32,
                (y as f32 + (sample_y as f32 + 0.5) / SAMPLES_PER_SIDE as f32) / size as f32,
            );
            if is_inside_rounded_square(point) {
                samples_in_shape += 1;
                if is_on_prompt_symbol(point) {
                    samples_on_symbol += 1;
                }
            }
        }
    }
    let total_samples = (SAMPLES_PER_SIDE * SAMPLES_PER_SIDE) as f32;
    let symbol_share = if samples_in_shape == 0 { 0.0 } else { samples_on_symbol as f32 / samples_in_shape as f32 };
    (samples_in_shape as f32 / total_samples, symbol_share)
}

/// Coordenadas de 0 a 1 en ambos ejes.
fn is_inside_rounded_square((x, y): (f32, f32)) -> bool {
    let beyond_straight_edge_x = ((x - 0.5).abs() - (0.5 - CORNER_RADIUS)).max(0.0);
    let beyond_straight_edge_y = ((y - 0.5).abs() - (0.5 - CORNER_RADIUS)).max(0.0);
    beyond_straight_edge_x.powi(2) + beyond_straight_edge_y.powi(2) <= CORNER_RADIUS.powi(2)
}

fn is_on_prompt_symbol(point: (f32, f32)) -> bool {
    let strokes = [
        ((0.27, 0.30), (0.47, 0.50)), // parte de arriba de ">"
        ((0.47, 0.50), (0.27, 0.70)), // parte de abajo de ">"
        ((0.55, 0.70), (0.75, 0.70)), // "_"
    ];
    strokes.iter().any(|&(start, end)| distance_to_segment(point, start, end) <= STROKE_HALF_WIDTH)
}

fn distance_to_segment(point: (f32, f32), start: (f32, f32), end: (f32, f32)) -> f32 {
    let segment = (end.0 - start.0, end.1 - start.1);
    let to_point = (point.0 - start.0, point.1 - start.1);
    let length_squared = segment.0.powi(2) + segment.1.powi(2);
    let along = ((to_point.0 * segment.0 + to_point.1 * segment.1) / length_squared).clamp(0.0, 1.0);
    let closest = (start.0 + along * segment.0, start.1 + along * segment.1);
    ((point.0 - closest.0).powi(2) + (point.1 - closest.1).powi(2)).sqrt()
}

fn mix(from: [u8; 3], to: [u8; 3], amount: f32) -> [u8; 3] {
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
    [channel(from[0], to[0]), channel(from[1], to[1]), channel(from[2], to[2])]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::egui_ui::theme::palette;

    #[test]
    fn icon_colors_match_the_theme() {
        let rgb = |color: eframe::egui::Color32| [color.r(), color.g(), color.b()];
        assert_eq!(BACKGROUND_RGB, rgb(palette::PRIMARY));
        assert_eq!(SYMBOL_RGB, rgb(palette::ON_PRIMARY));
    }

    #[test]
    fn corners_are_transparent_and_center_is_opaque() {
        let size = 32;
        let rgba = app_icon_rgba(size);
        assert_eq!(rgba.len(), (size * size * 4) as usize);
        assert_eq!(rgba[3], 0);
        let center = ((size / 2 * size + size / 2) * 4) as usize;
        assert_eq!(rgba[center + 3], 255);
    }
}
