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
use crate::documento::Documento;

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
    /// Nombre del archivo actual.
    current_file: String,

    files: HashMap<(String, FileType), HashMap<String, String>>
}

/// Enum que define los diferentes mensajes que puede recibir el componente `FileWorkspace`.
/// Permite abrir un archivo, cerrar el editor o ignorar un mensaje.
#[derive(Debug)]
pub enum FileWorkspaceMsg {
    /// Mensaje para abrir un archivo con nombre, contenido y cantidad de líneas.
    OpenFile(String, FileType),
    /// Mensaje para ignorar una acción.
    Ignore,
    /// Mensaje para cerrar el editor de archivos.
    CloseEditor,
    SubscribeFile(String, String, i32),
    ReloadFiles,
}

#[derive(Debug)]
pub enum FileWorkspaceOutputMessage {
    SubscribeFile(String),
    UnsubscribeFile(String),
}

#[relm4::component(pub)]
impl SimpleComponent for FileWorkspace {
    type Output = FileWorkspaceOutputMessage;
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
                    ) => FileWorkspaceMsg::SubscribeFile(file, content, qty),
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

        let mut files_map: HashMap<(String, FileType), HashMap<String, String>> = HashMap::new();

        if let Ok(docs) = get_file_content_workspace(&"docs.txt".to_string()) {
            for (nombre, mensajes) in docs {
                let file_type = if nombre.ends_with(".xlsx") {
                    FileType::Sheet
                } else {
                    FileType::Text
                };

                let inner_map: HashMap<String, String> = mensajes
                    .into_iter()
                    .enumerate()
                    .map(|(i, msg)| (format!("line_{}", i), msg))
                    .collect();

                files_map.insert((nombre, file_type), inner_map);
            }
        }

        let model = FileWorkspace {
            file_list_ctrl: list_files_cont,
            file_editor_ctrl: editor_file_cont,
            editor_visible: false,
            current_file: "".to_string(),
            files: files_map,
        };

        let list_box_widget = model.file_list_ctrl.widget();
        let editor_widget = model.file_editor_ctrl.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            FileWorkspaceMsg::SubscribeFile(file, _content, _qty) => {
                sender
                    .output(FileWorkspaceOutputMessage::SubscribeFile(file))
                    .unwrap();
            }

            FileWorkspaceMsg::OpenFile(file, file_type) => {
                self.current_file = file.clone();

                if let Some(inner_map) = self.files.get(&(file.clone(), file_type.clone())) {
                    let content = inner_map.get("content").cloned().unwrap_or_default();
                    let qty = content.lines().count() as i32;

                    self.file_editor_ctrl
                        .sender()
                        .send(FileEditorMessage::UpdateFile(file, qty, content))
                        .unwrap();

                    self.editor_visible = true;
                } else {
                    println!("Archivo no encontrado: {} ({:?})", file, file_type.clone());
                }
            }
            FileWorkspaceMsg::CloseEditor => {
                sender
                    .output(FileWorkspaceOutputMessage::UnsubscribeFile(self.current_file.clone()))
                    .unwrap();

                self.file_editor_ctrl
                    .sender()
                    .send(FileEditorMessage::ResetEditor)
                    .unwrap();
                self.editor_visible = false;
            }
            FileWorkspaceMsg::ReloadFiles => {
                let file_list_sender: relm4::Sender<FileFilterAction> =
                    self.file_list_ctrl.sender().clone();
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
) -> Vec<(std::string::String, FileType, std::string::String, i32)> {
    let docs = get_file_content_workspace(file_path).unwrap_or_else(|_| HashMap::new());
    // Convierte el HashMap a la lista que espera FileListView
    let files_list: Vec<(String, FileType, String, i32)> = docs
        .into_iter()
        .map(|(nombre, doc)| {
            match doc {
                Documento::Texto(lineas) => {
                    let contenido = lineas.join("\n");
                    let qty = lineas.len() as u8;
                    (nombre, FileType::Text, contenido, qty)
                }
                Documento::Calculo(filas) => {
                    let contenido = filas.iter().map(|fila| fila.join(",")).collect::<Vec<_>>().join("\n");
                    let qty = filas.len() as u8;
                    (nombre, FileType::Sheet, contenido, qty)
                }
            }
        })
        .collect();
    files_list
}

pub fn get_file_content_workspace(
    file_path: &String,
) -> Result<HashMap<String, Documento>, String> {
    let file = File::open(file_path).map_err(|_| "file-not-found".to_string())?;
    let reader = BufReader::new(file);
    let lines = reader.lines();

    let mut docs: HashMap<String, Documento> = HashMap::new();

    for line in lines {
        match line {
            Ok(read_line) => {
                let parts: Vec<&str> = read_line.split("/++/").collect();
                if parts.len() < 3 {
                    continue;
                }
                let tipo = parts[0];
                let doc_name = parts[1].to_string();
                let data = parts[2];

                match tipo {
                    "TXT" => {
                        let messages: Vec<String> = data
                            .split("/--/")
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .collect();
                        docs.insert(doc_name, Documento::Texto(messages));
                    }
                    "CALC" => {
                        let filas: Vec<Vec<String>> = data
                            .split("/--/")
                            .filter(|s| !s.is_empty())
                            .map(|fila| fila.split(',').map(|c| c.to_string()).collect())
                            .collect();
                        docs.insert(doc_name, Documento::Calculo(filas));
                    }
                    _ => {}
                }
            }
            Err(_) => return Err("unable-to-read-file".to_string()),
        }
    }

    Ok(docs)
}
