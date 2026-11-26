//! La ventana de la aplicación. Cada parte visible tiene su propio módulo:
//!
//! ```text
//! ┌───────────────────────────────────────────────────────┐
//! │ tab_bar: ☰ │ cmd 1 × │ PowerShell 2 × │ ➕  estado ⏻ │
//! │ macro_bar: 🏠 Inicio │ 📁 Trabajo ▾ │ ➕              │
//! ├────────────┬──────────────────────────────────────────┤
//! │ sidebar    │ terminal_view                            │
//! │ (comandos  │ (se escribe directamente aquí)           │
//! │ guardados) │                                          │
//! └────────────┴──────────────────────────────────────────┘
//! ```
//!
//! La barra de macros y el menú lateral se muestran y ocultan juntos (☰).
//! Las pestañas nuevas se abren con la terminal y en la carpeta de los ajustes,
//! o preguntando (`new_tab_dialog`); también desde "Abrir Console aquí".
//! Cerrar la ventana la esconde en la bandeja (`window_and_tray`). Al salir de
//! verdad (⏻) se pregunta qué hacer con las pestañas sin guardar
//! (`confirmation_dialog`). Los atajos de teclado están en `shortcuts`.

mod app_icon;
mod colors;
mod confirmation_dialog;
mod dialog_backdrop;
mod folder_picker;
mod fonts;
mod general_tab;
mod history_picker;
mod key_mapping;
mod library_dialog;
mod macro_bar;
mod macro_dialog;
mod macro_icons;
mod material_widgets;
mod new_tab_dialog;
mod places_tab;
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
mod terminal_scrollbar;
mod terminal_view;
mod theme;
mod tree_drag_and_drop;
mod updates;
mod window_and_tray;
mod window_frame;

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eframe::egui::{self, IconData, Rect, ViewportCommand};

use crate::application::app_settings_service::AppSettingsService;
use crate::application::command_library_service::CommandLibraryService;
use crate::application::macro_bar_service::MacroBarService;
use crate::application::saved_places_service::SavedPlacesService;
use crate::application::ports::{Clipboard, FolderContextMenu, WebBrowser};
use crate::application::ssh_settings::SshSettings;
use crate::application::terminal_tabs::TerminalTabs;
use crate::application::update_service::UpdateService;
use crate::domain::shell_profile::ShellProfile;
use crate::domain::saved_places::{ssh_command_line, PlaceId, PlaceTarget};
use crate::domain::startup::{NewTabPlan, PlannedFolder};
use crate::i18n::{tr, tr_args};
use confirmation_dialog::Confirmation;
use history_picker::HistoryPicker;
use library_dialog::LibraryDialog;
use macro_bar::MacroMenus;
use macro_dialog::MacroDialog;
use new_tab_dialog::NewTabDialog;
use rename_in_place::{PendingCommandClick, RenameInPlace};
use settings_window::SettingsWindow;
use terminal_scrollbar::TerminalScrollbar;
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
    /// La barra de macros: comandos que se ejecutan en el equipo, no en las pestañas.
    macro_bar: MacroBarService,
    /// Carpetas y servidores SSH de cada terminal, para el menú ➕.
    saved_places: SavedPlacesService,
    app_settings: AppSettingsService,
    /// Para volver a elegir idioma si en los ajustes se pone "automático".
    system_language: String,
    ssh_settings: SshSettings,
    clipboard: Box<dyn Clipboard>,
    /// Para abrir con Ctrl + clic los enlaces que salen en la terminal.
    web_browser: Box<dyn WebBrowser>,
    shell_profiles: Vec<ShellProfile>,
    /// "Abrir Console aquí" en el Explorador. `None` si no se puede poner.
    folder_context_menu: Option<Box<dyn FolderContextMenu>>,
    /// Si está puesto, para no preguntar al sistema en cada fotograma.
    folder_context_menu_is_enabled: bool,
    /// La carpeta elegida al abrir la app, si se preguntó: vale para las
    /// pestañas nuevas de esta vez.
    folder_chosen_at_start: Option<String>,
    /// Pregunta pendiente: terminal y/o carpeta de una pestaña nueva.
    new_tab_dialog: Option<NewTabDialog>,
    /// Carpetas de "Abrir Console aquí" que esperan a que se cierre la pregunta anterior.
    folders_waiting: VecDeque<String>,
    /// En el menú ➕, la terminal cuyas rutas guardadas se están mostrando.
    new_tab_submenu: Option<usize>,
    /// Ventana nativa, icono de la bandeja y avisos de "muéstrate".
    desktop: DesktopIntegration,
    /// El menú lateral y, con él, la barra de macros.
    sidebar_is_visible: bool,
    /// Lo escrito en el filtro del menú lateral.
    command_filter: String,
    /// Lo que se está renombrando en su sitio (un comando, una carpeta o una pestaña).
    renaming: Option<RenameInPlace>,
    pending_command_click: Option<PendingCommandClick>,
    library_dialog: Option<LibraryDialog>,
    macro_dialog: Option<MacroDialog>,
    /// Carpetas de la barra de macros desplegadas y su menú de clic derecho.
    macro_menus: MacroMenus,
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
    terminal_scrollbar: TerminalScrollbar,
    updates: Updates,
}

