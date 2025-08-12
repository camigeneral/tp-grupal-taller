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
use components::file_editor::FileEditorMessage;
use components::list_files::FileFilterAction;
use components::types::FileType;
use rusty_docs::document::Document;
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

    files: HashMap<(String, FileType), Document>,
}

/// Enum que define los diferentes mensajes que puede recibir el componente `FileWorkspace`.
/// Permite abrir un archivo, cerrar el editor o ignorar un mensaje.
#[allow(dead_code)]
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
    AddFile(String, FileType),

    UpdateFile(DocumentValueInfo),
    SendPrompt(DocumentValueInfo),
    UpdateLLMFile(String, usize, usize, String),
    ContentAddedSpreadSheet(DocumentValueInfo),
    UpdateFilesList(Vec<(String, FileType)>),
    UpdateAllFileData(String, Vec<String>),
    UpdateLineFile(String, DocumentValueInfo),
}

#[derive(Debug)]
pub enum FileWorkspaceOutputMessage {
    SubscribeFile(String),
    UnsubscribeFile(String),
    ContentAdded(DocumentValueInfo),
    ContentAddedSpreadSheet(DocumentValueInfo),
    SendPrompt(DocumentValueInfo),
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
                    FileEditorOutputMessage::SendPrompt(doc) => FileWorkspaceMsg::SendPrompt(doc),
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
                    doc_info,
                ));
            }
            FileWorkspaceMsg::ContentAdded(doc_info) => {
                let _ = sender.output(FileWorkspaceOutputMessage::ContentAdded(doc_info));
            }
            FileWorkspaceMsg::SendPrompt(doc_info) => {
                let _ = sender.output(FileWorkspaceOutputMessage::SendPrompt(doc_info));
            }
            FileWorkspaceMsg::SubscribeFile(file) => {
                sender
                    .output(FileWorkspaceOutputMessage::SubscribeFile(file.clone()))
                    .unwrap();
            }

            FileWorkspaceMsg::OpenFile(file, qty_subs, file_type, content) => {
                self.current_file = file.clone();
                self.current_file_type = file_type.clone();

                let qty = qty_subs.parse::<i32>().unwrap_or(0);

                match file_type {
                    FileType::Text => {
                        let items: Vec<String> = content
                            .split("<enter>")
                            .map(|s| decode_text(s.to_string()))
                            .collect();
                        let text_content = items.join("\n");
                        self.files.insert(
                            (file.clone(), file_type.clone()),
                            Document::Text(items.clone()),
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
                        let mut items: Vec<String> = content
                            .split(",")
                            .map(|s| decode_text(s.to_string()))
                            .collect();
                        while items.len() < 100 {
                            items.push(String::new());
                        }
                        self.files.insert(
                            (file.clone(), file_type.clone()),
                            Document::Spreadsheet(items.clone()),
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

            FileWorkspaceMsg::AddFile(file_name, file_type) => {
                let doc = if file_type == FileType::Sheet {
                    Document::Spreadsheet(vec!["".to_string(); 100])
                } else {
                    Document::Text(vec!["".to_string()])
                };
                self.files
                    .insert((file_name.clone(), file_type.clone()), doc);

                let archivos_tipos: Vec<(String, FileType)> = self
                    .files
                    .keys()
                    .map(|(name, tipo)| (name.clone(), tipo.clone()))
                    .collect();
                self.file_list_ctrl
                    .sender()
                    .send(FileFilterAction::UpdateFiles(archivos_tipos))
                    .unwrap();
            }

            FileWorkspaceMsg::UpdateAllFileData(file, content) => {
                let file_type = if file.ends_with(".xlsx") {
                    FileType::Sheet
                } else {
                    FileType::Text
                };

                let file_editor_sender = self.file_editor_ctrl.sender().clone();
                if let Some(doc) = self.files.get_mut(&(file.clone(), file_type.clone())) {
                    if let Document::Text(_lines) = doc {
                        self.files.insert(
                            (file.clone(), file_type.clone()),
                            Document::Text(content.clone()),
                        );
                    }
                    let val = content.join("\n");

                    file_editor_sender
                        .send(FileEditorMessage::UpdateFileContent(
                            file.clone(),
                            0,
                            val,
                            file_type.clone(),
                        ))
                        .unwrap();
                }
            }

            FileWorkspaceMsg::UpdateLLMFile(document, line, offset, content) => {
                let file_type = if document.ends_with(".xlsx") {
                    FileType::Sheet
                } else {
                    FileType::Text
                };

                let file_editor_sender = self.file_editor_ctrl.sender().clone();
                if let Some(doc) = self.files.get_mut(&(document.clone(), file_type.clone())) {
                    let mut new_content = String::new();

                    if let Document::Text(ref mut doc_lines) = doc {
                        let llm_parsed_content = content.replace("<enter>", "<space>");
                        println!("Insertado al final o al principio en documento '{}' en línea {}, offset {}: {}, cantidad de lieas {}", document, line, offset, llm_parsed_content,  doc_lines.len());
                        if line < doc_lines.len() {
                            let original_line_decoded = decode_text(doc_lines[line].to_string());
                            let parsed_content = decode_text(llm_parsed_content.to_string());
                            let mut new_line = String::new();

                            if original_line_decoded.trim().is_empty() {
                                new_line.push_str(&parsed_content);
                                new_content.push('\n');
                                doc_lines[line] = parse_text(new_line);
                            } else {
                                let byte_offset = original_line_decoded
                                    .char_indices()
                                    .nth(offset)
                                    .map(|(i, _)| i)
                                    .unwrap_or(original_line_decoded.len());

                                let before = &original_line_decoded[..byte_offset];
                                let after = &original_line_decoded[byte_offset..];

                                new_line.push_str(before);

                                if !after.starts_with(&parsed_content) {
                                    if !before.trim().is_empty() {
                                        new_line.push(' ');
                                    }
                                    new_line.push_str(&parsed_content);
                                    if !after.starts_with(' ') {
                                        new_line.push(' ');
                                    }
                                }

                                new_line.push_str(after);
                                doc_lines[line] = parse_text(new_line);
                            }
                        } else {
                            let parsed_content = &decode_text(llm_parsed_content.to_string());
                            let mut new_line = String::new();

                            new_line.push_str(parsed_content);
                            new_line.push(' ');
                            new_line = parse_text(new_line);
                            doc_lines.push(new_line);
                            println!("Insertado al final o al principio en documento '{}' en línea {}, offset {}: {}", document, line, offset, llm_parsed_content);
                        }
                        new_content = doc_lines.join("\n");
                    }

                    let mut document_info = DocumentValueInfo::new(new_content, line as i32);
                    document_info.decode_text();

                    file_editor_sender
                        .send(FileEditorMessage::UpdateFileContent(
                            document.clone(),
                            line as i32,
                            document_info.value.clone(),
                            file_type.clone(),
                        ))
                        .unwrap();
                }
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
                            Document::Spreadsheet(data) => {
                                if parsed_index < data.len() {
                                    data[parsed_index] = val.clone();
                                } else {
                                    while data.len() <= parsed_index {
                                        data.push(String::new());
                                    }
                                    data[parsed_index] = val.clone();
                                }
                            }
                            Document::Text(lines) => {
                                let val_clone = val.clone();
                                let splited_val = val_clone.split("<enter>").collect::<Vec<_>>();
                                lines[parsed_index] =
                                    decode_text(splited_val[0].to_string().clone());

                                let second_value = if (splited_val.len() <= 1)
                                    || splited_val[1].to_string().is_empty()
                                {
                                    "<enter>".to_string()
                                } else {
                                    splited_val[1].to_string()
                                };
                                lines.insert(parsed_index + 1, decode_text(second_value.clone()));

                                val = lines.iter().enumerate().fold(
                                    String::new(),
                                    |mut acc, (i, line)| {
                                        if i > 0 && line != "\n" {
                                            acc.push('\n');
                                        }
                                        acc.push_str(&decode_text(line.clone()));
                                        acc
                                    },
                                );
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
                        Document::Spreadsheet(vec![])
                    } else {
                        Document::Text(vec![])
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

pub fn parse_text(value: String) -> String {
    let val = value.clone();
    let mut value_clone = if value.trim_end_matches('\n').is_empty() {
        "<delete>".to_string()
    } else {
        val.replace('\n', "<enter>")
    };
    value_clone = value_clone.replace(' ', "<space>");
    value_clone
}

pub fn decode_text(value: String) -> String {
    let value_clone = value.clone();
    value_clone
        .replace("<space>", " ")
        .replace("<enter>", "\n")
        .replace("<delete>", "")
}
