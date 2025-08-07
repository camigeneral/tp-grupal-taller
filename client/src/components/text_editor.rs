extern crate gtk4;
extern crate relm4;

use self::gtk4::prelude::{
    BoxExt, ButtonExt, Cast, EditableExt, EventControllerExt, OrientableExt, TextBufferExt,
    TextViewExt, WidgetExt,
};
use self::relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};
use crate::components::structs::document_value_info::DocumentValueInfo;
use std::cell::RefCell;
use std::rc::Rc;
use std::fmt;

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
    prompt: String,
    programmatic_update: Rc<RefCell<bool>>, // Shared reference
    cursor_position: Rc<RefCell<Option<(i32, i32)>>>, // línea y offset
    selection_mode: SelectionMode,
    prompt_widget: Option<gtk::Entry>,
}

/// Enum que define los posibles mensajes que el editor de archivos puede recibir.
#[derive(Debug)]
pub enum TextEditorMessage {
    ContentAdded(DocumentValueInfo),
    UpdateFile(String, i32, String),
    SetPrompt(String),
    SendPrompt,
    ResetEditor,
    SetSelectionMode(SelectionMode),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectionMode {
    Cursor,
    WholeFile,
}

impl fmt::Display for SelectionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SelectionMode::Cursor => "cursor",
            SelectionMode::WholeFile => "whole-file",
        };
        write!(f, "{}", s)
    }
}

/// Enum que define los posibles mensajes de salida del editor de archivos.
#[allow(dead_code)]
#[derive(Debug)]
pub enum TextEditorOutputMessage {
    /// Mensaje que indica que se debe volver a la vista anterior.
    GoBack,
    ContentAdded(DocumentValueInfo),
    SendPrompt(DocumentValueInfo),
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
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 5,
                #[name = "prompt"]
                gtk::Entry {
                    set_text: &model.prompt,
                    connect_changed[sender] => move |entry| {
                        sender.input(TextEditorMessage::SetPrompt(entry.text().to_string()));
                    }
                },
                #[name = "mode_dropdown"]
                gtk::DropDown::from_strings(&["Todo el archivo", "Posición cursor"]) {
                    set_selected: match model.selection_mode {
                        SelectionMode::WholeFile => 0,
                        SelectionMode::Cursor => 1,
                    },
                    connect_selected_notify[sender] => move |dropdown| {
                        let index = dropdown.selected();
                        let mode = match index {
                            1 => SelectionMode::Cursor,
                            0 => SelectionMode::WholeFile,
                            _ => SelectionMode::Cursor,
                        };
                        sender.input(TextEditorMessage::SetSelectionMode(mode));
                    }
                },

                gtk::Button {
                    set_label: "Generar con IA",
                    connect_clicked[sender] => move |_| {
                        sender.input(TextEditorMessage::SendPrompt);

                    },
                    add_css_class: "back-button",
                    add_css_class: "button",
                },

            },
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Automatic,  // Scroll horizontal automático
                set_vscrollbar_policy: gtk::PolicyType::Automatic,
                #[wrap(Some)]
                #[name="textview"]
                set_child = &gtk::TextView {
                    set_buffer: Some(&model.buffer),
                    add_css_class: "file-text-area",
                    set_visible: true,

                    set_wrap_mode: gtk::WrapMode::None,
                    set_overwrite: false,
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
        let cursor_position = Rc::new(RefCell::new(None));

        let mut model = TextEditorModel {
            file_name,
            num_contributors,
            content,
            prompt: "".to_string(),
            content_changed_manually: true,
            programmatic_update: programmatic_update.clone(),
            cursor_position: cursor_position.clone(),
            buffer: gtk::TextBuffer::new(None),
            selection_mode: SelectionMode::WholeFile,
            prompt_widget: None,
        };

        model.buffer = gtk::TextBuffer::builder().text(&model.content).build();

        let sender = sender.clone();
        let buffer = model.buffer.clone();
        let cursor_position_clone = cursor_position.clone();

        buffer.connect_mark_set(move |_buffer, iter, _mark| {
            let line = iter.line();
            let offset = iter.line_offset();
            *cursor_position_clone.borrow_mut() = Some((line, offset));
        });

        let sender_insert = sender.clone();
        let widgets = view_output!();
        model.prompt_widget = Some(widgets.prompt.clone());

        let key_controller = gtk4::EventControllerKey::new();
        key_controller.connect_key_pressed(move |controller, key, _keycode, _state| {
            if key == gtk4::gdk::Key::Return || key == gtk4::gdk::Key::KP_Enter {
                if let Some(widget) = controller.widget() {
                    if let Ok(text_view) = widget.downcast::<gtk4::TextView>() {
                        let buffer = text_view.buffer();

                        // Usar la posición del cursor que ya tienes guardada
                        if let Some((line_number, cursor_offset)) = *cursor_position.borrow() {
                            println!("line number {line_number}, cursor_offset {cursor_offset}");
                            if let Some(line_start) = buffer.iter_at_line(line_number) {
                                let mut line_end = line_start;
                                line_end.forward_to_line_end();

                                let full_line_content: String =
                                    buffer.text(&line_start, &line_end, false).to_string();
                                let len = full_line_content.chars().count() as i32;
                                let final_string = if cursor_offset == len {
                                    full_line_content
                                } else {
                                    let cursor_pos = cursor_offset as usize;
                                    let before_cursor: String =
                                        full_line_content.chars().take(cursor_pos).collect();
                                    let after_cursor: String =
                                        full_line_content.chars().skip(cursor_pos).collect();

                                    format!("{}\n{}", before_cursor, after_cursor)
                                };

                                let doc_info: DocumentValueInfo =
                                    DocumentValueInfo::new(final_string, line_number);

                                sender_insert.input(TextEditorMessage::ContentAdded(doc_info));
                            }
                        }
                    }
                }
                gtk4::glib::Propagation::Proceed
            } else {
                gtk4::glib::Propagation::Proceed
            }
        });

        widgets.textview.add_controller(key_controller);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: TextEditorMessage, sender: ComponentSender<Self>) {
        match message {
            TextEditorMessage::SendPrompt => {
                println!("{}", self.prompt);
                if self.prompt.is_empty() {
                    return;
                }
                if let Some((line, offset)) = *self.cursor_position.borrow() {
                    let mut document = DocumentValueInfo::new(self.content.clone(), line);
                    document.offset = offset;
                    document.prompt = self.prompt.clone();
                    document.file = self.file_name.clone();
                    document.selection_mode = self.selection_mode.to_string();
                    self.prompt = "".to_string();
                    if let Some(widget) = &self.prompt_widget {
                        widget.set_text("");
                    }                    
                    let _ = sender.output(TextEditorOutputMessage::SendPrompt(document));
                }
            }
            TextEditorMessage::SetSelectionMode(mode) => {
                self.selection_mode = mode;
            }
            TextEditorMessage::SetPrompt(prompt) => {
                self.prompt = prompt;                
            }
            TextEditorMessage::ContentAdded(mut doc_info) => {
                if !self.content_changed_manually {
                    return;
                }
                doc_info.file = self.file_name.clone();
                doc_info.parse_text();
                let _ = sender.output(TextEditorOutputMessage::ContentAdded(doc_info));
            }

            TextEditorMessage::UpdateFile(file_name, contributors, content) => {
                *self.programmatic_update.borrow_mut() = true;
                self.content_changed_manually = false;

                self.file_name = file_name;
                self.num_contributors = contributors;                
                self.content = content;
                self.buffer.set_text(&self.content.to_string());

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
