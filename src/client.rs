extern crate relm4;
use self::relm4::Sender as UiSender;
use crate::app::AppMsg;
use crate::components::structs::document_value_info::DocumentValueInfo;
use commands::redis_parser::{format_resp_command, format_resp_publish};
use std::collections::HashMap;
use std::io::{BufReader, Write, BufWriter};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, Sender as MpscSender};
use std::sync::{Arc, Mutex};
use std::thread;
use utils::extract_document_name;

#[path = "utils/redis_parser.rs"]
mod redis_parser;

struct NodeConnectionParams {
    pub redis_socket: TcpStream,
    pub node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    pub last_command_sent: Arc<Mutex<String>>,
    pub ui_sender: Option<UiSender<AppMsg>>,
    pub address: String,
}

pub struct LocalClient {
    address: String,
    ui_sender: Option<UiSender<AppMsg>>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>,
    redis_socket: TcpStream,
    redis_sender: Option<MpscSender<String>>,
    rx_ui: Option<Receiver<String>>
}

impl LocalClient {
    fn new(port: u16, ui_sender: Option<UiSender<AppMsg>>, rx_ui: Option<Receiver<String>>) ->  Result<Self, Box<dyn std::error::Error>> {
        let address = format!("127.0.0.1:{}", port);
        let socket: TcpStream = match TcpStream::connect(address.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error al conectar al servidor: {}", e);
                return Err(Box::new(e));
            }
        };

