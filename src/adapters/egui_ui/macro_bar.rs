//! Barra de macros, debajo de la de pestañas (se muestra y se oculta con el
//! mismo botón que el menú lateral). Cada botón ejecuta un comando en el
//! equipo; las carpetas se despliegan como las de los marcadores de un
//! navegador, y las que van dentro se abren a un lado al pasar el ratón.
//!
//! Todo se maneja igual esté donde esté: clic para ejecutar o desplegar, y
//! clic derecho para el menú de opciones (editar, eliminar y, en una carpeta,
//! añadir dentro). Los desplegables y ese menú se dibujan aquí mismo, no con
//! los menús de egui: con ellos, un menú de clic derecho dentro de un
//! desplegable se cerraba antes de poder pulsarlo, y las subcarpetas no
//! tenían el mismo aspecto que las macros.
//!
//! Para reorganizarla se arrastran los botones y las filas. Al soltar sobre
//! uno, la posición decide (en vertical dentro de un desplegable):
//!
//! ```text
//!  ┌───────────────┐
//!  │ ◀ │ dentro │ ▶ │   ◀ delante · ▶ detrás · dentro (solo carpetas)
//!  └───────────────┘
//! ```

use eframe::egui::{
    self, pos2, vec2, Align, Align2, Area, Button, Color32, CursorIcon, DragAndDrop, FontId, Frame, Id, Key, Layout, Margin,
    Order, Pos2, Rect, Response, RichText, Rounding, ScrollArea, Sense, Stroke, Ui,
};
use egui_material_icons::icons::{
    ICON_ADD, ICON_CHEVRON_RIGHT, ICON_CREATE_NEW_FOLDER, ICON_DELETE, ICON_EDIT, ICON_FOLDER, ICON_PLAY_ARROW,
};

use super::macro_dialog::MacroDialog;
use super::macro_icons::icon_glyph;
use super::material_widgets::{menu_row, menu_row_with_color, paint_drag_ghost, subtle_icon_button};
use super::pointer_cursor::keep_arrow_over;
use super::theme::{palette, state_layer};
use super::ConsoleApp;
use crate::domain::macro_bar::{MacroBar, MacroBarItem, MacroId, MacroLocation, MacroSide};
use crate::i18n::tr;

const CHIP_HEIGHT: f32 = 28.0;
const ADD_BUTTON_WIDTH: f32 = 28.0 + 8.0;
/// Lo que separa la franja de la barra de pestañas y de la terminal: el
/// mismo hueco por arriba que por abajo.
const GAP_ABOVE_STRIP: f32 = 8.0;
const GAP_BELOW_STRIP: f32 = 8.0;
const STRIP_PADDING: f32 = 4.0;
const STRIP_ROUNDING: f32 = 12.0;
const DROP_LINE_WIDTH: f32 = 2.0;
const HINT_LINE_WIDTH: f32 = 1.0;
/// Filas de los desplegables: las mismas medidas que las de los menús de clic derecho.
const ROW_HEIGHT: f32 = 36.0;
const ROW_ICON_SIZE: f32 = 20.0;
const ROW_FONT_SIZE: f32 = 14.0;
const MIN_DROPDOWN_WIDTH: f32 = 200.0;
const MAX_DROPDOWN_WIDTH: f32 = 360.0;
/// Hueco entre un botón y su desplegable, y entre un desplegable y el de su subcarpeta.
const DROPDOWN_GAP: f32 = 4.0;

/// Los desplegables abiertos y el menú de clic derecho. Vive de un fotograma
/// a otro en `ConsoleApp`.
#[derive(Default)]
pub(super) struct MacroMenus {
    /// La carpeta de la barra desplegada y, detrás, las subcarpetas abiertas a su lado.
    open_folders: Vec<OpenFolder>,
    /// Dónde se pulsó la carpeta de la barra desplegada: pulsarla otra vez la cierra.
    open_chip: Option<Rect>,
    options_menu: Option<OptionsMenu>,
    /// Lo que ocupaban los desplegables y el menú de clic derecho en el
    /// fotograma anterior, para saber si una pulsación cae fuera de ellos.
    dropdown_rects: Vec<Rect>,
    options_menu_rect: Option<Rect>,
}

