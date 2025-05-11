extern crate gtk4;
extern crate relm4;

use self::gtk4::glib::clone;
use self::gtk4::prelude::{
    BoxExt, ButtonExt, OrientableExt, TextBufferExt, TextViewExt, WidgetExt,
};
use self::relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

#[derive(Debug)]
pub struct FileEditor {
    file_name: String,
    qty_contributors: u8,
    content: String,
    buffer: gtk::TextBuffer,
    content_changed_manually: bool,
}

#[derive(Debug)]
pub enum FileEditorMsg {
    TextChanged(String),
    UpdateFile(String, u8, String),
    Reset,
}

#[derive(Debug)]
pub enum FileEditorOutput {
    Back,
}

#[relm4::component(pub)]
impl SimpleComponent for FileEditor {
    type Input = FileEditorMsg;
    type Output = FileEditorOutput;
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
                    sender.output(FileEditorOutput::Back).unwrap();
                },
            },

            #[name="file_label"]
            gtk::Label {
                #[watch]
                set_label: &format!("Editando archivo: {} ({} colaboradores)", model.file_name, model.qty_contributors),
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
        (file_name, qty_contributors, content): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = FileEditor {
            file_name,
            qty_contributors,
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
                sender.input(FileEditorMsg::TextChanged(text));
            }
        ));

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: FileEditorMsg, _sender: ComponentSender<Self>) {
        match message {
            FileEditorMsg::TextChanged(new_text) => {
                self.buffer.set_text(&new_text);
            }
            FileEditorMsg::UpdateFile(file_name, contributors, content) => {
                println!(
                    "Actualizando editor con archivo: {} contribuidos: {}",
                    file_name, contributors
                );
                self.file_name = file_name;
                self.qty_contributors = contributors;
                self.content = content;
                self.buffer.set_text(&self.content);
                self.content_changed_manually = true;
            }
            FileEditorMsg::Reset => {
                self.buffer.set_text("");
                self.content.clear();
                self.file_name.clear();
                self.qty_contributors = 0;
            }
        }
    }
}
