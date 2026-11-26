use eframe::egui::{self, vec2, Align, Area, Frame, Id, Label, Layout, Margin, Order, PopupCloseBehavior, Rect, RichText, ScrollArea, Ui};
use egui_material_icons::icons::{ICON_ADD, ICON_DNS, ICON_FOLDER, ICON_HOME, ICON_LEFT_PANEL_CLOSE, ICON_LEFT_PANEL_OPEN, ICON_TERMINAL};

use super::material_widgets::{
    icon_button, menu_row, menu_row_with_submenu, terminal_tab, window_control_button, RenameOutcome, WindowControl,
};
use super::rename_in_place::{rename_field_for, RenameTarget};
use super::theme::palette;
use super::{window_frame, ConsoleApp};
use crate::domain::app_settings::ShortcutAction;
use crate::domain::saved_places::{PlaceId, PlaceTarget, SavedPlace};
use crate::domain::tab_journal::TabId;
use crate::i18n::{tr, tr_args};

/// Hueco que se deja tras las pestañas para el botón `+`.
const NEW_TAB_BUTTON_WIDTH: f32 = 36.0 + 8.0;
const STATUS_MESSAGE_MAX_WIDTH: f32 = 320.0;

enum TabBarAction {
    ToggleSidebar,
    ActivateTab(usize),
    CloseTab(usize),
    StartRename(TabId, String),
    FinishRename(RenameOutcome),
    OpenTabWithProfile(usize),
    /// Una ruta guardada de esa terminal (carpeta o servidor SSH).
    OpenPlace(usize, PlaceId),
    HideToTray,
    Exit,
}