#[derive(Clone, Copy)]
struct OpenFolder {
    id: MacroId,
    /// Esquina superior izquierda del desplegable.
    position: Pos2,
}

/// El menú de clic derecho: sobre qué se abrió y dónde.
#[derive(Clone, Copy)]
struct OptionsMenu {
    target: OptionsTarget,
    position: Pos2,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OptionsTarget {
    /// El fondo de la barra o el botón ➕: crear en la barra.
    Bar,
    Item(MacroId),
}

impl MacroMenus {
    fn close_all(&mut self) {
        self.open_folders.clear();
        self.open_chip = None;
        self.options_menu = None;
    }

    fn open_options(&mut self, target: OptionsTarget, position: Pos2) {
        self.options_menu = Some(OptionsMenu { target, position });
    }

    /// Despliega una carpeta de la barra (cerrando lo que hubiera abierto).
    fn open_bar_folder(&mut self, id: MacroId, chip: Rect) {
        self.open_folders = vec![OpenFolder { id, position: chip.left_bottom() + vec2(0.0, DROPDOWN_GAP) }];
        self.open_chip = Some(chip);
        self.options_menu = None;
    }

    fn is_bar_folder_open(&self, id: MacroId) -> bool {
        self.open_folders.first().is_some_and(|open| open.id == id)
    }

    /// Una pulsación fuera de todo lo abierto lo cierra; una en un desplegable
    /// (fuera del menú de clic derecho) cierra solo ese menú.
    fn close_on_press_outside(&mut self, egui_context: &egui::Context) {
        let pressed_at = egui_context.input(|input| input.pointer.any_pressed().then(|| input.pointer.interact_pos()).flatten());
        let Some(point) = pressed_at else { return };
        let inside_options_menu = self.options_menu_rect.is_some_and(|rect| rect.contains(point));
        let inside_dropdown = self.dropdown_rects.iter().any(|rect| rect.contains(point));
        let inside_open_chip = self.open_chip.is_some_and(|chip| chip.contains(point));
        if inside_options_menu {
            return;
        }
        self.options_menu = None;
        if !inside_dropdown && !inside_open_chip {
            self.close_all();
        }
    }
}

/// Lo que viaja con el puntero mientras se arrastra un botón.
#[derive(Clone)]
struct DraggedMacro {
    id: MacroId,
    label: String,
    glyph: &'static str,
}

/// Lo pedido al pulsar algo en la barra; se aplica después de dibujarla.
enum MacroBarAction {
    Run(MacroId),
    OpenDialog(MacroDialog),
    Edit(MacroId),
    Remove(MacroId),
    MoveBeside(MacroId, MacroId, MacroSide),
    MoveInto(MacroId, MacroLocation),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy)]
enum DropZone {
    Before,
    Inside,
    After,
}

impl ConsoleApp {
    /// Hay que llamarla justo después de la barra de pestañas, para quedar debajo.
    pub(super) fn show_macro_bar(&mut self, egui_context: &egui::Context) {
        let mut actions = Vec::new();
        if !self.sidebar_is_visible || self.a_dialog_is_open() {
            self.macro_menus.close_all();
        }
        self.macro_menus.close_on_press_outside(egui_context);
        if egui_context.input(|input| input.key_pressed(Key::Escape)) {
            self.macro_menus.close_all();
        }

        let bar = self.macro_bar.bar();
        let menus = &mut self.macro_menus;
        let panel_frame = Frame::none().fill(palette::SURFACE).inner_margin(Margin { left: 8.0, right: 8.0, top: GAP_ABOVE_STRIP, bottom: GAP_BELOW_STRIP });
        // Una franja redondeada de otro tono, para que no se confunda con la barra de pestañas.
        let strip_frame = Frame::none()
            .fill(palette::SURFACE_CONTAINER_LOW)
            .rounding(Rounding::same(STRIP_ROUNDING))
            .inner_margin(Margin::symmetric(6.0, STRIP_PADDING));
        egui::TopBottomPanel::top("macro_bar").frame(panel_frame).show_animated(egui_context, self.sidebar_is_visible, |ui| {
            strip_frame.show(ui, |ui| show_strip(ui, bar, menus, &mut actions));
        });

        let dropdown_rects = show_open_folders(egui_context, bar, menus, &mut actions);
        let options_menu_rect = show_options_menu(egui_context, bar, menus, &mut actions);
        self.macro_menus.dropdown_rects = dropdown_rects;
        self.macro_menus.options_menu_rect = options_menu_rect;
        show_dragged_macro_under_pointer(egui_context);

        for action in actions {
            self.apply_macro_bar_action(action);
        }
    }

