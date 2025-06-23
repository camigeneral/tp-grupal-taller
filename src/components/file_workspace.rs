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
use crate::documento::Documento;
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
    /// Nombre del archivo actual.
    current_file: String,

    current_file_type: FileType,

    files: HashMap<(String, FileType), Documento>,
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
    ContentAdded(String, i32),
    ContentAddedSpreadSheet(String, String, String)
}

#[derive(Debug)]
pub enum FileWorkspaceOutputMessage {
    SubscribeFile(String),
    UnsubscribeFile(String),
    ContentAdded(String, String, i32),
    ContentAddedSpreadSheet(String, String, String, String)
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
        let redis_nodes_files: [&str; 3] = [
            "redis_node_0_5460.rdb",
            "redis_node_5460_10921.rdb",
            "redis_node_10921_16383.rdb",
        ];

        let files_list = get_all_files_list(&redis_nodes_files);

        let list_files_cont: Controller<FileListView> =
            FileListView::builder().launch(files_list.clone()).forward(
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
                    FileEditorOutputMessage::ContentAdded(new_text, offset) => FileWorkspaceMsg::ContentAdded(new_text, offset),
                    FileEditorOutputMessage::ContentAddedSpreadSheet(row, col, text ) =>  FileWorkspaceMsg::ContentAddedSpreadSheet(row, col, text ),
                },
            );

        let mut files_map: HashMap<(String, FileType), Documento> = HashMap::new();

        let redis_nodes_files: [&str; 3] = [
            "redis_node_0_5460.rdb",
            "redis_node_5460_10921.rdb",
            "redis_node_10921_16383.rdb",
        ];

        let docs = get_all_files_content(&redis_nodes_files);
        for (nombre, mensajes) in docs {
            let file_type = if nombre.ends_with(".xlsx") {
                FileType::Sheet
            } else {
                FileType::Text
            };
            files_map.insert((nombre, file_type), mensajes);
        }
        let model = FileWorkspace {
            file_list_ctrl: list_files_cont,
            file_editor_ctrl: editor_file_cont,
            editor_visible: false,
            current_file: "".to_string(),
            files: files_map,
            current_file_type: FileType::Text,
        };

        let list_box_widget = model.file_list_ctrl.widget();
        let editor_widget = model.file_editor_ctrl.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            FileWorkspaceMsg::ContentAddedSpreadSheet(row, col, text) => {
                let _ = sender.output(FileWorkspaceOutputMessage::ContentAddedSpreadSheet(self.current_file.clone(), row, col, text));
            }
            FileWorkspaceMsg::ContentAdded(text, offset) => {
                let _ = sender.output(FileWorkspaceOutputMessage::ContentAdded(self.current_file.clone(), text, offset));
            }

            FileWorkspaceMsg::SubscribeFile(file, _content, _qty) => {
                sender
                    .output(FileWorkspaceOutputMessage::SubscribeFile(file.clone()))
                    .unwrap();

                let file_type = if file.ends_with(".xlsx") {
                    FileType::Sheet
                } else {
                    FileType::Text
                };

                sender.input(FileWorkspaceMsg::OpenFile(file, file_type));
            }

            FileWorkspaceMsg::OpenFile(file, file_type) => {
                self.current_file = file.clone();
                self.current_file_type = file_type.clone();

                if let Some(doc) = self.files.get(&(file.clone(), file_type.clone())) {
                    let (content, qty) = match doc {
                        Documento::Texto(lineas) => (lineas.join("\n"), lineas.len() as i32),
                        Documento::Calculo(filas) => (
                            filas.join("\n"), filas.len() as i32
                        ),
                    };

                    self.file_editor_ctrl
                        .sender()
                        .send(FileEditorMessage::UpdateFile(file, qty, content, file_type))
                        .unwrap();

                    self.editor_visible = true;
                }
            }
            FileWorkspaceMsg::CloseEditor => {
                sender
                    .output(FileWorkspaceOutputMessage::UnsubscribeFile(
                        self.current_file.clone(),
                    ))
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

                let redis_nodes_files: [&str; 3] = [
                    "redis_node_0_5460.rdb",
                    "redis_node_5460_10921.rdb",
                    "redis_node_10921_16383.rdb",
                ];

                let mut files_map: HashMap<(String, FileType), Documento> = HashMap::new();
                let new_files: Vec<(String, FileType, String, i32)> = get_all_files_list(&redis_nodes_files);
                let docs: HashMap<String, Documento> = get_all_files_content(&redis_nodes_files);
                for (nombre, mensajes) in docs {
                    let file_type = if nombre.ends_with(".xlsx") {
                        FileType::Sheet
                    } else {
                        FileType::Text
                    };
                    files_map.insert((nombre, file_type), mensajes);
                }
            
                self.files = files_map.clone();
                glib::timeout_add_local(Duration::from_millis(100), move || {
                    file_list_sender
                        .send(FileFilterAction::UpdateFiles(new_files.clone()))
                        .unwrap();

                    if let Some((file_name, file_type, _, qty)) = new_files
                        .iter()
                        .find(|(name, _, _, _)| *name == current_file)
                    {
                        if let Some(doc) = files_map.get(&(file_name.clone(), file_type.clone())) {
                            let content = match doc {
                                Documento::Texto(lineas) => lineas.join("\n"),
                                Documento::Calculo(filas) => filas.join("\n"),
                            };
                            file_editor_sender
                                .send(FileEditorMessage::UpdateFile(
                                    file_name.clone(),
                                    *qty,
                                    content,
                                    file_type.clone(),
                                ))
                                .unwrap();
                        }
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
    let files_list: Vec<(String, FileType, String, i32)> = docs
        .into_iter()
        .map(|(nombre, doc)| match doc {
            Documento::Texto(lineas) => {
                let contenido = lineas.join("\n");
                let qty = lineas.len() as i32;
                (nombre, FileType::Text, contenido, qty)
            }
            Documento::Calculo(filas) => {
                let contenido = filas.join("\n");
                let qty = filas.len() as i32;
                (nombre, FileType::Sheet, contenido, qty)
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
                if parts.len() < 2 {
                    continue;
                }
                let doc_name = parts[0].to_string();
                let data = parts[1];

                if doc_name.ends_with(".txt") {
                    let messages: Vec<String> = data
                        .split("/--/")
                        .map(|s| s.to_string())
                        .collect();
                    docs.insert(doc_name, Documento::Texto(messages));
                } else if doc_name.ends_with(".xlsx") {
                    let calc_entries: Vec<String> = data
                        .split("/--/")
                        .map(|s| s.to_string())
                        .collect();                    
                    docs.insert(doc_name, Documento::Calculo(calc_entries));
                }
            }
            Err(_) => return Err("unable-to-read-file".to_string()),
        }
    }

    Ok(docs)
}

fn get_all_files_list(file_paths: &[&str]) -> Vec<(String, FileType, String, i32)> {
    let mut files_list = Vec::new();
    for path in file_paths {
        let list = get_files_list(&path.to_string());
        files_list.extend(list);
    }
    files_list
}

fn get_all_files_content(file_paths: &[&str]) -> HashMap<String, Documento> {
    let mut docs = HashMap::new();
    for path in file_paths {
        if let Ok(file_docs) = get_file_content_workspace(&path.to_string()) {
            for (nombre, doc) in file_docs {
                docs.insert(nombre, doc);
            }
        }
    }
    docs
}
