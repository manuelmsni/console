//! Piezas de interfaz con aspecto Material Design que egui no trae de serie.
//! Cada una se dibuja a mano: se reserva un rectángulo, se mira si el ratón
//! está encima y se pinta fondo, icono y texto.

use std::sync::Arc;

use eframe::egui::text::{CCursor, CCursorRange, LayoutJob, TextWrapping};
use eframe::egui::text_edit::TextEditState;
use eframe::egui::{
    pos2, vec2, Align, Align2, Button, Color32, Context, FontId, Galley, Id, Key, LayerId, Layout, Margin, Order,
    Painter, Pos2, Rect, Response, RichText, Rounding, Sense, Shape, Stroke, TextEdit, Ui, UiBuilder,
};
use egui_material_icons::icons::{
    ICON_CHEVRON_RIGHT, ICON_CLOSE, ICON_DELETE, ICON_EXPAND_MORE, ICON_LIBRARY_ADD, ICON_PLAY_ARROW, ICON_SYNC,
    ICON_TERMINAL,
};

use super::theme::{palette, state_layer};
use crate::application::terminal_tabs::TabOverview;
use crate::i18n::tr;

const ICON_SIZE: f32 = 20.0;
const LABEL_FONT_SIZE: f32 = 14.0;

/// Botón redondo con solo un icono.
pub fn icon_button(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.circle_filled(rect.center(), 18.0, state_layer(response.is_pointer_button_down_on()));
    }
    let icon_color = if response.hovered() { palette::ON_SURFACE } else { palette::ON_SURFACE_VARIANT };
    painter.text(rect.center(), Align2::CENTER_CENTER, icon, FontId::proportional(ICON_SIZE), icon_color);
    response.on_hover_text(tooltip)
}

/// Un botón de icono pequeño y apagado, para algo que debe estar a mano sin
/// llamar la atención.
pub fn subtle_icon_button(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(SUBTLE_BUTTON_SIZE, SUBTLE_BUTTON_SIZE), Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.circle_filled(rect.center(), SUBTLE_BUTTON_SIZE / 2.0, state_layer(response.is_pointer_button_down_on()));
    }
    let icon_color = if response.hovered() { palette::ON_SURFACE_VARIANT } else { palette::OUTLINE };
    painter.text(rect.center(), Align2::CENTER_CENTER, icon, FontId::proportional(SUBTLE_ICON_SIZE), icon_color);
    response.on_hover_text(tooltip)
}

const SUBTLE_BUTTON_SIZE: f32 = 28.0;
const SUBTLE_ICON_SIZE: f32 = 16.0;

/// Botones de la esquina de la ventana.
#[derive(Clone, Copy)]
pub enum WindowControl {
    Minimize,
    Maximize,
    Restore,
    HideToTray,
    Exit,
}

const WINDOW_CONTROL_SIZE: f32 = 30.0;
const WINDOW_CONTROL_STROKE_WIDTH: f32 = 1.4;

/// Botón pequeño y redondo con el pictograma dibujado con líneas, para que
/// quede exactamente en el centro del círculo.
pub fn window_control_button(ui: &mut Ui, control: WindowControl, tooltip: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(WINDOW_CONTROL_SIZE, WINDOW_CONTROL_SIZE), Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.circle_filled(rect.center(), WINDOW_CONTROL_SIZE / 2.0, state_layer(response.is_pointer_button_down_on()));
    }
    let color = if response.hovered() { palette::ON_SURFACE } else { palette::ON_SURFACE_VARIANT };
    paint_window_control_symbol(painter, rect.center(), control, Stroke::new(WINDOW_CONTROL_STROKE_WIDTH, color));
    response.on_hover_text(tooltip)
}

