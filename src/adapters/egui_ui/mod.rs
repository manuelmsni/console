//! La ventana de la aplicación. Cada parte visible tiene su propio módulo:
//!
//! ```text
//! ┌───────────────────────────────────────────────────────┐
//! │ tab_bar: ☰ │ cmd 1 × │ PowerShell 2 × │ ➕  estado ⏻ │
//! ├────────────┬──────────────────────────────────────────┤
//! │ sidebar    │ terminal_view                            │
//! │ (comandos  │ (se escribe directamente aquí)           │
//! │ guardados) │                                          │
//! └────────────┴──────────────────────────────────────────┘
//! ```
//!
//! Cerrar la ventana la esconde en la bandeja (`window_and_tray`). Al salir de
//! verdad (⏻) se pregunta qué hacer con las pestañas sin guardar
//! (`confirmation_dialog`). Los atajos de teclado están en `shortcuts`.

mod app_icon;
mod colors;
mod confirmation_dialog;
mod dialog_backdrop;
mod fonts;
mod general_tab;
mod history_picker;
mod key_mapping;
mod library_dialog;
mod material_widgets;
mod pointer_cursor;
mod rename_in_place;
#[cfg(feature = "portable")]
mod unlock_screen;
#[cfg(feature = "portable")]
pub use unlock_screen::PortableStartup;
mod settings_window;
mod shortcuts;
mod shortcuts_tab;
mod sidebar;
mod ssh_keys_tab;
mod tab_bar;
mod terminal_view;
mod theme;
mod tree_drag_and_drop;
mod updates;
mod window_and_tray;
mod window_frame;

use std::time::{Duration, Instant};

use eframe::egui::{self, IconData, Rect, ViewportCommand};

use crate::application::app_settings_service::AppSettingsService;
use crate::application::command_library_service::CommandLibraryService;
use crate::application::ports::{Clipboard, WebBrowser};
use crate::application::ssh_settings::SshSettings;
use crate::application::terminal_tabs::TerminalTabs;
use crate::application::update_service::UpdateService;
use crate::domain::shell_profile::ShellProfile;
use crate::i18n::{tr, tr_args};
use confirmation_dialog::Confirmation;
use history_picker::HistoryPicker;
use library_dialog::LibraryDialog;
use rename_in_place::{PendingCommandClick, RenameInPlace};
use settings_window::SettingsWindow;
pub use window_and_tray::DesktopIntegration;
use updates::Updates;

const WINDOW_ICON_SIZE: u32 = 64;

/// Icono de la ventana en la barra de tareas.
/// Tipos de letra y colores de la aplicación. Se ponen al crear la ventana,
/// antes de dibujar nada (también la pantalla de contraseña de la portable).
pub fn prepare_look(egui_context: &egui::Context) {
    fonts::install_fonts(egui_context);
    theme::apply_material_theme(egui_context);
}

pub fn window_icon() -> IconData {
    IconData { rgba: app_icon::app_icon_rgba(WINDOW_ICON_SIZE), width: WINDOW_ICON_SIZE, height: WINDOW_ICON_SIZE }
}

pub struct ConsoleApp {
    egui_context: egui::Context,
    terminal_tabs: TerminalTabs,
    command_library: CommandLibraryService,
    app_settings: AppSettingsService,
    /// Para volver a elegir idioma si en los ajustes se pone "automático".
    system_language: String,
    ssh_settings: SshSettings,
    clipboard: Box<dyn Clipboard>,
    /// Para abrir con Ctrl + clic los enlaces que salen en la terminal.
    web_browser: Box<dyn WebBrowser>,
    shell_profiles: Vec<ShellProfile>,
    /// Ventana nativa, icono de la bandeja y avisos de "muéstrate".
    desktop: DesktopIntegration,
    sidebar_is_visible: bool,
    /// Lo escrito en el filtro del menú lateral.
    command_filter: String,
    /// Lo que se está renombrando en su sitio (un comando, una carpeta o una pestaña).
    renaming: Option<RenameInPlace>,
    pending_command_click: Option<PendingCommandClick>,
    library_dialog: Option<LibraryDialog>,
    settings_window: Option<SettingsWindow>,
    /// Pregunta pendiente antes de perder algo (cerrar pestaña guardada, salir sin guardar).
    confirmation: Option<Confirmation>,
    history_picker: Option<HistoryPicker>,
    /// Pestaña que ya se desplazó a la vista en la barra; si la activa es otra, hay que desplazarla.
    tab_brought_into_view: Option<usize>,
    /// Dónde se dibujó la terminal, para colocar encima el panel de historial.
    terminal_card: Option<Rect>,
    exit_is_confirmed: bool,
    /// Aviso pequeño junto a los botones de la ventana; desaparece solo.
    status_message: Option<StatusMessage>,
    terminal_should_take_focus: bool,
    /// Lo que la rueda del ratón ha girado sin llegar a una línea entera.
    unapplied_scroll_lines: f32,
    updates: Updates,
}

