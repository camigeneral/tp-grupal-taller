extern crate relm4;
extern crate gtk4;
mod components;
use components::header::{HeaderModel, NavbarInput};
use gtk4::{prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt, WidgetExt}, CssProvider};

use relm4::{gtk, Component, ComponentParts, ComponentSender, Controller, RelmApp, RelmWidgetExt, SimpleComponent, ComponentController};

struct AppModel {    
    current_view: String,
    header_cont: Controller<HeaderModel>
}

#[derive(Debug)]
enum AppMsg {
    Connect,    
    ShowHome,
    ShowDocuments,
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();    

    view! {
        gtk::Window {
            set_title: Some("Rusty Docs"),
            set_default_width: 800,
            set_width_request: 800,            
            set_default_height: 600,
            
            
            #[name="main_container"]
            gtk::Box {            
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 5,
                set_margin_all: 10,
                set_hexpand: true,
                set_vexpand: true,
                append: model.header_cont.widget(),

                #[name="body_container"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,                
                    set_vexpand: true,
                    set_hexpand: true,
                    set_margin_all: 10,
                    #[name="side_bar_container"]
                    gtk::Box {
                        add_css_class: "card",
                        add_css_class: "sidebar-card",
                        set_orientation: gtk::Orientation::Vertical,
                        set_halign: gtk::Align::Start,
                        set_margin_end: 15,
                        gtk::Box {                            
                            set_margin_all: 10,
                            set_orientation: gtk::Orientation::Vertical,
                            set_halign: gtk::Align::Center,
                            gtk::Button {
                                set_margin_all: 10,
                                set_label: "Home",
                                add_css_class: "button",
                                connect_clicked => AppMsg::ShowHome
                            },
                            gtk::Button {
                                set_margin_all: 10,
                                set_label: "Mis documentos",
                                add_css_class: "button",
                                connect_clicked => AppMsg::ShowDocuments
                            },                                                        
                        }
                    },                    
                    #[name="body"]
                    gtk::Box {
                        add_css_class: "card",
                        add_css_class: "content-card",
                        set_hexpand: true,
                        set_vexpand: true,
                        set_valign: gtk::Align::Fill,
                        set_orientation: gtk::Orientation::Vertical,
                        add_css_class: "header-filter",                                                        
                            set_orientation: gtk::Orientation::Horizontal,                                                
                            set_halign: gtk::Align::Fill,                                                          
                            gtk::Button {                                          
                                set_hexpand: true,
                                
                                set_label: "Todos",
                                add_css_class: "button",
                                add_css_class: "filter",                                                    
                                connect_clicked => AppMsg::Connect,                                
                            },
                            gtk::Button {                                          
                                set_hexpand: true,
                                
                                set_label: "Texto",
                                add_css_class: "button",
                                add_css_class: "filter",                        
                                connect_clicked => AppMsg::Connect,                                
                            },
                            gtk::Button {         
                                set_hexpand: true,
                                                                                                 
                                set_label: "Cálculo",
                                add_css_class: "button",
                                add_css_class: "filter",  
                                add_css_class: "last",                        
                                connect_clicked => AppMsg::Connect,                                
                            },
                        },
                        
                        #[name="files_container"]
                        gtk::Box{
                            match model.current_view.as_str()  {
                                "Home" => {
                                    gtk::Label {
                                        set_text: "Bienvenido a Home!",
                                        set_margin_all: 20,
                                    }
                                }
                                "Documents" => {
                                    gtk::Label {
                                        set_text: "Aquí están tus documentos.",
                                        add_css_class: "content",
                                        set_margin_all: 20,
                                    }
                                }                                
                                _ => {
                                    gtk::Label {
                                        set_text: "Selecciona una opción del menú.",
                                        add_css_class: "content",
                                        set_margin_all: 20,
                                    }
                                }
                            }
                        },                        
                    }
                }
            }
        }    


    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {

        
        let css_provider = CssProvider::new();
        css_provider.load_from_path("app.css");

        
        gtk4::style_context_add_provider_for_display(
            &gtk4::gdk::Display::default().expect("Could not get default display"),
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );        

        let header_model = HeaderModel::builder().launch(()).forward(
            sender.input_sender(), |msg:components::header::NavbarOutput | match msg {
                _ => AppMsg::Connect
            }            
        );

        let model = AppModel {             
            current_view: "Home".to_string(),
            header_cont: header_model
         };

        let widgets = view_output!();
    
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            AppMsg::ShowHome => self.current_view = "Home".to_string(),
            AppMsg::ShowDocuments => self.current_view = "Documents".to_string(),
            AppMsg::Connect => {
                self.header_cont.sender().send(NavbarInput::SetConnectionStatus(true)).unwrap();
            },
            _ => self.current_view = "Home".to_string()
        }
    }    
}

fn main() {

    let app = RelmApp::new("relm4.test.simple");    
    
    app.run::<AppModel>(());
}