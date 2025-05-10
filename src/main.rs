extern crate relm4;
extern crate gtk4;
mod components;
use components::header::{HeaderModel, NavbarInput};
use components::files_manager::FilesManager;
use gtk4::{prelude::{BoxExt, GtkWindowExt, OrientableExt, WidgetExt}, CssProvider};

use relm4::{gtk, Component, ComponentParts, ComponentSender, Controller, RelmApp, RelmWidgetExt, SimpleComponent, ComponentController};

struct AppModel {    
    header_cont: Controller<HeaderModel>,
    files_manager_cont: Controller<FilesManager>
}

#[derive(Debug)]
enum AppMsg {
    Connect,
    Noop    
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
                append: model.files_manager_cont.widget()                
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
        let files_manager_model = FilesManager::builder().launch(()).forward(
            sender.input_sender(), |msg: () | match msg {
                _ => AppMsg::Noop
            }            
        );

        let model = AppModel {             
            header_cont: header_model,
            files_manager_cont: files_manager_model
         };

        let widgets = view_output!();
    
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {            
            AppMsg::Connect => {
                self.header_cont.sender().send(NavbarInput::SetConnectionStatus(true)).unwrap();
            },   
            AppMsg::Noop => {}         
        }
    }    
}

fn main() {

    let app = RelmApp::new("relm4.test.simple");    
    
    app.run::<AppModel>(());
}