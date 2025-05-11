extern crate gtk4;
extern crate relm4;

use crate::components::file_editor::FileEditorOutput;

use self::gtk4::prelude::{OrientableExt, WidgetExt};
use self::relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};
use super::file_editor::FileEditor;
use super::list_files::ListFiles;
use components::file_editor::FileEditorMsg;
use components::types::FileType;

#[derive(Debug)]
pub struct FilesManager {
    file_list_cont: Controller<ListFiles>,
    file_editor_cont: Controller<FileEditor>,
    show_editor: bool,
}

#[derive(Debug)]
pub enum File {
    SelectedFile(String, String, u8),
    Noop,
    HideEditor,
}

#[relm4::component(pub)]
impl SimpleComponent for FilesManager {
    type Output = ();
    type Init = ();
    type Input = File;

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
                list_box -> gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    #[watch]
                    set_visible: !model.show_editor
                },

                #[local_ref]
                editor -> gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    #[watch]
                    set_visible: model.show_editor
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

        let list_files_cont = ListFiles::builder().launch(files_list).forward(
            sender.input_sender(),
            |msg: crate::components::list_files::FilterFiles| match msg {
                crate::components::list_files::FilterFiles::FileSelected(
                    file,
                    _file_type,
                    content,
                    qty,
                ) => File::SelectedFile(file, content, qty),
                _ => File::Noop,
            },
        );
        let editor_file_cont = FileEditor::builder()
            .launch(("".to_string(), 0, "".to_string()))
            .forward(sender.input_sender(), |msg: FileEditorOutput| match msg {
                FileEditorOutput::Back => File::HideEditor,
                _ => File::Noop,
            });
        let model = FilesManager {
            file_list_cont: list_files_cont,
            file_editor_cont: editor_file_cont,
            show_editor: false,
        };

        let list_box = model.file_list_cont.widget();
        let editor = model.file_editor_cont.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            File::SelectedFile(file, content, qty) => {
                self.file_editor_cont
                    .sender()
                    .send(FileEditorMsg::UpdateFile(file, qty, content))
                    .unwrap();
                self.show_editor = true;
            }
            File::HideEditor => {
                self.file_editor_cont
                    .sender()
                    .send(FileEditorMsg::Reset)
                    .unwrap();
                self.show_editor = false;
            }
            _ => {}
        }
    }
}
