//! Tema Material Design 3, variante oscura con la paleta de referencia de Google.

use eframe::egui::{self, vec2, Color32, CursorIcon, FontId, Margin, Rounding, Shadow, Stroke, TextStyle};

/// Colores con los nombres de los "roles" de Material 3, para que se pueda
/// buscar en la documentación de Material qué va en cada sitio.
pub mod palette {
    use eframe::egui::Color32;

    pub const SURFACE_CONTAINER_LOWEST: Color32 = Color32::from_rgb(0x0F, 0x0D, 0x13);
    pub const SURFACE: Color32 = Color32::from_rgb(0x14, 0x12, 0x18);
    pub const SURFACE_CONTAINER_LOW: Color32 = Color32::from_rgb(0x1D, 0x1B, 0x20);
    pub const SURFACE_CONTAINER_HIGH: Color32 = Color32::from_rgb(0x2B, 0x29, 0x30);
    pub const SURFACE_CONTAINER_HIGHEST: Color32 = Color32::from_rgb(0x36, 0x34, 0x3B);
    pub const ON_SURFACE: Color32 = Color32::from_rgb(0xE6, 0xE0, 0xE9);
    pub const ON_SURFACE_VARIANT: Color32 = Color32::from_rgb(0xCA, 0xC4, 0xD0);
    pub const OUTLINE: Color32 = Color32::from_rgb(0x93, 0x8F, 0x99);
    pub const PRIMARY: Color32 = Color32::from_rgb(0xD0, 0xBC, 0xFF);
    pub const ON_PRIMARY: Color32 = Color32::from_rgb(0x38, 0x1E, 0x72);
    pub const SECONDARY_CONTAINER: Color32 = Color32::from_rgb(0x4A, 0x44, 0x58);
    pub const ON_SECONDARY_CONTAINER: Color32 = Color32::from_rgb(0xE8, 0xDE, 0xF8);
    pub const ERROR: Color32 = Color32::from_rgb(0xF2, 0xB8, 0xB5);
    /// Material no define un color de "todo bien"; este verde suave encaja con la paleta.
    pub const SUCCESS: Color32 = Color32::from_rgb(0x8B, 0xD7, 0x9B);
}

/// Capa semitransparente que Material pone encima de un elemento al pasar
/// el ratón (8 %) o al pulsarlo (12 %).
pub fn state_layer(is_pressed: bool) -> Color32 {
    let opacity = if is_pressed { 0.12 } else { 0.08 };
    palette::ON_SURFACE.gamma_multiply(opacity)
}

const TEXT_STROKE_WIDTH: f32 = 1.0;
/// Grosor del borde de un campo de texto con el cursor dentro.
const FOCUS_OUTLINE_WIDTH: f32 = 2.0;

pub fn apply_material_theme(egui_context: &egui::Context) {
    let mut style = (*egui_context.style()).clone();

    style.text_styles = [
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Body, FontId::proportional(14.0)),
        (TextStyle::Button, FontId::proportional(14.0)),
        (TextStyle::Heading, FontId::proportional(22.0)),
        (TextStyle::Monospace, FontId::monospace(14.0)),
    ]
    .into();

    style.spacing.item_spacing = vec2(8.0, 4.0);
    style.spacing.button_padding = vec2(12.0, 6.0);
    style.spacing.menu_margin = Margin::same(8.0);
    style.spacing.window_margin = Margin::same(24.0);
    style.spacing.interact_size.y = 32.0;

    let visuals = &mut style.visuals;
    visuals.dark_mode = true;
    // La mano sobre todo lo que se puede pulsar (botones, pestañas, filas,
    // menús...). Los campos de texto y la consola ponen el cursor de texto.
    visuals.interact_cursor = Some(CursorIcon::PointingHand);
    visuals.override_text_color = Some(palette::ON_SURFACE);
    visuals.panel_fill = palette::SURFACE;
    visuals.window_fill = palette::SURFACE_CONTAINER_HIGH;
    visuals.extreme_bg_color = palette::SURFACE_CONTAINER_HIGHEST;
    visuals.faint_bg_color = palette::SURFACE_CONTAINER_LOW;
    visuals.window_rounding = Rounding::same(28.0);
    visuals.menu_rounding = Rounding::same(12.0);
    visuals.window_stroke = Stroke::NONE;
    visuals.window_shadow = Shadow { offset: vec2(0.0, 8.0), blur: 32.0, spread: 0.0, color: Color32::from_black_alpha(140) };
    visuals.popup_shadow = Shadow { offset: vec2(0.0, 4.0), blur: 16.0, spread: 0.0, color: Color32::from_black_alpha(120) };
    visuals.selection.bg_fill = palette::PRIMARY.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(FOCUS_OUTLINE_WIDTH, palette::PRIMARY);
    visuals.hyperlink_color = palette::PRIMARY;

    // Sin líneas separadoras: en Material las zonas se distinguen por su color de fondo.
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_stroke = Stroke::NONE;
    widgets.noninteractive.fg_stroke = Stroke::new(TEXT_STROKE_WIDTH, palette::ON_SURFACE);
    for state in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.active, &mut widgets.open] {
        state.rounding = Rounding::same(8.0);
        state.bg_stroke = Stroke::NONE;
        state.expansion = 0.0;
    }
    widgets.inactive.weak_bg_fill = palette::SECONDARY_CONTAINER;
    widgets.inactive.bg_fill = palette::SURFACE_CONTAINER_HIGHEST;
    widgets.inactive.fg_stroke = Stroke::new(TEXT_STROKE_WIDTH, palette::ON_SECONDARY_CONTAINER);
    widgets.hovered.weak_bg_fill = palette::SURFACE_CONTAINER_HIGHEST;
    widgets.hovered.bg_fill = palette::SURFACE_CONTAINER_HIGHEST;
    widgets.hovered.fg_stroke = Stroke::new(TEXT_STROKE_WIDTH, palette::ON_SURFACE);
    widgets.active.weak_bg_fill = palette::SECONDARY_CONTAINER;
    widgets.active.bg_fill = palette::SECONDARY_CONTAINER;
    widgets.active.fg_stroke = Stroke::new(TEXT_STROKE_WIDTH, palette::ON_SURFACE);
    widgets.open.weak_bg_fill = palette::SURFACE_CONTAINER_HIGHEST;
    widgets.open.fg_stroke = Stroke::new(TEXT_STROKE_WIDTH, palette::ON_SURFACE);

    egui_context.set_style(style);
}
