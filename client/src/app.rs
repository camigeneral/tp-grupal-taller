extern crate gtk4;
extern crate relm4;
use self::gtk4::{
    prelude::{BoxExt, ButtonExt, EditableExt, GtkWindowExt, OrientableExt, PopoverExt, WidgetExt},
    CssProvider,
};
use crate::components::structs::document_value_info::DocumentValueInfo;
use crate::components::{
    error_modal::ErrorModal,
    loading_modal::{LoadingModalModel, LoadingModalMsg},
    login::{LoginForm, LoginOutput},
};
use app::gtk4::glib::Propagation;
use client::LocalClient;
use components::error_modal::ErrorModalMsg;
use components::file_workspace::{FileWorkspace, FileWorkspaceMsg, FileWorkspaceOutputMessage};
use components::header::{NavbarModel, NavbarMsg, NavbarOutput};
use components::types::FileType;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::thread;

use std::sync::mpsc::{channel, Sender};

use self::gtk::gdk_pixbuf::Pixbuf;
use self::gtk::prelude::*;
use self::relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};
use std::io::Cursor;

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
    username: String,
    current_file: String,
    subscribed_files: HashMap<String, bool>,
    error_modal: Controller<ErrorModal>,
    new_file_popover: Option<gtk::Popover>,
    file_name: String,
    loading_modal: Controller<LoadingModalModel>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum AppMsg {
    Connect,
    LoginSuccess(String),
    CommandChanged(String),
    ExecuteCommand,
    GetFiles,
    CloseApplication,
    RefreshData(DocumentValueInfo),
    CreateFile(String, String, String),
    SubscribeFile(String),
    UnsubscribeFile(String),
    PrepareAndExecuteCommand(String, String),
    ManageResponse(String),
    ManageSubscribeResponse(String, String, String),
    ManageUnsubscribeResponse(String),
    SetContentFileCommand(String),
    Error(String),
    ErrorLLM(String),
    /// Mensaje para alternar la visibilidad del popover para nuevos archivos.
    ToggleNewFilePopover,
    SetFileName(String),
    /// Mensaje para crear un documento de tipo texto.
    CreateTextDocument,
    /// Mensaje para crear un documento de tipo hoja de cálculo.
    CreateSpreadsheetDocument,
    AddContent(DocumentValueInfo),
    AddContentSpreadSheet(DocumentValueInfo),
    UpdateFilesList,
    FilesLoaded,
    ReloadFile(String, String),
    AddFile(String),
    SendPrompt(DocumentValueInfo),
    UpdateAllFileData(String, Vec<String>),
    UpdateLineFile(String, String, String, String),
    PublishLlmResponse(Vec<String>),
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = u16;
    type Input = AppMsg;
    type Output = ();

    view! {
    gtk::Window {
        connect_map: |window| {
            window.maximize();
        },
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
                    set_hexpand: true,

                    #[name = "logo_box"]
                    gtk::Box {
                        set_halign: gtk::Align::Center,
                        // Eliminamos gtk::Image aquí, lo agregaremos manualmente
                    },

                     gtk::Box {
                        set_hexpand: true,
                        set_halign: gtk::Align::End,

                        #[name="new_file_button"]
                        gtk::Button {
                            add_css_class: "new-file",
                            add_css_class: "button",
                            set_label: "Nuevo Archivo",
                            connect_clicked => AppMsg::ToggleNewFilePopover,
                        },

                        #[name="reload_button"]
                        gtk::Button {
                            add_css_class: "reload",
                            add_css_class: "button",
                            set_label: "Reload",
                            connect_clicked => AppMsg::GetFiles,
                            set_margin_start: 20,
                            set_margin_end: 11,
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
                                        sender.input(AppMsg::SetFileName(entry.text().to_string()));
                                    }
                                },
                                gtk::Button {
                                    set_label: "Hoja de texto",
                                    connect_clicked => AppMsg::CreateTextDocument,
                                },
                                gtk::Button {
                                    set_label: "Hoja de cálculo",
                                    connect_clicked => AppMsg::CreateSpreadsheetDocument	,
                                }
                            },
                        },
                    }
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
        let error_modal = ErrorModal::builder()
            .transient_for(&root)
            .launch(())
            .detach();

        let loading_modal = LoadingModalModel::builder()
            .transient_for(&root)
            .launch(())
            .detach();

        let header_model = NavbarModel::builder().launch(()).forward(
            sender.input_sender(),
            |output| match output {
                NavbarOutput::ToggleConnectionRequested => AppMsg::Connect,
                NavbarOutput::CreateFileRequested(file_id, content) => {
                    AppMsg::CreateFile(file_id, content, "txt".to_string())
                }
            },
        );

        let files_manager_model = FileWorkspace::builder().launch(()).forward(
            sender.input_sender(),
            |command: FileWorkspaceOutputMessage| match command {
                FileWorkspaceOutputMessage::SubscribeFile(file) => AppMsg::SubscribeFile(file),
                FileWorkspaceOutputMessage::UnsubscribeFile(file) => AppMsg::UnsubscribeFile(file),
                FileWorkspaceOutputMessage::ContentAdded(doc_info) => AppMsg::AddContent(doc_info),
                FileWorkspaceOutputMessage::ContentAddedSpreadSheet(doc_info) => {
                    AppMsg::AddContentSpreadSheet(doc_info)
                }
                FileWorkspaceOutputMessage::FilesLoaded => AppMsg::FilesLoaded,
                FileWorkspaceOutputMessage::SendPrompt(doc_info) => AppMsg::SendPrompt(doc_info),
            },
        );

        let login_form_model =
            LoginForm::builder()
                .launch(())
                .forward(sender.input_sender(), |output| match output {
                    LoginOutput::LoginRequested(username, password) => {
                        let command = format!("AUTH {} {}", username, password);
                        AppMsg::PrepareAndExecuteCommand(command, username)
                    }
                });

        let mut model = AppModel {
            header_cont: header_model,
            files_manager_cont: files_manager_model,
            login_form_cont: login_form_model,
            is_logged_in: false,
            command: "".to_string(),
            command_sender: None,
            username: "".to_string(),
            current_file: "".to_string(),
            subscribed_files: HashMap::new(),
            error_modal,
            new_file_popover: None,
            file_name: "".to_string(),
            loading_modal,
        };

        let sender_clone = sender.clone();

        root.connect_close_request(move |_| {
            sender_clone.input(AppMsg::CommandChanged("close".to_string()));
            sender_clone.input(AppMsg::ExecuteCommand);
            Propagation::Proceed
        });
        let widgets = view_output!();
        model.new_file_popover = Some(widgets.new_file_popover.clone());
        let ui_sender: relm4::Sender<AppMsg> = sender.input_sender().clone();
        let (tx, rx) = channel::<String>();
        let command_sender = Some(tx.clone());
        model.command_sender = command_sender;

        // Cargar la imagen embebida para el logo principal
        let image_bytes = include_bytes!("components/assets/logo.png");
        let pixbuf = Pixbuf::from_read(Cursor::new(image_bytes)).expect("falló al leer imagen");
        let image = gtk::Image::from_pixbuf(Some(&pixbuf));
        image.set_widget_name("AppLogo");
        image.set_valign(gtk::Align::Center);
        image.set_halign(gtk::Align::Center);
        image.set_margin_bottom(0);
        image.set_margin_start(100);
        image.set_margin_top(20);

        // Agregar la imagen manualmente al logo_box
        if let Some(logo_box) = widgets.logo_box.clone().dynamic_cast::<gtk::Box>().ok() {
            logo_box.append(&image);
        }
        thread::spawn(
            move || match LocalClient::new(port, Some(ui_sender), Some(rx)) {
                Ok(mut client) => client.run(),
                Err(e) => {
                    eprintln!("Error al iniciar el cliente: {:?}", e);
                }
            },
        );

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppMsg::Connect => {
                if self.is_logged_in {
                    if let Err(_e) = self
                        .header_cont
                        .sender()
                        .send(NavbarMsg::SetConnectionStatus(true))
                    {
                        eprintln!("Failed to send message");
                    }
                }
            }
            AppMsg::Error(error_message) => {
                self.error_modal.emit(ErrorModalMsg::Show(error_message));
            }
            AppMsg::ErrorLLM(error_message) => {
                self.error_modal.emit(ErrorModalMsg::Show(error_message));
                self.loading_modal.emit(LoadingModalMsg::Hide);
            }
            AppMsg::PrepareAndExecuteCommand(command, username) => {
                self.command = command;
                self.username = username;
                sender.input(AppMsg::ExecuteCommand);
            }
            AppMsg::LoginSuccess(username) => {
                if self
                    .header_cont
                    .sender()
                    .send(NavbarMsg::SetLoggedInUser(username))
                    .is_err()
                {
                    eprintln!("Failed to send message");
                }

                if self
                    .header_cont
                    .sender()
                    .send(NavbarMsg::SetConnectionStatus(true))
                    .is_err()
                {
                    eprintln!("Failed to send message");
                }
                self.files_manager_cont.emit(FileWorkspaceMsg::ReloadFiles);
                self.is_logged_in = true;
            }

            AppMsg::CommandChanged(command) => {
                self.command = command;
            }

            AppMsg::FilesLoaded => {
                sender.input(AppMsg::LoginSuccess(self.username.clone()));
            }

            AppMsg::ManageResponse(resp) => {
                if resp != "OK" {
                    return;
                }
                if self.command.contains("AUTH") {
                    sender.input(AppMsg::GetFiles);
                }
            }
            AppMsg::GetFiles => {
                sender.input(AppMsg::UpdateFilesList);
            }
            AppMsg::ManageSubscribeResponse(file, qty_subs, content) => {
                if self.current_file != file {
                    return;
                }

                let file_type = if file.ends_with(".xlsx") {
                    FileType::Sheet
                } else {
                    FileType::Text
                };

                self.subscribed_files
                    .insert(self.current_file.clone(), true);
                self.files_manager_cont.emit(FileWorkspaceMsg::OpenFile(
                    self.current_file.clone(),
                    qty_subs,
                    file_type,
                    content,
                ));
            }

            AppMsg::CreateFile(file_id, content, _file_type) => {
                self.command = format!("set {} {}", file_id, content);
                sender.input(AppMsg::ExecuteCommand);
            }

            AppMsg::SendPrompt(doc_info) => {
                self.command = match doc_info.selection_mode.as_str() {
                    "cursor" => format!(
                        "change-line|{}|{}|{}|{}",
                        doc_info.file, doc_info.index, doc_info.offset, doc_info.prompt
                    ),
                    "whole-file" => format!("request-file|{}|{}", doc_info.file, doc_info.prompt),
                    _ => String::new(),
                };

                self.loading_modal.emit(LoadingModalMsg::Show);
                sender.input(AppMsg::ExecuteCommand);
            }
            AppMsg::AddContent(doc_info) => {
                self.command = format!(
                    "WRITE|{}|{}|{}|{}",
                    doc_info.index, doc_info.value, doc_info.timestamp, doc_info.file
                );

                sender.input(AppMsg::ExecuteCommand);
            }
            AppMsg::AddContentSpreadSheet(doc_info) => {
                self.command = format!(
                    "WRITE|{}|{}|{}|{}",
                    doc_info.index, doc_info.value, doc_info.index, doc_info.file
                );
                sender.input(AppMsg::ExecuteCommand);
            }

            AppMsg::SetContentFileCommand(command) => {
                self.command = command;
                sender.input(AppMsg::ExecuteCommand);
            }

            AppMsg::SubscribeFile(file) => {
                self.current_file = file;

                self.command = format!("subscribe {}", self.current_file);

                sender.input(AppMsg::ExecuteCommand);
            }

            AppMsg::UnsubscribeFile(file) => {
                self.command = format!("unsubscribe {}", file);
                self.current_file = "".to_string();
                sender.input(AppMsg::ExecuteCommand);
            }

            AppMsg::ManageUnsubscribeResponse(response) => {
                if response == "OK" {
                    self.subscribed_files.remove(&self.current_file);
                } else {
                    println!("Error al desuscribirse: {}", response);
                }

                self.command = "".to_string();
                self.current_file = "".to_string();
            }

            AppMsg::ExecuteCommand => {
                if let Some(channel_sender) = &self.command_sender {
                    if let Err(e) = channel_sender.send(self.command.to_string()) {
                        println!("Error enviando comando: {}", e);
                    }
                } else {
                    println!("No hay un canal de comando disponible.");
                }
            }
            AppMsg::RefreshData(doc_info) => {
                self.files_manager_cont
                    .emit(FileWorkspaceMsg::UpdateFile(doc_info));
            }

            AppMsg::UpdateAllFileData(file, content) => {
                let mut updated_content: Vec<String> = Vec::new();

                for coded_text in content {
                    let mut document = DocumentValueInfo::new(coded_text, 0);
                    document.decode_text();
                    updated_content.push(document.value.trim_end_matches('\r').to_string().clone());
                }

                self.loading_modal.emit(LoadingModalMsg::Hide);
                self.files_manager_cont
                    .emit(FileWorkspaceMsg::UpdateAllFileData(file, updated_content));
            }

            AppMsg::UpdateLineFile(file, line, content, offset) => {
                let parsed_index = match line.parse::<usize>() {
                    Ok(idx) => idx,
                    Err(e) => {
                        println!("Error parseando índice: {}", e);
                        return;
                    }
                };
                let parsed_offset = match offset.parse::<usize>() {
                    Ok(idx) => idx,
                    Err(e) => {
                        println!("Error parseando índice: {}", e);
                        return;
                    }
                };

                self.loading_modal.emit(LoadingModalMsg::Hide);
                self.files_manager_cont
                    .emit(FileWorkspaceMsg::UpdateLLMFile(
                        file,
                        parsed_index,
                        parsed_offset,
                        content,
                    ));
            }

            AppMsg::CloseApplication => {
                if let Some(channel_sender) = &self.command_sender {
                    if let Err(e) = channel_sender.send("close".to_string()) {
                        eprintln!("Error al enviar comando de cierre: {:?}", e);
                    }
                }
            }

            AppMsg::ToggleNewFilePopover => {
                if let Some(popover) = &self.new_file_popover {
                    popover.popup();
                }
            }

            AppMsg::SetFileName(file_name) => {
                self.file_name = file_name;
            }

            AppMsg::CreateTextDocument => {
                if let Some(popover) = &self.new_file_popover {
                    popover.popdown();
                }
                if self.file_name.trim().is_empty() {
                    println!("El nombre del archivo es obligatorio.");
                    return;
                }
                let file_id = format!(
                    "{}.txt",
                    self.file_name.split(' ').collect::<Vec<&str>>().join("_")
                );
                sender.input(AppMsg::CreateFile(
                    file_id,
                    "\"\"".to_string(),
                    "txt".to_string(),
                ));
            }

            AppMsg::CreateSpreadsheetDocument => {
                if let Some(popover) = &self.new_file_popover {
                    popover.popdown();
                }
                if self.file_name.trim().is_empty() {
                    println!("El nombre del archivo es obligatorio.");
                    return;
                }
                let file_id = format!(
                    "{}.xlsx",
                    self.file_name.split(' ').collect::<Vec<&str>>().join("_")
                );
                sender.input(AppMsg::CreateFile(
                    file_id,
                    "\"\"".to_string(),
                    "xlsx".to_string(),
                ));
            }

            AppMsg::UpdateFilesList => {
                let mut doc_names: HashSet<String> = HashSet::new();
                if let Ok(entries) = fs::read_dir("./redis_server/rdb_files") {
                    for entry in entries.map_while(Result::ok) {
                        let path = entry.path();
                        let fname = path
                            .file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or("")
                            .to_string();
                        if fname.starts_with("redis_node_") && fname.ends_with(".rdb") {
                            if let Ok(file) = fs::File::open(&path) {
                                use std::io::{BufRead, BufReader};
                                let reader = BufReader::new(file);
                                for line in reader.lines().flatten() {
                                    if let Some((doc_name, _)) = line.split_once("/++/") {
                                        if !doc_name.trim().is_empty() {
                                            doc_names.insert(doc_name.trim().to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let mut doc_names_vec: Vec<String> = doc_names.into_iter().collect();
                doc_names_vec.sort();
                let archivos_tipos: Vec<(String, FileType)> = doc_names_vec
                    .into_iter()
                    .filter(|name| !name.is_empty())
                    .map(|name| {
                        let tipo = if name.ends_with(".xlsx") {
                            FileType::Sheet
                        } else {
                            FileType::Text
                        };
                        (name, tipo)
                    })
                    .collect();
                self.files_manager_cont
                    .emit(FileWorkspaceMsg::UpdateFilesList(archivos_tipos));
            }

            AppMsg::ReloadFile(file_id, content) => {
                let file_type = if file_id.ends_with(".xlsx") {
                    FileType::Sheet
                } else {
                    FileType::Text
                };
                let mut doc_file = DocumentValueInfo::new(content.clone(), 0);
                doc_file.decode_text();
                self.files_manager_cont.emit(FileWorkspaceMsg::OpenFile(
                    file_id.clone(),
                    "1".to_string(), // qty_subs
                    file_type,
                    doc_file.value,
                ));
            }
            AppMsg::AddFile(file_name) => {
                let doc_type = if file_name.ends_with("txt") {
                    FileType::Text
                } else {
                    FileType::Sheet
                };
                self.files_manager_cont
                    .emit(FileWorkspaceMsg::AddFile(file_name, doc_type));
            }
            AppMsg::PublishLlmResponse(resp_command) => {
                let resp_command_str = resp_command
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
                    .join("|");
                self.command = format!(
                    "client-llm-response|{}|{}",
                    resp_command[0], resp_command_str
                );
                sender.input(AppMsg::ExecuteCommand);
            }
        }
    }
}
