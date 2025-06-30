extern crate gtk4;
extern crate relm4;
use self::gtk4::prelude::{PopoverExt, WidgetExt};
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
                #[watch]
                set_visible: model.is_connected,
            },
         },
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = NavbarModel {
            is_connected: false,
            new_file_popover: None,
            username: "".to_string(),
            file_name: "".to_string(),
        };

        let widgets = view_output!();
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
                    return;
                }
                let file_id = format!("{}.txt", self.file_name.trim());
                if let Err(_e) = sender.output(NavbarOutput::CreateFileRequested(file_id, "".to_string())) {
                    eprintln!("Failed to send message");
                }
            }
            NavbarMsg::SetFileName(file_name) => self.file_name = file_name,
            NavbarMsg::CreateSpreadsheetDocument => {
                if let Some(popover) = &self.new_file_popover {
                    popover.popdown();
                }
                if self.file_name.trim().is_empty() {
                    return;
                }
                let file_id = format!("{}.xlsx", self.file_name.trim());
                if let Err(_e) = sender.output(NavbarOutput::CreateFileRequested(file_id, "".to_string())) {
                    eprintln!("Failed to send message");
                }
            }
            NavbarMsg::SetLoggedInUser(username) => {
                self.username = username;
            }
        }
    }
}
