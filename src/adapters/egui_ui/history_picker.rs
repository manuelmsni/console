//! Panel que sube desde abajo con el historial de comandos de la pestaña.
//! Se abre con Ctrl+↑ en el prompt; se elige con ↑/↓ y Enter o con clic, y se
//! cierra con Esc, clic fuera o empezando a escribir. Cada fila tiene, al
//! pasar el ratón, botones para guardarla como comando o borrarla (también Supr).

use std::time::{Duration, Instant};

use eframe::egui::{
    self, pos2, Align, Align2, Area, Event, Frame, Id, Key, Margin, Modifiers, Order, Rect, Rounding, ScrollArea,
};

use super::library_dialog::LibraryDialog;
use super::material_widgets::history_row;
use super::theme::palette;
use super::ConsoleApp;
use crate::domain::command_library::TreeLocation;
use crate::i18n::tr;

const SLIDE_UP_DURATION: Duration = Duration::from_millis(160);
const ROW_HEIGHT: f32 = 34.0;
const MAX_PANEL_WIDTH: f32 = 760.0;
/// Fracción de la altura de la terminal que puede ocupar el panel como mucho.
const MAX_HEIGHT_FRACTION: f32 = 0.6;
/// Saltos de Re Pág / Av Pág.
const PAGE_SIZE: isize = 10;

pub(super) struct HistoryPicker {
    /// Del más antiguo (arriba) al más reciente (abajo, junto al prompt).
    commands: Vec<String>,
    highlighted: usize,
    /// Al moverse con el teclado, la lista se desplaza para que se vea la fila marcada.
    keep_highlighted_visible: bool,
    opened_at: Instant,
    /// Altura real del panel en el último dibujo, para la animación de subida.
    measured_height: f32,
}

enum PickerOutcome {
    StillOpen,
    Chosen(String),
    Cancelled,
    /// Borrar del historial; el panel sigue abierto.
    Forget(String),
    /// Guardar como comando: se cierra el panel y se abre el diálogo.
    SaveAsCommand(String),
}

impl HistoryPicker {
    fn showing(commands: Vec<String>) -> Self {
        let newest = commands.len().saturating_sub(1);
        // La altura real se conoce tras el primer dibujo; hasta entonces basta una estimación.
        let estimated_height = commands.len() as f32 * (ROW_HEIGHT + 2.0) + 16.0;
        Self {
            commands,
            highlighted: newest,
            keep_highlighted_visible: true,
            opened_at: Instant::now(),
            measured_height: estimated_height,
        }
    }

    fn move_highlight_by(&mut self, rows: isize) {
        let last = self.commands.len().saturating_sub(1) as isize;
        self.highlighted = (self.highlighted as isize + rows).clamp(0, last) as usize;
        self.keep_highlighted_visible = true;
    }

    fn highlighted_command(&self) -> String {
        self.commands[self.highlighted].clone()
    }

    fn remove(&mut self, command: &str) {
        self.commands.retain(|listed| listed != command);
        self.highlighted = self.highlighted.min(self.commands.len().saturating_sub(1));
    }

    /// De 0 (escondido abajo) a 1 (del todo arriba), frenando al final.
    fn slide_progress(&self) -> f32 {
        let linear = (self.opened_at.elapsed().as_secs_f32() / SLIDE_UP_DURATION.as_secs_f32()).min(1.0);
        1.0 - (1.0 - linear).powi(3)
    }
}

impl ConsoleApp {
    pub(super) fn open_history_picker(&mut self) {
        let Some(session) = self.terminal_tabs.active_session_mut() else { return };
        let commands = session.command_history().to_vec();
        if !commands.is_empty() {
            self.history_picker = Some(HistoryPicker::showing(commands));
        }
    }

    /// Se llama antes de dibujar la terminal, para que las teclas del panel
    /// no le lleguen a la consola.
    pub(super) fn handle_history_picker_keys(&mut self, egui_context: &egui::Context) {
        let Some(picker) = &mut self.history_picker else { return };

        let outcome = egui_context.input_mut(|input| {
            if input.consume_key(Modifiers::NONE, Key::Escape) {
                return PickerOutcome::Cancelled;
            }
            if input.consume_key(Modifiers::NONE, Key::Enter) {
                return PickerOutcome::Chosen(picker.highlighted_command());
            }
            if input.consume_key(Modifiers::NONE, Key::Delete) {
                return PickerOutcome::Forget(picker.highlighted_command());
            }
            // Empezar a escribir cierra el panel y lo escrito llega a la consola.
            if input.events.iter().any(|event| matches!(event, Event::Text(_) | Event::Paste(_))) {
                return PickerOutcome::Cancelled;
            }
            for (key, rows) in [(Key::ArrowUp, -1), (Key::ArrowDown, 1), (Key::PageUp, -PAGE_SIZE), (Key::PageDown, PAGE_SIZE)] {
                if input.consume_key(Modifiers::NONE, key) {
                    picker.move_highlight_by(rows);
                }
            }
            PickerOutcome::StillOpen
        });
        self.apply_picker_outcome(outcome);
    }

