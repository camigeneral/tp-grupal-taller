extern crate gtk4;
extern crate relm4;

use std::collections::HashMap;

use self::gtk::prelude::*;
use self::relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};

/// Modelo para el formulario de login.
#[derive(Debug)]
pub struct LoginForm {
    username: String,
    password: String,
    users: HashMap<String, String>,
    error_message: String,
}

/// Mensajes que puede recibir el formulario de login.
#[derive(Debug)]
pub enum LoginMsg {
    UsernameChanged(String),
    PasswordChanged(String),
    Submit,
}

/// Resultado del login.
#[derive(Debug)]
pub enum LoginOutput {
    LoginSuccess(String),
    //LoginFailure(String),
}

#[relm4::component(pub)]
impl SimpleComponent for LoginForm {
    type Init = HashMap<String, String>;

    type Input = LoginMsg;

    type Output = LoginOutput;

    view! {
        #[name = "login_form"]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 10,

            gtk::Label {
                set_label: "Nombre de usuario"
            },

            #[name = "username_entry"]
            gtk::Entry {
                connect_changed[sender] => move |entry| {
                    sender.input(LoginMsg::UsernameChanged(entry.text().to_string()));
                }
            },

            gtk::Label {
                set_label: "Contraseña"
            },

            #[name = "password_entry"]
            gtk::Entry {
                set_visibility: false,
                connect_changed[sender] => move |entry| {
                    sender.input(LoginMsg::PasswordChanged(entry.text().to_string()));
                }
            },

            gtk::Button {
                set_label: "Iniciar sesión",
                connect_clicked[sender] => move |_| {
                    sender.input(LoginMsg::Submit);
                }
            },

            #[name = "error_form_label"]
            gtk::Label {
                set_wrap: true,
                set_css_classes: &["error"],
                #[watch]
                set_visible: model.error_message != "",
                #[watch]
                set_label: &(model.error_message)
            }
        }
    }

    fn init(
        users: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = LoginForm {
            username: String::new(),
            password: String::new(),
            users,
            error_message: "".to_string(),
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            LoginMsg::UsernameChanged(new_username) => {
                self.username = new_username;
            }
            LoginMsg::PasswordChanged(new_password) => {
                self.password = new_password;
            }
            LoginMsg::Submit => {
                let user_ok = self.users.get(&self.username);
                match user_ok {
                    Some(expected_pass) if expected_pass == &self.password => {
                        self.error_message = "".to_string();
                        sender
                            .output(LoginOutput::LoginSuccess(self.username.clone()))
                            .unwrap();
                    }
                    _ => {
                        self.error_message = "Credenciales inválidas".to_string();
                    }
                }
            }
        }
    }
}