fn paint_window_control_symbol(painter: &Painter, center: Pos2, control: WindowControl, stroke: Stroke) {
    match control {
        WindowControl::Minimize => {
            painter.line_segment([center - vec2(5.0, 0.0), center + vec2(5.0, 0.0)], stroke);
        }
        WindowControl::Maximize => {
            painter.rect_stroke(Rect::from_center_size(center, vec2(10.0, 10.0)), Rounding::same(1.5), stroke);
        }
        WindowControl::Restore => {
            // Un cuadrado delante y, asomando por detrás, la esquina de otro.
            let front = Rect::from_center_size(center + vec2(-1.5, 1.5), vec2(8.0, 8.0));
            painter.rect_stroke(front, Rounding::same(1.5), stroke);
            let back_corner = vec![
                pos2(front.left() + 3.0, front.top() - 3.0),
                pos2(front.right() + 3.0, front.top() - 3.0),
                pos2(front.right() + 3.0, front.bottom() - 3.0),
            ];
            painter.add(Shape::line(back_corner, stroke));
        }
        WindowControl::HideToTray => {
            painter.line_segment([center - vec2(4.5, 4.5), center + vec2(4.5, 4.5)], stroke);
            painter.line_segment([center + vec2(-4.5, 4.5), center + vec2(4.5, -4.5)], stroke);
        }
        WindowControl::Exit => paint_power_symbol(painter, center, stroke),
    }
}

/// Círculo abierto por arriba con una raya vertical en la abertura.
fn paint_power_symbol(painter: &Painter, center: Pos2, stroke: Stroke) {
    let radius = 5.5;
    let half_gap = 0.65; // radianes a cada lado de la vertical
    let segments = 24;
    let arc: Vec<Pos2> = (0..=segments)
        .map(|step| {
            let angle = half_gap + step as f32 * (std::f32::consts::TAU - 2.0 * half_gap) / segments as f32;
            pos2(center.x + radius * angle.sin(), center.y - radius * angle.cos())
        })
        .collect();
    painter.add(Shape::line(arc, stroke));
    painter.line_segment([pos2(center.x, center.y - radius - 1.0), pos2(center.x, center.y - 0.5)], stroke);
}

/// Lo escrito al renombrar algo en su sitio (una pestaña, una fila del árbol).
pub struct RenameField {
    pub text: String,
    /// La primera vez se le da el foco y se selecciona todo el texto.
    pub needs_focus: bool,
}