    pub(super) fn show_history_picker(&mut self, egui_context: &egui::Context) {
        let (Some(picker), Some(terminal_card)) = (&mut self.history_picker, self.terminal_card) else { return };

        let width = (terminal_card.width() - 32.0).min(MAX_PANEL_WIDTH);
        let list_height =
            (picker.commands.len() as f32 * (ROW_HEIGHT + 2.0)).min(terminal_card.height() * MAX_HEIGHT_FRACTION);

        // Se ancla por su borde inferior al de la terminal (como una hoja que sube
        // desde abajo), así queda pegado mida lo que mida. Al abrirse, empieza
        // escondido justo debajo y sube su propia altura.
        let progress = picker.slide_progress();
        let still_hidden_distance = picker.measured_height * (1.0 - progress);
        let bottom_center = pos2(terminal_card.center().x, terminal_card.bottom() + still_hidden_distance);
        if progress < 1.0 {
            egui_context.request_repaint();
        }

        let mut outcome = PickerOutcome::StillOpen;
        let panel = Area::new(Id::new("history_picker"))
            .order(Order::Foreground)
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(bottom_center)
            .constrain(false)
            .show(egui_context, |ui| {
                // Mientras sube no debe asomar por debajo de la terminal.
                ui.set_clip_rect(terminal_card);
                Frame::none()
                    .fill(palette::SURFACE_CONTAINER_HIGH)
                    .rounding(Rounding { nw: 16.0, ne: 16.0, sw: 0.0, se: 0.0 })
                    .shadow(ui.visuals().popup_shadow)
                    .inner_margin(Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.set_width(width - 16.0);
                        ScrollArea::vertical().max_height(list_height).auto_shrink([false, true]).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for (index, command) in picker.commands.iter().enumerate() {
                                let is_highlighted = index == picker.highlighted;
                                let row = history_row(ui, command, is_highlighted);
                                if is_highlighted && picker.keep_highlighted_visible {
                                    row.row.scroll_to_me(Some(Align::Center));
                                }
                                if row.forget_clicked {
                                    outcome = PickerOutcome::Forget(command.clone());
                                } else if row.save_clicked {
                                    outcome = PickerOutcome::SaveAsCommand(command.clone());
                                } else if row.row.clicked() {
                                    outcome = PickerOutcome::Chosen(command.clone());
                                }
                            }
                        });
                        picker.keep_highlighted_visible = false;
                    });
            });

        if let Some(picker) = &mut self.history_picker {
            picker.measured_height = panel.response.rect.height();
        }
        if clicked_outside(egui_context, panel.response.rect) {
            outcome = PickerOutcome::Cancelled;
        }
        self.apply_picker_outcome(outcome);
    }

    pub(super) fn close_history_picker(&mut self) {
        self.history_picker = None;
    }

    fn apply_picker_outcome(&mut self, outcome: PickerOutcome) {
        match outcome {
            PickerOutcome::StillOpen => {}
            PickerOutcome::Cancelled => {
                self.close_history_picker();
                self.terminal_should_take_focus = true;
            }
            PickerOutcome::Chosen(command) => {
                self.close_history_picker();
                if let Some(session) = self.terminal_tabs.active_session_mut() {
                    session.replace_prompt_line_with(&command);
                }
                self.terminal_should_take_focus = true;
            }
            PickerOutcome::Forget(command) => self.forget_from_history(&command),
            PickerOutcome::SaveAsCommand(command) => {
                self.close_history_picker();
                self.library_dialog = Some(LibraryDialog::create_command_at(TreeLocation::Root, command));
            }
        }
    }

    /// Lo quita del historial de la pestaña y de la lista; si no queda nada, cierra el panel.
    fn forget_from_history(&mut self, command: &str) {
        if let Err(error) = self.terminal_tabs.forget_command_in_active_tab(command) {
            self.show_status(error);
        }
        let Some(picker) = &mut self.history_picker else { return };
        picker.remove(command);
        if picker.commands.is_empty() {
            self.close_history_picker();
            self.terminal_should_take_focus = true;
        }
        self.show_status(tr("history.forgotten"));
    }
}

fn clicked_outside(egui_context: &egui::Context, panel_rect: Rect) -> bool {
    egui_context.input(|input| {
        input.pointer.any_pressed()
            && input.pointer.interact_pos().is_some_and(|pointer| !panel_rect.contains(pointer))
    })
}
