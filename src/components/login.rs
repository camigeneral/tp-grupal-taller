extern crate gtk4;
extern crate relm4;

use self::gtk::prelude::*;
use self::relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};

/// Modelo para el formulario de login.
#[derive(Debug)]
pub struct LoginForm {
    username: String,
    password: String,
    error_message: String,
}

/// Mensajes que puede recibir el formulario de login.
#[derive(Debug)]
pub enum LoginMsg {
    UsernameChanged(String),
    PasswordChanged(String),
    SetErrorForm(String),
    Submit,
}

/// Resultado del login.
#[derive(Debug)]
pub enum LoginOutput {
    LoginRequested(String, String),
}

#[relm4::component(pub)]
impl SimpleComponent for LoginForm {
    type Init = ();

    type Input = LoginMsg;

    type Output = LoginOutput;

    view! {
        #[name = "login_form"]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_valign: gtk::Align::Center,
            set_halign: gtk::Align::Center,

            gtk::Box {
                set_halign: gtk::Align::Center,
                set_margin_bottom: 80,

                gtk::Image {
                    set_from_file: Some("src/components/images/logo.png"),
                    set_widget_name: "LoginLogo", 
                }
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_halign: gtk::Align::Center,

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
                    set_visible: !model.error_message.is_empty(),
                    #[watch]
                    set_label: &(model.error_message)
                }
            }
        }
    }

    fn init(
        _users: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = LoginForm {
            username: String::new(),
            password: String::new(),
            error_message: "".to_string(),
        };

        let provider = gtk::CssProvider::new();
        provider.load_from_data(
            "#LoginLogo {
                transform: scale(25);
                transform-origin: center;
            }"
        );

        gtk::style_context_add_provider_for_display(
            &root.display(),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

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
                sender
                    .output(LoginOutput::LoginRequested(
                        self.username.clone(),
                        self.password.clone(),
                    ))
                    .unwrap();
            }
            LoginMsg::SetErrorForm(error) => self.error_message = error,
        }
    }
}