    fn apply_macro_bar_action(&mut self, action: MacroBarAction) {
        let result = match action {
            MacroBarAction::Run(id) => self.macro_bar.run(id).map(|message| self.show_status(message)),
            MacroBarAction::OpenDialog(dialog) => {
                self.macro_dialog = Some(dialog);
                Ok(())
            }
            MacroBarAction::Edit(id) => {
                self.macro_dialog = MacroDialog::edit(self.macro_bar.bar(), id);
                Ok(())
            }
            MacroBarAction::Remove(id) => {
                self.macro_dialog = MacroDialog::confirm_removal(self.macro_bar.bar(), id);
                Ok(())
            }
            MacroBarAction::MoveBeside(id, sibling, side) => self.macro_bar.move_beside(id, sibling, side),
            MacroBarAction::MoveInto(id, location) => self.macro_bar.move_into(id, location),
        };
        if let Err(error) = result {
            self.show_status(error);
        }
    }
}

/// La franja con los botones de la barra, el ➕ y el hueco para soltar al final.
fn show_strip(ui: &mut Ui, bar: &MacroBar, menus: &mut MacroMenus, actions: &mut Vec<MacroBarAction>) {
    // Los botones sin fondo hasta pasar el ratón por encima, como los marcadores.
    ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    ui.spacing_mut().button_padding = vec2(10.0, 4.0);
    ui.spacing_mut().item_spacing.x = 4.0;

    // El fondo solo sirve para el clic derecho: sigue con la flecha normal.
    let background = ui.interact(ui.max_rect(), Id::new("macro_bar_background"), Sense::click());
    keep_arrow_over(ui.ctx(), background.id);
    if background.secondary_clicked() {
        open_options_at_pointer(ui, menus, OptionsTarget::Bar);
    }

    // Todo en una fila de altura fija, centrado en vertical.
    let row_size = vec2(ui.available_width(), CHIP_HEIGHT);
    ui.allocate_ui_with_layout(row_size, Layout::left_to_right(Align::Center), |ui| {
        ScrollArea::horizontal()
            .id_salt("macro_bar_scroll")
            .max_width((ui.available_width() - ADD_BUTTON_WIDTH).max(0.0))
            .drag_to_scroll(false)
            .show(ui, |ui| {
                ui.allocate_ui_with_layout(vec2(0.0, CHIP_HEIGHT), Layout::left_to_right(Align::Center), |ui| {
                    if bar.items().is_empty() {
                        ui.label(RichText::new(tr("macros.empty_bar")).small().color(palette::OUTLINE));
                    }
                    for item in bar.items() {
                        show_chip(ui, item, menus, actions);
                    }
                });
            });
        let add_button = subtle_icon_button(ui, ICON_ADD, &tr("macros.add_tooltip"));
        if add_button.clicked() {
            menus.close_all();
            menus.open_options(OptionsTarget::Bar, add_button.rect.left_bottom() + vec2(0.0, DROPDOWN_GAP));
        }
        accept_drop_at_end_of_bar(ui, actions);
    });
}

