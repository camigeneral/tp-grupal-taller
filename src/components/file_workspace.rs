extern crate gtk4;
extern crate relm4;

use crate::components::file_editor::FileEditorOutputMessage;

use self::gtk4::prelude::{OrientableExt, WidgetExt};
use self::relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};
use super::file_editor::FileEditorModel;
use super::list_files::FileListView;
use components::file_editor::FileEditorMessage;
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
        let files_list = vec![
            (
                "Documento.txt".to_string(),
                FileType::Text,
                "Contenido del archivo Documento".to_string(),
                6,
            ),
            (
                "Informe.txt".to_string(),
                FileType::Text,
                "Contenido del archivo Informe".to_string(),
                8,
            ),
            (
                "Presupuesto.xlsx".to_string(),
                FileType::Sheet,
                "Contenido del archivo de calculo Presupuesto".to_string(),
                10,
            ),
            (
                "Datos.xlsx".to_string(),
                FileType::Sheet,
                "Contenido del archivo de calculo Datos".to_string(),
                3,
            ),
            (
                "Notas.txt".to_string(),
                FileType::Text,
                "Contenido del archivo de Notas".to_string(),
                1,
            ),
            (
                "Análisis.xlsx".to_string(),
                FileType::Sheet,
                "Contenido del archivo de calculo Analisis".to_string(),
                2,
            ),
        ];

        let list_files_cont = FileListView::builder().launch(files_list).forward(
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
        };

        let list_box_widget = model.file_list_ctrl.widget();
        let editor_widget = model.file_editor_ctrl.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            FileWorkspaceMsg::OpenFile(file, content, qty) => {
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
            _ => {}
        }
    }
}
