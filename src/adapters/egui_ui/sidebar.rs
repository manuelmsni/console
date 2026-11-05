//! Menú lateral con el árbol de comandos guardados. Para reorganizarlo se
//! arrastran las filas (ver `tree_drag_and_drop`) o se usa "Mover a". Bajo
//! el título hay un filtro por nombre o comando.

use eframe::egui::{self, vec2, Align, Frame, Id, Key, Label, Layout, Margin, RichText, ScrollArea, TextEdit, Ui};
use egui_material_icons::icons::{
    ICON_CLOSE, ICON_CONTENT_COPY, ICON_CREATE_NEW_FOLDER, ICON_DELETE, ICON_DRIVE_FILE_MOVE, ICON_EDIT, ICON_FOLDER, ICON_FOLDER_OPEN,
    ICON_KEYBOARD, ICON_LIBRARY_ADD, ICON_PLAY_ARROW, ICON_SEARCH, ICON_SETTINGS, ICON_TERMINAL,
};

use super::library_dialog::LibraryDialog;
use super::material_widgets::{icon_button, menu_row, menu_row_with_color, tree_placeholder, tree_row, RenameOutcome, TreeRow};
use super::rename_in_place::{rename_field_for, RenameInPlace, RenameTarget};
use super::theme::palette;
use super::tree_drag_and_drop::{
    accept_drop_on_row, make_row_draggable, root_drop_area, show_dragged_node_under_pointer, DraggedNode,
    DropRequest, DropTargetRow,
};
use super::ConsoleApp;
use crate::domain::command_library::{CommandFolder, CommandLibrary, LibraryNode, LibraryNodeId, SavedCommand, TreeLocation};
use crate::i18n::tr;

/// Por debajo de esto, iconos, flechas y botones se pisarían entre sí.
const MINIMUM_DRAWER_WIDTH: f32 = 200.0;
/// Fracción de la ventana que puede ocupar el menú como mucho.
const MAXIMUM_DRAWER_SHARE: f32 = 0.5;
const FILTER_ROW_HEIGHT: f32 = 36.0;
/// Tres botones de icono de la cabecera y los huecos entre ellos.
const HEADER_BUTTONS_WIDTH: f32 = 3.0 * 36.0 + 2.0 * 8.0;

/// Lo que el usuario ha pedido al pulsar algo en el menú lateral. Se aplica
/// después de dibujar, porque mientras se dibuja el árbol no se puede cambiar.
enum SidebarAction {
    TypeIntoActiveTerminal(String),
    /// Clic simple en un comando: se escribe si no resulta ser un doble clic.
    TypeAfterDoubleClickWait(String),
    StartRename(RenameTarget, String),
    FinishRename(RenameOutcome),
    RunInActiveTerminal(String),
    CopyToClipboard(String),
    OpenDialog(LibraryDialog),
    MoveNode(DropRequest),
    OpenSettings,
    SetFolderOpen(LibraryNodeId, bool),
}