/// Un botón de la barra: una macro se ejecuta al pulsarlo; una carpeta se despliega.
fn show_chip(ui: &mut Ui, item: &MacroBarItem, menus: &mut MacroMenus, actions: &mut Vec<MacroBarAction>) {
    let is_open = menus.is_bar_folder_open(item.id());
    let button = Button::new(chip_text(item))
        .min_size(vec2(0.0, CHIP_HEIGHT))
        .rounding(Rounding::same(CHIP_HEIGHT / 2.0))
        .selected(is_open)
        .sense(Sense::click_and_drag());
    let response = ui.add(button);
    let response = match item {
        MacroBarItem::Macro(macro_item) => response.on_hover_text(macro_item.command_line()),
        MacroBarItem::Folder(_) => response,
    };

    if response.secondary_clicked() {
        open_options_at_pointer(ui, menus, OptionsTarget::Item(item.id()));
    } else if response.clicked() {
        match item {
            MacroBarItem::Macro(macro_item) => {
                actions.push(MacroBarAction::Run(macro_item.id));
                menus.close_all();
            }
            MacroBarItem::Folder(_) if is_open => menus.close_all(),
            MacroBarItem::Folder(folder) => menus.open_bar_folder(folder.id, response.rect),
        }
    } else if let MacroBarItem::Folder(folder) = item {
        // Con un desplegable abierto, pasar a otra carpeta de la barra la despliega a ella.
        let another_is_open = !menus.open_folders.is_empty() && !is_open;
        if another_is_open && response.hovered() && menus.options_menu.is_none() && !is_dragging(ui) {
            menus.open_bar_folder(folder.id, response.rect);
        }
    }
    make_draggable(&response, item);
    accept_drop(ui, &response, item, Direction::Horizontal, actions);
}

/// Icono y nombre; una carpeta sin icono propio lleva el de carpeta.
fn chip_text(item: &MacroBarItem) -> RichText {
    let text = match (item_glyph(item), item.name()) {
        (Some(glyph), "") => glyph.to_string(),
        (Some(glyph), name) => format!("{glyph}  {name}"),
        (None, name) => name.to_string(),
    };
    RichText::new(text).size(13.0).color(palette::ON_SURFACE_VARIANT)
}

fn item_glyph(item: &MacroBarItem) -> Option<&'static str> {
    icon_glyph(item.icon()).or(item.is_folder().then_some(ICON_FOLDER))
}

/// El nombre, o el comando si la macro no tiene nombre.
fn item_label(item: &MacroBarItem) -> String {
    match item {
        MacroBarItem::Macro(macro_item) if macro_item.name.is_empty() => macro_item.command_line(),
        MacroBarItem::Folder(_) if item.name().is_empty() => tr("macros.unnamed_folder"),
        _ => item.name().to_string(),
    }
}

/// Dibuja los desplegables abiertos, uno al lado de otro. Devuelve lo que ocupan.
fn show_open_folders(
    egui_context: &egui::Context,
    bar: &MacroBar,
    menus: &mut MacroMenus,
    actions: &mut Vec<MacroBarAction>,
) -> Vec<Rect> {
    let mut rects = Vec::new();
    let mut level = 0;
    while let Some(open) = menus.open_folders.get(level).copied() {
        let Some(MacroBarItem::Folder(folder)) = bar.find(open.id) else {
            // La carpeta ya no existe (se ha borrado o movido): se cierra desde aquí.
            menus.open_folders.truncate(level);
            break;
        };
        let width = dropdown_width(egui_context, &folder.children);
        let area = Area::new(Id::new(("macro_folder_dropdown", level)))
            .order(Order::Foreground)
            .fixed_pos(open.position)
            .show(egui_context, |ui| {
                Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if folder.children.is_empty() {
                        let placeholder = empty_folder_row(ui, width);
                        if placeholder.secondary_clicked() {
                            open_options_at_pointer(ui, menus, OptionsTarget::Item(folder.id));
                        }
                    }
                    for child in &folder.children {
                        show_dropdown_row(ui, child, level, width, menus, actions);
                    }
                });
            });
        rects.push(area.response.rect);
        level += 1;
    }
    rects
}

