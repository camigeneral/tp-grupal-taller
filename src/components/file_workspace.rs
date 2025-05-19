extern crate gtk4;
extern crate relm4;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Duration;

use self::gtk4::glib;
use self::gtk4::prelude::{OrientableExt, WidgetExt};
use self::relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};
use super::file_editor::FileEditorModel;
use super::list_files::FileListView;
use crate::components::file_editor::FileEditorOutputMessage;
use components::file_editor::FileEditorMessage;
use components::list_files::FileFilterAction;
use components::types::FileType;

#[derive(Debug)]
/// Estructura principal que gestiona el espacio de trabajo de archivos, que incluye una lista de archivos
/// y un editor de archivos. Mantiene el estado de la visibilidad del editor de archivos.
pub struct FileWorkspace {
    /// Controlador para la vista de la lista de archivos.
    file_list_ctrl: Controller<FileListView>,
    /// Controlador para el modelo del editor de archivos.
    file_editor_ctrl: Controller<FileEditorModel>,
    /// Bandera que indica si el editor de archivos está visible.
    editor_visible: bool,

    current_file: String,
}

/// Enum que define los diferentes mensajes que puede recibir el componente `FileWorkspace`.
/// Permite abrir un archivo, cerrar el editor o ignorar un mensaje.
#[derive(Debug)]
pub enum FileWorkspaceMsg {
    /// Mensaje para abrir un archivo con nombre, contenido y cantidad de líneas.
    OpenFile(String, String, u8),
    /// Mensaje para ignorar una acción.
    Ignore,
    /// Mensaje para cerrar el editor de archivos.
    CloseEditor,
    ReloadFiles,
}

#[relm4::component(pub)]
impl SimpleComponent for FileWorkspace {
    type Output = ();
    type Init = ();
    type Input = FileWorkspaceMsg;

    view! {
        #[name="body_container"]
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_vexpand: true,
            set_hexpand: true,
            set_margin_all: 10,
            #[name="body"]
            gtk::Box {
                add_css_class: "card",
                add_css_class: "content-card",
                set_hexpand: true,
                set_vexpand: true,
                set_valign: gtk::Align::Fill,
                set_orientation: gtk::Orientation::Vertical,
                #[local_ref]
                list_box_widget -> gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    #[watch]
                    set_visible: !model.editor_visible
                },

                #[local_ref]
                editor_widget -> gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    #[watch]
                    set_visible: model.editor_visible
                }

            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let list_files_cont = FileListView::builder()
            .launch(get_files_list(&"docs.txt".to_string()))
            .forward(
                sender.input_sender(),
                |msg: crate::components::list_files::FileFilterAction| match msg {
                    crate::components::list_files::FileFilterAction::SelectFile(
                        file,
                        _file_type,
                        content,
                        qty,
                    ) => FileWorkspaceMsg::OpenFile(file, content, qty),
                    _ => FileWorkspaceMsg::Ignore,
                },
            );
        let editor_file_cont = FileEditorModel::builder()
            .launch(("".to_string(), 0, "".to_string()))
            .forward(
                sender.input_sender(),
                |msg: FileEditorOutputMessage| match msg {
                    FileEditorOutputMessage::GoBack => FileWorkspaceMsg::CloseEditor,
                },
            );
        let model = FileWorkspace {
            file_list_ctrl: list_files_cont,
            file_editor_ctrl: editor_file_cont,
            editor_visible: false,
            current_file: "".to_string(),
        };

        let list_box_widget = model.file_list_ctrl.widget();
        let editor_widget = model.file_editor_ctrl.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            FileWorkspaceMsg::OpenFile(file, content, qty) => {
                self.current_file = file.clone();
                self.file_editor_ctrl
                    .sender()
                    .send(FileEditorMessage::UpdateFile(file, qty, content))
                    .unwrap();
                self.editor_visible = true;
            }
            FileWorkspaceMsg::CloseEditor => {
                self.file_editor_ctrl
                    .sender()
                    .send(FileEditorMessage::ResetEditor)
                    .unwrap();
                self.editor_visible = false;
            }
            FileWorkspaceMsg::ReloadFiles => {
                let file_list_sender: relm4::Sender<FileFilterAction> = self.file_list_ctrl.sender().clone();
                let file_editor_sender = self.file_editor_ctrl.sender().clone();

                let current_file = self.current_file.clone();

                glib::timeout_add_local(Duration::from_millis(100), move || {
                    let new_files = get_files_list(&"docs.txt".to_string());
                    file_list_sender
                        .send(FileFilterAction::UpdateFiles(new_files.clone()))
                        .unwrap();

                    if let Some((file_name, _, new_content, qty)) = new_files
                        .iter()
                        .find(|(name, _, _, _)| *name == current_file)
                    {
                        file_editor_sender
                            .send(FileEditorMessage::UpdateFile(
                                file_name.clone(),
                                *qty,
                                new_content.clone(),
                            ))
                            .unwrap();
                    }

                    glib::ControlFlow::Break
                });
            }

            _ => {}
        }
    }
}

fn get_files_list(
    file_path: &String,
) -> Vec<(std::string::String, FileType, std::string::String, u8)> {
    let docs = get_file_content_workspace(file_path).unwrap_or_else(|_| HashMap::new());
    // Convierte el HashMap a la lista que espera FileListView
    let files_list: Vec<(String, FileType, String, u8)> = docs
        .into_iter()
        .map(|(nombre, mensajes)| {
            let contenido = mensajes.join("\n");
            let qty = mensajes.len() as u8;
            let file_type = if nombre.ends_with(".xlsx") {
                FileType::Sheet
            } else {
                FileType::Text
            };
            (nombre, file_type, contenido, qty)
        })
        .collect();

    files_list
}

pub fn get_file_content_workspace(
    file_path: &String,
) -> Result<HashMap<String, Vec<String>>, String> {
    let file = File::open(file_path).map_err(|_| "file-not-found".to_string())?;
    let reader = BufReader::new(file);
    let lines = reader.lines();

    let mut docs: HashMap<String, Vec<String>> = HashMap::new();

    for line in lines {
        match line {
            Ok(read_line) => {
                let parts: Vec<&str> = read_line.split("/++/").collect();
                if parts.len() != 2 {
                    continue;
                }

                let doc_name = parts[0].to_string();
                let messages_str = parts[1];

                let messages: Vec<String> = messages_str
                    .split("/--/")
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();

                docs.insert(doc_name, messages);
            }
            Err(_) => return Err("unable-to-read-file".to_string()),
        }
    }

    Ok(docs)
}
