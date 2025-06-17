extern crate gtk4;
extern crate relm4;
use self::gtk4::{
    prelude::{BoxExt, ButtonExt, EditableExt, GtkWindowExt, OrientableExt, WidgetExt},
    CssProvider,
};
use crate::components::login::{LoginForm, LoginMsg, LoginOutput};
use app::gtk4::glib::Propagation;
use client::client_run;
use components::file_workspace::{FileWorkspace, FileWorkspaceMsg, FileWorkspaceOutputMessage};
use components::header::{NavbarModel, NavbarMsg, NavbarOutput};
use std::collections::HashMap;
use std::thread;

use std::sync::mpsc::{channel, Sender};

use self::relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};

/// Modelo principal de la aplicación que contiene los controladores de los componentes.
///
/// # Campos
/// - `header_cont`: Controlador para la barra de navegación superior
/// - `files_manager_cont`: Controlador para el área de gestión de archivos
/// - `login_form_cont`: Controlador para el formulario de login
/// - `is_logged_in`: Indica si el usuario ha iniciado sesión
#[derive(Debug)]
pub struct AppModel {
    header_cont: Controller<NavbarModel>,
    files_manager_cont: Controller<FileWorkspace>,
    login_form_cont: Controller<LoginForm>,
    is_logged_in: bool,
    command: String,
    command_sender: Option<Sender<String>>,
    username: String
}

#[derive(Debug)]
pub enum AppMsg {
    Connect,
    Ignore,
    LoginSuccess(String),
    LoginFailure(String),
    Logout,
    CommandChanged(String),
    ExecuteCommand,
    CloseApplication,
    RefreshData,
    CreateFile(String, String),
    SubscribeFile(String),
    PrepareAndExecuteCommand(String, String),
    ManageResponse(String),
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = u16;
    type Input = AppMsg;
    type Output = ();

