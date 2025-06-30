extern crate gtk4;
extern crate relm4;
use crate::components::text_editor::TextEditorOutputMessage;

use self::gtk4::prelude::{BoxExt, ButtonExt, OrientableExt, WidgetExt};
use self::relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};
use components::spreadsheet::SpreadsheetOutput;

use crate::components::structs::document_value_info::DocumentValueInfo;
use components::spreadsheet::SpreadsheetModel;
use components::spreadsheet::SpreadsheetMsg;
use components::text_editor::TextEditorMessage;
use components::text_editor::TextEditorModel;
use components::types::FileType;

/// Estructura que representa el modelo del editor de archivos. Contiene información sobre el archivo
/// que se está editando, el contenido del archivo y el estado de cambios manuales en el contenido.
#[derive(Debug)]
pub struct FileEditorModel {
    /// Controlador para el modelo de la hoja de cálculo.
    spreadsheet_ctrl: Controller<SpreadsheetModel>,

    /// Bandera que indica si la hoja de cálculo está visible.
    spreadsheet_visible: bool,

    /// Controlador para el modelo del editor de texto.
    text_editor_ctrl: Controller<TextEditorModel>,

    /// Bandera que indica si el editor de texto está visible.
    text_editor_visible: bool,

    /// Nombre del archivo que se está editando.
    file_name: String,
    /// Número de colaboradores que están trabajando en el archivo.
    num_contributors: i32,
    /// Contenido del archivo.
    content: String,
}

/// Enum que define los posibles mensajes que el editor de archivos puede recibir.
#[derive(Debug)]
pub enum FileEditorMessage {
    ContentAdded(DocumentValueInfo),
    ContentAddedSpreadSheet(DocumentValueInfo),
    UpdateFile(String, i32, String, FileType),
    UpdateFileContent(String, i32, String, FileType),
    ResetEditor,
}

/// Enum que define los posibles mensajes de salida del editor de archivos.
#[derive(Debug)]
pub enum FileEditorOutputMessage {
    ContentAdded(DocumentValueInfo),
    ContentAddedSpreadSheet(DocumentValueInfo),
    /// Mensaje que indica que se debe volver a la vista anterior.
    GoBack,
}

#[relm4::component(pub)]
impl SimpleComponent for FileEditorModel {
    type Input = FileEditorMessage;
    type Output = FileEditorOutputMessage;
    type Init = (String, i32, String);

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 8,
            set_margin_all: 12,
            set_hexpand: true,
            set_vexpand: true,

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 20,
                #[name="back_button"]
                gtk::Button {
                    set_label: "Volver",
                    connect_clicked[sender] => move |_| {
                        if sender.output(FileEditorOutputMessage::GoBack).is_err() {
                            eprintln!("Failed to send message");
                        }
                    },
                    add_css_class: "back-button",
                    add_css_class: "button",
                },

                #[name="file_label"]
                gtk::Label {
                    #[watch]
                    set_label: &format!("Editando archivo: {}", model.file_name),
                    set_xalign: 0.0,
                },
            },
            #[local_ref]
            spreadsheet_widget -> gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                #[watch]
                set_visible: model.spreadsheet_visible
            },

            #[local_ref]
            text_widget -> gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                #[watch]
                set_visible: model.text_editor_visible
            },
        }
    }

    fn init(
        (file_name, num_contributors, content): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let spreadsheet_cont = SpreadsheetModel::builder().launch(()).forward(
            sender.input_sender(),
            |msg| match msg {
                SpreadsheetOutput::GoBack => FileEditorMessage::ResetEditor,
                SpreadsheetOutput::ContentChanged(content) => {
                    FileEditorMessage::ContentAddedSpreadSheet(content)
                }
            },
        );

        let text_editor_cont = TextEditorModel::builder()
            .launch((file_name.clone(), num_contributors, content.clone()))
            .forward(sender.input_sender(), |msg| match msg {
                TextEditorOutputMessage::GoBack => FileEditorMessage::ResetEditor,
                TextEditorOutputMessage::ContentAdded(doc_info) => {
                    FileEditorMessage::ContentAdded(doc_info)
                }
            });

        let model = FileEditorModel {
            file_name,
            num_contributors,
            content,
            spreadsheet_ctrl: spreadsheet_cont,
            text_editor_ctrl: text_editor_cont,
            spreadsheet_visible: false,
            text_editor_visible: true,
        };

        let spreadsheet_widget = model.spreadsheet_ctrl.widget();
        let text_widget = model.text_editor_ctrl.widget();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: FileEditorMessage, sender: ComponentSender<Self>) {
        match message {
            FileEditorMessage::ContentAddedSpreadSheet(doc_info) => {
                let _ = sender.output(FileEditorOutputMessage::ContentAddedSpreadSheet(doc_info));
            }

            FileEditorMessage::ContentAdded(doc_info) => {
                let _ = sender.output(FileEditorOutputMessage::ContentAdded(doc_info));
            }
            FileEditorMessage::UpdateFile(file_name, contributors, content, file_type) => {
                self.file_name = file_name.clone();
                self.num_contributors = contributors;
                self.content = content.clone();

                match file_type {
                    FileType::Text => {
                        self.text_editor_visible = true;
                        self.spreadsheet_visible = false;
                        self.text_editor_ctrl.emit(TextEditorMessage::UpdateFile(
                            file_name.clone(),
                            contributors,
                            content.clone(),
                        ));
                    }
                    FileType::Sheet => {
                        self.text_editor_visible = false;
                        self.spreadsheet_visible = true;
                        let filas: Vec<String> =
                            content.split("\n").map(|s| s.to_string()).collect();

                            if self.spreadsheet_ctrl.sender().send(SpreadsheetMsg::UpdateSheet(file_name.clone(), filas)).is_err() {
                                eprintln!("Failed to send message");
                            }
                    }
                    _ => {
                        self.text_editor_visible = true;
                        self.spreadsheet_visible = false;
                    }
                }
            }
            FileEditorMessage::UpdateFileContent(file_name, index, content, file_type) => {
                self.file_name = file_name.clone();
                self.content = content.clone();

                match file_type {
                    FileType::Text => {
                        self.text_editor_visible = true;
                        self.spreadsheet_visible = false;
                        self.text_editor_ctrl.emit(TextEditorMessage::UpdateFile(
                            file_name.clone(),
                            index,
                            content.clone(),
                        ));
                    }
                    FileType::Sheet => {
                        self.text_editor_visible = false;
                        self.spreadsheet_visible = true;
                        if self.spreadsheet_ctrl
                            .sender()
                            .send(SpreadsheetMsg::UpdateSheetContent(
                                file_name.clone(),
                                index,
                                content,
                            )).is_err()
                        {
                            eprintln!("Failed to send  message");
                        }
                    }
                    _ => {
                        self.text_editor_visible = true;
                        self.spreadsheet_visible = false;
                    }
                }
            }
            FileEditorMessage::ResetEditor => {
                self.text_editor_ctrl.emit(TextEditorMessage::ResetEditor);
            }
        }
    }
}