impl ConsoleApp {
    pub fn new(
        egui_context: egui::Context,
        mut desktop: DesktopIntegration,
        terminal_tabs: TerminalTabs,
        command_library: CommandLibraryService,
        macro_bar: MacroBarService,
        saved_places: SavedPlacesService,
        app_settings: AppSettingsService,
        ssh_settings: SshSettings,
        clipboard: Box<dyn Clipboard>,
        web_browser: Box<dyn WebBrowser>,
        folder_context_menu: Option<Box<dyn FolderContextMenu>>,
        shell_profiles: Vec<ShellProfile>,
        update_service: Option<UpdateService>,
        system_language: String,
        startup_warning: Option<String>,
    ) -> Self {
        let tray_problem = desktop.take_problem();
        let folder_context_menu_is_enabled = folder_context_menu.as_ref().is_some_and(|menu| menu.is_enabled());
        let mut app = Self {
            egui_context,
            terminal_tabs,
            command_library,
            macro_bar,
            saved_places,
            app_settings,
            system_language,
            ssh_settings,
            clipboard,
            web_browser,
            shell_profiles,
            folder_context_menu,
            folder_context_menu_is_enabled,
            folder_chosen_at_start: None,
            new_tab_dialog: None,
            folders_waiting: VecDeque::new(),
            new_tab_submenu: None,
            desktop,
            sidebar_is_visible: true,
            command_filter: String::new(),
            renaming: None,
            pending_command_click: None,
            library_dialog: None,
            macro_dialog: None,
            macro_menus: MacroMenus::default(),
            settings_window: None,
            confirmation: None,
            history_picker: None,
            tab_brought_into_view: None,
            terminal_card: None,
            exit_is_confirmed: false,
            status_message: startup_warning.or(tray_problem).map(|warning| StatusMessage::new(warning, STARTUP_WARNING_DURATION)),
            terminal_should_take_focus: false,
            unapplied_scroll_lines: 0.0,
            terminal_scrollbar: TerminalScrollbar::default(),
            updates: Updates::new(update_service),
        };
        app.reopen_previous_tabs_or_open_a_new_one();
        app
    }

