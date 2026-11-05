//! Actualizaciones, sin molestar: se busca en segundo plano al abrir la app (y
//! una vez al día si sigue abierta). Si hay una versión nueva para este
//! programa, solo aparece un botón pequeño en la barra de pestañas; al pulsarlo
//! se ven las novedades y se elige actualizar, más tarde o saltar esa versión.
//! Si falla la búsqueda automática (sin internet, por ejemplo), no se avisa.
//!
//! Cuando no hay nada que actualizar, en ese mismo sitio hay una taza de café
//! pequeña: abre la misma ventana, que entonces dice qué versión es, si es la
//! última, y enlaza al repositorio. Debajo, siempre, la sección de apoyo al
//! creador ("invítame a un café"), completamente voluntaria.

use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Align2, Button, Key, Label, Layout, RichText, Rounding, ScrollArea, Ui};
use egui_material_icons::icons::{
    ICON_CHECK_CIRCLE, ICON_CODE, ICON_FAVORITE, ICON_LOCAL_CAFE, ICON_OPEN_IN_NEW, ICON_RESTART_ALT, ICON_UPGRADE,
};

use super::dialog_backdrop::{backdrop_was_clicked, keep_dialog_on_top};
use super::material_widgets::{filled_button, subtle_icon_button, text_button};
use super::theme::palette;
use super::ConsoleApp;
use crate::application::update_service::UpdateService;
use crate::domain::app_settings::UpdateSettings;
use crate::domain::updates::{UpdateOffer, Version};
use crate::i18n::{tr, tr_args};
use crate::project;

/// Con la app abierta mucho tiempo, se vuelve a mirar una vez al día.
const TIME_BETWEEN_AUTOMATIC_CHECKS: Duration = Duration::from_secs(24 * 60 * 60);
const DIALOG_WIDTH: f32 = 460.0;
const NOTES_MAX_HEIGHT: f32 = 220.0;
/// La versión de este programa, para mostrarla aunque no se pueda actualizar.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Lo que se sabe tras la última búsqueda.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LastCheck {
    NotYet,
    UpToDate,
    Failed,
}

pub(super) struct Updates {
    /// `None` si no se puede actualizar (no se sabe dónde está el ejecutable).
    service: Option<UpdateService>,
    phase: UpdatePhase,
    last_automatic_check: Option<Instant>,
    /// Buscada desde los ajustes: entonces sí se dice si no hay nada nuevo o si falla.
    check_was_requested: bool,
    dialog_is_open: bool,
    last_check: LastCheck,
    /// Instalada la versión nueva: al salir, se arranca.
    pub(super) restart_after_exit: bool,
}

enum UpdatePhase {
    Idle,
    Checking(Receiver<Result<Option<UpdateOffer>, String>>),
    Available(UpdateOffer),
    Installing(UpdateOffer, Receiver<Result<(), String>>),
    /// El ejecutable ya es el nuevo: falta reiniciar.
    ReadyToRestart(Version),
}

impl Updates {
    pub(super) fn new(service: Option<UpdateService>) -> Self {
        Self {
            service,
            phase: UpdatePhase::Idle,
            last_automatic_check: None,
            check_was_requested: false,
            dialog_is_open: false,
            last_check: LastCheck::NotYet,
            restart_after_exit: false,
        }
    }

    pub(super) fn current_version(&self) -> Option<Version> {
        self.service.as_ref().map(UpdateService::current_version)
    }

    pub(super) fn is_checking(&self) -> bool {
        matches!(self.phase, UpdatePhase::Checking(_))
    }

    fn start_checking(&mut self, skipped: Option<Version>, egui_context: &egui::Context) {
        let Some(service) = self.service.clone() else { return };
        let (send_result, result) = mpsc::channel();
        let context_to_wake_up = egui_context.clone();
        thread::spawn(move || {
            let _ = send_result.send(service.find_update(skipped));
            context_to_wake_up.request_repaint();
        });
        self.phase = UpdatePhase::Checking(result);
    }

    fn start_installing(&mut self, offer: UpdateOffer, egui_context: &egui::Context) {
        let Some(service) = self.service.clone() else { return };
        let (send_result, result) = mpsc::channel();
        let context_to_wake_up = egui_context.clone();
        let offer_to_install = offer.clone();
        thread::spawn(move || {
            let _ = send_result.send(service.download_and_install(&offer_to_install));
            context_to_wake_up.request_repaint();
        });
        self.phase = UpdatePhase::Installing(offer, result);
    }
}

/// Lo que se ha pulsado en el diálogo o en el botón de la barra.
enum UpdateChoice {
    OpenDialog,
    /// Desde la taza: la ventana, y buscar si aún no se sabe si es la última.
    OpenAbout,
    CheckAgain,
    Install(UpdateOffer),
    Later,
    Skip(Version),
    OpenWebPage(String),
    RestartNow,
}

