extern crate gtk4;
extern crate relm4;

use self::gtk4::prelude::{BoxExt, ButtonExt, OrientableExt, WidgetExt};
use self::relm4::factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque};
use self::relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};
use components::types::FileType;

#[derive(Debug)]
struct ListItem {
    name: String,
    file_type: FileType,
    content: String,
    qty: u8,
}

#[relm4::factory]
impl FactoryComponent for ListItem {
    type Init = (String, FileType, String, u8);
    type Input = ();
    type Output = FilterFiles;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[name="file_button"]
        gtk::Button {
            set_hexpand: true,
            add_css_class: "file_button",
            set_halign: gtk::Align::Fill,
            set_valign: gtk::Align::Center,
            connect_clicked[sender, name = self.name.clone(), file_type = self.file_type.clone(), content = self.content.clone(), qty = self.qty.clone()] => move |_| {
                sender.output(FilterFiles::FileSelected(name.clone(), file_type.clone(), content.clone(), qty)).unwrap();
            },
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 8,
                gtk::Image {
                    set_icon_name: Some(match self.file_type {
                        FileType::Text => "text-x-generic",
                        FileType::Sheet => "x-office-spreadsheet",
                        _ => "text-x-generic"
                    }),
                    set_pixel_size: 50,
                },
                gtk::Label {
                    set_label: &self.name,
                }
            }
        }
    }
    

    fn init_model(value: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            name: value.0,
            file_type: value.1,
            content: value.2,
            qty: value.3,
        }
    }
}

#[derive(Debug)]
pub struct ListFiles {
    current_filter: FileType,
    all_files: Vec<(String, FileType, String, u8)>,
    visible_files: FactoryVecDeque<ListItem>,
}

#[derive(Debug)]
pub enum FilterFiles {
    Text,
    Sheet,
    Any,
    FileSelected(String, FileType, String, u8),
}

#[relm4::component(pub)]
impl SimpleComponent for ListFiles {
    type Output = FilterFiles;
    type Init = Vec<(String, FileType, String, u8)>;
    type Input = FilterFiles;
    view! {
        #[name="container_2"]
        gtk::Box {
            gtk::Box {
                add_css_class: "header-filter",
                set_orientation: gtk::Orientation::Horizontal,
                set_halign: gtk::Align::Fill,
                gtk::Button {
                    set_hexpand: true,
                    set_label: "Todos",
                    add_css_class: "button",
                    add_css_class: "filter",
                    connect_clicked => FilterFiles::Any,
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
            
            gtk::ScrolledWindow {
                set_hexpand: true,
                set_vexpand: true,                
                #[local_ref]
                files_container -> gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 5,
                    set_margin_top: 10
                }
            }
        }
    }

    fn init(
        files_list: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = ListFiles {
            current_filter: FileType::Any,
            visible_files: FactoryVecDeque::builder()
                .launch_default()
                .forward(sender.input_sender(), |msg| msg),
            all_files: files_list.clone(),
        };

        for file in files_list {
            model.visible_files.guard().push_back(file);
        }

        let files_container = model.visible_files.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            FilterFiles::Any => {
                println!("Filtro: Todos los archivos");
                self.current_filter = FileType::Any;
                self.apply_filter();
            }
            FilterFiles::Text => {
                println!("Filtro: Archivos de texto");
                self.current_filter = FileType::Text;
                self.apply_filter();
            }
            FilterFiles::Sheet => {
                println!("Filtro: Hojas de cálculo");
                self.current_filter = FileType::Sheet;
                self.apply_filter();
            }
            FilterFiles::FileSelected(file_name, file_type, content, qty) => sender
                .output(FilterFiles::FileSelected(
                    file_name, file_type, content, qty,
                ))
                .unwrap(),
        }
    }
}

impl ListFiles {
    fn apply_filter(&mut self) {
        self.visible_files.guard().clear();
        for (name, file_type, content, qty) in &self.all_files {
            if self.current_filter == FileType::Any || *file_type == self.current_filter {
                self.visible_files.guard().push_back((
                    name.clone(),
                    file_type.clone(),
                    content.clone(),
                    qty.clone(),
                ));
            }
        }
    }
}