impl RenameField {
    pub fn starting_with(current_name: &str) -> Self {
        Self { text: current_name.to_string(), needs_focus: true }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RenameOutcome {
    StillEditing,
    /// Enter, o clic fuera del campo.
    Confirmed,
    /// Esc.
    Cancelled,
}

/// Campo de texto encima del nombre, para renombrar sin abrir un diálogo.
/// Va en una capa hija para no mover nada de alrededor.
fn show_rename_field(ui: &mut Ui, area: Rect, field: &mut RenameField) -> RenameOutcome {
    let id = Id::new("rename_in_place");
    let mut child = ui.new_child(UiBuilder::new().max_rect(area).layout(Layout::left_to_right(Align::Center)));
    let response = child.add_sized(
        area.size(),
        TextEdit::singleline(&mut field.text).id(id).font(FontId::proportional(LABEL_FONT_SIZE)).margin(Margin::symmetric(6.0, 2.0)),
    );
    if std::mem::take(&mut field.needs_focus) {
        response.request_focus();
        let mut state = TextEditState::load(ui.ctx(), id).unwrap_or_default();
        let whole_text = CCursorRange::two(CCursor::new(0), CCursor::new(field.text.chars().count()));
        state.cursor.set_char_range(Some(whole_text));
        state.store(ui.ctx(), id);
        return RenameOutcome::StillEditing;
    }
    if !response.lost_focus() {
        RenameOutcome::StillEditing
    } else if ui.input(|input| input.key_pressed(Key::Escape)) {
        RenameOutcome::Cancelled
    } else {
        RenameOutcome::Confirmed
    }
}

pub struct TabResponse {
    pub tab: Response,
    pub selected: bool,
    pub close_requested: bool,
    /// Se ha hecho doble clic en el nombre: hay que empezar a renombrarla.
    pub rename_requested: bool,
    /// `Some` mientras se está renombrando esta pestaña.
    pub rename: Option<RenameOutcome>,
}

/// Pestaña de terminal: icono, nombre y una × para cerrarla. La activa lleva
/// el nombre en color primario y una barrita debajo. Con cambios sin guardar,
/// en el sitio de la × hay un punto (al pasar el ratón vuelve la ×); con
/// autoguardado, un icono de sincronización tras el nombre. Con `rename_field`,
/// en lugar del nombre hay un campo para cambiarlo.
pub fn terminal_tab(ui: &mut Ui, overview: &TabOverview, is_active: bool, rename_field: Option<&mut RenameField>) -> TabResponse {
    let label_color = if is_active { palette::PRIMARY } else { palette::ON_SURFACE_VARIANT };
    let label_galley =
        ui.painter().layout_no_wrap(overview.label.clone(), FontId::proportional(LABEL_FONT_SIZE), label_color);

    let (padding, gap, close_size) = (14.0, 8.0, 22.0);
    let autosave_icon_width = if overview.autosaves { 18.0 } else { 0.0 };
    let label_width = if rename_field.is_some() { label_galley.size().x.max(140.0) } else { label_galley.size().x };
    let width = padding + ICON_SIZE + gap + label_width + autosave_icon_width + gap + close_size + padding / 2.0;
    let (rect, tab) = ui.allocate_exact_size(vec2(width, 40.0), Sense::click());

    let close_rect = Rect::from_center_size(pos2(rect.right() - padding / 2.0 - close_size / 2.0, rect.center().y), vec2(close_size, close_size));
    let close_button = ui.interact(close_rect, tab.id.with("close"), Sense::click());
    let tab_is_hovered = ui.rect_contains_pointer(rect);

    let painter = ui.painter().clone();
    if tab_is_hovered {
        painter.rect_filled(rect.shrink2(vec2(0.0, 2.0)), Rounding::same(8.0), state_layer(tab.is_pointer_button_down_on()));
    }

    let icon_center = pos2(rect.left() + padding + ICON_SIZE / 2.0, rect.center().y);
    painter.text(icon_center, Align2::CENTER_CENTER, ICON_TERMINAL, FontId::proportional(18.0), label_color);
    let label_top_left = pos2(icon_center.x + ICON_SIZE / 2.0 + gap, rect.center().y - label_galley.size().y / 2.0);
    let label_right = label_top_left.x + label_width;
    let rename = match rename_field {
        Some(field) => {
            let area = Rect::from_min_max(pos2(label_top_left.x - 4.0, rect.top() + 6.0), pos2(label_right + 4.0, rect.bottom() - 6.0));
            Some(show_rename_field(ui, area, field))
        }
        None => {
            painter.galley(label_top_left, label_galley, label_color);
            None
        }
    };

    if overview.autosaves {
        let sync_center = pos2(label_right + autosave_icon_width / 2.0 + 2.0, rect.center().y);
        painter.text(sync_center, Align2::CENTER_CENTER, ICON_SYNC, FontId::proportional(14.0), palette::PRIMARY);
    }

    if tab_is_hovered {
        if close_button.hovered() {
            painter.circle_filled(close_rect.center(), close_size / 2.0, state_layer(close_button.is_pointer_button_down_on()));
        }
        painter.text(close_rect.center(), Align2::CENTER_CENTER, ICON_CLOSE, FontId::proportional(16.0), palette::ON_SURFACE_VARIANT);
    } else if overview.has_unsaved_changes {
        painter.circle_filled(close_rect.center(), 4.0, label_color);
    } else if is_active {
        painter.text(close_rect.center(), Align2::CENTER_CENTER, ICON_CLOSE, FontId::proportional(16.0), palette::ON_SURFACE_VARIANT);
    }

    if is_active {
        let indicator = Rect::from_min_max(pos2(rect.left() + padding, rect.bottom() - 3.0), pos2(rect.right() - padding / 2.0, rect.bottom()));
        painter.rect_filled(indicator, Rounding { nw: 3.0, ne: 3.0, sw: 0.0, se: 0.0 }, palette::PRIMARY);
    }

    let tab = match (overview.has_unsaved_changes, overview.autosaves) {
        (_, true) => tab.on_hover_text(tr("tabs.tooltip.autosaves")),
        (true, false) => tab.on_hover_text(tr("tabs.tooltip.unsaved")),
        (false, false) => tab,
    };
    TabResponse {
        selected: tab.clicked() && !close_button.clicked(),
        close_requested: close_button.clicked() || tab.middle_clicked(),
        rename_requested: tab.double_clicked() && !close_button.clicked(),
        rename,
        tab,
    }
}

/// Pestaña de una ventana con secciones (por ejemplo, los ajustes): icono y
/// nombre, en color primario y con una barrita debajo si es la activa.
pub fn section_tab(ui: &mut Ui, icon: &str, label: &str, is_active: bool) -> Response {
    let color = if is_active { palette::PRIMARY } else { palette::ON_SURFACE_VARIANT };
    let label_galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(LABEL_FONT_SIZE), color);
    let width = 14.0 + ICON_SIZE + 8.0 + label_galley.size().x + 14.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 40.0), Sense::click());

    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect.shrink2(vec2(0.0, 2.0)), Rounding::same(8.0), state_layer(response.is_pointer_button_down_on()));
    }
    let icon_center = pos2(rect.left() + 14.0 + ICON_SIZE / 2.0, rect.center().y);
    painter.text(icon_center, Align2::CENTER_CENTER, icon, FontId::proportional(18.0), color);
    painter.galley(pos2(icon_center.x + ICON_SIZE / 2.0 + 8.0, rect.center().y - label_galley.size().y / 2.0), label_galley, color);
    if is_active {
        let indicator = Rect::from_min_max(pos2(rect.left() + 14.0, rect.bottom() - 3.0), pos2(rect.right() - 14.0, rect.bottom()));
        painter.rect_filled(indicator, Rounding { nw: 3.0, ne: 3.0, sw: 0.0, se: 0.0 }, palette::PRIMARY);
    }
    response
}