impl ConsoleApp {
    pub fn new(
        egui_context: egui::Context,
        mut desktop: DesktopIntegration,
        terminal_tabs: TerminalTabs,
        command_library: CommandLibraryService,
        app_settings: AppSettingsService,
        ssh_settings: SshSettings,
        clipboard: Box<dyn Clipboard>,
        web_browser: Box<dyn WebBrowser>,
        shell_profiles: Vec<ShellProfile>,
        update_service: Option<UpdateService>,
        system_language: String,
        startup_warning: Option<String>,
    ) -> Self {
        let tray_problem = desktop.take_problem();
        let mut app = Self {
            egui_context,
            terminal_tabs,
            command_library,
            app_settings,
            system_language,
            ssh_settings,
            clipboard,
            web_browser,
            shell_profiles,
            desktop,
            sidebar_is_visible: true,
            command_filter: String::new(),
            renaming: None,
            pending_command_click: None,
            library_dialog: None,
            settings_window: None,
            confirmation: None,
            history_picker: None,
            tab_brought_into_view: None,
            terminal_card: None,
            exit_is_confirmed: false,
            status_message: startup_warning.or(tray_problem).map(|warning| StatusMessage::new(warning, STARTUP_WARNING_DURATION)),
            terminal_should_take_focus: false,
            unapplied_scroll_lines: 0.0,
            updates: Updates::new(update_service),
        };
        app.reopen_previous_tabs_or_open_a_new_one();
        app
    }

    fn reopen_previous_tabs_or_open_a_new_one(&mut self) {
        match self.terminal_tabs.reopen_tabs_from_journals(&self.shell_profiles) {
            Ok(0) => {}
            Ok(1) => {
                self.show_status(tr("tabs.reopened_one"));
                self.terminal_should_take_focus = true;
            }
            Ok(reopened) => {
                self.show_status(tr_args("tabs.reopened_many", &[("count", &reopened)]));
                self.terminal_should_take_focus = true;
            }
            Err(error) => self.show_status(error),
        }
        if self.terminal_tabs.is_empty() {
            if let Some(default_profile) = self.shell_profiles.first().cloned() {
                self.open_tab_with(&default_profile);
            }
        }
    }

    fn open_tab_with(&mut self, profile: &ShellProfile) {
        match self.terminal_tabs.open_tab(profile) {
            Ok(()) => self.terminal_should_take_focus = true,
            Err(error) => self.show_status(error),
        }
    }

    fn run_in_active_terminal(&mut self, command_line: &str) {
        match self.terminal_tabs.active_session_mut() {
            Some(session) => session.run_command_line(command_line),
            None => self.show_status(tr("tabs.none_open")),
        }
        self.terminal_should_take_focus = true;
    }

    /// Abre una pestaña de la terminal principal y ejecuta en ella el comando
    /// (por ejemplo, un `ssh`).
    fn open_tab_running(&mut self, command_line: &str) {
        let Some(profile) = self.shell_profiles.first().cloned() else { return };
        match self.terminal_tabs.open_tab(&profile) {
            Ok(()) => self.run_in_active_terminal(command_line),
            Err(error) => self.show_status(error),
        }
    }

    /// Con una ventana encima (ajustes, diálogos), el teclado no debe llegar a la consola.
    fn a_dialog_is_open(&self) -> bool {
        self.library_dialog.is_some()
            || self.settings_window.is_some()
            || self.confirmation.is_some()
            || self.update_dialog_is_open()
    }

    /// Escribe el texto en la terminal sin pulsar Enter, para poder retocarlo.
    fn type_into_active_terminal(&mut self, text: &str) {
        match self.terminal_tabs.active_session_mut() {
            Some(session) => session.paste_text(text),
            None => self.show_status(tr("tabs.none_open")),
        }
        self.terminal_should_take_focus = true;
    }

    fn copy_to_clipboard(&mut self, text: &str) {
        let message = match self.clipboard.copy_text(text) {
            Ok(()) => tr("clipboard.copied"),
            Err(error) => error,
        };
        self.show_status(message);
    }