impl ConsoleApp {
    /// En cada fotograma: recoge lo que hayan terminado los hilos y, si toca,
    /// empieza una búsqueda automática.
    pub(super) fn keep_updates_going(&mut self, egui_context: &egui::Context) {
        self.collect_update_results();

        let settings = self.app_settings.settings().updates;
        let updates = &mut self.updates;
        if !settings.check_automatically || !matches!(updates.phase, UpdatePhase::Idle) || updates.service.is_none() {
            return;
        }
        let waited = updates.last_automatic_check.map(|checked_at| checked_at.elapsed());
        match waited {
            Some(waited) if waited < TIME_BETWEEN_AUTOMATIC_CHECKS => {
                egui_context.request_repaint_after(TIME_BETWEEN_AUTOMATIC_CHECKS - waited);
            }
            _ => {
                updates.last_automatic_check = Some(Instant::now());
                updates.check_was_requested = false;
                updates.start_checking(settings.skipped_version, egui_context);
            }
        }
    }

    /// Desde los ajustes: buscar ahora (también la versión que se saltó).
    pub(super) fn check_for_updates_now(&mut self) {
        if self.updates.service.is_none() {
            return;
        }
        if matches!(self.updates.phase, UpdatePhase::Available(_)) {
            self.updates.dialog_is_open = true;
            return;
        }
        if matches!(self.updates.phase, UpdatePhase::Idle) {
            self.updates.check_was_requested = true;
            let egui_context = self.egui_context.clone();
            self.updates.start_checking(None, &egui_context);
        }
    }

    fn collect_update_results(&mut self) {
        let updates = &mut self.updates;
        let mut message = None;
        let mut restart = false;
        match &updates.phase {
            UpdatePhase::Checking(result) => match result.try_recv() {
                Ok(Ok(Some(offer))) => {
                    // Si la ventana ya estaba abierta (desde la taza), pasa a ofrecerla.
                    updates.dialog_is_open |= updates.check_was_requested;
                    updates.last_check = LastCheck::UpToDate;
                    updates.phase = UpdatePhase::Available(offer);
                }
                Ok(Ok(None)) => {
                    updates.last_check = LastCheck::UpToDate;
                    if updates.check_was_requested {
                        let version = updates.current_version().map(|version| version.to_string()).unwrap_or_default();
                        message = Some(tr_args("updates.up_to_date", &[("version", &version)]));
                    }
                    updates.phase = UpdatePhase::Idle;
                }
                Ok(Err(error)) => {
                    updates.last_check = LastCheck::Failed;
                    if updates.check_was_requested {
                        message = Some(error);
                    }
                    updates.phase = UpdatePhase::Idle;
                }
                Err(_) => {}
            },
            UpdatePhase::Installing(offer, result) => match result.try_recv() {
                Ok(Ok(())) => {
                    updates.dialog_is_open = false;
                    updates.phase = UpdatePhase::ReadyToRestart(offer.version);
                    restart = true;
                }
                Ok(Err(error)) => {
                    message = Some(error);
                    updates.phase = UpdatePhase::Available(offer.clone());
                }
                Err(_) => {}
            },
            _ => {}
        }
        if let Some(message) = message {
            self.show_status(message);
        }
        if restart {
            self.restart_into_new_version();
        }
    }

    /// Sale como siempre (preguntando por las pestañas sin guardar) y, al
    /// salir, arranca la versión nueva.
    fn restart_into_new_version(&mut self) {
        self.updates.restart_after_exit = true;
        self.exit_app();
    }

    /// Lo que hay que hacer justo antes de cerrar la ventana.
    pub(super) fn start_new_version_if_installed(&mut self) {
        if !self.updates.restart_after_exit {
            return;
        }
        if let Some(Err(error)) = self.updates.service.as_ref().map(UpdateService::start_new_program) {
            self.show_status(error);
        }
    }

    /// El botón discreto de la barra de pestañas, si hay algo que mostrar.
    pub(super) fn show_update_button(&mut self, ui: &mut Ui) {
        let choice = match &self.updates.phase {
            UpdatePhase::Available(offer) => {
                let label = format!("{ICON_UPGRADE} {}", offer.version);
                pill_button(ui, &label, &tr_args("updates.available_tooltip", &[("version", &offer.version)]))
                    .then_some(UpdateChoice::OpenDialog)
            }
            UpdatePhase::Installing(..) => {
                ui.spinner();
                None
            }
            UpdatePhase::ReadyToRestart(version) => {
                let label = format!("{ICON_RESTART_ALT} {}", tr("updates.restart_short"));
                let tooltip = tr_args("updates.restart_tooltip", &[("version", version)]);
                pill_button(ui, &label, &tooltip).then_some(UpdateChoice::RestartNow)
            }
            // Nada que actualizar: la taza, discreta, en el mismo sitio.
            UpdatePhase::Idle | UpdatePhase::Checking(_) => {
                subtle_icon_button(ui, ICON_LOCAL_CAFE, &tr("about.tooltip")).clicked().then_some(UpdateChoice::OpenAbout)
            }
        };
        if let Some(choice) = choice {
            self.apply_update_choice(choice);
        }
    }

