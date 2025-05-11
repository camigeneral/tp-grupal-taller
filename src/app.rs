extern crate gtk4;
extern crate relm4;
use self::gtk4::{
    prelude::{BoxExt, GtkWindowExt, OrientableExt, WidgetExt},
    CssProvider,
};
use components::file_workspace::FileWorkspace;
use components::header::{NavbarModel, NavbarMsg, NavbarOutput};

use self::relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};

/// Modelo principal de la aplicación que contiene los controladores de los componentes.
///
/// # Campos
/// - `header_cont`: Controlador para la barra de navegación superior
/// - `files_manager_cont`: Controlador para el área de gestión de archivos
pub struct AppModel {
    header_cont: Controller<NavbarModel>,
    files_manager_cont: Controller<FileWorkspace>,
}

#[derive(Debug)]
pub enum AppMsg {
    /// Establece conexión con el server
    Connect,
    /// Mensaje vacío (no operation)
    Ignore,
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
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 5,
            set_margin_all: 10,
            set_hexpand: true,
            set_vexpand: true,
            append: model.files_manager_cont.widget()
            }
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

        let header_model = NavbarModel::builder().launch(()).forward(
            sender.input_sender(),
            |msg: NavbarOutput| match msg {
                _ => AppMsg::Connect,
            },
        );
        let files_manager_model = FileWorkspace::builder().launch(()).forward(
            sender.input_sender(),
            |msg: ()| match msg {
                _ => AppMsg::Ignore,
            },
        );

        let model = AppModel {
            header_cont: header_model,
            files_manager_cont: files_manager_model,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            AppMsg::Connect => {
                self.header_cont
                    .sender()
                    .send(NavbarMsg::SetConnectionStatus(true))
                    .unwrap();
            }
            AppMsg::Ignore => {}
        }
    }
}
