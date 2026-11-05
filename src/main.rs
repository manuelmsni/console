// En la versión final de Windows no se abre la ventana negra de consola detrás
// de la app (en los demás sistemas esta línea no hace nada).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapters;
mod application;
mod domain;
mod i18n;
mod project;

use std::sync::Arc;

use eframe::egui;

use adapters::data_folder::{rename_files_of_older_versions, DataFolder};
use adapters::egui_ui::{prepare_look, window_icon, ConsoleApp, DesktopIntegration};
use adapters::encrypted_tab_journal_store::EncryptedTabJournalStore;
use adapters::json_app_settings_store::JsonAppSettingsStore;
use adapters::json_command_library_store::JsonCommandLibraryStore;
use adapters::json_ssh_server_store::JsonSshServerStore;
use adapters::github_releases::GitHubReleases;
use adapters::platform::{
    available_shell_profiles, claim_single_instance, system_display_language, wait_for_process_to_exit, InstanceStartup,
    ShowWindowRequests, SystemClock, SystemSshTools, SystemWebBrowser, OPERATING_SYSTEM,
};
use adapters::self_replacing_program::{SelfReplacingProgram, AFTER_UPDATE_ARGUMENT};
use adapters::pty_shell::PtyShellLauncher;
use adapters::system_clipboard::SystemClipboard;
use adapters::vt100_emulator::Vt100EmulatorFactory;
use application::app_settings_service::AppSettingsService;
use application::command_library_service::CommandLibraryService;
use application::ssh_settings::SshSettings;
use application::terminal_tabs::{AutosavePolicy, TerminalTabs};
use application::update_service::UpdateService;
use domain::updates::{BuildKind, Version};


/// Versión instalada: los datos van en la carpeta del usuario, cifrados con su
/// cuenta del sistema (sin contraseña).
#[cfg(not(feature = "portable"))]
fn main() -> eframe::Result<()> {
    use adapters::data_folder::rename_user_folder_of_older_versions;
    use adapters::platform::SystemUserCipher;

    wait_for_previous_version_after_update();
    let show_window_requests = match claim_single_instance("") {
        InstanceStartup::FirstInstance(show_window_requests) => show_window_requests,
        // Ya hay una consola abierta y se le ha pedido que se muestre: no abrimos otra.
        InstanceStartup::AlreadyRunning => return Ok(()),
    };
    let system_language = system_display_language();
    let data_location = DataFolder::user_config_location();
    rename_user_folder_of_older_versions(&data_location);
    rename_files_of_older_versions(&data_location);
    let data = DataFolder::new(data_location, Arc::new(SystemUserCipher));
    // Los ajustes se leen antes de crear la ventana: el idioma hace falta ya para su título.
    let (app_settings, settings_warning) = load_settings_and_language(&data, &system_language);

    let startup = Startup { data, app_settings, settings_warning, system_language };
    eframe::run_native(
        "Console",
        window_options(),
        Box::new(|creation_context| {
            let desktop = prepare_window(creation_context, show_window_requests);
            Ok(Box::new(build_console_app(creation_context.egui_ctx.clone(), desktop, startup)))
        }),
    )
}

/// Versión portable: los datos van en la carpeta `data` junto al programa,
/// cifrados con una contraseña que se pide al abrir (y se crea la primera vez).
#[cfg(feature = "portable")]
fn main() -> eframe::Result<()> {
    use adapters::data_folder::rename_portable_folder_of_older_versions;
    use adapters::egui_ui::PortableStartup;
    use adapters::file_cipher::FileCipher;

    wait_for_previous_version_after_update();
    let data_location = DataFolder::next_to_program_location();
    rename_portable_folder_of_older_versions(&data_location);
    rename_files_of_older_versions(&data_location);
    let show_window_requests = match claim_single_instance(&portable_instance_scope(&data_location)) {
        InstanceStartup::FirstInstance(show_window_requests) => show_window_requests,
        InstanceStartup::AlreadyRunning => return Ok(()),
    };
    // Hasta saber la contraseña no se pueden leer los ajustes: de momento, el
    // idioma del sistema.
    let system_language = system_display_language();
    i18n::set_language(None, &system_language);

    eframe::run_native(
        "Console",
        window_options(),
        Box::new(move |creation_context| {
            let desktop = prepare_window(creation_context, show_window_requests);
            let egui_context = creation_context.egui_ctx.clone();
            let key_folder = data_location.clone();
            let open_console = Box::new(move |cipher: Arc<dyn FileCipher>, desktop: DesktopIntegration| {
                let data = DataFolder::new(data_location, cipher);
                let (app_settings, settings_warning) = load_settings_and_language(&data, &system_language);
                egui_context.send_viewport_cmd(egui::ViewportCommand::Title(i18n::tr("app.name")));
                let startup = Startup { data, app_settings, settings_warning, system_language };
                build_console_app(egui_context, desktop, startup)
            });
            Ok(Box::new(PortableStartup::new(&key_folder, desktop, open_console)))
        }),
    )
}