/// Cómo se ve una fila del árbol de comandos.
pub struct TreeRow<'a> {
    pub depth: usize,
    /// `Some(abierta)` para carpetas, que llevan flecha de desplegar.
    pub expander: Option<bool>,
    pub icon: &'a str,
    pub icon_color: Color32,
    pub label: &'a str,
    pub has_run_button: bool,
    /// `Some` mientras se está renombrando: en lugar del nombre hay un campo.
    pub rename_field: Option<&'a mut RenameField>,
}

pub struct TreeRowResponse {
    pub row: Response,
    pub run_clicked: bool,
    /// `Some` mientras se está renombrando esta fila.
    pub rename: Option<RenameOutcome>,
}

const TREE_INDENT: f32 = 16.0;
const TREE_ROW_HEIGHT: f32 = 30.0;
const DROP_LINE_WIDTH: f32 = 2.0;

pub fn tree_row(ui: &mut Ui, row_id: Id, spec: TreeRow) -> TreeRowResponse {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), TREE_ROW_HEIGHT), Sense::hover());
    // Se puede pulsar y también arrastrar, para reorganizar el árbol.
    let row = ui.interact(rect, row_id, Sense::click_and_drag());
    let row_is_hovered = ui.rect_contains_pointer(rect);

    let run_rect = Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), vec2(28.0, 28.0));
    let run_button = spec.has_run_button.then(|| ui.interact(run_rect, row_id.with("run"), Sense::click()));

    let painter = ui.painter_at(rect);
    if row_is_hovered {
        painter.rect_filled(rect, Rounding::same(TREE_ROW_HEIGHT / 2.0), state_layer(row.is_pointer_button_down_on()));
    }

    let mut x = rect.left() + 8.0 + spec.depth as f32 * TREE_INDENT;
    if let Some(is_open) = spec.expander {
        let chevron = if is_open { ICON_EXPAND_MORE } else { ICON_CHEVRON_RIGHT };
        painter.text(pos2(x + 9.0, rect.center().y), Align2::CENTER_CENTER, chevron, FontId::proportional(18.0), palette::ON_SURFACE_VARIANT);
    }
    x += 22.0;
    painter.text(pos2(x + 9.0, rect.center().y), Align2::CENTER_CENTER, spec.icon, FontId::proportional(18.0), spec.icon_color);
    x += 28.0;
    // El nombre se corta con "…" antes de llegar al botón ▶ (o al borde).
    let label_end = if spec.has_run_button { run_rect.left() - 4.0 } else { rect.right() - 8.0 };
    let rename = match spec.rename_field {
        Some(field) => {
            let area = Rect::from_min_max(pos2(x - 6.0, rect.top() + 4.0), pos2(label_end, rect.bottom() - 4.0));
            Some(show_rename_field(ui, area, field))
        }
        None => {
            let label = single_line_cut_to_width(&painter, spec.label, label_end - x, palette::ON_SURFACE);
            painter.galley(pos2(x, rect.center().y - label.size().y / 2.0), label, palette::ON_SURFACE);
            None
        }
    };

    if let Some(run_button) = &run_button {
        if run_button.hovered() {
            painter.circle_filled(run_rect.center(), 14.0, palette::PRIMARY.gamma_multiply(0.16));
        }
        let run_color = if row_is_hovered { palette::PRIMARY } else { palette::OUTLINE };
        painter.text(run_rect.center(), Align2::CENTER_CENTER, ICON_PLAY_ARROW, FontId::proportional(ICON_SIZE), run_color);
    }

    TreeRowResponse { row, run_clicked: run_button.is_some_and(|button| button.clicked()), rename }
}