    /// Busca sin avisos aparte: el resultado se ve en la propia ventana.
    fn check_quietly_for_the_about_window(&mut self) {
        if self.updates.service.is_none() || !matches!(self.updates.phase, UpdatePhase::Idle) {
            return;
        }
        self.updates.check_was_requested = false;
        let egui_context = self.egui_context.clone();
        self.updates.start_checking(None, &egui_context);
    }

    /// Arriba, la actualización si la hay (o qué versión es esta); debajo, el apoyo.
    pub(super) fn show_update_dialog(&mut self, egui_context: &egui::Context) {
        if !self.updates.dialog_is_open {
            return;
        }
        let update = match &self.updates.phase {
            UpdatePhase::Available(offer) => Some((offer.clone(), false)),
            UpdatePhase::Installing(offer, _) => Some((offer.clone(), true)),
            _ => None,
        };
        let about = AboutStatus {
            is_checking: self.updates.is_checking(),
            can_check: self.updates.service.is_some(),
            last_check: self.updates.last_check,
            installed_version: match &self.updates.phase {
                UpdatePhase::ReadyToRestart(version) => Some(*version),
                _ => None,
            },
        };

        let mut choice = None;
        let clicked_outside = backdrop_was_clicked(egui_context);
        let window = egui::Window::new("about_and_updates")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(egui_context, |ui| {
                ui.set_width(DIALOG_WIDTH);
                match &update {
                    Some((offer, is_installing)) => show_update_section(ui, offer, *is_installing, &mut choice),
                    None => show_about_section(ui, &about, &mut choice),
                }
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(12.0);
                show_support_section(ui, &mut choice);
                if update.is_none() {
                    ui.add_space(16.0);
                    let button_row = egui::vec2(ui.available_width(), 40.0);
                    ui.allocate_ui_with_layout(button_row, Layout::right_to_left(Align::Center), |ui| {
                        if text_button(ui, &tr("about.close")).clicked() {
                            choice = Some(UpdateChoice::Later);
                        }
                    });
                }
            });
        keep_dialog_on_top(egui_context, &window);
        // Fuera o Esc: cerrar (mientras descarga, sigue en segundo plano).
        if choice.is_none() && (clicked_outside || egui_context.input(|input| input.key_pressed(Key::Escape))) {
            choice = Some(UpdateChoice::Later);
        }
        if let Some(choice) = choice {
            self.apply_update_choice(choice);
        }
    }
    fn apply_update_choice(&mut self, choice: UpdateChoice) {
        match choice {
            UpdateChoice::OpenDialog => self.updates.dialog_is_open = true,
            UpdateChoice::OpenAbout => {
                self.updates.dialog_is_open = true;
                if self.updates.last_check != LastCheck::UpToDate {
                    self.check_quietly_for_the_about_window();
                }
            }
            UpdateChoice::CheckAgain => self.check_quietly_for_the_about_window(),
            UpdateChoice::Later => self.updates.dialog_is_open = false,
            UpdateChoice::Install(offer) => {
                let egui_context = self.egui_context.clone();
                self.updates.start_installing(offer, &egui_context);
            }
            UpdateChoice::Skip(version) => {
                self.updates.dialog_is_open = false;
                self.updates.phase = UpdatePhase::Idle;
                let settings = UpdateSettings { skipped_version: Some(version), ..self.app_settings.settings().updates };
                if let Err(error) = self.app_settings.set_update_settings(settings) {
                    self.show_status(error);
                }
            }
            UpdateChoice::OpenWebPage(url) => {
                if let Err(error) = self.web_browser.open(&url) {
                    self.show_status(error);
                }
            }
            UpdateChoice::RestartNow => self.restart_into_new_version(),
        }
    }

    /// Mientras el diálogo de actualizar está abierto, el teclado no va a la consola.
    pub(super) fn update_dialog_is_open(&self) -> bool {
        self.updates.dialog_is_open
    }
}

/// Un botón pequeño y redondeado, que se ve sin llamar demasiado la atención.
fn pill_button(ui: &mut Ui, label: &str, tooltip: &str) -> bool {
    let button = Button::new(RichText::new(label).small().color(palette::ON_SECONDARY_CONTAINER))
        .fill(palette::SECONDARY_CONTAINER)
        .rounding(Rounding::same(14.0));
    ui.add(button).on_hover_text(tooltip).clicked()
}

