extern crate gtk4;
extern crate relm4;

use self::gtk4::prelude::{BoxExt, GtkWindowExt, OrientableExt, WidgetExt};
use self::relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};

#[derive(Debug)]
pub struct LoadingModalModel {
    is_visible: bool,
}

#[derive(Debug)]
pub enum LoadingModalMsg {
    Show,
    Hide,
}

#[relm4::component(pub)]
impl SimpleComponent for LoadingModalModel {
    type Init = ();
    type Input = LoadingModalMsg;
    type Output = ();

    view! {
        #[root]
        gtk::Window {
            set_modal: true,
            set_resizable: false,
            set_default_size: (400, 200),
            set_decorated: false,
            #[watch]
            set_visible: model.is_visible,
            set_css_classes: &["loading-modal"],

            gtk::Box {
                set_hexpand: true,
                set_vexpand: true,
                set_valign: gtk::Align::Center,
                set_halign: gtk::Align::Center,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 20,
                    set_width_request: 320,
                    set_height_request: 220,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 20,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Center,

                        gtk::Label {
                            set_label: "🤖",
                            set_css_classes: &["ai-icon"],
                            set_halign: gtk::Align::Center,
                        },

                        gtk::Spinner {
                            set_spinning: true,
                            set_size_request: (40, 40),
                            set_halign: gtk::Align::Center,
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 8,
                            set_halign: gtk::Align::Center,

                            gtk::Label {
                                set_label: "Generando contenido",
                                set_css_classes: &["loading-title"],
                                set_halign: gtk::Align::Center,
                            },

                            gtk::Label {
                                set_label: "La IA está trabajando en tu solicitud...",
                                set_css_classes: &["loading-subtitle"],
                                set_halign: gtk::Align::Center,
                            },
                        },
                    }
                }

            }
        }
    }

    fn init(_init: (), root: Self::Root, _sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = LoadingModalModel { is_visible: false };
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            LoadingModalMsg::Show => self.is_visible = true,
            LoadingModalMsg::Hide => self.is_visible = false,
        }
    }
}