impl ConsoleApp {
    pub(super) fn show_saved_commands_sidebar(&mut self, egui_context: &egui::Context) {
        let mut actions = Vec::new();

        let drawer_frame = Frame::none().fill(palette::SURFACE_CONTAINER_LOW).inner_margin(Margin::symmetric(12.0, 12.0));
        egui::SidePanel::left("saved_commands")
            .resizable(true)
            .default_width(260.0)
            .min_width(MINIMUM_DRAWER_WIDTH)
            // Tope de seguridad: el menú nunca ocupa más de media ventana.
            .max_width((egui_context.screen_rect().width() * MAXIMUM_DRAWER_SHARE).max(MINIMUM_DRAWER_WIDTH))
            .frame(drawer_frame)
            .show_animated(egui_context, self.sidebar_is_visible, |ui| {
                show_drawer_header(ui, &mut actions);
                ui.add_space(4.0);
                show_filter_field(ui, &mut self.command_filter);
                ui.add_space(8.0);

                ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let library = self.command_library.library();
                    let is_filtering = !self.command_filter.trim().is_empty();
                    let matching_nodes;
                    let (nodes, folders_to_open) = if is_filtering {
                        matching_nodes = library.nodes_matching(&self.command_filter);
                        (matching_nodes.as_slice(), library.folders_leading_to_matches(&self.command_filter))
                    } else {
                        (library.root_nodes(), Vec::new())
                    };
                    if library.root_nodes().is_empty() {
                        tree_placeholder(ui, 0, &tr("sidebar.no_commands"));
                    } else if nodes.is_empty() {
                        tree_placeholder(ui, 0, &tr("sidebar.filter_no_matches"));
                    }
                    let mut tree = TreeContext { library, folders_to_open, renaming: &mut self.renaming, actions: &mut actions };
                    show_nodes(ui, nodes, 0, &mut tree);
                    if let Some(drop) = root_drop_area(ui) {
                        actions.push(SidebarAction::MoveNode(drop));
                    }
                });
            });
        show_dragged_node_under_pointer(egui_context);

        for action in actions {
            self.apply_sidebar_action(action);
        }
    }

    fn apply_sidebar_action(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::TypeIntoActiveTerminal(command_text) => self.type_into_active_terminal(&command_text),
            SidebarAction::TypeAfterDoubleClickWait(command_text) => self.type_command_after_double_click_wait(command_text),
            SidebarAction::StartRename(target, current_name) => self.start_renaming(target, &current_name),
            SidebarAction::FinishRename(outcome) => self.finish_renaming(outcome),
            SidebarAction::RunInActiveTerminal(command_text) => self.run_in_active_terminal(&command_text),
            SidebarAction::CopyToClipboard(command_text) => self.copy_to_clipboard(&command_text),
            SidebarAction::OpenDialog(dialog) => self.library_dialog = Some(dialog),
            SidebarAction::OpenSettings => self.open_settings(),
            SidebarAction::SetFolderOpen(folder, is_open) => {
                if let Err(error) = self.command_library.set_folder_open(folder, is_open) {
                    self.show_status(error);
                }
            }
            SidebarAction::MoveNode(request) => {
                let result = match request {
                    DropRequest::Beside { node_id, sibling, side } => self.command_library.move_node_beside(node_id, sibling, side),
                    DropRequest::Into { node_id, destination } => self.command_library.move_node(node_id, destination),
                };
                if let Err(error) = result {
                    self.show_status(error);
                }
            }
        }
    }
}

/// Permite arrastrar esta fila y soltar otras sobre ella.
fn handle_row_drag_and_drop(ui: &Ui, target: DropTargetRow, name: &str, actions: &mut Vec<SidebarAction>) {
    let dragged = DraggedNode { id: target.node_id, name: name.to_string(), is_folder: target.is_folder };
    make_row_draggable(target.response, dragged);
    if let Some(drop) = accept_drop_on_row(ui, target) {
        actions.push(SidebarAction::MoveNode(drop));
    }
}

fn show_drawer_header(ui: &mut Ui, actions: &mut Vec<SidebarAction>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        // El título se corta con "…" si no cabe junto a los botones.
        let title_width = (ui.available_width() - HEADER_BUTTONS_WIDTH).max(0.0);
        ui.allocate_ui_with_layout(vec2(title_width, 36.0), Layout::left_to_right(Align::Center), |ui| {
            ui.add(Label::new(RichText::new(tr("sidebar.title")).size(16.0).strong().color(palette::ON_SURFACE_VARIANT)).truncate());
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, ICON_SETTINGS, &tr("settings.title")).clicked() {
                actions.push(SidebarAction::OpenSettings);
            }
            if icon_button(ui, ICON_LIBRARY_ADD, &tr("library.dialog.new_command")).clicked() {
                actions.push(SidebarAction::OpenDialog(LibraryDialog::create_command_at(TreeLocation::Root, String::new())));
            }
            if icon_button(ui, ICON_CREATE_NEW_FOLDER, &tr("library.dialog.new_folder")).clicked() {
                actions.push(SidebarAction::OpenDialog(LibraryDialog::create_folder_at(TreeLocation::Root)));
            }
        });
    });
}