/// Una fila de un desplegable: icono y nombre y, si es una carpeta, la flecha.
/// Al pasar por una carpeta se abre a un lado; por una macro, se cierran las
/// subcarpetas que hubiera abiertas.
fn show_dropdown_row(
    ui: &mut Ui,
    item: &MacroBarItem,
    level: usize,
    width: f32,
    menus: &mut MacroMenus,
    actions: &mut Vec<MacroBarAction>,
) {
    let is_open_subfolder = menus.open_folders.get(level + 1).is_some_and(|open| open.id == item.id());
    let response = dropdown_row(ui, item, width, is_open_subfolder);
    let response = match item {
        MacroBarItem::Macro(macro_item) => response.on_hover_text(macro_item.command_line()),
        MacroBarItem::Folder(_) => response,
    };

    if response.secondary_clicked() {
        open_options_at_pointer(ui, menus, OptionsTarget::Item(item.id()));
    } else if response.clicked() {
        match item {
            MacroBarItem::Macro(macro_item) => {
                actions.push(MacroBarAction::Run(macro_item.id));
                menus.close_all();
            }
            MacroBarItem::Folder(folder) => open_subfolder(menus, level, folder.id, response.rect),
        }
    } else if response.hovered() && menus.options_menu.is_none() && !is_dragging(ui) {
        match item {
            MacroBarItem::Folder(folder) if !is_open_subfolder => open_subfolder(menus, level, folder.id, response.rect),
            MacroBarItem::Folder(_) => {}
            MacroBarItem::Macro(_) => menus.open_folders.truncate(level + 1),
        }
    }
    make_draggable(&response, item);
    accept_drop(ui, &response, item, Direction::Vertical, actions);
}

fn open_subfolder(menus: &mut MacroMenus, level: usize, folder: MacroId, row: Rect) {
    let menu_margin = 8.0;
    menus.open_folders.truncate(level + 1);
    menus.open_folders.push(OpenFolder { id: folder, position: pos2(row.right() + menu_margin + DROPDOWN_GAP, row.top() - menu_margin) });
}

fn dropdown_row(ui: &mut Ui, item: &MacroBarItem, width: f32, is_highlighted: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    if response.hovered() || is_highlighted {
        painter.rect_filled(rect, Rounding::same(8.0), state_layer(response.is_pointer_button_down_on()));
    }
    let glyph = item_glyph(item).unwrap_or(ICON_PLAY_ARROW);
    let center_y = rect.center().y;
    painter.text(pos2(rect.left() + 22.0, center_y), Align2::CENTER_CENTER, glyph, FontId::proportional(ROW_ICON_SIZE), palette::ON_SURFACE);
    painter.text(pos2(rect.left() + 44.0, center_y), Align2::LEFT_CENTER, item_label(item), FontId::proportional(ROW_FONT_SIZE), palette::ON_SURFACE);
    if item.is_folder() {
        let arrow = pos2(rect.right() - 16.0, center_y);
        painter.text(arrow, Align2::CENTER_CENTER, ICON_CHEVRON_RIGHT, FontId::proportional(ROW_ICON_SIZE), palette::ON_SURFACE_VARIANT);
    }
    response
}

/// En una carpeta vacía, una fila apagada (con clic derecho para añadir dentro).
fn empty_folder_row(ui: &mut Ui, width: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click());
    keep_arrow_over(ui.ctx(), response.id);
    ui.painter().text(
        pos2(rect.left() + 44.0, rect.center().y),
        Align2::LEFT_CENTER,
        tr("macros.empty_folder"),
        FontId::proportional(ROW_FONT_SIZE),
        palette::OUTLINE,
    );
    response
}

/// Lo justo para el nombre más largo (con icono y flecha), dentro de unos límites.
fn dropdown_width(egui_context: &egui::Context, items: &[MacroBarItem]) -> f32 {
    let longest_label = egui_context.fonts(|fonts| {
        items
            .iter()
            .map(|item| fonts.layout_no_wrap(item_label(item), FontId::proportional(ROW_FONT_SIZE), Color32::WHITE).size().x)
            .fold(0.0, f32::max)
    });
    (longest_label + 44.0 + 40.0).clamp(MIN_DROPDOWN_WIDTH, MAX_DROPDOWN_WIDTH)
}

fn open_options_at_pointer(ui: &Ui, menus: &mut MacroMenus, target: OptionsTarget) {
    if let Some(pointer) = ui.ctx().pointer_latest_pos() {
        if target == OptionsTarget::Bar {
            menus.open_folders.clear();
            menus.open_chip = None;
        }
        menus.open_options(target, pointer);
    }
}

