//! Versión portable: antes de la consola, una pantalla que pide la contraseña
//! con la que están cifrados los datos (o que la crea, la primera vez).

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

use eframe::egui::{self, vec2, Align, Frame, Id, Key, Layout, Margin, RichText, Rounding, TextEdit, Ui};
use zeroize::Zeroizing;

use super::material_widgets::{filled_button, window_control_button, WindowControl};
use super::{pointer_cursor, window_frame};
use super::theme::palette;
use super::{ConsoleApp, DesktopIntegration};
use crate::adapters::file_cipher::FileCipher;
use crate::adapters::password_cipher::{KeyDerivationCost, PasswordCipher, PasswordKeyFile, UnlockError};
use crate::domain::master_password::{check_new_password, NewPasswordProblem, MINIMUM_PASSWORD_LENGTH};
use crate::i18n::{tr, tr_args};

const CARD_WIDTH: f32 = 440.0;
const BUTTON_ROW_HEIGHT: f32 = 40.0;

/// Crea la consola con el cifrado ya listo.
pub type OpenConsole = Box<dyn FnOnce(Arc<dyn FileCipher>, DesktopIntegration) -> ConsoleApp>;

pub struct PortableStartup {
    screen: Screen,
}

/// Mientras se pide la contraseña, la ventana ya se puede esconder en la
/// bandeja; al desbloquear, la bandeja pasa a la consola.

enum Screen {
    AskingPassword(PasswordPrompt),
    Console(Box<ConsoleApp>),
}

impl PortableStartup {
    pub fn new(data_folder: &Path, desktop: DesktopIntegration, open_console: OpenConsole) -> Self {
        let key_file = PasswordKeyFile::in_folder(data_folder);
        let prompt = PasswordPrompt {
            is_first_time: !key_file.exists(),
            key_file,
            data_folder: data_folder.display().to_string(),
            password: Zeroizing::new(String::new()),
            repeated: Zeroizing::new(String::new()),
            problem: None,
            checking: None,
            needs_focus: true,
            open_console: Some(open_console),
            desktop: Some(desktop),
        };
        Self { screen: Screen::AskingPassword(prompt) }
    }
}

