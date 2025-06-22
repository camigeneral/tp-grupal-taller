extern crate gtk4;
extern crate relm4;
use self::gtk4::prelude::{
    BoxExt, OrientableExt, TextBufferExt, TextBufferExtManual, TextViewExt, WidgetExt,
};
use std::rc::Rc;
use std::cell::RefCell;
use self::relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

/// Estructura que representa el modelo del editor de archivos. Contiene información sobre el archivo
/// que se está editando, el contenido del archivo y el estado de cambios manuales en el contenido.
#[derive(Debug, Clone)]
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
    programmatic_update: Rc<RefCell<bool>>, // Shared reference
}

/// Enum que define los posibles mensajes que el editor de archivos puede recibir.
#[derive(Debug)]
pub enum TextEditorMessage {
    ContentAdded(String, i32),
    ContentRemoved(i32, i32),
    UpdateFile(String, i32, String),
    ResetEditor,
    EnterPressed(i32),  
    TabPressed(i32),    
}

/// Enum que define los posibles mensajes de salida del editor de archivos.
#[derive(Debug)]
pub enum TextEditorOutputMessage {
    /// Mensaje que indica que se debe volver a la vista anterior.
    GoBack,
    ContentAdded(String, i32),
    ContentRemoved(i32, i32),
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

        let programmatic_update = Rc::new(RefCell::new(false));

        let mut model = TextEditorModel {
            file_name,
            num_contributors,
            content,
            content_changed_manually: true,
            programmatic_update: programmatic_update.clone(), 

            buffer: gtk::TextBuffer::new(None),
        };

        model.buffer = gtk::TextBuffer::builder().text(&model.content).build();

        let sender = sender.clone();

        let sender_insert = sender.clone();
        let programmatic_update_insert = programmatic_update.clone();

        model
            .buffer
            .connect_insert_text(move |_buffer, iter, text| {
                if *programmatic_update_insert.borrow() {
                    return;
                }

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
        let programmatic_update_delete = programmatic_update.clone();

        model
            .buffer
            .connect_delete_range(move |_buffer, start, end| {
                if *programmatic_update_delete.borrow() {
                    return; 
                }
                
                sender_delete.input(TextEditorMessage::ContentRemoved(
                    start.offset(),
                    end.offset(),
                ));
            });

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: TextEditorMessage, sender: ComponentSender<Self>) {    
        match message {
            TextEditorMessage::EnterPressed(offset) => {
                if !self.content_changed_manually {
                    return;
                }

                let _ = sender.output(TextEditorOutputMessage::ContentAdded(
                    "<enter>".to_string(), offset));                

            }
            TextEditorMessage::TabPressed(offset) => {
                if !self.content_changed_manually {
                    return;
                }

                let _ = sender.output(TextEditorOutputMessage::ContentAdded(
                    "<tab>".to_string(), offset));  
            }
            TextEditorMessage::ContentAdded(mut new_text, offset) => {
                if !self.content_changed_manually  {
                    return;
                }                
                if new_text == " " {
                    new_text = "<space>".to_string();
                }
                let _ = sender.output(TextEditorOutputMessage::ContentAdded(new_text, offset));                
            }
            TextEditorMessage::ContentRemoved(start_offset, end_offset) => {
                if !self.content_changed_manually  {
                    return;
                }
            }
            TextEditorMessage::UpdateFile(file_name, contributors, content) => {
                *self.programmatic_update.borrow_mut() = true;
                self.content_changed_manually = false;
            
                self.file_name = file_name;
                self.num_contributors = contributors;
                self.content = content;
            
                let insert_mark = self.buffer.get_insert();
                let iter = self.buffer.iter_at_mark(&insert_mark);
                let cursor_offset = iter.offset();
                  
                self.buffer.set_text(&self.content);
            
                let new_content_length = self.buffer.char_count();
                
                if new_content_length > 0 {
                    let safe_offset = cursor_offset.min(new_content_length - 1);
                    let new_iter = self.buffer.iter_at_offset(safe_offset);
                    self.buffer.place_cursor(&new_iter);
                } else {
                    let start_iter = self.buffer.start_iter();
                    self.buffer.place_cursor(&start_iter);
                }
            
                self.content_changed_manually = true;
                *self.programmatic_update.borrow_mut() = false;
            }
            TextEditorMessage::ResetEditor => {
                *self.programmatic_update.borrow_mut() = true;
                self.content_changed_manually = false; 

                self.buffer.set_text("");
                self.content.clear();
                self.file_name.clear();
                self.num_contributors = 0;
                
                *self.programmatic_update.borrow_mut() = false;
            }
        }
    }
}