/// El menú de clic derecho, igual para un botón de la barra que para una fila
/// de un desplegable. Devuelve lo que ocupa.
fn show_options_menu(
    egui_context: &egui::Context,
    bar: &MacroBar,
    menus: &mut MacroMenus,
    actions: &mut Vec<MacroBarAction>,
) -> Option<Rect> {
    let menu = menus.options_menu?;
    let target_item = match menu.target {
        OptionsTarget::Bar => None,
        OptionsTarget::Item(id) => match bar.find(id) {
            Some(item) => Some(item),
            None => {
                menus.options_menu = None;
                return None;
            }
        },
    };
    let mut chosen = None;
    let area = Area::new(Id::new("macro_options_menu"))
        .order(Order::Foreground)
        .fixed_pos(menu.position)
        .constrain(true)
        .show(egui_context, |ui| {
            Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_width(MIN_DROPDOWN_WIDTH);
                ui.spacing_mut().item_spacing.y = 0.0;
                chosen = options_menu_contents(ui, target_item);
            })
        });
    // Encima de los desplegables, que se dibujan antes.
    egui_context.move_to_top(area.response.layer_id);
    if let Some(action) = chosen {
        actions.push(action);
        menus.close_all();
    }
    Some(area.response.rect)
}

fn options_menu_contents(ui: &mut Ui, item: Option<&MacroBarItem>) -> Option<MacroBarAction> {
    let mut chosen = None;
    let location_for_new = match item {
        None => Some(MacroLocation::Bar),
        Some(MacroBarItem::Folder(folder)) => Some(MacroLocation::InsideFolder(folder.id)),
        Some(MacroBarItem::Macro(_)) => None,
    };
    if let Some(location) = location_for_new {
        if menu_row(ui, ICON_ADD, &tr("macros.menu.new_macro")).clicked() {
            chosen = Some(MacroBarAction::OpenDialog(MacroDialog::new_macro(location)));
        }
        if menu_row(ui, ICON_CREATE_NEW_FOLDER, &tr("macros.menu.new_folder")).clicked() {
            chosen = Some(MacroBarAction::OpenDialog(MacroDialog::new_folder(location)));
        }
    }
    if let Some(item) = item {
        if location_for_new.is_some() {
            ui.separator();
        }
        if menu_row(ui, ICON_EDIT, &tr("macros.menu.edit")).clicked() {
            chosen = Some(MacroBarAction::Edit(item.id()));
        }
        if menu_row_with_color(ui, ICON_DELETE, &tr("macros.menu.delete"), palette::ERROR).clicked() {
            chosen = Some(MacroBarAction::Remove(item.id()));
        }
    }
    chosen
}

fn is_dragging(ui: &Ui) -> bool {
    DragAndDrop::has_payload_of_type::<DraggedMacro>(ui.ctx())
}

fn make_draggable(response: &Response, item: &MacroBarItem) {
    if response.drag_started() {
        let label = if item.name().is_empty() { tr("macros.unnamed") } else { item.name().to_string() };
        response.dnd_set_drag_payload(DraggedMacro { id: item.id(), label, glyph: item_glyph(item).unwrap_or(ICON_PLAY_ARROW) });
    }
}

/// Si se arrastra algo sobre este botón o fila, dibuja dónde caerá y, al
/// soltar, pide moverlo.
fn accept_drop(ui: &Ui, target: &Response, item: &MacroBarItem, direction: Direction, actions: &mut Vec<MacroBarAction>) {
    let Some(dragged) = target.dnd_hover_payload::<DraggedMacro>() else { return };
    if dragged.id == item.id() {
        return;
    }
    let Some(pointer) = ui.ctx().pointer_latest_pos() else { return };
    let zone = drop_zone_at(target.rect, pointer, direction, item.is_folder());
    paint_drop_hint(ui, target.rect, zone, direction);
    if target.dnd_release_payload::<DraggedMacro>().is_none() {
        return;
    }
    actions.push(match zone {
        DropZone::Before => MacroBarAction::MoveBeside(dragged.id, item.id(), MacroSide::Before),
        DropZone::After => MacroBarAction::MoveBeside(dragged.id, item.id(), MacroSide::After),
        DropZone::Inside => MacroBarAction::MoveInto(dragged.id, MacroLocation::InsideFolder(item.id())),
    });
}

