extern crate gtk4;
extern crate relm4;
use self::gtk4::{
    prelude::{BoxExt, ButtonExt, EditableExt, EntryExt, GtkWindowExt, OrientableExt, WidgetExt},
    CssProvider,
};
use crate::components::login::{LoginForm, LoginOutput};
use components::file_workspace::FileWorkspace;
use components::header::{NavbarModel, NavbarMsg, NavbarOutput};
use std::collections::HashMap;

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
pub struct AppModel {
    header_cont: Controller<NavbarModel>,
    files_manager_cont: Controller<FileWorkspace>,
    login_form_cont: Controller<LoginForm>,
    is_logged_in: bool,
    command: String,
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
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
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

        let mut users = HashMap::new();
        users.insert("fran".to_string(), "123123".to_string());
        users.insert("cami".to_string(), "123123".to_string());
        users.insert("valen".to_string(), "123123".to_string());
        users.insert("rama".to_string(), "123123".to_string());

        let header_model = NavbarModel::builder().launch(()).forward(
            sender.input_sender(),
            |output| match output {
                NavbarOutput::ToggleConnectionRequested => AppMsg::Connect,
            },
        );

        let files_manager_model = FileWorkspace::builder()
            .launch(())
            .forward(sender.input_sender(), |_: ()| AppMsg::Ignore);

        let login_form_model = LoginForm::builder().launch(users).forward(
            sender.input_sender(),
            |output| match output {
                LoginOutput::LoginSuccess(username) => AppMsg::LoginSuccess(username),
            },
        );

        let model = AppModel {
            header_cont: header_model,
            files_manager_cont: files_manager_model,
            login_form_cont: login_form_model,
            is_logged_in: false,
            command: "".to_string(),
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            AppMsg::Connect => {
                if self.is_logged_in {
                    self.header_cont
                        .sender()
                        .send(NavbarMsg::SetConnectionStatus(true))
                        .unwrap();
                }
            }
            AppMsg::Ignore => {}
            AppMsg::LoginSuccess(username) => {
                self.header_cont
                    .sender()
                    .send(NavbarMsg::SetLoggedInUser(username))
                    .unwrap();

                self.header_cont
                    .sender()
                    .send(NavbarMsg::SetConnectionStatus(true))
                    .unwrap();
                self.is_logged_in = true;
            }
            AppMsg::LoginFailure(_error) => {}
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
            AppMsg::CommandChanged(command) => self.command = command,

            AppMsg::ExecuteCommand => {
                println!("Se ejecuto el siguiente comando: {}", self.command)
            }
        }
    }
}