impl eframe::App for PortableStartup {
    fn update(&mut self, egui_context: &egui::Context, frame: &mut eframe::Frame) {
        match &mut self.screen {
            Screen::Console(console) => eframe::App::update(console.as_mut(), egui_context, frame),
            Screen::AskingPassword(prompt) => {
                // "Salir" desde el icono de la bandeja: aquí no hay nada que guardar.
                if prompt.desktop.as_ref().is_some_and(DesktopIntegration::exit_was_requested_from_tray) {
                    egui_context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                show_window_bar(egui_context, prompt.desktop.as_ref());
                let console = prompt.show(egui_context);
                pointer_cursor::show_hand_over_clickable_things(egui_context);
                window_frame::allow_resizing_from_edges(egui_context);
                if let Some(console) = console {
                    self.screen = Screen::Console(Box::new(console));
                    egui_context.request_repaint();
                }
            }
        }
    }
}

/// La ventana no tiene la barra de título de Windows: esta hace sus veces
/// (arrastrar para moverla, doble clic para maximizar) y lleva los botones de
/// minimizar, maximizar, esconder en la bandeja (X) y salir (⏻, sin crear ni
/// abrir nada), igual que en la consola.
fn show_window_bar(egui_context: &egui::Context, desktop: Option<&DesktopIntegration>) {
    let bar = Frame::none().fill(palette::SURFACE).inner_margin(Margin::symmetric(8.0, 6.0));
    egui::TopBottomPanel::top("unlock_window_bar").frame(bar).show_separator_line(false).show(egui_context, |ui| {
        let title_bar = window_frame::title_bar_background(ui);
        window_frame::move_or_maximize_from_title_bar(egui_context, &title_bar);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if window_control_button(ui, WindowControl::Exit, &tr("window.exit")).clicked() {
                egui_context.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if window_control_button(ui, WindowControl::HideToTray, &tr("window.hide_to_tray")).clicked() {
                // Sin icono en la bandeja no habría forma de volver: se cierra.
                if !desktop.is_some_and(DesktopIntegration::hide_to_tray) {
                    egui_context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
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
        });
    });
}

struct PasswordPrompt {
    /// No hay `key.json`: hay que crear la contraseña (y escribirla dos veces).
    is_first_time: bool,
    key_file: PasswordKeyFile,
    data_folder: String,
    password: Zeroizing<String>,
    repeated: Zeroizing<String>,
    problem: Option<String>,
    /// Sacar la clave de la contraseña tarda un momento: se hace aparte para
    /// que la ventana no se quede congelada.
    checking: Option<Receiver<Result<PasswordCipher, String>>>,
    needs_focus: bool,
    open_console: Option<OpenConsole>,
    /// Se entrega a la consola al desbloquear.
    desktop: Option<DesktopIntegration>,
}

impl PasswordPrompt {
    /// Devuelve la consola en cuanto la contraseña es buena.
    fn show(&mut self, egui_context: &egui::Context) -> Option<ConsoleApp> {
        if let Some(cipher) = self.take_check_result() {
            let open_console = self.open_console.take()?;
            let desktop = self.desktop.take()?;
            return Some(open_console(Arc::new(cipher), desktop));
        }

        let background = Frame::none().fill(palette::SURFACE);
        egui::CentralPanel::default().frame(background).show(egui_context, |ui| {
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.add_space((ui.available_height() / 2.0 - 190.0).max(24.0));
                let card = Frame::none()
                    .fill(palette::SURFACE_CONTAINER_HIGH)
                    .rounding(Rounding::same(28.0))
                    .inner_margin(Margin::same(28.0));
                card.show(ui, |ui| {
                    ui.set_width(CARD_WIDTH);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| self.show_card_contents(ui));
                });
            });
        });
        None
    }

    fn show_card_contents(&mut self, ui: &mut Ui) {
        let (title, explanation) = if self.is_first_time {
            (tr("unlock.create.title"), tr_args("unlock.create.explanation", &[("count", &MINIMUM_PASSWORD_LENGTH)]))
        } else {
            (tr("unlock.open.title"), tr("unlock.open.explanation"))
        };
        ui.label(RichText::new(title).size(22.0).color(palette::ON_SURFACE));
        ui.add_space(12.0);
        ui.label(RichText::new(explanation).color(palette::ON_SURFACE_VARIANT));
        ui.add_space(6.0);
        ui.label(RichText::new(tr_args("unlock.data_folder", &[("path", &self.data_folder)])).small().color(palette::OUTLINE));
        ui.add_space(20.0);

        let is_checking = self.checking.is_some();
        let mut pressed_enter = false;
        ui.add_enabled_ui(!is_checking, |ui| {
            let first_field = password_field(ui, "unlock_password", &mut self.password, &tr("unlock.password"));
            if std::mem::take(&mut self.needs_focus) {
                first_field.request_focus();
            }
            pressed_enter |= first_field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
            if self.is_first_time {
                ui.add_space(8.0);
                let second_field = password_field(ui, "unlock_repeated", &mut self.repeated, &tr("unlock.repeat_password"));
                pressed_enter |= second_field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
            }
        });

        if let Some(problem) = &self.problem {
            ui.add_space(10.0);
            ui.label(RichText::new(problem).color(palette::ERROR));
        }
        ui.add_space(20.0);
        // Limitada a la altura de un botón: sin límite ocuparía todo el alto.
        let button_row = vec2(ui.available_width(), BUTTON_ROW_HEIGHT);
        ui.allocate_ui_with_layout(button_row, Layout::right_to_left(Align::Center), |ui| {
            if is_checking {
                ui.spinner();
                ui.label(RichText::new(tr("unlock.checking")).color(palette::ON_SURFACE_VARIANT));
                return;
            }
            let button = if self.is_first_time { tr("unlock.create.button") } else { tr("unlock.open.button") };
            if filled_button(ui, &button).clicked() || pressed_enter {
                self.start_checking(ui.ctx());
            }
        });
    }

    fn start_checking(&mut self, egui_context: &egui::Context) {
        self.problem = None;
        if self.is_first_time {
            if let Err(problem) = check_new_password(&self.password, &self.repeated) {
                self.problem = Some(match problem {
                    NewPasswordProblem::TooShort => tr_args("unlock.error.too_short", &[("count", &MINIMUM_PASSWORD_LENGTH)]),
                    NewPasswordProblem::DoesNotMatch => tr("unlock.error.does_not_match"),
                });
                return;
            }
        }

        let (send_result, result) = mpsc::channel();
        let key_file = self.key_file.clone();
        let is_first_time = self.is_first_time;
        let password = self.password.clone();
        let context_to_wake_up = egui_context.clone();
        thread::spawn(move || {
            let outcome = if is_first_time {
                key_file.create(&password, KeyDerivationCost::RECOMMENDED)
            } else {
                key_file.unlock(&password).map_err(|error| match error {
                    UnlockError::WrongPassword => tr("unlock.error.wrong_password"),
                    UnlockError::KeyFileUnreadable(problem) => problem,
                })
            };
            let _ = send_result.send(outcome);
            context_to_wake_up.request_repaint();
        });
        self.checking = Some(result);
    }

    /// El cifrado si ya se ha comprobado la contraseña y era buena.
    fn take_check_result(&mut self) -> Option<PasswordCipher> {
        let outcome = self.checking.as_ref()?.try_recv().ok()?;
        self.checking = None;
        match outcome {
            Ok(cipher) => Some(cipher),
            Err(problem) => {
                self.problem = Some(problem);
                self.password.clear();
                self.repeated.clear();
                self.needs_focus = true;
                None
            }
        }
    }
}

fn password_field(ui: &mut Ui, id: &str, text: &mut String, hint: &str) -> egui::Response {
    ui.add(
        TextEdit::singleline(text)
            .id(Id::new(id))
            .password(true)
            .hint_text(hint)
            .desired_width(f32::INFINITY)
            .margin(Margin::symmetric(12.0, 10.0)),
    )
}