/// Lo que se muestra de esta versión cuando no hay nada que actualizar.
struct AboutStatus {
    is_checking: bool,
    /// `false` si este programa no se puede actualizar solo.
    can_check: bool,
    last_check: LastCheck,
    /// Ya se instaló una versión nueva y falta reiniciar.
    installed_version: Option<Version>,
}

fn show_about_section(ui: &mut Ui, about: &AboutStatus, choice: &mut Option<UpdateChoice>) {
    ui.label(RichText::new(tr_args("about.title", &[("version", &APP_VERSION)])).size(22.0));
    ui.add_space(8.0);
    if let Some(version) = about.installed_version {
        ui.label(RichText::new(tr_args("updates.restart_tooltip", &[("version", &version)])).color(palette::ON_SURFACE_VARIANT));
        ui.add_space(8.0);
        if filled_button(ui, &format!("{ICON_RESTART_ALT} {}", tr("updates.restart_short"))).clicked() {
            *choice = Some(UpdateChoice::RestartNow);
        }
    } else if about.is_checking {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new(tr("about.checking")).color(palette::ON_SURFACE_VARIANT));
        });
    } else if about.last_check == LastCheck::UpToDate {
        ui.label(RichText::new(format!("{ICON_CHECK_CIRCLE} {}", tr("about.up_to_date"))).color(palette::SUCCESS));
    } else if about.can_check {
        ui.horizontal(|ui| {
            if about.last_check == LastCheck::Failed {
                ui.label(RichText::new(tr("about.check_failed")).color(palette::ON_SURFACE_VARIANT));
            }
            if ui.link(tr("about.check_again")).clicked() {
                *choice = Some(UpdateChoice::CheckAgain);
            }
        });
    }
    ui.add_space(8.0);
    if ui.link(format!("{ICON_CODE} {}", tr("about.source_code"))).clicked() {
        *choice = Some(UpdateChoice::OpenWebPage(project::REPOSITORY_PAGE.to_string()));
    }
}

fn show_update_section(ui: &mut Ui, offer: &UpdateOffer, is_installing: bool, choice: &mut Option<UpdateChoice>) {
    ui.label(RichText::new(tr_args("updates.dialog.title", &[("version", &offer.version)])).size(22.0));
    ui.add_space(6.0);
    ui.label(RichText::new(tr_args("updates.dialog.current_version", &[("version", &APP_VERSION)])).color(palette::ON_SURFACE_VARIANT));
    if !offer.notes.trim().is_empty() {
        ui.add_space(12.0);
        ScrollArea::vertical().max_height(NOTES_MAX_HEIGHT).show(ui, |ui| {
            ui.add(Label::new(RichText::new(offer.notes.trim()).color(palette::ON_SURFACE)).wrap());
        });
    }
    ui.add_space(8.0);
    if ui.link(format!("{ICON_OPEN_IN_NEW} {}", tr("updates.dialog.see_on_github"))).clicked() {
        *choice = Some(UpdateChoice::OpenWebPage(offer.web_page.clone()));
    }
    ui.add_space(16.0);
    let button_row = egui::vec2(ui.available_width(), 40.0);
    ui.allocate_ui_with_layout(button_row, Layout::right_to_left(Align::Center), |ui| {
        if is_installing {
            ui.spinner();
            ui.label(RichText::new(tr("updates.dialog.installing")).color(palette::ON_SURFACE_VARIANT));
            return;
        }
        if filled_button(ui, &tr("updates.dialog.install")).clicked() {
            *choice = Some(UpdateChoice::Install(offer.clone()));
        }
        if text_button(ui, &tr("updates.dialog.later")).clicked() {
            *choice = Some(UpdateChoice::Later);
        }
        if text_button(ui, &tr("updates.dialog.skip")).clicked() {
            *choice = Some(UpdateChoice::Skip(offer.version));
        }
    });
}

/// "Invítame a un café": completamente voluntario, sin insistir.
fn show_support_section(ui: &mut Ui, choice: &mut Option<UpdateChoice>) {
    ui.label(RichText::new(format!("{ICON_LOCAL_CAFE} {}", tr("support.title"))).size(16.0).color(palette::ON_SURFACE));
    ui.add_space(6.0);
    ui.label(RichText::new(tr("support.explanation")).color(palette::ON_SURFACE_VARIANT));
    ui.add_space(10.0);
    let label = format!("{ICON_FAVORITE} {}", tr("support.button"));
    let button = Button::new(RichText::new(label).color(palette::ON_SECONDARY_CONTAINER))
        .fill(palette::SECONDARY_CONTAINER)
        .rounding(Rounding::same(20.0))
        .min_size(egui::vec2(0.0, 36.0));
    if ui.add(button).clicked() {
        *choice = Some(UpdateChoice::OpenWebPage(project::SUPPORT_PAGE.to_string()));
    }
}