/// Línea que indica dónde caerá lo que se arrastra: encima o debajo de la fila.
pub fn paint_drop_line(ui: &Ui, row_rect: Rect, depth: usize, at_top: bool) {
    let y = if at_top { row_rect.top() } else { row_rect.bottom() };
    let start = pos2(row_rect.left() + 8.0 + depth as f32 * TREE_INDENT + 22.0, y);
    let painter = ui.painter();
    painter.line_segment([start, pos2(row_rect.right() - 8.0, y)], Stroke::new(DROP_LINE_WIDTH, palette::PRIMARY));
    painter.circle_stroke(start, 4.0, Stroke::new(DROP_LINE_WIDTH, palette::PRIMARY));
}

/// Recuadro que indica que lo arrastrado entrará dentro de esta carpeta.
pub fn paint_drop_into(ui: &Ui, row_rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(row_rect, Rounding::same(TREE_ROW_HEIGHT / 2.0), palette::PRIMARY.gamma_multiply(0.12));
    painter.rect_stroke(row_rect, Rounding::same(TREE_ROW_HEIGHT / 2.0), Stroke::new(DROP_LINE_WIDTH, palette::PRIMARY));
}

/// Etiqueta que acompaña al puntero mientras se arrastra algo.
pub fn paint_drag_ghost(egui_context: &Context, icon: &str, label: &str) {
    let Some(pointer) = egui_context.pointer_latest_pos() else { return };
    let painter = egui_context.layer_painter(LayerId::new(Order::Tooltip, Id::new("drag_ghost")));

    let label_galley = painter.layout_no_wrap(label.to_string(), FontId::proportional(LABEL_FONT_SIZE), palette::ON_SURFACE);
    let ghost_rect = Rect::from_min_size(pointer + vec2(14.0, 6.0), vec2(label_galley.size().x + 52.0, 34.0));
    painter.rect_filled(ghost_rect, Rounding::same(17.0), palette::SURFACE_CONTAINER_HIGHEST);
    painter.text(pos2(ghost_rect.left() + 20.0, ghost_rect.center().y), Align2::CENTER_CENTER, icon, FontId::proportional(18.0), palette::PRIMARY);
    painter.galley(pos2(ghost_rect.left() + 38.0, ghost_rect.center().y - label_galley.size().y / 2.0), label_galley, palette::ON_SURFACE);
}

/// Una línea de texto que, si no cabe en `max_width`, termina en "…".
fn single_line_cut_to_width(painter: &Painter, text: &str, max_width: f32, color: Color32) -> Arc<Galley> {
    single_line_cut_to_width_in_font(painter, text, FontId::proportional(LABEL_FONT_SIZE), max_width, color)
}

fn single_line_cut_to_width_in_font(
    painter: &Painter,
    text: &str,
    font: FontId,
    max_width: f32,
    color: Color32,
) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = TextWrapping { max_width: max_width.max(0.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    painter.layout_job(job)
}

/// Texto gris dentro del árbol, por ejemplo "Carpeta vacía".
pub fn tree_placeholder(ui: &mut Ui, depth: usize, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(8.0 + depth as f32 * TREE_INDENT + 50.0);
        ui.label(RichText::new(text).italics().color(palette::OUTLINE));
    });
}

pub struct HistoryRowResponse {
    pub row: Response,
    pub forget_clicked: bool,
    pub save_clicked: bool,
}

const HISTORY_ROW_HEIGHT: f32 = 34.0;
const HISTORY_ROW_BUTTON_SIZE: f32 = 28.0;

