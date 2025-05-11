extern crate gtk4;
extern crate relm4;

use self::gtk4::prelude::{OrientableExt, WidgetExt};
use self::relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};
use super::list_files::ListFiles;
use components::types::FileType;

#[derive(Debug)]
pub struct FilesManager {
    file_list_cont: Controller<ListFiles>,
}

#[derive(Debug)]
pub enum FilterFiles {
    SelectedFile(String),
}

#[relm4::component(pub)]
impl SimpleComponent for FilesManager {
    type Output = ();
    type Init = ();
    type Input = FilterFiles;

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
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let files_list = vec![
            ("Documento.txt".to_string(), FileType::Text),
            ("Informe.txt".to_string(), FileType::Text),
            ("Presupuesto.xlsx".to_string(), FileType::Sheet),
            ("Datos.xlsx".to_string(), FileType::Sheet),
            ("Notas.txt".to_string(), FileType::Text),
            ("Análisis.xlsx".to_string(), FileType::Sheet),
        ];

        let list_files_cont = ListFiles::builder().launch(files_list).detach();

        let model = FilesManager {
            file_list_cont: list_files_cont,
        };

        let list_box = model.file_list_cont.widget();

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            FilterFiles::SelectedFile(file) => {
                println!("Archivo a entrar desde el padre: {}", file)
            }
        }
    }
}
