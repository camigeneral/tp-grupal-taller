extern crate gtk4;
extern crate relm4;
use self::gtk4::prelude::{
    BoxExt, OrientableExt, TextBufferExt, TextBufferExtManual, TextViewExt, WidgetExt,
};

use self::relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

/// Estructura que representa el modelo del editor de archivos. Contiene información sobre el archivo
/// que se está editando, el contenido del archivo y el estado de cambios manuales en el contenido.
#[derive(Debug)]
pub struct TextEditorModel {
    /// Nombre del archivo que se está editando.
    file_name: String,
    /// Número de colaboradores que están trabajando en el archivo.
    num_contributors: i32,
    /// Contenido del archivo.
    content: String,
    /// Buffer de texto usado para mostrar el contenido en el editor.
    buffer: gtk::TextBuffer,
    /// Indica si el contenido del archivo ha sido modificado manualmente en el editor.
    content_changed_manually: bool,
}

/// Enum que define los posibles mensajes que el editor de archivos puede recibir.
#[derive(Debug)]
pub enum TextEditorMessage {
    ContentAdded(String, i32),
    ContentRemoved(i32, i32),
    UpdateFile(String, i32, String),
    ResetEditor,
    EnterPressed(i32),  // Nuevo mensaje para Enter
    TabPressed(i32),    // Nuevo mensaje para Tab
}

/// Enum que define los posibles mensajes de salida del editor de archivos.
#[derive(Debug)]
pub enum TextEditorOutputMessage {
    /// Mensaje que indica que se debe volver a la vista anterior.
    GoBack,
}

#[relm4::component(pub)]
impl SimpleComponent for TextEditorModel {
    type Input = TextEditorMessage;
    type Output = TextEditorOutputMessage;
    type Init = (String, i32, String);

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 8,
            set_margin_all: 12,
            set_hexpand: true,
            set_vexpand: true,
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
        println!("{:#?}, {:#?}", file_name, content);
        let mut model = TextEditorModel {
            file_name,
            num_contributors,
            content,
            content_changed_manually: false,
            buffer: gtk::TextBuffer::new(None),
        };

        model.buffer = gtk::TextBuffer::builder().text(&model.content).build();

        let sender = sender.clone();

        let sender_insert = sender.clone();
        model
            .buffer
            .connect_insert_text(move |_buffer, iter, text| {
                let offset = iter.offset();
                
                if text == "\n" {
                    sender_insert.input(TextEditorMessage::EnterPressed(offset));
                } else if text == "\t" {
                    sender_insert.input(TextEditorMessage::TabPressed(offset));
                } else {
                    sender_insert.input(TextEditorMessage::ContentAdded(
                        text.to_string(),
                        offset,
                    ));
                }
            });

        let sender_delete = sender.clone();
        model
            .buffer
            .connect_delete_range(move |_buffer, start, end| {
                sender_delete.input(TextEditorMessage::ContentRemoved(
                    start.offset(),
                    end.offset(),
                ));
            });

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: TextEditorMessage, _sender: ComponentSender<Self>) {
        match message {
            TextEditorMessage::ContentAdded(new_text, offset) => {
                println!("Texto añadido: '{}' en posición: {}", new_text, offset);
            }
            TextEditorMessage::ContentRemoved(start_offset, end_offset) => {
                println!("Texto eliminado desde: {} hasta: {}", start_offset, end_offset);
            }
            TextEditorMessage::EnterPressed(offset) => {
                println!("¡Enter presionado! en posición: {}", offset);
                // Aquí puedes agregar tu lógica específica para Enter
            }
            TextEditorMessage::TabPressed(offset) => {
                println!("¡Tab presionado! en posición: {}", offset);
                // Aquí puedes agregar tu lógica específica para Tab
            }
            TextEditorMessage::UpdateFile(file_name, contributors, content) => {
                self.file_name = file_name;
                self.num_contributors = contributors;
                self.content = content;
                self.buffer.set_text(&self.content);
                self.content_changed_manually = true;
            }
            TextEditorMessage::ResetEditor => {
                self.buffer.set_text("");
                self.content.clear();
                self.file_name.clear();
                self.num_contributors = 0;
            }
        }
    }
}