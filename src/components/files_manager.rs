extern crate relm4;
extern crate gtk4;

use self::gtk4::prelude::{ ButtonExt, OrientableExt, WidgetExt};
use self::relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

#[derive(Debug)]
pub struct FilesManager {
    current_view: FileType
}

#[derive(Debug)]
pub enum FileType {
    Text,
    Sheet,
    All    
}
#[derive(Debug)]
pub enum FilterFiles {
    Text,
    Sheet,
    All   
}

#[relm4::component(pub)]
impl SimpleComponent for FilesManager {
    type Output = ();
    type Init = ();
    type Input = FilterFiles;
    view! {
        #[name="body_container"]
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,                
            set_vexpand: true,
            set_hexpand: true,
            set_margin_all: 10,                                      
            #[name="body"]
            gtk::Box {
                add_css_class: "card",
                add_css_class: "content-card",
                set_hexpand: true,
                set_vexpand: true,
                set_valign: gtk::Align::Fill,
                set_orientation: gtk::Orientation::Vertical,
                gtk::Box {
                    add_css_class: "header-filter",                                                        
                    set_orientation: gtk::Orientation::Horizontal,                                                
                    set_halign: gtk::Align::Fill,                                                          
                    gtk::Button {                                          
                        set_hexpand: true,
                        
                        set_label: "Todos",
                        add_css_class: "button",
                        add_css_class: "filter",                                                    
                        connect_clicked => FilterFiles::All,                                
                    },
                    gtk::Button {                                          
                        set_hexpand: true,
                        
                        set_label: "Texto",
                        add_css_class: "button",
                        add_css_class: "filter",                        
                        connect_clicked => FilterFiles::Text,                                
                    },
                    gtk::Button {         
                        set_hexpand: true,
                                                                                            
                        set_label: "Cálculo",
                        add_css_class: "button",
                        add_css_class: "filter",  
                        add_css_class: "last",                        
                        connect_clicked => FilterFiles::Sheet,       
                    },
                },                   
                #[name="files_container"]
                gtk::Box{
                    match model.current_view  {
                        FileType::All => {
                            gtk::Label {
                                set_text: "Todos los archivos!",
                                set_margin_all: 20,
                            }
                        }
                        FileType::Text => {
                            gtk::Label {
                                set_text: "Archivos de texto.",
                                add_css_class: "content",
                                set_margin_all: 20,
                            }
                        }                                
                        FileType::Sheet => {
                            gtk::Label {
                                set_text: "Archivos de calculo.",
                                add_css_class: "content",
                                set_margin_all: 20,
                            }
                        }
                    }
                },  
                },                                                                  
            }            
    }


    fn init(
            _init: Self::Init,
            root: Self::Root,
            _sender: ComponentSender<Self>,
        ) -> ComponentParts<Self> {

        let model = FilesManager {
            current_view: FileType::All
        };


        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
    
    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            FilterFiles::All => self.current_view = FileType::All,
            FilterFiles::Text => self.current_view = FileType::Text,
            FilterFiles::Sheet => self.current_view = FileType::Sheet
        }
    }
}