/// Fila del panel de historial: un comando en letra de terminal. La fila
/// marcada (la que se elige con Enter) lleva fondo de color. Al pasar el
/// ratón aparecen a la derecha los botones de guardar y de borrar.
pub fn history_row(ui: &mut Ui, command: &str, is_highlighted: bool) -> HistoryRowResponse {
    let (rect, row) = ui.allocate_exact_size(vec2(ui.available_width(), HISTORY_ROW_HEIGHT), Sense::click());
    let row_is_hovered = ui.rect_contains_pointer(rect);

    let button_center_y = rect.center().y;
    let forget_rect = Rect::from_center_size(
        pos2(rect.right() - 6.0 - HISTORY_ROW_BUTTON_SIZE / 2.0, button_center_y),
        vec2(HISTORY_ROW_BUTTON_SIZE, HISTORY_ROW_BUTTON_SIZE),
    );
    let save_rect = forget_rect.translate(vec2(-HISTORY_ROW_BUTTON_SIZE - 2.0, 0.0));
    let forget_button = ui.interact(forget_rect, row.id.with("forget"), Sense::click());
    let save_button = ui.interact(save_rect, row.id.with("save"), Sense::click());

    let painter = ui.painter_at(rect);
    if is_highlighted {
        painter.rect_filled(rect, Rounding::same(8.0), palette::SECONDARY_CONTAINER);
    } else if row_is_hovered {
        painter.rect_filled(rect, Rounding::same(8.0), state_layer(row.is_pointer_button_down_on()));
    }

    // Con los botones a la vista, el texto se corta antes de llegar a ellos.
    let text_end = if row_is_hovered { save_rect.left() - 4.0 } else { rect.right() - 12.0 };
    let text_color = if is_highlighted { palette::ON_SECONDARY_CONTAINER } else { palette::ON_SURFACE };
    let text_left = rect.left() + 12.0;
    let text = single_line_cut_to_width_in_font(&painter, command, FontId::monospace(14.0), text_end - text_left, text_color);
    painter.galley(pos2(text_left, rect.center().y - text.size().y / 2.0), text, text_color);

    if row_is_hovered {
        paint_small_icon_button(&painter, save_rect, &save_button, ICON_LIBRARY_ADD, palette::ON_SURFACE_VARIANT);
        paint_small_icon_button(&painter, forget_rect, &forget_button, ICON_DELETE, palette::ERROR);
    }

    HistoryRowResponse {
        row,
        forget_clicked: forget_button.on_hover_text(tr("history.tooltip.forget")).clicked(),
        save_clicked: save_button.on_hover_text(tr("history.tooltip.save_as_command")).clicked(),
    }
}

fn paint_small_icon_button(painter: &Painter, rect: Rect, button: &Response, icon: &str, color: Color32) {
    if button.hovered() {
        painter.circle_filled(rect.center(), rect.width() / 2.0, state_layer(button.is_pointer_button_down_on()));
    }
    painter.text(rect.center(), Align2::CENTER_CENTER, icon, FontId::proportional(18.0), color);
}

/// Opción de un menú desplegable, con icono a la izquierda.
pub fn menu_row(ui: &mut Ui, icon: &str, label: &str) -> Response {
    menu_row_with_color(ui, icon, label, palette::ON_SURFACE)
}

pub fn menu_row_with_color(ui: &mut Ui, icon: &str, label: &str, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width().max(200.0), 36.0), Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, Rounding::same(8.0), state_layer(response.is_pointer_button_down_on()));
    }
    painter.text(pos2(rect.left() + 22.0, rect.center().y), Align2::CENTER_CENTER, icon, FontId::proportional(ICON_SIZE), color);
    painter.text(pos2(rect.left() + 44.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(LABEL_FONT_SIZE), color);
    response
}

/// Botón principal de un diálogo (Guardar, Eliminar...).
pub fn filled_button(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        Button::new(RichText::new(label).color(palette::ON_PRIMARY).strong())
            .fill(palette::PRIMARY)
            .rounding(Rounding::same(20.0))
            .min_size(vec2(96.0, 40.0)),
    )
}

/// Botón secundario sin fondo (Cancelar).
pub fn text_button(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        Button::new(RichText::new(label).color(palette::PRIMARY).strong())
            .fill(Color32::TRANSPARENT)
            .rounding(Rounding::same(20.0))
            .min_size(vec2(96.0, 40.0)),
    )
}