/// Campo para filtrar el árbol por nombre o comando (como un `LIKE '%texto%'`).
/// Esc o la ✕ lo vacían.
fn show_filter_field(ui: &mut Ui, filter: &mut String) {
    // De derecha a izquierda: primero la ✕ (solo si hay algo escrito; si no,
    // el campo ocupa todo el ancho) y luego el campo, que ocupa exactamente lo
    // que queda. Si el campo se calculara restando anchos, el redondeo a
    // píxeles podía pasarse un poco del panel, y el panel crecía un poco más
    // en cada fotograma hasta ocupar toda la ventana. La fila se limita a su
    // altura: sin límite, ocuparía todo el alto del panel y no dejaría sitio a la lista.
    let row_size = vec2(ui.available_width(), FILTER_ROW_HEIGHT);
    ui.allocate_ui_with_layout(row_size, Layout::right_to_left(Align::Center), |ui| {
        if !filter.is_empty() && icon_button(ui, ICON_CLOSE, &tr("sidebar.filter_clear")).clicked() {
            filter.clear();
        }
        let field = ui.add(
            // Id fijo: si dependiera del orden, la ✕ que aparece con la primera
            // letra lo cambiaría y el campo perdería el foco.
            TextEdit::singleline(filter)
                .id(Id::new("command_filter"))
                .hint_text(format!("{ICON_SEARCH}  {}", tr("sidebar.filter_hint")))
                .desired_width(f32::INFINITY)
                .margin(Margin::symmetric(10.0, 8.0)),
        );
        if field.lost_focus() && ui.input(|input| input.key_pressed(Key::Escape)) {
            filter.clear();
        }
    });
}

/// Lo que necesitan todas las filas del árbol al dibujarse.
struct TreeContext<'a> {
    library: &'a CommandLibrary,
    /// Con un filtro puesto, las carpetas que llevan hasta lo que coincide se
    /// ven abiertas aunque estén plegadas (si no, quedaría escondido). Las
    /// demás se quedan como el usuario las dejó.
    folders_to_open: Vec<LibraryNodeId>,
    renaming: &'a mut Option<RenameInPlace>,
    actions: &'a mut Vec<SidebarAction>,
}

fn show_nodes(ui: &mut Ui, nodes: &[LibraryNode], depth: usize, tree: &mut TreeContext) {
    for node in nodes {
        match node {
            LibraryNode::Folder(folder) => show_folder(ui, folder, depth, tree),
            LibraryNode::Command(command) => show_command(ui, command, depth, tree),
        }
    }
}

/// Clic: abrir o cerrar. Doble clic en el nombre: renombrar ahí mismo.
fn show_folder(ui: &mut Ui, folder: &CommandFolder, depth: usize, tree: &mut TreeContext) {
    let is_open = folder.is_open || tree.folders_to_open.contains(&folder.id);
    let rename_target = RenameTarget::LibraryNode(folder.id);

    let folder_row = tree_row(
        ui,
        Id::new(("library_row", folder.id)),
        TreeRow {
            depth,
            expander: Some(is_open),
            icon: if is_open { ICON_FOLDER_OPEN } else { ICON_FOLDER },
            icon_color: palette::PRIMARY,
            label: &folder.name,
            has_run_button: false,
            rename_field: rename_field_for(tree.renaming, rename_target),
        },
    );
    if let Some(outcome) = folder_row.rename {
        tree.actions.push(SidebarAction::FinishRename(outcome));
    } else if folder_row.row.double_clicked() {
        tree.actions.push(SidebarAction::StartRename(rename_target, folder.name.clone()));
    }
    if folder_row.row.clicked() {
        tree.actions.push(SidebarAction::SetFolderOpen(folder.id, !is_open));
    }
    let drop_target = DropTargetRow { response: &folder_row.row, node_id: folder.id, is_folder: true, depth };
    handle_row_drag_and_drop(ui, drop_target, &folder.name, tree.actions);

    let (library, actions) = (tree.library, &mut *tree.actions);
    let inside_this_folder = TreeLocation::InsideFolder(folder.id);
    folder_row.row.context_menu(|ui| {
        if menu_row(ui, ICON_LIBRARY_ADD, &tr("sidebar.menu.new_command_here")).clicked() {
            actions.push(SidebarAction::OpenDialog(LibraryDialog::create_command_at(inside_this_folder, String::new())));
            ui.close_menu();
        }
        if menu_row(ui, ICON_CREATE_NEW_FOLDER, &tr("sidebar.menu.new_subfolder")).clicked() {
            actions.push(SidebarAction::OpenDialog(LibraryDialog::create_folder_at(inside_this_folder)));
            ui.close_menu();
        }
        if menu_row(ui, ICON_EDIT, &tr("sidebar.menu.rename")).clicked() {
            actions.push(SidebarAction::StartRename(rename_target, folder.name.clone()));
            ui.close_menu();
        }
        show_move_menu(ui, library, folder.id, actions);
        if menu_row_with_color(ui, ICON_DELETE, &tr("library.dialog.delete"), palette::ERROR).clicked() {
            actions.push(SidebarAction::OpenDialog(LibraryDialog::confirm_removal(folder.id, &folder.name)));
            ui.close_menu();
        }
    });

    if is_open {
        if folder.children.is_empty() {
            tree_placeholder(ui, depth + 1, &tr("sidebar.empty_folder"));
        }
        show_nodes(ui, &folder.children, depth + 1, tree);
    }
}

