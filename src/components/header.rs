extern crate relm4;
extern crate gtk4;
use self::gtk4::prelude::{BoxExt, ButtonExt, OrientableExt, PopoverExt, WidgetExt};
use self::relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

#[derive(Debug)]
pub struct HeaderModel {
    is_connected: bool,
    popover: Option<gtk::Popover>,
}

#[derive(Debug)]
pub enum NavbarInput {
    SetConnectionStatus(bool),
    TogglePopover, 
    CreateTextSheet,
    CreateSpreadsheet
}

#[derive(Debug)]
pub enum NavbarOutput {
    ToggleConnection
}


#[relm4::component(pub)]
impl SimpleComponent for HeaderModel {
    type Init = ();

    type Input = NavbarInput;
    type Output = NavbarOutput;    

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
            },
            gtk::Box {
                #[name="new_file_button"]
                gtk::Button {
                    set_margin_all: 10,
                    set_margin_top: 50,
                    add_css_class: "new-file",
                    add_css_class: "button",
                    set_label: "Nuevo Archivo",
                    connect_clicked => NavbarInput::TogglePopover,
                },
                #[name="popover"]
                gtk::Popover {
                    set_has_arrow: true,
                    set_autohide: true,
                    set_position: gtk::PositionType::Bottom,                                                                                                
                    #[name="popover_content"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 5,
                        gtk::Button {
                            set_label: "Hoja de texto",
                            connect_clicked => NavbarInput::CreateTextSheet,
                        },
                        gtk::Button {
                            set_label: "Hoja de cálculo",
                            connect_clicked => NavbarInput::CreateSpreadsheet,
                        }
                    },                                
                }
            }            
        },
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {

        let mut model = HeaderModel {             
            is_connected: false,
            popover: None,
         };

        let widgets = view_output!();
        model.popover = Some(widgets.popover.clone());
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            NavbarInput::SetConnectionStatus(status) => {
                self.is_connected = status;
            }
            NavbarInput::TogglePopover => {
                if let Some(popover) = &self.popover {
                    popover.popup();
                }
            }
            NavbarInput::CreateTextSheet => {
                if let Some(popover) = &self.popover {
                    popover.popdown();
                }
                println!("Crear hoja de texto");
            }
            NavbarInput::CreateSpreadsheet => {
                if let Some(popover) = &self.popover {
                    popover.popdown();
                }
                println!("Crear hoja de cálculo");
            }
        }
    }        
}