    view! {
    gtk::Window {
        set_default_width: 800,
        set_width_request: 800,
        set_default_height: 600,
        #[wrap(Some)]
        set_titlebar = model.header_cont.widget(),

        #[name="main_container"]
        gtk::Box {
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 5,
                set_margin_all: 10,
                set_hexpand: true,
                set_vexpand: true,
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 15,
                    gtk::Label {
                        set_label: "Comandos:",
                    },
                    #[name = "command_entry"]
                    gtk::Entry {
                        connect_changed[sender] => move |entry| {
                            sender.input(AppMsg::CommandChanged(entry.text().to_string()));
                        }
                    },
                    gtk::Button {
                        set_label: "Ejecutar",
                        add_css_class: "execute-command",
                        connect_clicked[sender] => move |_| {
                            sender.input(AppMsg::ExecuteCommand);
                        }
                    },
                },

                append: model.files_manager_cont.widget(),
                #[watch]
                set_visible: model.is_logged_in
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_hexpand: true,
                set_vexpand: true,
                append: model.login_form_cont.widget(),
                #[watch]
                set_visible: !model.is_logged_in
            },

        },

    }
    }

    fn init(
        port: Self::Init,
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

        let header_model = NavbarModel::builder().launch(()).forward(
            sender.input_sender(),
            |output| match output {
                NavbarOutput::ToggleConnectionRequested => AppMsg::Connect,
                NavbarOutput::CreateFileRequested(file_id, content) => {
                    AppMsg::CreateFile(file_id, content)
                }
            },
        );

        let files_manager_model = FileWorkspace::builder().launch(()).forward(
            sender.input_sender(),
            |command: FileWorkspaceOutputMessage| match command {
                FileWorkspaceOutputMessage::SubscribeFile(file) => AppMsg::SubscribeFile(file),
            },
        );

        let login_form_model = LoginForm::builder().launch(()).forward(
            sender.input_sender(),
            |output| match output {
                LoginOutput::LoginRequested(username, password) => {
                    let command = format!("AUTH {} {}", username, password);                    
                    AppMsg::PrepareAndExecuteCommand(command, username)
                },
            },
        );

        let mut model = AppModel {
            header_cont: header_model,
            files_manager_cont: files_manager_model,
            login_form_cont: login_form_model,
            is_logged_in: false,
            command: "".to_string(),
            command_sender: None,
            username: "".to_string()
        };

        let sender_clone = sender.clone();

        root.connect_close_request(move |_| {
            sender_clone.input(AppMsg::CommandChanged("CLOSE".to_string()));
            sender_clone.input(AppMsg::ExecuteCommand);
            Propagation::Proceed
        });
        let widgets = view_output!();
        let ui_sender: relm4::Sender<AppMsg> = sender.input_sender().clone();
        let (tx, rx) = channel::<String>();
        let command_sender = Some(tx.clone());
        model.command_sender = command_sender;

        thread::spawn(move || {
            if let Err(e) = client_run(port, rx, Some(ui_sender)) {
                eprintln!("Error al iniciar el cliente: {:?}", e);
            }
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppMsg::Connect => {
                if self.is_logged_in {
                    self.header_cont
                        .sender()
                        .send(NavbarMsg::SetConnectionStatus(true))
                        .unwrap();
                }
            }
            AppMsg::PrepareAndExecuteCommand(command, username) => {
                self.command = command;
                self.username = username;
                sender.input(AppMsg::ExecuteCommand);
            }
            AppMsg::Ignore => {}
            AppMsg::LoginSuccess(username) => {
                self.header_cont
                    .sender()
                    .send(NavbarMsg::SetLoggedInUser(username))
                    .unwrap();

                let files_manager_cont_sender = self.files_manager_cont.sender().clone();

                files_manager_cont_sender
                    .send(FileWorkspaceMsg::ReloadFiles)
                    .unwrap();
                self.header_cont
                    .sender()
                    .send(NavbarMsg::SetConnectionStatus(true))
                    .unwrap();
                self.files_manager_cont.emit(FileWorkspaceMsg::ReloadFiles);
                self.is_logged_in = true;
            }
            AppMsg::LoginFailure(error) => {
                self.login_form_cont.emit(LoginMsg::SetErrorForm(error));                
            }
            AppMsg::Logout => {
                self.header_cont
                    .sender()
                    .send(NavbarMsg::SetConnectionStatus(false))
                    .unwrap();

                self.header_cont
                    .sender()
                    .send(NavbarMsg::SetLoggedInUser("".to_string()))
                    .unwrap();

                self.is_logged_in = false;
            }
            AppMsg::CommandChanged(command) => {
                self.command = command;
                println!("comando {}", self.command);
            },

            AppMsg::ManageResponse(resp) => {
                if resp != "OK" {
                    return;
                }
                if self.command.contains("AUTH") {
                    sender.input(AppMsg::LoginSuccess(self.username.clone()));
                }
            }

            AppMsg::CreateFile(_file_id, _content) => {
                /* println!("Se ejecuto el siguiente comando: {:#?}", self.command);
                if let Some(channel_sender) = &self.command_sender {
                    if let Err(e) = channel_sender.send(ClientCommand::CreateFile{ file_id, content }) {
                        println!("Error enviando comando: {}", e);
                    } else {
                        self.files_manager_cont.emit(FileWorkspaceMsg::ReloadFiles);
                    }
                } */
            }

            AppMsg::SubscribeFile(_file) => {
                /* if let Some(channel_sender) = &self.command_sender {
                    if let Err(e) = channel_sender.send("SUBSCRIBE ".to_string() + &file) {
                        println!("Error enviando comando: {}", e);
                    } else {
                    }
                } */
                /* self.files_manager_cont.emit(FileWorkspaceMsg::OpenFile(file.clone(), "".to_string(), 1)); */
            }

            AppMsg::ExecuteCommand => {
                println!("Se ejecuto el siguiente comando: {:#?}", self.command);
                if let Some(channel_sender) = &self.command_sender {
                    if let Err(e) = channel_sender.send(self.command.to_string()) {
                        println!("Error enviando comando: {}", e);
                    }
                } else {
                    println!("No hay un canal de comando disponible.");
                }
            }
            AppMsg::RefreshData => {
                self.files_manager_cont.emit(FileWorkspaceMsg::ReloadFiles);
            }

            AppMsg::CloseApplication => {
                if let Some(channel_sender) = &self.command_sender {
                    println!("Enviando comando de cierre al servidor");
                    if let Err(e) = channel_sender.send("CLOSE".to_string()) {
                        eprintln!("Error al enviar comando de cierre: {:?}", e);
                    }
                }
            }
        }
    }
}
