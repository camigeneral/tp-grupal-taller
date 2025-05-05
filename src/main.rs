extern crate relm4;
extern crate gtk4;
use gtk4::prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt, WidgetExt};
use relm4::{gtk, Component, ComponentParts, ComponentSender, RelmApp, RelmWidgetExt, SimpleComponent};

struct AppModel {
    counter: u8,
}

#[derive(Debug)]
enum AppMsg {
    Connect,
    Disconnect,
}

#[relm4::component]
impl Component for AppModel {
    type Init = u8;

    type Input = AppMsg;
    type Output = ();
    type CommandOutput = (); 

    view! {
        gtk::Window {
            set_title: Some("Rusty Docs"),
            set_default_width: 800,
            set_default_height: 600,
            #[name="main_container"]
            gtk::Box {            
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 5,
                #[name="header"]
                gtk::Box {
                    set_spacing: 10,
                    set_halign: gtk::Align::End,
                    set_orientation: gtk::Orientation::Horizontal,
                    gtk::Button {
                        set_margin_all:10,
                        set_label: "Conectar",
                    }
                },
                #[name="body_container"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_hexpand: true,
                    set_vexpand: true,
                    #[name="side_bar_container"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_halign: gtk::Align::Start,
                        gtk::Box {
                            set_width_request: 200, 
                            set_margin_all: 10,
                            set_orientation: gtk::Orientation::Vertical,
                            set_halign: gtk::Align::Center,
                            gtk::Button {
                                set_margin_all:10,
                                set_label: "Home",
                            },
                            gtk::Button {
                                set_margin_all:10,
                                set_label: "Mis documentos",
                            },
                            
                            gtk::Button {
                                set_margin_all:10,
                                set_margin_top: 50,
                                set_label: "Nuevo Archivo",
                            }
                        }
                    },                    

                }

            }
        }
    }

    // Initialize the UI.
    fn init(
        counter: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = AppModel { counter };

        // Insert the macro code generation here
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update_cmd(
        &mut self,
        _message: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    )
     {
        // vacío si no usás comandos
    }
}

fn main() {
    let app = RelmApp::new("relm4.test.simple");
    app.run::<AppModel>(0);
}