    fn show_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(StatusMessage::new(message.into(), STATUS_MESSAGE_DURATION));
    }

    /// Cerrar la ventana desde Windows (Alt+F4, barra de tareas) la esconde en
    /// la bandeja en lugar de cerrarla.
    fn hide_to_tray_instead_of_closing(&mut self, egui_context: &egui::Context) {
        let close_was_requested = egui_context.input(|input| input.viewport().close_requested());
        if !close_was_requested || self.exit_is_confirmed {
            return;
        }
        egui_context.send_viewport_cmd(ViewportCommand::CancelClose);
        self.hide_to_tray();
    }

    /// Las consolas siguen funcionando; se vuelve desde el icono de la bandeja.
    /// Si no hay icono, no habría forma de volver, así que se sale.
    fn hide_to_tray(&mut self) {
        if !self.desktop.hide_to_tray() {
            self.exit_app();
        }
    }
}

impl eframe::App for ConsoleApp {
    fn update(&mut self, egui_context: &egui::Context, _frame: &mut eframe::Frame) {
        self.terminal_tabs.forward_all_shell_output();

        if let Some(problem) = self.terminal_tabs.take_new_journal_problem() {
            self.show_status(problem);
        }

        self.hide_to_tray_instead_of_closing(egui_context);
        if self.desktop.exit_was_requested_from_tray() {
            self.exit_app();
        }

        // Minimizada, la ventana mide 0×0: dibujar ahí encogería el menú lateral
        // y las consolas, y así se quedarían al restaurarla.
        if window_frame::is_minimized(egui_context) {
            return;
        }

        self.keep_updates_going(egui_context);
        self.type_pending_command_when_due(egui_context);
        self.handle_history_picker_keys(egui_context);
        self.handle_window_shortcuts(egui_context);

        // Los paneles laterales se dibujan antes que el central para que este
        // ocupe solo el hueco que queda.
        self.show_tab_bar(egui_context);
        self.show_saved_commands_sidebar(egui_context);
        self.show_active_terminal(egui_context);
        self.show_history_picker(egui_context);
        self.show_library_dialog(egui_context);
        self.show_settings_window(egui_context);
        self.show_update_dialog(egui_context);
        self.show_confirmation(egui_context);
        // Al final, cuando ya se sabe qué hay bajo el ratón. Los bordes, lo
        // último: el cursor de estirar gana al de lo que haya debajo.
        pointer_cursor::show_hand_over_clickable_things(egui_context);
        window_frame::allow_resizing_from_edges(egui_context);

        if self.terminal_tabs.has_scheduled_input() {
            egui_context.request_repaint_after(Duration::from_millis(16));
        }
        // La ventana solo se redibuja cuando pasa algo: hay que despertarla
        // cuando a una pestaña con autoguardado le toque guardarse.
        if let Some(wait) = self.terminal_tabs.time_until_next_autosave() {
            egui_context.request_repaint_after(wait);
        }
    }
}

/// Lo que se ve un aviso normal ("Copiado", "Pestaña guardada"...).
const STATUS_MESSAGE_DURATION: Duration = Duration::from_secs(4);
/// Los problemas al arrancar (un fichero dañado...) se dejan más tiempo, para
/// que dé tiempo a leerlos.
const STARTUP_WARNING_DURATION: Duration = Duration::from_secs(15);
/// Al final, el aviso se desvanece en lugar de desaparecer de golpe.
const STATUS_MESSAGE_FADE: Duration = Duration::from_millis(600);

struct StatusMessage {
    text: String,
    shown_at: Instant,
    lasts: Duration,
}

impl StatusMessage {
    fn new(text: String, lasts: Duration) -> Self {
        Self { text, shown_at: Instant::now(), lasts }
    }

    /// De 1 (se ve del todo) a 0 (ya ha desaparecido).
    fn visibility(&self) -> f32 {
        let remaining = self.lasts.saturating_sub(self.shown_at.elapsed());
        (remaining.as_secs_f32() / STATUS_MESSAGE_FADE.as_secs_f32()).min(1.0)
    }

    /// Cuándo hay que volver a dibujar: enseguida mientras se desvanece y, si
    /// no, justo cuando empiece a desvanecerse.
    fn next_change_in(&self) -> Duration {
        let remaining = self.lasts.saturating_sub(self.shown_at.elapsed());
        remaining.saturating_sub(STATUS_MESSAGE_FADE)
    }
}
