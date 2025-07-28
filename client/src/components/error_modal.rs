extern crate gtk4;
extern crate relm4;
use self::gtk4::prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt, WidgetExt};

use self::relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

#[derive(Debug)]
pub enum ErrorModalMsg {
    Show(String),
    Hide,
}

// Modelo para el modal de error
#[derive(Debug)]
pub struct ErrorModal {
    visible: bool,
    error_message: String,
}

// Componente del modal de error
#[relm4::component(pub)]
impl SimpleComponent for ErrorModal {
    type Input = ErrorModalMsg;
    type Output = ();
    type Init = ();

    view! {
        #[root]
        gtk::Window {
            set_title: Some("Ups! hubo un error"),
            set_modal: true,
            set_resizable: false,
            set_default_size: (400, 200),
            #[watch]
            set_visible: model.visible,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_margin_all: 20,
                set_spacing: 15,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,

                    gtk::Image {
                        set_icon_name: Some("dialog-error"),
                        set_pixel_size: 48,
                    },
                },

                gtk::ScrolledWindow {
                    set_hexpand: true,
                    set_vexpand: true,
                    set_policy: (gtk::PolicyType::Automatic, gtk::PolicyType::Automatic),

                    gtk::Label {
                        #[watch]
                        set_text: &model.error_message,
                        set_wrap: true,
                        set_halign: gtk::Align::Start,
                        set_valign: gtk::Align::Start,
                        set_selectable: true,
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_halign: gtk::Align::End,

                    gtk::Button {
                        set_label: "Cerrar",
                        connect_clicked => ErrorModalMsg::Hide,
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = ErrorModal {
            visible: false,
            error_message: String::new(),
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            ErrorModalMsg::Show(message) => {
                self.error_message = message;
                self.visible = true;
            }
            ErrorModalMsg::Hide => {
                self.visible = false;
            }
        }
    }
}