/// Cada carpeta portable es una consola distinta: una no impide abrir otra (ni
/// la instalada), pero la misma no se abre dos veces.
#[cfg(feature = "portable")]
fn portable_instance_scope(data_location: &std::path::Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data_location.hash(&mut hasher);
    format!("portable-{:016x}", hasher.finish())
}

/// Tras actualizar, la versión anterior arranca esta con `--after-update <pid>`:
/// hay que esperar a que se cierre, o la instancia única haría que esta se
/// cerrase pensando que ya hay una consola abierta.
fn wait_for_previous_version_after_update() {
    let arguments: Vec<String> = std::env::args().collect();
    if let [_, flag, process_id, ..] = arguments.as_slice() {
        if flag == AFTER_UPDATE_ARGUMENT {
            if let Ok(process_id) = process_id.parse() {
                wait_for_process_to_exit(process_id);
            }
        }
    }
}

/// Buscar e instalar versiones nuevas de este mismo tipo de programa (sistema,
/// y normal o portable).
fn update_service() -> Option<UpdateService> {
    let current_version = Version::parse(env!("CARGO_PKG_VERSION")).expect("la versión de Cargo.toml es x.y.z");
    let build_kind = BuildKind { operating_system: OPERATING_SYSTEM, is_portable: cfg!(feature = "portable") };
    let installer = SelfReplacingProgram::current()?;
    let service = UpdateService::new(
        Arc::new(GitHubReleases::of(project::REPOSITORY, current_version)),
        Arc::new(installer),
        current_version,
        build_kind,
    );
    service.clean_up_previous_program();
    Some(service)
}

fn window_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(i18n::tr("app.name"))
            .with_inner_size([1100.0, 700.0])
            // Sin la barra de título de Windows: la barra de pestañas hace sus veces.
            .with_decorations(false)
            .with_icon(window_icon()),
        ..Default::default()
    }
}

/// Aspecto, icono de la bandeja y avisos de "muéstrate": nada más crear la
/// ventana, antes que la consola (en la portable, antes de pedir la contraseña).
fn prepare_window(creation_context: &eframe::CreationContext, show_window_requests: Option<ShowWindowRequests>) -> DesktopIntegration {
    prepare_look(&creation_context.egui_ctx);
    DesktopIntegration::connect(creation_context, show_window_requests)
}

fn load_settings_and_language(data: &DataFolder, system_language: &str) -> (AppSettingsService, Option<String>) {
    let (app_settings, settings_warning) = AppSettingsService::load(Box::new(JsonAppSettingsStore::in_folder(data)));
    i18n::set_language(app_settings.settings().language.as_deref(), system_language);
    (app_settings, settings_warning)
}

/// Lo que se prepara antes de crear la consola.
struct Startup {
    data: DataFolder,
    app_settings: AppSettingsService,
    settings_warning: Option<String>,
    system_language: String,
}

/// Raíz de composición: el único sitio donde se decide qué adaptador
/// concreto implementa cada puerto.
fn build_console_app(egui_context: egui::Context, desktop: DesktopIntegration, startup: Startup) -> ConsoleApp {
    let Startup { data, app_settings, settings_warning, system_language } = startup;
    let context_to_wake_up = egui_context.clone();
    let redraw_when_output_arrives = Arc::new(move || context_to_wake_up.request_repaint());

    let terminal_tabs = TerminalTabs::new(
        Box::new(PtyShellLauncher::new(redraw_when_output_arrives)),
        Box::new(Vt100EmulatorFactory),
        Box::new(EncryptedTabJournalStore::in_folder(&data)),
        Box::new(SystemClock),
        AutosavePolicy::from_settings(&app_settings.settings().autosave),
    );
    let (command_library, startup_warning) = CommandLibraryService::load(Box::new(JsonCommandLibraryStore::in_folder(&data)));
    let (ssh_settings, ssh_warning) =
        SshSettings::load(Arc::new(SystemSshTools::for_current_user()), Box::new(JsonSshServerStore::in_folder(&data)));

    ConsoleApp::new(
        egui_context,
        desktop,
        terminal_tabs,
        command_library,
        app_settings,
        ssh_settings,
        Box::new(SystemClipboard),
        Box::new(SystemWebBrowser),
        available_shell_profiles(),
        update_service(),
        system_language,
        startup_warning.or(settings_warning).or(ssh_warning),
    )
}
