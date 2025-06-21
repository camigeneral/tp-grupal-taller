extern crate gtk4;
extern crate relm4;
use self::gtk4::prelude::{ButtonExt, PopoverExt, WidgetExt, OrientableExt, BoxExt, EditableExt};
use self::relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};

/// Modelo que representa la barra de navegación (navbar). Gestiona el estado de la conexión y
/// el popover que permite crear nuevos documentos.
#[derive(Debug)]
pub struct NavbarModel {
    /// Indica si el sistema está conectado o no.
    is_connected: bool,
    /// Indica si el usuario esta
    username: String,

    file_name: String,

    /// Popover que contiene las opciones para crear nuevos documentos.
    new_file_popover: Option<gtk::Popover>,
}

/// Enum que define los diferentes mensajes que puede recibir el componente `NavbarModel`.
/// Permite cambiar el estado de conexión, mostrar el popover para nuevos archivos, y crear documentos.
#[derive(Debug)]
pub enum NavbarMsg {
    /// Mensaje para establecer el estado de conexión.
    SetConnectionStatus(bool),
    /// Mensaje para alternar la visibilidad del popover para nuevos archivos.
    ToggleNewFilePopover,
    /// Mensaje para crear un documento de tipo texto.
    CreateTextDocument,
    /// Mensaje para crear un documento de tipo hoja de cálculo.
    CreateSpreadsheetDocument,

    SetLoggedInUser(String),
    SetFileName(String),
}

/// Enum que define las salidas posibles del componente `NavbarModel`.
/// Actualmente solo existe un mensaje para solicitar el cambio de estado de conexión.
#[derive(Debug)]
pub enum NavbarOutput {
    /// Solicita alternar el estado de conexión.
    ToggleConnectionRequested,
    CreateFileRequested(String, String),
}

#[relm4::component(pub)]
impl SimpleComponent for NavbarModel {
    type Init = ();

    type Input = NavbarMsg;
    type Output = NavbarOutput;

    view! {
        #[name="header"]
        gtk::HeaderBar {
            set_show_title_buttons: true,

            pack_start = &gtk::Label {
                #[watch]
                set_label: &(model.username),
                add_css_class: "username"
            },
            #[wrap(Some)]
            set_title_widget = &gtk::Box {
                #[name="new_file_button"]
                gtk::Button {
                    add_css_class: "new-file",
                    add_css_class: "button",
                    set_label: "Nuevo Archivo",
                    connect_clicked => NavbarMsg::ToggleNewFilePopover,
                },
                #[name="new_file_popover"]
                gtk::Popover {
                    set_has_arrow: true,
                    set_autohide: true,
                    set_position: gtk::PositionType::Bottom,
                    #[name="popover_content"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 5,
                        gtk::Label {
                            set_label: "Nombre del archivo:",
                        },
                        #[name = "file_name"]
                        gtk::Entry {
                            connect_changed[sender] => move |entry| {
                                sender.input(NavbarMsg::SetFileName(entry.text().to_string()));
                            }
                        },
                        gtk::Button {
                            set_label: "Hoja de texto",
                            connect_clicked => NavbarMsg::CreateTextDocument,
                        },
                        gtk::Button {
                            set_label: "Hoja de cálculo",
                            connect_clicked => NavbarMsg::CreateSpreadsheetDocument	,
                        }
                    },
                },
                #[watch]
                set_visible: model.is_connected,
            },

            pack_end = &gtk::Button {
                #[watch]
                set_label: &(if model.is_connected { "Cerrar sesión" } else { "Conectarse" }),
                connect_clicked[sender] => move |_| {
                    sender.output(NavbarOutput::ToggleConnectionRequested).unwrap();
                },
                #[watch]
                set_visible: model.is_connected,
            },
         },
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = NavbarModel {
            is_connected: false,
            new_file_popover: None,
            username: "".to_string(),
            file_name: "".to_string(),
        };

        let widgets = view_output!();
        model.new_file_popover = Some(widgets.new_file_popover.clone());
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            NavbarMsg::SetConnectionStatus(status) => {
                self.is_connected = status;
            }
            NavbarMsg::ToggleNewFilePopover => {
                if let Some(popover) = &self.new_file_popover {
                    popover.popup();
                }
            }
            NavbarMsg::CreateTextDocument => {
                if let Some(popover) = &self.new_file_popover {
                    popover.popdown();
                }
                if self.file_name.trim().is_empty() {
                    println!("El nombre del archivo es obligatorio.");
                    return;
                }
                let file_id = format!("{}.txt", self.file_name.trim());
                sender
                    .output(NavbarOutput::CreateFileRequested(
                        file_id,
                        "".to_string(),
                    ))
                    .unwrap();
            }
            NavbarMsg::SetFileName(file_name) => { self.file_name = file_name }
            NavbarMsg::CreateSpreadsheetDocument => {
                if let Some(popover) = &self.new_file_popover {
                    popover.popdown();
                }
                if self.file_name.trim().is_empty() {
                    println!("El nombre del archivo es obligatorio.");
                    return;
                }
                let file_id = format!("{}.xlsx", self.file_name.trim());
                sender
                    .output(NavbarOutput::CreateFileRequested(
                        file_id,
                        "".to_string(),
                    ))
                    .unwrap();
            }
            NavbarMsg::SetLoggedInUser(username) => {
                self.username = username;
            }
        }
    }
}
