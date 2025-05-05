extern crate relm4;
extern crate gtk4;
use gtk4::{prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt, StyleContextExt, WidgetExt}, CssProvider};
use relm4::{gtk, Component, ComponentParts, ComponentSender, RelmApp, RelmWidgetExt, SimpleComponent};

struct AppModel {
    counter: u8,
    current_view: String
}

#[derive(Debug)]
enum AppMsg {
    Connect,
    Disconnect,
    ShowHome,
    ShowDocuments,
    ShowNewFile,
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
            set_width_request: 800,            
            set_default_height: 600,
            
            
            #[name="main_container"]
            gtk::Box {            
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 5,
                set_margin_all: 10,
                set_hexpand: true,
                set_vexpand: true,
                #[name="header"]
                gtk::Box {                    
                    add_css_class: "header",
                    set_spacing: 10,
                    
                    set_orientation: gtk::Orientation::Horizontal,                    
                    set_margin_all: 10,  
                    set_halign: gtk::Align::Fill,  
                    gtk::Box {
                        set_hexpand: true,
                    },
                    gtk::Button {                                          
                        set_margin_all: 10,
                        set_label: "Conectar",
                        add_css_class: "button",
                        add_css_class: "connect",                        
                        connect_clicked => AppMsg::Connect,
                        
                    }
                },
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
                            
                            gtk::Button {
                                set_margin_all: 10,
                                set_margin_top: 50,
                                add_css_class: "new-file",
                                add_css_class: "button",
                                set_label: "Nuevo Archivo",
                                connect_clicked => AppMsg::ShowNewFile
                            }
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
                        set_halign: gtk::Align::Fill,
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
                            "NewFile" => {
                                gtk::Label {
                                    set_text: "Crea un nuevo archivo.",
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
                    }
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
        let model = AppModel { counter, current_view: "Home".to_string() };
        let css_provider = CssProvider::new();
        
        // Cargar el CSS directamente como string para mayor facilidad
        // También puedes mantener el css_provider.load_from_path("app.css") si prefieres usar un archivo
        css_provider.load_from_path("app.css");

        // Aplicar el CSS a toda la aplicación
        gtk4::style_context_add_provider_for_display(
            &gtk4::gdk::Display::default().expect("Could not get default display"),
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // Insert the macro code generation here
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>, _root: &Self::Root) {
        match message {
            AppMsg::ShowHome => self.current_view = "Home".to_string(),
            AppMsg::ShowNewFile => self.current_view = "NewFile".to_string(),
            AppMsg::ShowDocuments => self.current_view = "Documents".to_string(),
            AppMsg::Connect => self.current_view = "Documents".to_string(),
            AppMsg::Disconnect => self.current_view = "Documents".to_string()
        }
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
    // Forzar tema claro antes de iniciar la aplicación
    let app = RelmApp::new("relm4.test.simple");    
    
    app.run::<AppModel>(0);
}