impl ConsoleApp {
    pub(super) fn show_tab_bar(&mut self, egui_context: &egui::Context) {
        let mut actions = Vec::new();

        let bar_frame = Frame::none().fill(palette::SURFACE).inner_margin(Margin { left: 8.0, right: 16.0, top: 6.0, bottom: 2.0 });
        egui::TopBottomPanel::top("tab_bar").frame(bar_frame).show(egui_context, |ui| {
            // Sin barra de título de Windows, esta barra hace de barra de título.
            let title_bar = window_frame::title_bar_background(ui);
            window_frame::move_or_maximize_from_title_bar(egui_context, &title_bar);

            // Primero lo de la derecha (botones de la ventana y estado); las
            // pestañas usan solo el hueco que queda, así nunca se pisan.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.show_window_controls(ui, egui_context, &mut actions);
                self.show_update_button(ui);
                self.show_status_message(ui);
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    self.show_sidebar_toggle(ui, &mut actions);
                    ui.add_space(8.0);
                    self.show_tabs(ui, &mut actions);
                    self.show_new_tab_button(ui, &mut actions);
                });
            });
        });

        for action in actions {
            self.apply_tab_bar_action(action);
        }
    }

    /// De derecha a izquierda: salir, esconder, maximizar y minimizar.
    fn show_window_controls(&self, ui: &mut Ui, egui_context: &egui::Context, actions: &mut Vec<TabBarAction>) {
        ui.spacing_mut().item_spacing.x = 2.0;
        if window_control_button(ui, WindowControl::Exit, &tr("window.exit")).clicked() {
            actions.push(TabBarAction::Exit);
        }
        if window_control_button(ui, WindowControl::HideToTray, &tr("window.hide_to_tray")).clicked() {
            actions.push(TabBarAction::HideToTray);
        }
        let (maximize_control, maximize_tooltip) = if window_frame::is_maximized(egui_context) {
            (WindowControl::Restore, tr("window.restore"))
        } else {
            (WindowControl::Maximize, tr("window.maximize"))
        };
        if window_control_button(ui, maximize_control, &maximize_tooltip).clicked() {
            window_frame::toggle_maximized(egui_context);
        }
        if window_control_button(ui, WindowControl::Minimize, &tr("window.minimize")).clicked() {
            window_frame::minimize(egui_context);
        }
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.add_space(8.0);
    }

    /// Recortado con "…" para que un mensaje largo no quite sitio a las pestañas
    /// (al pasar el ratón se ve entero).
    /// Pasados unos segundos se desvanece y desaparece.
    fn show_status_message(&mut self, ui: &mut Ui) {
        let Some(message) = &self.status_message else { return };
        let visibility = message.visibility();
        if visibility <= 0.0 {
            self.status_message = None;
            return;
        }
        ui.ctx().request_repaint_after(message.next_change_in());
        let color = palette::ON_SURFACE_VARIANT.gamma_multiply(visibility);
        let width = STATUS_MESSAGE_MAX_WIDTH.min(ui.available_width() * 0.4);
        ui.allocate_ui_with_layout(vec2(width, 32.0), Layout::right_to_left(Align::Center), |ui| {
            ui.add(Label::new(RichText::new(&message.text).small().color(color)).truncate());
        });
    }

    fn show_sidebar_toggle(&self, ui: &mut Ui, actions: &mut Vec<TabBarAction>) {
        let (sidebar_icon, sidebar_tooltip) = if self.sidebar_is_visible {
            (ICON_LEFT_PANEL_CLOSE, tr("sidebar.hide"))
        } else {
            (ICON_LEFT_PANEL_OPEN, tr("sidebar.show"))
        };
        if icon_button(ui, sidebar_icon, &sidebar_tooltip).clicked() {
            actions.push(TabBarAction::ToggleSidebar);
        }
    }

    /// Si no caben, se desplazan en horizontal (con la rueda). Al cambiar de
    /// pestaña, la activa se desplaza hasta quedar a la vista. Doble clic en el
    /// nombre: renombrarla ahí mismo.
    fn show_tabs(&mut self, ui: &mut Ui, actions: &mut Vec<TabBarAction>) {
        let active_index = self.terminal_tabs.active_index();
        let active_tab_must_be_brought_into_view = active_index != self.tab_brought_into_view;
        let tabs = self.terminal_tabs.tabs_overview();

        ScrollArea::horizontal()
            .id_salt("tabs")
            .max_width((ui.available_width() - NEW_TAB_BUTTON_WIDTH).max(0.0))
            .drag_to_scroll(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (index, overview) in tabs.into_iter().enumerate() {
                        let is_active = active_index == Some(index);
                        let Some(tab_id) = self.terminal_tabs.tab_id(index) else { continue };
                        let rename_field = rename_field_for(&mut self.renaming, RenameTarget::Tab(tab_id));
                        let tab = terminal_tab(ui, &overview, is_active, rename_field);
                        if is_active && active_tab_must_be_brought_into_view {
                            tab.tab.scroll_to_me(None);
                        }
                        if let Some(outcome) = tab.rename {
                            actions.push(TabBarAction::FinishRename(outcome));
                        } else if tab.close_requested {
                            actions.push(TabBarAction::CloseTab(index));
                        } else if tab.rename_requested {
                            let current_title = self.terminal_tabs.title_of(index).unwrap_or_default().to_string();
                            actions.push(TabBarAction::StartRename(tab_id, current_title));
                        } else if tab.selected {
                            actions.push(TabBarAction::ActivateTab(index));
                        }
                    }
                });
            });
        self.tab_brought_into_view = active_index;
    }

    /// El menú ➕: una opción por terminal. Las que tienen rutas guardadas
    /// las despliegan al pasar el ratón; pulsar la terminal la abre como siempre.
    fn show_new_tab_button(&mut self, ui: &mut Ui, actions: &mut Vec<TabBarAction>) {
        let tooltip = tr_args("tabs.new_tab_tooltip", &[("shortcut", &self.app_settings.shortcut(ShortcutAction::NewTab))]);
        let new_tab_button = icon_button(ui, ICON_ADD, &tooltip);
        let new_tab_menu_id = ui.make_persistent_id("new_tab_menu");
        if new_tab_button.clicked() {
            ui.memory_mut(|memory| memory.toggle_popup(new_tab_menu_id));
        }
        if !ui.memory(|memory| memory.is_popup_open(new_tab_menu_id)) {
            self.new_tab_submenu = None;
        }
        let mut open_submenu = self.new_tab_submenu;
        egui::popup_below_widget(ui, new_tab_menu_id, &new_tab_button, PopupCloseBehavior::CloseOnClick, |ui| {
            for (index, profile) in self.shell_profiles.iter().enumerate() {
                let places = self.saved_places.places().places_of(&profile.display_name);
                if places.is_empty() {
                    let row = menu_row(ui, ICON_TERMINAL, &profile.display_name);
                    if row.hovered() {
                        open_submenu = None;
                    }
                    if row.clicked() {
                        actions.push(TabBarAction::OpenTabWithProfile(index));
                    }
                    continue;
                }
                let row = menu_row_with_submenu(ui, ICON_TERMINAL, &profile.display_name, open_submenu == Some(index));
                if row.hovered() {
                    open_submenu = Some(index);
                }
                if row.clicked() {
                    actions.push(TabBarAction::OpenTabWithProfile(index));
                }
                if open_submenu == Some(index) {
                    show_places_submenu(ui.ctx(), row.rect, index, places, actions);
                }
            }
        });
        self.new_tab_submenu = open_submenu;
    }

    fn apply_tab_bar_action(&mut self, action: TabBarAction) {
        // El historial abierto es el de la pestaña actual; al cambiar de pestaña ya no vale.
        self.close_history_picker();
        match action {
            TabBarAction::ToggleSidebar => self.sidebar_is_visible = !self.sidebar_is_visible,
            TabBarAction::ActivateTab(index) => {
                self.terminal_tabs.activate_tab(index);
                self.terminal_should_take_focus = true;
            }
            TabBarAction::CloseTab(index) => self.close_tab_asking_if_saved(index),
            TabBarAction::StartRename(tab, current_title) => self.start_renaming(RenameTarget::Tab(tab), &current_title),
            TabBarAction::FinishRename(outcome) => self.finish_renaming(outcome),
            TabBarAction::OpenTabWithProfile(index) => {
                if let Some(profile) = self.shell_profiles.get(index).cloned() {
                    self.open_tab_with(&profile);
                }
            }
            TabBarAction::OpenPlace(index, place) => {
                if let Some(profile) = self.shell_profiles.get(index).cloned() {
                    self.open_place(&profile, place);
                }
            }
            TabBarAction::HideToTray => self.hide_to_tray(),
            TabBarAction::Exit => self.exit_app(),
        }
    }
}