        Ok(Self {
            address,
            ui_sender, 
            node_streams: Arc::new(Mutex::new(HashMap::new())),
            last_command_sent: Arc::new(Mutex::new("".to_string())),
            redis_socket: socket,
            redis_sender: None,
            rx_ui
        })
    }

    
    fn spwan_writer_channel(&mut self) {
        let (socket_tx, socket_rx) = channel::<String>();
        let redis_socket = match self.redis_socket.try_clone() {
            Ok(clone) => clone,
            Err(e) => {
                eprintln!("Error al clonar el socket: {}", e);
                return;
            }
        };
        self.redis_sender = Some(socket_tx);

        thread::spawn(move || {
            let mut writer = BufWriter::new(redis_socket);
            for msg in socket_rx {
                if let Err(e) = writer.write_all(msg.as_bytes()) {
                    eprintln!("Error escribiendo en el socket Redis: {}", e);
                    break;
                }
                let _ = writer.flush();
            }            
        });
    }
    
    fn register_redis_socket_in_map(&self) -> std::io::Result<()> {
        let socket_clone = self.redis_socket.try_clone()
            .map_err(|e| {
                eprintln!("Failed to clone socket: {}", e);
                std::io::Error::other("Socket clone failed")
            })?;

        let mut locked_map = self.node_streams.lock()
            .map_err(|_| std::io::Error::other("Failed to lock node_streams"))?;

        locked_map.insert(self.address.clone(), socket_clone);

        Ok(())
    }

    fn register_and_connect_node(&self) -> std::io::Result<()>{
        let redis_socket = match self.redis_socket.try_clone() {
                Ok(clone) => clone,
                Err(e) => {
                    eprintln!("Error al clonar el socket: {}", e);
                    return Err(e);
                }
            };

        let _ = self.register_redis_socket_in_map();

        let (connect_node_sender, connect_nodes_receiver) = channel::<TcpStream>();
        let connect_node_sender_cloned = connect_node_sender.clone();
        let params = NodeConnectionParams::from(self);
        thread::spawn(move || {
            if let Err(e) = connect_to_nodes(
                connect_node_sender_cloned,
                connect_nodes_receiver,
                params
            ) {
                eprintln!("Error en la conexión con el nodo: {}", e);
            }
        });

        let _ = connect_node_sender.send(redis_socket);
        Ok(())

    }


    fn run(&mut self) {

        self.spwan_writer_channel();
        let initial_command = redis_parser::format_resp_command(&["Cliente"]);
        if let Some(redis_sender) = &self.redis_sender {
            let _ = redis_sender.send(initial_command);
        }

        let _ = self.register_and_connect_node();
        
        self.read_comming_messages();
    }

    fn get_resp_command(&self, parts: Vec<&str>, command: &str) -> String {
        if parts.is_empty() {
            return String::new();
        }
        let cmd = parts[0];
        if cmd.eq_ignore_ascii_case("AUTH")
            || cmd.eq_ignore_ascii_case("subscribe")
            || cmd.eq_ignore_ascii_case("unsubscribe")
            || cmd.eq_ignore_ascii_case("get_files")
            || cmd.eq_ignore_ascii_case("set")
        {
            format_resp_command(&parts)
        } else if cmd.to_uppercase().contains("WRITE") {
            let splited_command: Vec<&str> = command.split('|').collect();
            let client_command = format_resp_command(&splited_command).to_string();
            let key = splited_command.get(4).unwrap_or(&"");
            format_resp_publish(key, &client_command)
        } else {
            let key = parts.get(1).unwrap_or(&"");
            format_resp_publish(key, command)
        }
    }

    fn set_last_command(&self, resp_command: String)->  std::io::Result<()>  {
        let mut last_command = match self.last_command_sent.lock() {
            Ok(locked) => locked,
            Err(e) => {
                eprintln!("Error al bloquear el mutex de last_command_sent: {}", e);
                return Err(std::io::Error::other("Mutex lock failed"));
            }
        };
        *last_command = resp_command.clone();
        Ok(())
    }
    
    fn read_comming_messages(&mut self) -> std::io::Result<()> {
        let rx_ui = match &self.rx_ui {
            Some(rx) => rx,
            None => return Ok(()),
        };

        let redis_sender = match &self.redis_sender {
            Some(tx) => tx,
            None => return Ok(()),
        };

        for command in rx_ui {
            let trimmed_command = command.to_string().trim().to_lowercase();
            let parts: Vec<&str> = command.split_whitespace().collect();
            if trimmed_command == "close" {
                println!("Desconectando del servidor");            
                let resp_command = redis_parser::format_resp_publish(parts[0], parts.get(1).unwrap_or(&""));

                println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

                if let Err(e) = redis_sender.send(resp_command) {
                    eprintln!("Error al escribir en el socket: {}", e);
                    return Ok(());
                }
                break;
            } else {
                println!("Enviando: {:?}", command);            
                let resp_command = self.get_resp_command(parts, &command);

                let _ = self.set_last_command(resp_command.clone());
                
                println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

                if let Err(e) = redis_sender.send(resp_command) {
                    eprintln!("Error al escribir en el socket: {}", e);
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

impl From<&LocalClient> for NodeConnectionParams {
    fn from(client: &LocalClient) -> Self {
        NodeConnectionParams {
            redis_socket: client.redis_socket.try_clone().unwrap(),
            node_streams: Arc::clone(&client.node_streams),
            last_command_sent: Arc::clone(&client.last_command_sent),
            ui_sender: client.ui_sender.clone(),
            address: client.address.clone(),
        }
    }
}


pub fn client_run(
    port: u16,
    rx: Receiver<String>,
    ui_sender: Option<UiSender<AppMsg>>,
) -> std::io::Result<()> {
    let node_streams: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));
    let last_command_sent: Arc<Mutex<String>> = Arc::new(Mutex::new("".to_string()));

    let address = format!("127.0.0.1:{}", port);
    let cloned_address = address.clone();

    println!("Conectándome al server de redis en {:?}", address);
    let mut socket: TcpStream = match TcpStream::connect(address.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error al conectar al servidor: {}", e);
            return Err(e);
        }
    };

    let command = "Cliente\r\n".to_string();

    println!("Enviando: {:?}", command);
    let parts: Vec<&str> = command.split_whitespace().collect();
    let resp_command = redis_parser::format_resp_command(&parts);

    println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

    if let Err(e) = socket.write_all(resp_command.as_bytes()) {
        eprintln!("Error al escribir en el socket: {}", e);
        return Err(e);
    }

    let redis_socket = match socket.try_clone() {
        Ok(clone) => clone,
        Err(e) => {
            eprintln!("Error al clonar el socket: {}", e);
            return Err(e);
        }
    };

    let redis_socket_clone_for_hashmap = match socket.try_clone() {
        Ok(clone) => clone,
        Err(e) => {
            eprintln!("Error al clonar el socket para hashmap: {}", e);
            return Err(e);
        }
    };

    {
        let mut locked_node_streams = match node_streams.lock() {
            Ok(locked) => locked,
            Err(e) => {
                eprintln!("Error al bloquear el mutex de node_streams: {}", e);
                return Err(std::io::Error::other("Mutex lock failed"));
            }
        };
        locked_node_streams.insert(cloned_address, redis_socket_clone_for_hashmap);
    }

    let cloned_node_streams = Arc::clone(&node_streams);
    let cloned_last_command = Arc::clone(&last_command_sent);

    let (connect_node_sender, connect_nodes_receiver) = channel::<TcpStream>();
    let connect_node_sender_cloned = connect_node_sender.clone();

    thread::spawn(move || {
        if let Err(e) = connect_to_nodes(
            connect_node_sender_cloned,
            connect_nodes_receiver,
            ui_sender,
            cloned_node_streams,
            cloned_last_command,
        ) {
            eprintln!("Error en la conexión con el nodo: {}", e);
        }
    });

    let _ = connect_node_sender.send(redis_socket);

    for command in rx {
        let trimmed_command = command.to_string().trim().to_lowercase();
        if trimmed_command == "close" {
            println!("Desconectando del servidor");
            let parts: Vec<&str> = trimmed_command.split_whitespace().collect();
            let resp_command = redis_parser::format_resp_publish(parts[0], parts[1]);

            println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

            if let Err(e) = socket.write_all(resp_command.as_bytes()) {
                eprintln!("Error al escribir en el socket: {}", e);
                return Err(e);
            }
            break;
        } else {
            println!("Enviando: {:?}", command);

            let parts: Vec<&str> = command.split_whitespace().collect();
            let resp_command = if parts[0] == "AUTH"
                || parts[0] == "subscribe"
                || parts[0] == "unsubscribe"
                || parts[0] == "get_files"
                || parts[0] == "set"
            {
                format_resp_command(&parts)
            } else if parts[0].contains("WRITE") {
                let splited_command: Vec<&str> = command.split("|").collect();
                let client_command = format_resp_command(&splited_command).clone().to_string();
                format_resp_publish(splited_command[4], &client_command)
            } else {
                format_resp_publish(parts[1], &command)
            };

            {
                let mut last_command = match last_command_sent.lock() {
                    Ok(locked) => locked,
                    Err(e) => {
                        eprintln!("Error al bloquear el mutex de last_command_sent: {}", e);
                        return Err(std::io::Error::other("Mutex lock failed"));
                    }
                };
                *last_command = resp_command.clone();
            }

            println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

            if let Err(e) = socket.write_all(resp_command.as_bytes()) {
                eprintln!("Error al escribir en el socket: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}

fn listen_to_redis_response(
    client_socket: TcpStream,
    ui_sender: Option<Sender<AppMsg>>,
    connect_node_sender: MpscSender<TcpStream>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>,
) -> std::io::Result<()> {
    let client_socket_cloned = match client_socket.try_clone() {
        Ok(clone) => clone,
        Err(e) => {
            eprintln!("Error al clonar el socket del cliente: {}", e);
            return Err(std::io::Error::other("Socket clone failed"));
        }
    };

    let mut reader: BufReader<TcpStream> = BufReader::new(client_socket);

    loop {
        let (response, _) = match redis_parser::parse_resp_command(&mut reader) {
            Ok((parts, s)) => (parts, s),
            Err(e) => {
                eprintln!("Error al leer línea desde el socket: {}", e);
                break;
            }
        };

        if response.is_empty() {
            break;
        }

        let local_addr = match client_socket_cloned.local_addr() {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("Error al obtener la dirección local: {}", e);
                return Err(e);
            }
        };

        println!("Respuesta de redis: {}", response.join(" "));

        let first_response = response[0].to_uppercase();

        match first_response.as_str() {
            s if s.starts_with("-ERR") => {
                let error_message = if response.len() > 1 {
                    response[1..].join(" ")
                } else {
                    "Error desconocido".to_string()
                };
                if let Some(sender) = &ui_sender {
                    let _ = sender.send(AppMsg::Error(format!(
                        "Hubo un problema: {}",
                        error_message
                    )));
                }
            }
            "ASK" => {
                if response.len() < 3 {
                    println!("Nodo de redireccion no disponible");
                } else {
                    let _ = send_command_to_nodes(
                        connect_node_sender.clone(),
                        node_streams.clone(),
                        last_command_sent.clone(),
                        response,
                    );
                }
            }
            "STATUS" => {
                let socket = response[2].clone();
                let doc = response[1].clone();
                let content = response[3].clone();
                if socket != local_addr.to_string() {
                    continue;
                }

                if let Some(sender) = &ui_sender {
                    let mut document = DocumentValueInfo::new(content, 0);
                    document.decode_text();
                    let _ = sender.send(AppMsg::ManageSubscribeResponse(
                        doc.to_string(),
                        "1".to_string(),
                        document.value.to_string(),
                    ));
                }
            }

            "WRITE" => {
                if let Some(sender) = &ui_sender {
                    let index = match response[1].parse::<i32>() {
                        Ok(i) => i,
                        Err(_) => {
                            eprintln!("Error parsing index from response: {:?}", response[1]);
                            break;
                        }
                    };

                    let text = response[2].to_string();
                    let file = response[4].to_string();

                    let split_text = text.split("<enter>").collect::<Vec<_>>();

                    if split_text.len() == 2 {
                        let (before_newline, after_newline) = (split_text[0], split_text[1]);

                        for (offset, content) in [(0, before_newline), (1, after_newline)] {
                            let mut doc_info =
                                DocumentValueInfo::new(content.to_string(), index + offset);
                            doc_info.file = file.clone();
                            doc_info.decode_text();
                            let _ = sender.send(AppMsg::RefreshData(doc_info));
                        }
                    } else {
                        let mut doc_info = DocumentValueInfo::new(text, index);
                        doc_info.file = file.clone();
                        doc_info.decode_text();
                        let _ = sender.send(AppMsg::RefreshData(doc_info));
                    }
                }
            }

            "FILES" => {
                let archivos = if response.len() > 1 {
                    response[1..].to_vec()
                } else {
                    vec![]
                };
                if let Some(sender) = &ui_sender {
                    let _ = sender.send(AppMsg::UpdateFilesList(archivos));
                }
            }
            _ => {
                if let Some(sender) = &ui_sender {
                    let _ = sender.send(AppMsg::ManageResponse(response[0].clone()));
                }
                if let Ok(last_command) = last_command_sent.lock() {
                    // Verifica si el comando fue SET y extrae el nombre del archivo
                    if last_command.to_uppercase().contains("SET") {
                        let lines: Vec<&str> = last_command.split("\r\n").collect();
                        if lines.len() >= 5 {
                            let file_name = lines[4];
                            if let Some(sender) = &ui_sender {
                                let _ = sender.send(AppMsg::AddFile(file_name.to_string()));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn send_command_to_nodes(
    connect_node_sender: MpscSender<TcpStream>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>,
    response: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let last_line_cloned = match last_command_sent.lock() {
        Ok(locked) => locked.clone(),
        Err(e) => {
            eprintln!("Error al bloquear el mutex de last_command_sent: {}", e);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Mutex lock failed: {}", e),
            )));
        }
    };

    let mut locked_node_streams = match node_streams.lock() {
        Ok(locked) => locked,
        Err(e) => {
            eprintln!("Error al bloquear el mutex de node_streams: {}", e);
            return Err(Box::new(std::io::Error::other(format!(
                "Mutex lock failed: {}",
                e
            ))));
        }
    };

    let new_node_address = response[2].to_string();

    println!("Ultimo comando ejecutado: {:#?}", last_line_cloned);
    println!("Redirigiendo a nodo: {}", new_node_address);

    if let Some(stream) = locked_node_streams.get_mut(&new_node_address) {
        println!("Usando conexión existente al nodo {}", new_node_address);
        if let Err(e) = stream.write_all(last_line_cloned.as_bytes()) {
            eprintln!("Error al escribir en el nodo: {}", e);
            return Err(Box::new(e));
        }
    } else {
        println!("Creando nueva conexión al nodo {}", new_node_address);
        let parts: Vec<&str> = "connect".split_whitespace().collect();
        let resp_command = redis_parser::format_resp_command(&parts);
        let mut final_stream = match TcpStream::connect(new_node_address.clone()) {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Error al conectar con el nuevo nodo: {}", e);
                return Err(Box::new(e));
            }
        };
        if let Err(e) = final_stream.write_all(resp_command.as_bytes()) {
            eprintln!("Error al escribir en el nuevo nodo: {}", e);
            return Err(Box::new(e));
        }

        let mut cloned_stream_to_connect = match final_stream.try_clone() {
            Ok(clone) => clone,
            Err(e) => {
                eprintln!("Error al clonar el socket: {}", e);
                return Err(Box::new(e));
            }
        };
        locked_node_streams.insert(new_node_address, final_stream);

        if let Err(e) = connect_node_sender.send(cloned_stream_to_connect.try_clone()?) {
            eprintln!("Error al enviar el nodo conectado: {}", e);
            return Err(Box::new(e));
        }
        std::thread::sleep(std::time::Duration::from_millis(2));

        // vuelvo a hacer subscribe
        if let Some(doc_name) = extract_document_name(&last_line_cloned) {
            let subscribe_command = format!(
                "*2\r\n$9\r\nsubscribe\r\n${}\r\n{}\r\n",
                doc_name.len(),
                doc_name
            );
            if let Err(e) = cloned_stream_to_connect.write_all(subscribe_command.as_bytes()) {
                eprintln!("Error subscribing to doc: {}", doc_name);
                return Err(Box::new(e));
            }
        }

        if let Err(e) = cloned_stream_to_connect.write_all(last_line_cloned.as_bytes()) {
            eprintln!("Error al reenviar el último comando: {}", e);
            return Err(Box::new(e));
        }
    }
    Ok(())
}

fn connect_to_nodes(
    sender: MpscSender<TcpStream>,
    reciever: Receiver<TcpStream>,
    params: NodeConnectionParams
) -> std::io::Result<()> {
    for stream in reciever {
        let cloned_node_streams = Arc::clone(&params.node_streams);
        let cloned_last_command = Arc::clone(&params.last_command_sent);
        let cloned_sender = params.ui_sender.clone();
        let cloned_own_sender = sender.clone();

        thread::spawn(move || {
            if let Err(e) = listen_to_redis_response(
                stream,
                cloned_sender,
                cloned_own_sender,
                cloned_node_streams,
                cloned_last_command,
            ) {
                eprintln!("Error en la conexión con el nodo: {}", e);
            }
        });
    }

    Ok(())
}