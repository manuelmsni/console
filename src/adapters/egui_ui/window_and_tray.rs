//! Cerrar la ventana la esconde en la bandeja del sistema (junto al reloj) y
//! las consolas siguen funcionando. Desde el icono se vuelve a mostrar o se
//! sale de verdad.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::app_icon::app_icon_rgba;
use crate::adapters::platform::{MainWindow, ShowWindowRequests};
use crate::i18n::{tr, tr_args};

const TRAY_ICON_SIZE: u32 = 32;

/// Lo que une la ventana al escritorio: la ventana nativa (para esconderla y
/// volver a mostrarla), el icono de la bandeja y los avisos de "muéstrate"
/// cuando se lanza el programa otra vez. Se prepara al crear la ventana, antes
/// que la consola, para que también funcione en la pantalla de contraseña de
/// la portable.
pub struct DesktopIntegration {
    main_window: Option<MainWindow>,
    system_tray: Option<SystemTray>,
    /// Por qué no hay icono en la bandeja, si no se pudo crear.
    problem: Option<String>,
}

impl DesktopIntegration {
    pub fn connect(creation_context: &eframe::CreationContext, show_window_requests: Option<ShowWindowRequests>) -> Self {
        let egui_context = creation_context.egui_ctx.clone();
        let main_window = MainWindow::of(creation_context);
        // Si se lanza el programa otra vez, se muestra esta ventana en vez de abrir otra.
        if let (Some(requests), Some(window)) = (show_window_requests, main_window) {
            let context_to_wake_up = egui_context.clone();
            requests.answer_with(move || {
                window.show();
                context_to_wake_up.request_repaint();
            });
        }
        let (system_tray, problem) = match main_window.map(|window| SystemTray::create(window, egui_context)) {
            Some(Ok(tray)) => (Some(tray), None),
            Some(Err(error)) => (None, Some(error)),
            None => (None, None),
        };
        Self { main_window, system_tray, problem }
    }

    /// Esconde la ventana en la bandeja. `false` si no hay icono en la
    /// bandeja: entonces no habría forma de volver, y hay que salir.
    pub fn hide_to_tray(&self) -> bool {
        match (&self.system_tray, self.main_window) {
            (Some(_), Some(window)) => {
                window.hide();
                true
            }
            _ => false,
        }
    }

    /// `true` una sola vez por cada vez que se pulse "Salir" en el icono.
    pub fn exit_was_requested_from_tray(&self) -> bool {
        self.system_tray.as_ref().is_some_and(SystemTray::take_exit_request)
    }

    /// El problema al crear el icono de la bandeja, para avisar una sola vez.
    pub fn take_problem(&mut self) -> Option<String> {
        self.problem.take()
    }
}

/// El icono de la bandeja y lo que se ha pedido desde él.
struct SystemTray {
    _tray_icon: TrayIcon,
    _menu_items: [MenuItem; 2],
    exit_requested: Arc<AtomicBool>,
}

impl SystemTray {
    fn create(window: MainWindow, egui_context: egui::Context) -> Result<Self, String> {
        let show_item = MenuItem::new(tr("tray.show"), true, None);
        let exit_item = MenuItem::new(tr("tray.exit"), true, None);
        let menu = Menu::with_items(&[&show_item, &exit_item]).map_err(|error| error.to_string())?;
        let icon = Icon::from_rgba(app_icon_rgba(TRAY_ICON_SIZE), TRAY_ICON_SIZE, TRAY_ICON_SIZE)
            .map_err(|error| error.to_string())?;

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip(tr("app.name"))
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|error| tr_args("tray.error.create", &[("error", &error)]))?;

        let exit_requested = Arc::new(AtomicBool::new(false));
        listen_to_tray_menu(&show_item, &exit_item, window, egui_context.clone(), exit_requested.clone());
        listen_to_tray_clicks(window, egui_context);

        Ok(Self { _tray_icon: tray_icon, _menu_items: [show_item, exit_item], exit_requested })
    }

    /// `true` una sola vez por cada vez que se pulse "Salir" en el icono.
    fn take_exit_request(&self) -> bool {
        self.exit_requested.swap(false, Ordering::SeqCst)
    }
}

fn listen_to_tray_menu(
    show_item: &MenuItem,
    exit_item: &MenuItem,
    window: MainWindow,
    egui_context: egui::Context,
    exit_requested: Arc<AtomicBool>,
) {
    let (show_id, exit_id) = (show_item.id().clone(), exit_item.id().clone());
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == exit_id {
            exit_requested.store(true, Ordering::SeqCst);
        }
        // Para salir también se muestra la ventana: ahí se pregunta si guardar las pestañas.
        if event.id == show_id || event.id == exit_id {
            window.show();
            egui_context.request_repaint();
        }
    }));
}

fn listen_to_tray_clicks(window: MainWindow, egui_context: egui::Context) {
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
            window.show();
            egui_context.request_repaint();
        }
    }));
}
