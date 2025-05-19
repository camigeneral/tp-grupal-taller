extern crate gtk4;
extern crate relm4;

use self::gtk4::prelude::{BoxExt, ButtonExt, OrientableExt, WidgetExt};
use self::relm4::factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque};
use self::relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};
use components::types::FileType;

/// Estructura que representa un ítem de la lista de archivos. Cada ítem contiene el nombre, tipo,
/// contenido y cantidad de un archivo.
#[derive(Debug)]
struct FileListItem {
    /// Nombre del archivo.
    name: String,
    /// Tipo de archivo (texto o hoja de cálculo).
    file_type: FileType,
    /// Contenido del archivo.
    content: String,
    /// Cantidad de elementos relacionados con el archivo.
    qty: u8,
}

/// Estructura que representa la vista de la lista de archivos. Contiene una lista de archivos
/// completa, la lista filtrada según el tipo de archivo y el filtro seleccionado.
#[derive(Debug)]
pub struct FileListView {
    /// Filtro seleccionado para los archivos (todos, texto o cálculo).
    selected_filter: FileType,
    /// Lista completa de archivos.
    all_filles: Vec<(String, FileType, String, u8)>,
    /// Lista filtrada de archivos.
    filtered_files: FactoryVecDeque<FileListItem>,
}

/// Enum que define las acciones disponibles para filtrar o seleccionar archivos en la vista.
#[derive(Debug)]
pub enum FileFilterAction {
    /// Muestra todos los archivos sin ningún filtro.
    ShowAll,
    /// Muestra solo los archivos de tipo texto.
    TextFiles,
    /// Muestra solo los archivos de tipo hoja de cálculo.
    SpreadsheetFiles,
    /// Selecciona un archivo específico.
    SelectFile(String, FileType, String, u8),
    UpdateFiles(Vec<(String, FileType, String, u8)>),
}

#[relm4::component(pub)]
impl SimpleComponent for FileListView {
    type Output = FileFilterAction;
    type Init = Vec<(String, FileType, String, u8)>;
    type Input = FileFilterAction;
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
                    connect_clicked => FileFilterAction::ShowAll,
                },
                gtk::Button {
                    set_hexpand: true,

                    set_label: "Texto",
                    add_css_class: "button",
                    add_css_class: "filter",
                    connect_clicked => FileFilterAction::TextFiles,
                },
                gtk::Button {
                    set_hexpand: true,

                    set_label: "Cálculo",
                    add_css_class: "button",
                    add_css_class: "filter",
                    add_css_class: "last",
                    connect_clicked => FileFilterAction::SpreadsheetFiles,
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
        let mut model = FileListView {
            selected_filter: FileType::Any,
            filtered_files: FactoryVecDeque::builder()
                .launch_default()
                .forward(sender.input_sender(), |msg| msg),
            all_filles: files_list.clone(),
        };

        for file in files_list {
            model.filtered_files.guard().push_back(file);
        }

        let files_container = model.filtered_files.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            FileFilterAction::ShowAll => {
                println!("Filtro: Todos los archivos");
                self.selected_filter = FileType::Any;
                self.update_file_list_based_on_filter();
            }
            FileFilterAction::TextFiles => {
                println!("Filtro: Archivos de texto");
                self.selected_filter = FileType::Text;
                self.update_file_list_based_on_filter();
            }
            FileFilterAction::SpreadsheetFiles => {
                println!("Filtro: Hojas de cálculo");
                self.selected_filter = FileType::Sheet;
                self.update_file_list_based_on_filter();
            }
            FileFilterAction::SelectFile(file_name, file_type, content, qty) => sender
                .output(FileFilterAction::SelectFile(
                    file_name, file_type, content, qty,
                ))
                .unwrap(),

            FileFilterAction::UpdateFiles(new_files) => {
                self.all_filles = new_files;
                self.update_file_list_based_on_filter();
            }
        }
    }
}

impl FileListView {
    fn update_file_list_based_on_filter(&mut self) {
        self.filtered_files.guard().clear();
        for (name, file_type, content, qty) in &self.all_filles {
            if self.selected_filter == FileType::Any || *file_type == self.selected_filter {
                self.filtered_files.guard().push_back((
                    name.clone(),
                    file_type.clone(),
                    content.clone(),
                    (*qty),
                ));
            }
        }
    }
}

#[relm4::factory]
impl FactoryComponent for FileListItem {
    type Init = (String, FileType, String, u8);
    type Input = ();
    type Output = FileFilterAction;
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
                sender.output(FileFilterAction::SelectFile(name.clone(), file_type.clone(), content.clone(), qty)).unwrap();
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
