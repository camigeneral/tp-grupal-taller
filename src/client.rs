extern crate relm4;
use self::relm4::Sender;
use crate::app::AppMsg;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, channel, Sender as MpscSender};
use std::sync::{Arc, Mutex};
use std::thread;

#[path = "utils/redis_parser.rs"]
mod redis_parser;

pub fn client_run(
    port: u16,
    rx: Receiver<String>,
    ui_sender: Option<Sender<AppMsg>> ,
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
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Mutex lock failed"));
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
        if trimmed_command == "close"  {
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
            let resp_command = if parts[0] == "AUTH" || parts[0] == "subscribe"  || parts[0] == "unsubscribe" {
                                    redis_parser::format_resp_command(&parts)
                                }else{
                                    redis_parser::format_resp_publish(parts[1], &command)
                                };

            {
                let mut last_command = match last_command_sent.lock() {
                    Ok(locked) => locked,
                    Err(e) => {
                        eprintln!("Error al bloquear el mutex de last_command_sent: {}", e);
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Mutex lock failed"));
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
    ui_sender: Option<Sender<AppMsg>> ,
    connect_node_sender: MpscSender<TcpStream>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>> ,
    last_command_sent: Arc<Mutex<String>> ,
) -> std::io::Result<()> {
    let client_socket_cloned = match client_socket.try_clone() {
        Ok(clone) => clone,
        Err(e) => {
            eprintln!("Error al clonar el socket del cliente: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Socket clone failed"));
        }
    };

    let mut reader = BufReader::new(client_socket);

    loop {
        let mut line = String::new();
        let bytes_read = match reader.read_line(&mut line) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("Error al leer línea desde el socket: {}", e);
                return Err(e);
            }
        };

        if bytes_read == 0 {
            break;
        }

        println!("Respuesta de redis: {}", line);

        let response: Vec<&str> = line.split_whitespace().collect();

        let first = response[0].to_uppercase();
        let first_response = first.as_str();

        match first_response {
            s if s.starts_with("-ERR") => {
                let error_message = if response.len() > 1 {
                    response[1..].join(" ")
                } else {
                    "Error desconocido".to_string()
                };
                if let Some(sender) = &ui_sender {
                    let _ = sender.send(AppMsg::Error(format!("Hubo un problema: {}", error_message)));
                }
            }
            "ASK" => {
                if response.len() < 3 {
                    println!("Nodo de redireccion no disponible");
                } else {
                    let _ = send_command_to_nodes(
                        ui_sender.clone(),
                        connect_node_sender.clone(),
                        node_streams.clone(),
                        last_command_sent.clone(),
                        response,
                    );
                }
            }
            "STATUS" => {
                let response_status: Vec<&str> = response[1].split('|').collect();
                let socket = response_status[0];
                let local_addr = match client_socket_cloned.local_addr() {
                    Ok(addr) => addr,
                    Err(e) => {
                        eprintln!("Error al obtener la dirección local: {}", e);
                        return Err(e);
                    }
                };

                if socket != local_addr.to_string() {
                    continue;
                }

                if let Some(sender) = &ui_sender {
                    let _ = sender.send(AppMsg::ManageSubscribeResponse(
                        response_status[1].to_string(),
                    ));
                }
            }
            "WRITTEN" => {
                // se va a procesar lo que otro agregó
            }
            _ => {
                if let Some(sender) = &ui_sender {
                    let _ = sender.send(AppMsg::ManageResponse(first));
                }
            }
        }
    }
    Ok(())
}

fn send_command_to_nodes(
    _ui_sender: Option<Sender<AppMsg>> ,
    connect_node_sender: MpscSender<TcpStream>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>> ,
    last_command_sent: Arc<Mutex<String>> ,
    response: Vec<&str>,
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
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Mutex lock failed: {}", e),
            )));
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
    ui_sender: Option<Sender<AppMsg>> ,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>> ,
    last_command_sent: Arc<Mutex<String>> ,
) -> std::io::Result<()> {
    for stream in reciever {
        let cloned_node_streams = Arc::clone(&node_streams);
        let cloned_last_command = Arc::clone(&last_command_sent);
        let cloned_sender = ui_sender.clone();
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
