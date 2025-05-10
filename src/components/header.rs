extern crate relm4;
extern crate gtk4;
use self::gtk4::{prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt, PopoverExt, StyleContextExt, WidgetExt}, CssProvider};
use self::relm4::{gtk, Component, ComponentParts, ComponentSender, RelmApp, RelmWidgetExt, SimpleComponent};

#[derive(Debug)]
pub struct HeaderModel {
    is_connected: bool,
}

#[derive(Debug)]
pub enum NavbarInput {
    SetConnectionStatus(bool),
}

#[derive(Debug)]
pub enum NavbarOutput {
    ToggleConnection
}


#[relm4::component(pub)]
impl Component for HeaderModel {
    type Init = ();

    type Input = NavbarInput;
    type Output = NavbarOutput;
    type CommandOutput = ();

    view! {
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
                #[watch]
                set_label: &format!("{}", if model.is_connected { "Conectado" } else { "Desconectado" }),
                add_css_class: "button",
                add_css_class: "connect",                                        
                connect_clicked[sender] => move |_| {
                    sender.output(NavbarOutput::ToggleConnection).unwrap();
                },                
            }
        },
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {

        let model = HeaderModel {             
            is_connected: false
         };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>, _root: &Self::Root) {
        match message {
            NavbarInput::SetConnectionStatus(status) => {
                self.is_connected = status;
            }
        }
    }        
}