/// Soltar en el hueco libre de la barra lo saca de las carpetas y lo deja al final.
fn accept_drop_at_end_of_bar(ui: &mut Ui, actions: &mut Vec<MacroBarAction>) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), CHIP_HEIGHT), Sense::hover());
    if is_dragging(ui) {
        let is_over = response.dnd_hover_payload::<DraggedMacro>().is_some();
        let color = if is_over { palette::PRIMARY } else { palette::OUTLINE };
        let hint = rect.shrink2(vec2(4.0, 1.0));
        if is_over {
            ui.painter().rect_filled(hint, Rounding::same(CHIP_HEIGHT / 2.0), palette::PRIMARY.gamma_multiply(0.12));
        }
        ui.painter().rect_stroke(hint, Rounding::same(CHIP_HEIGHT / 2.0), Stroke::new(HINT_LINE_WIDTH, color));
        ui.painter().text(hint.center(), Align2::CENTER_CENTER, tr("macros.drop_on_bar"), FontId::proportional(12.0), color);
    }
    if let Some(dragged) = response.dnd_release_payload::<DraggedMacro>() {
        actions.push(MacroBarAction::MoveInto(dragged.id, MacroLocation::Bar));
    }
}

fn drop_zone_at(rect: Rect, pointer: Pos2, direction: Direction, is_folder: bool) -> DropZone {
    let fraction = match direction {
        Direction::Horizontal => (pointer.x - rect.left()) / rect.width(),
        Direction::Vertical => (pointer.y - rect.top()) / rect.height(),
    };
    if is_folder {
        match fraction {
            f if f < 0.25 => DropZone::Before,
            f if f > 0.75 => DropZone::After,
            _ => DropZone::Inside,
        }
    } else if fraction < 0.5 {
        DropZone::Before
    } else {
        DropZone::After
    }
}

fn paint_drop_hint(ui: &Ui, rect: Rect, zone: DropZone, direction: Direction) {
    let stroke = Stroke::new(DROP_LINE_WIDTH, palette::PRIMARY);
    let painter = ui.ctx().layer_painter(ui.layer_id());
    let line = match (zone, direction) {
        (DropZone::Inside, _) => {
            painter.rect_filled(rect, Rounding::same(8.0), palette::PRIMARY.gamma_multiply(0.12));
            painter.rect_stroke(rect, Rounding::same(8.0), stroke);
            return;
        }
        (DropZone::Before, Direction::Horizontal) => [rect.left_top(), rect.left_bottom()],
        (DropZone::After, Direction::Horizontal) => [rect.right_top(), rect.right_bottom()],
        (DropZone::Before, Direction::Vertical) => [rect.left_top(), rect.right_top()],
        (DropZone::After, Direction::Vertical) => [rect.left_bottom(), rect.right_bottom()],
    };
    painter.line_segment(line, stroke);
}

/// Mientras se arrastra, una etiqueta sigue al puntero.
fn show_dragged_macro_under_pointer(egui_context: &egui::Context) {
    let Some(dragged) = DragAndDrop::payload::<DraggedMacro>(egui_context) else { return };
    egui_context.set_cursor_icon(CursorIcon::Grabbing);
    paint_drag_ghost(egui_context, dragged.glyph, &dragged.label);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chip() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 28.0))
    }

    #[test]
    fn dropping_on_the_middle_of_a_folder_goes_inside() {
        let middle = pos2(50.0, 14.0);
        assert!(matches!(drop_zone_at(chip(), middle, Direction::Horizontal, true), DropZone::Inside));
        assert!(matches!(drop_zone_at(chip(), middle, Direction::Horizontal, false), DropZone::After));
    }

    #[test]
    fn the_edges_place_it_beside() {
        assert!(matches!(drop_zone_at(chip(), pos2(5.0, 14.0), Direction::Horizontal, true), DropZone::Before));
        assert!(matches!(drop_zone_at(chip(), pos2(95.0, 14.0), Direction::Horizontal, true), DropZone::After));
        assert!(matches!(drop_zone_at(chip(), pos2(95.0, 2.0), Direction::Vertical, false), DropZone::Before));
    }
}