    /// Las pestañas guardadas vuelven; si se ha lanzado con una carpeta, se
    /// abre además una pestaña allí; si no hay ninguna, la de inicio.
    fn reopen_previous_tabs_or_open_a_new_one(&mut self) {
        let folder_for_reopened_tabs = self.app_settings.settings().start.folder_for_new_tabs(None);
        match self.terminal_tabs.reopen_tabs_from_journals(&self.shell_profiles, folder_for_reopened_tabs.as_deref()) {
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
        self.folders_waiting.extend(self.desktop.take_folders_to_open());
        if self.folders_waiting.is_empty() && self.terminal_tabs.is_empty() {
            let plan = NewTabPlan::at_app_start(&self.app_settings.settings().start, &self.shell_profiles);
            self.follow_new_tab_plan(plan, true);
        }
        self.open_next_folder_waiting();
    }

    /// "Abrir Console aquí": una pestaña en cada carpeta pedida, de una en una
    /// si hay que preguntar la terminal.
    fn open_next_folder_waiting(&mut self) {
        self.folders_waiting.extend(self.desktop.take_folders_to_open());
        while self.new_tab_dialog.is_none() {
            let Some(folder) = self.folders_waiting.pop_front() else { break };
            let plan = NewTabPlan::in_folder(folder, &self.app_settings.settings().start, &self.shell_profiles);
            self.follow_new_tab_plan(plan, false);
        }
    }

    /// Abre la pestaña o, si falta algo por decidir, pregunta.
    /// `is_app_start`: la carpeta que se elija valdrá para las pestañas nuevas de esta vez.
    fn follow_new_tab_plan(&mut self, plan: NewTabPlan, is_app_start: bool) {
        if plan.needs_asking() {
            let marked = self.app_settings.settings().start.shell.resolve_without_asking(&self.shell_profiles).cloned();
            self.new_tab_dialog = Some(NewTabDialog::from_plan(plan, &self.shell_profiles, marked.as_ref(), is_app_start));
            return;
        }
        let Some(profile) = plan.shell else { return };
        let folder = match plan.folder {
            PlannedFolder::Known(folder) => Some(folder),
            PlannedFolder::Home | PlannedFolder::Ask => None,
        };
        self.open_tab_in(&profile, folder.as_deref());
    }

    /// Una pestaña nueva con el atajo: con la terminal de los ajustes (o preguntando).
    fn open_new_tab_as_configured(&mut self) {
        let start = &self.app_settings.settings().start;
        let plan = NewTabPlan::new_tab(start, &self.shell_profiles, self.folder_chosen_at_start.as_deref());
        self.follow_new_tab_plan(plan, false);
    }

    /// Una pestaña de esa terminal en la carpeta de las pestañas nuevas.
    fn open_tab_with(&mut self, profile: &ShellProfile) {
        let folder = self.folder_for_new_tabs();
        self.open_tab_in(profile, folder.as_deref());
    }

    /// `false` si no se pudo abrir (ya se ha avisado).
    fn open_tab_in(&mut self, profile: &ShellProfile, folder: Option<&str>) -> bool {
        match self.terminal_tabs.open_tab(profile, folder) {
            Ok(()) => {
                self.terminal_should_take_focus = true;
                true
            }
            Err(error) => {
                self.show_status(error);
                false
            }
        }
    }

    /// Una pestaña de esa terminal en una ruta guardada: en la carpeta, o
    /// entrando por SSH en el servidor (y su carpeta), y con su comando
    /// lanzado allí si tiene. Se llama como la ruta.
    fn open_place(&mut self, profile: &ShellProfile, place_id: PlaceId) {
        let Some(place) = self.saved_places.places().find(place_id).cloned() else { return };
        let opened = match &place.target {
            PlaceTarget::LocalFolder(folder) => {
                if !std::path::Path::new(folder).is_dir() {
                    self.show_status(tr_args("new_tab.error.no_such_folder", &[("folder", folder)]));
                    return;
                }
                let opened = self.open_tab_in(profile, Some(folder));
                if let (true, Some(command)) = (opened, &place.command) {
                    self.run_in_active_terminal(command);
                }
                opened
            }
            PlaceTarget::Ssh { destination, port, remote_folder } => {
                let command = ssh_command_line(profile, destination, *port, remote_folder.as_ref(), place.command.as_deref());
                let folder = self.folder_for_new_tabs();
                let opened = self.open_tab_in(profile, folder.as_deref());
                if opened {
                    self.run_in_active_terminal(&command);
                }
                opened
            }
        };
        if opened {
            if let Some(tab) = self.terminal_tabs.active_index().and_then(|index| self.terminal_tabs.tab_id(index)) {
                let _ = self.terminal_tabs.rename_tab(tab, &place.tab_title());
            }
        }
    }

    fn folder_for_new_tabs(&self) -> Option<String> {
        self.app_settings.settings().start.folder_for_new_tabs(self.folder_chosen_at_start.as_deref())
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
        let start = &self.app_settings.settings().start;
        let Some(profile) = start.shell.resolve_without_asking(&self.shell_profiles).cloned() else { return };
        let folder = self.folder_for_new_tabs();
        match self.terminal_tabs.open_tab(&profile, folder.as_deref()) {
            Ok(()) => self.run_in_active_terminal(command_line),
            Err(error) => self.show_status(error),
        }
    }

    /// Con una ventana encima (ajustes, diálogos), el teclado no debe llegar a la consola.
    fn a_dialog_is_open(&self) -> bool {
        self.library_dialog.is_some()
            || self.macro_dialog.is_some()
            || self.settings_window.is_some()
            || self.confirmation.is_some()
            || self.new_tab_dialog.is_some()
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

        self.open_next_folder_waiting();
        self.keep_updates_going(egui_context);
        self.type_pending_command_when_due(egui_context);
        self.handle_history_picker_keys(egui_context);
        self.handle_window_shortcuts(egui_context);

        // Los paneles laterales se dibujan antes que el central para que este
        // ocupe solo el hueco que queda.
        self.show_tab_bar(egui_context);
        self.show_macro_bar(egui_context);
        self.show_saved_commands_sidebar(egui_context);
        self.show_active_terminal(egui_context);
        self.show_history_picker(egui_context);
        self.show_library_dialog(egui_context);
        self.show_macro_dialog(egui_context);
        self.show_settings_window(egui_context);
        self.show_update_dialog(egui_context);
        self.show_new_tab_dialog(egui_context);
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
