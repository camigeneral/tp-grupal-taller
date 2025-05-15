extern crate gtk4;
extern crate relm4;

use self::gtk4::prelude::{
    BoxExt, ButtonExt, OrientableExt, TextBufferExt, TextBufferExtManual, TextViewExt, WidgetExt,
};

use self::relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

/// Estructura que representa el modelo del editor de archivos. Contiene información sobre el archivo
/// que se está editando, el contenido del archivo y el estado de cambios manuales en el contenido.
#[derive(Debug)]
pub struct FileEditorModel {
    /// Nombre del archivo que se está editando.
    file_name: String,
    /// Número de colaboradores que están trabajando en el archivo.
    num_contributors: u8,
    /// Contenido del archivo.
    content: String,
    /// Buffer de texto usado para mostrar el contenido en el editor.
    buffer: gtk::TextBuffer,
    /// Indica si el contenido del archivo ha sido modificado manualmente en el editor.
    content_changed_manually: bool,
}

/// Enum que define los posibles mensajes que el editor de archivos puede recibir.
#[derive(Debug)]
pub enum FileEditorMessage {
    ContentAdded(String, i32),
    ContentRemoved(i32, i32),
    UpdateFile(String, u8, String),
    ResetEditor,
}

/// Enum que define los posibles mensajes de salida del editor de archivos.
#[derive(Debug)]
pub enum FileEditorOutputMessage {
    /// Mensaje que indica que se debe volver a la vista anterior.
    GoBack,
}

#[relm4::component(pub)]
impl SimpleComponent for FileEditorModel {
    type Input = FileEditorMessage;
    type Output = FileEditorOutputMessage;
    type Init = (String, u8, String);

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
                #[name="back"]
                gtk::Button {
                    set_label: "Volver",
                    connect_clicked[sender] => move |_| {
                        sender.output(FileEditorOutputMessage::GoBack).unwrap();
                    },
                },

                #[name="file_label"]
                gtk::Label {
                    #[watch]
                    set_label: &format!("Editando archivo: {} ({} colaboradores)", model.file_name, model.num_contributors),
                    set_xalign: 0.0,
                },
                gtk::Button {
                    set_label: "Desuscribirse",
                    add_css_class: "unsubscribe",
                    add_css_class: "button",
                },
            },
            gtk::ScrolledWindow {
                set_vexpand: true,
                #[wrap(Some)]
                #[name="textview"]
                set_child = &gtk::TextView {
                    set_buffer: Some(&model.buffer),
                    add_css_class: "file-text-area",
                    set_visible: true,

                    set_wrap_mode: gtk::WrapMode::Word,
                    set_overwrite: true,
                },
            }
        }
    }

    fn init(
        (file_name, num_contributors, content): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = FileEditorModel {
            file_name,
            num_contributors,
            content,
            content_changed_manually: false,
            buffer: gtk::TextBuffer::new(None),
        };

        model.buffer = gtk::TextBuffer::builder().text(&model.content).build();

        let sender = sender.clone();

        // Pensar como hacer para que al resetear no mande el mensaje a la api para borrar el contenido

        let sender_insert = sender.clone();
        model
            .buffer
            .connect_insert_text(move |_buffer, iter, text| {
                sender_insert.input(FileEditorMessage::ContentAdded(
                    text.to_string(),
                    iter.offset(),
                ));
            });

        let sender_delete = sender.clone();
        model
            .buffer
            .connect_delete_range(move |_buffer, start, end| {
                sender_delete.input(FileEditorMessage::ContentRemoved(
                    start.offset(),
                    end.offset(),
                ));
            });

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: FileEditorMessage, _sender: ComponentSender<Self>) {
        match message {
            FileEditorMessage::ContentAdded(new_text, offset) => {
                println!("Nuevo caracter: {}, en offset: {}", new_text, offset)
                //Llamado a la api para insertar caracter
            }
            FileEditorMessage::ContentRemoved(start_offset, end_offset) => {
                println!(
                    "Caracter eliminado en start: {}, en end offset: {}",
                    start_offset, end_offset
                )
            }
            FileEditorMessage::UpdateFile(file_name, contributors, content) => {
                println!(
                    "Actualizando editor con archivo: {} contribuidos: {}",
                    file_name, contributors
                );
                self.file_name = file_name;
                self.num_contributors = contributors;
                self.content = content;
                self.buffer.set_text(&self.content);
                self.content_changed_manually = true;
            }
            FileEditorMessage::ResetEditor => {
                self.buffer.set_text("");
                self.content.clear();
                self.file_name.clear();
                self.num_contributors = 0;
            }
        }
    }
}