/// ▶: ejecutar. Clic en el nombre: escribirlo en la consola (tras comprobar
/// que no es un doble clic). Doble clic en el nombre: renombrar ahí mismo.
fn show_command(ui: &mut Ui, command: &SavedCommand, depth: usize, tree: &mut TreeContext) {
    let rename_target = RenameTarget::LibraryNode(command.id);
    let command_row = tree_row(
        ui,
        Id::new(("library_row", command.id)),
        TreeRow {
            depth,
            expander: None,
            icon: ICON_TERMINAL,
            icon_color: palette::ON_SURFACE_VARIANT,
            label: &command.name,
            has_run_button: true,
            rename_field: rename_field_for(tree.renaming, rename_target),
        },
    );

    if let Some(outcome) = command_row.rename {
        tree.actions.push(SidebarAction::FinishRename(outcome));
    } else if command_row.run_clicked {
        tree.actions.push(SidebarAction::RunInActiveTerminal(command.command_text.clone()));
    } else if command_row.row.double_clicked() {
        tree.actions.push(SidebarAction::StartRename(rename_target, command.name.clone()));
    } else if command_row.row.clicked() {
        tree.actions.push(SidebarAction::TypeAfterDoubleClickWait(command.command_text.clone()));
    }
    let drop_target = DropTargetRow { response: &command_row.row, node_id: command.id, is_folder: false, depth };
    handle_row_drag_and_drop(ui, drop_target, &command.name, tree.actions);

    let row_width = command_row.row.rect.width();
    let command_preview = command_row.row.on_hover_ui(|ui| {
        // El recuadro mide lo mismo que la fila; si el comando es más largo, baja de línea.
        ui.set_width(row_width - ui.spacing().menu_margin.sum().x);
        ui.add(Label::new(&command.command_text).wrap());
    });
    let (library, actions) = (tree.library, &mut *tree.actions);
    command_preview.context_menu(|ui| {
        if menu_row(ui, ICON_PLAY_ARROW, &tr("sidebar.menu.run")).clicked() {
            actions.push(SidebarAction::RunInActiveTerminal(command.command_text.clone()));
            ui.close_menu();
        }
        if menu_row(ui, ICON_KEYBOARD, &tr("sidebar.menu.type_without_running")).clicked() {
            actions.push(SidebarAction::TypeIntoActiveTerminal(command.command_text.clone()));
            ui.close_menu();
        }
        if menu_row(ui, ICON_CONTENT_COPY, &tr("common.copy")).clicked() {
            actions.push(SidebarAction::CopyToClipboard(command.command_text.clone()));
            ui.close_menu();
        }
        if menu_row(ui, ICON_EDIT, &tr("sidebar.menu.edit")).clicked() {
            actions.push(SidebarAction::OpenDialog(LibraryDialog::edit_command(command)));
            ui.close_menu();
        }
        show_move_menu(ui, library, command.id, actions);
        if menu_row_with_color(ui, ICON_DELETE, &tr("library.dialog.delete"), palette::ERROR).clicked() {
            actions.push(SidebarAction::OpenDialog(LibraryDialog::confirm_removal(command.id, &command.name)));
            ui.close_menu();
        }
    });
}
fn show_move_menu(ui: &mut Ui, library: &CommandLibrary, node_id: LibraryNodeId, actions: &mut Vec<SidebarAction>) {
    ui.menu_button(format!("{ICON_DRIVE_FILE_MOVE}   {}", tr("sidebar.menu.move_to")), |ui| {
        for destination in library.destinations_for_moving(node_id) {
            if menu_row(ui, ICON_FOLDER, &destination.label).clicked() {
                actions.push(SidebarAction::MoveNode(DropRequest::Into { node_id, destination: destination.location }));
                ui.close_menu();
            }
        }
    });
}
