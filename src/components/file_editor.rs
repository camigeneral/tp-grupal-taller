extern crate gtk4;
extern crate relm4;

use self::gtk4::glib::clone;
use self::gtk4::prelude::{
    BoxExt, ButtonExt, OrientableExt, TextBufferExt, TextViewExt, WidgetExt,
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
    /// Mensaje que indica que el contenido del archivo ha cambiado.
    ContentChanged(String),
    /// Mensaje para actualizar el editor con un nuevo archivo, número de colaboradores y contenido.
    UpdateFile(String, u8, String),
    /// Mensaje para resetear el editor de archivos.
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

            gtk::ScrolledWindow {
                set_vexpand: true,
                #[wrap(Some)]
                set_child = &gtk::TextView {
                    set_buffer: Some(&model.buffer),
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

        model.buffer.connect_end_user_action(clone!(
            #[strong]
            sender,
            move |buffer| {
                let text = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), false)
                    .to_string();
                sender.input(FileEditorMessage::ContentChanged(text));
            }
        ));

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: FileEditorMessage, _sender: ComponentSender<Self>) {
        match message {
            FileEditorMessage::ContentChanged(new_text) => {
                self.buffer.set_text(&new_text);
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