/// Las rutas de una terminal, a la derecha de su opción en el menú ➕. Si
/// tienen alias o comando, al pasar el ratón se ve a dónde llevan y qué lanzan.
fn show_places_submenu(egui_context: &egui::Context, row: Rect, profile_index: usize, places: &[SavedPlace], actions: &mut Vec<TabBarAction>) {
    Area::new(Id::new("new_tab_places").with(profile_index))
        // Por encima del menú del que sale.
        .order(Order::Tooltip)
        .fixed_pos(row.right_top() + vec2(8.0, -8.0))
        .show(egui_context, |ui| {
            Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(240.0);
                if menu_row(ui, ICON_HOME, &tr("places.default_folder")).clicked() {
                    actions.push(TabBarAction::OpenTabWithProfile(profile_index));
                }
                ui.separator();
                for place in places {
                    let icon = match place.target {
                        PlaceTarget::LocalFolder(_) => ICON_FOLDER,
                        PlaceTarget::Ssh { .. } => ICON_DNS,
                    };
                    let mut row = menu_row(ui, icon, &place.label());
                    let mut details = Vec::new();
                    if place.alias.is_some() {
                        details.push(place.target.to_string());
                    }
                    if let Some(command) = &place.command {
                        details.push(format!("▶ {command}"));
                    }
                    if !details.is_empty() {
                        row = row.on_hover_text(details.join("\n"));
                    }
                    if row.clicked() {
                        actions.push(TabBarAction::OpenPlace(profile_index, place.id));
                    }
                }
            });
        });
}
