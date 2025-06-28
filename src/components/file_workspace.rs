extern crate gtk4;
extern crate relm4;

use self::gtk4::prelude::{OrientableExt, WidgetExt};
use self::relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};
use super::file_editor::FileEditorModel;
use super::list_files::FileListView;
use crate::components::file_editor::FileEditorOutputMessage;
use crate::components::structs::document_value_info::DocumentValueInfo;
use crate::document::Documento;
use components::file_editor::FileEditorMessage;
use components::list_files::FileFilterAction;
use components::types::FileType;
use std::collections::HashMap;

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
    OpenFile(String, String, FileType, String),
    /// Mensaje para ignorar una acción.
    Ignore,
    /// Mensaje para cerrar el editor de archivos.
    CloseEditor,
    SubscribeFile(String),
    ReloadFiles,
    ContentAdded(DocumentValueInfo),

    UpdateFile(DocumentValueInfo),
    ContentAddedSpreadSheet(DocumentValueInfo),
    UpdateFilesList(Vec<(String, FileType)>),
}

#[derive(Debug)]
pub enum FileWorkspaceOutputMessage {
    SubscribeFile(String),
    UnsubscribeFile(String),
    ContentAdded(DocumentValueInfo),
    ContentAddedSpreadSheet(DocumentValueInfo),
    FilesLoaded,
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
        let list_files_cont: Controller<FileListView> = FileListView::builder().launch(()).forward(
            sender.input_sender(),
            |msg: crate::components::list_files::FileFilterAction| match msg {
                crate::components::list_files::FileFilterAction::SelectFile(file, _file_type) => {
                    FileWorkspaceMsg::SubscribeFile(file)
                }
                _ => FileWorkspaceMsg::Ignore,
            },
        );
        let editor_file_cont = FileEditorModel::builder()
            .launch(("".to_string(), 0, "".to_string()))
            .forward(
                sender.input_sender(),
                |msg: FileEditorOutputMessage| match msg {
                    FileEditorOutputMessage::GoBack => FileWorkspaceMsg::CloseEditor,
                    FileEditorOutputMessage::ContentAdded(doc_info) => {
                        FileWorkspaceMsg::ContentAdded(doc_info)
                    }
                    FileEditorOutputMessage::ContentAddedSpreadSheet(doc_info) => {
                        FileWorkspaceMsg::ContentAddedSpreadSheet(doc_info)
                    }
                },
            );

        let model = FileWorkspace {
            file_list_ctrl: list_files_cont,
            file_editor_ctrl: editor_file_cont,
            editor_visible: false,
            current_file: "".to_string(),
            files: HashMap::new(),
            current_file_type: FileType::Text,
        };

        let list_box_widget = model.file_list_ctrl.widget();
        let editor_widget = model.file_editor_ctrl.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            FileWorkspaceMsg::ContentAddedSpreadSheet(mut doc_info) => {
                doc_info.file = self.current_file.clone();
                let _ = sender.output(FileWorkspaceOutputMessage::ContentAddedSpreadSheet(
                    doc_info
                ));
            }
            FileWorkspaceMsg::ContentAdded(doc_info) => {
                let _ = sender.output(FileWorkspaceOutputMessage::ContentAdded(doc_info));
            }

            FileWorkspaceMsg::SubscribeFile(file) => {
                sender
                    .output(FileWorkspaceOutputMessage::SubscribeFile(file.clone()))
                    .unwrap();
            }

            FileWorkspaceMsg::OpenFile(file, qty_subs, file_type, content) => {
                self.current_file = file.clone();
                self.current_file_type = file_type.clone();

                let mut items: Vec<String> = content.split(',').map(|s| s.to_string()).collect();
                let qty = qty_subs.parse::<i32>().unwrap_or(0);

                match file_type {
                    FileType::Text => {
                        let text_content = items.join("\n");
                        self.files.insert(
                            (file.clone(), file_type.clone()),
                            Documento::Texto(items.clone()),
                        );
                        self.file_editor_ctrl
                            .sender()
                            .send(FileEditorMessage::UpdateFile(
                                file,
                                qty,
                                text_content,
                                file_type,
                            ))
                            .unwrap();
                    }
                    FileType::Sheet => {
                        while items.len() < 100 {
                            items.push(String::new());
                        }
                        self.files.insert(
                            (file.clone(), file_type.clone()),
                            Documento::Calculo(items.clone()),
                        );
                        let sheet_content = items.join("\n");
                        self.file_editor_ctrl
                            .sender()
                            .send(FileEditorMessage::UpdateFile(
                                file,
                                qty,
                                sheet_content,
                                file_type,
                            ))
                            .unwrap();
                    }
                    _ => {}
                }

                self.editor_visible = true;
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

            FileWorkspaceMsg::UpdateFile(doc_info) => {
                let file_type = if doc_info.file.ends_with(".xlsx") {
                    FileType::Sheet
                } else {
                    FileType::Text
                };
                let mut val = doc_info.value.trim_end_matches('\r').to_string();
                let file_editor_sender = self.file_editor_ctrl.sender().clone();

                if let Some(doc) = self
                    .files
                    .get_mut(&(doc_info.file.clone(), file_type.clone()))
                {
                    if doc_info.index >= 0 {
                        let parsed_index = doc_info.index as usize;
                        match doc {
                            Documento::Calculo(data) => {
                                if parsed_index < data.len() {
                                    data[parsed_index] = val.clone();
                                } else {
                                    while data.len() <= parsed_index {
                                        data.push(String::new());
                                    }
                                    data[parsed_index] = val.clone();
                                }
                            }
                            Documento::Texto(lines) => {
                                if parsed_index < lines.len() {
                                    lines[parsed_index] = val.clone();
                                } else {
                                    while lines.len() < parsed_index {
                                        lines.push(String::new());
                                    }
                                    lines.insert(parsed_index, val.clone());
                                }
                                val = lines.join("\n");
                            }
                        }

                        file_editor_sender
                            .send(FileEditorMessage::UpdateFileContent(
                                doc_info.file.clone(),
                                doc_info.index,
                                val,
                                file_type.clone(),
                            ))
                            .unwrap();
                    }
                }
            }

            FileWorkspaceMsg::UpdateFilesList(archivos_tipos) => {
                // Limpiar y actualizar self.files solo con los nombres y tipos
                for (name, tipo) in &archivos_tipos {
                    let doc = if *tipo == FileType::Sheet {
                        Documento::Calculo(vec![])
                    } else {
                        Documento::Texto(vec![])
                    };
                    self.files.insert((name.clone(), tipo.clone()), doc);
                }
                self.file_list_ctrl
                    .sender()
                    .send(FileFilterAction::UpdateFiles(archivos_tipos))
                    .unwrap();
                sender
                    .output(FileWorkspaceOutputMessage::FilesLoaded)
                    .unwrap();
            }

            _ => {}
        }
    }
}
