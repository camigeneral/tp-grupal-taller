extern crate relm4;
// use self::relm4::Sender;
use std::collections::HashMap;
use std::env::args;
use std::io::Write;
use std::io::{BufRead, BufReader};
#[allow(unused_imports)]
use std::net::TcpListener;
use std::net::TcpStream;
#[allow(unused_imports)]
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::{channel, Sender as MpscSender};
use std::sync::{Arc, Mutex};
use std::thread;
#[allow(unused_imports)]
use std::time::Duration;
#[path = "utils/logger.rs"]
mod logger;


pub fn main() -> Result<(), Box<dyn std::error::Error>> {

    let redis_port = 4000;
    let main_address = format!("127.0.0.1:{}", redis_port);

    let node_streams: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));
    let last_command_sent: Arc<Mutex<String>> = Arc::new(Mutex::new("".to_string()));

    let config_path = "redis.conf";
    let log_path = logger::get_log_path_from_config(config_path);
    // Canal para conectar y lanzar escuchas por cada nodo
    let (connect_node_sender, connect_nodes_receiver) = channel::<TcpStream>();

    use std::fs;
    if fs::metadata(&log_path).map(|m| m.len() > 0).unwrap_or(false) {
        let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .and_then(|mut file| writeln!(file, ""));
    }

    println!("Conectándome al server de redis en {:?}", main_address);
    let mut socket: TcpStream = TcpStream::connect(&main_address)?;
    logger::log_event(&log_path, &format!("Microservicio conectandose al server de redis en {:?}", main_address));
    let redis_socket = socket.try_clone()?;
    let redis_socket_clone_for_hashmap = socket.try_clone()?;

    let command = "Microservicio\r\n".to_string();

    println!("Enviando: {:?}", command);
    logger::log_event(&log_path, &format!("Microservicio envia {:?}", command));

    {
        let cloned_node_streams = Arc::clone(&node_streams);
        let cloned_last_command = Arc::clone(&last_command_sent);
        let connect_node_sender_cloned = connect_node_sender.clone();
    
        thread::spawn(move || {
            if let Err(e) = connect_to_nodes(
                connect_node_sender_cloned,
                connect_nodes_receiver,
                cloned_node_streams,
                cloned_last_command,
                &log_path,
            ) {
                eprintln!("Error en la conexión con el nodo: {}", e);
                // logger::log_event(&log_path, &format!("Error en la conexión con el nodo: {}", cloned_last_command.lock().unwrap()));
            }
        });
    }   

    
    {
        node_streams.lock().unwrap().insert(main_address.clone(), redis_socket_clone_for_hashmap);
    }

    let parts: Vec<&str> = command.split_whitespace().collect();
    let resp_command = format_resp_command(&parts);
    println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));
    socket.write_all(resp_command.as_bytes())?;

    
    connect_node_sender.send(redis_socket)?;

    
    let otros_puertos = vec![4001, 4002]; // <-- agregá más si hace falta
    for port in otros_puertos {
        let addr = format!("127.0.0.1:{}", port);
        match TcpStream::connect(&addr) {
            Ok(mut extra_socket) => {
                println!("Microservicio conectado a nodo adicional: {}", addr);

                // Identificarse como microservicio también
                let parts: Vec<&str> = "Microservicio".split_whitespace().collect();
                let resp_command = format_resp_command(&parts);
                extra_socket.write_all(resp_command.as_bytes())?;

                let clone_for_map = extra_socket.try_clone()?;
                node_streams.lock().unwrap().insert(addr.clone(), clone_for_map);

                connect_node_sender.send(extra_socket)?;
            }
            Err(e) => {
                eprintln!("Error al conectar con nodo {}: {}", addr, e);
            }
        }
    }

    loop {}
}

fn connect_to_nodes(
    sender: MpscSender<TcpStream>,
    reciever: Receiver<TcpStream>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>,
    log_path: &str,
) -> std::io::Result<()> {
    for stream in reciever {
        let cloned_node_streams = Arc::clone(&node_streams);
        let cloned_last_command = Arc::clone(&last_command_sent);
        let cloned_own_sender = sender.clone();
        let log_path_clone = log_path.to_string();

        thread::spawn(move || {
            if let Err(e) = listen_to_redis_response(
                stream,
                cloned_own_sender,
                cloned_node_streams,
                cloned_last_command,
                &log_path_clone,
            ) {
                eprintln!("Error en la conexión con el nodo: {}", e);
            }
        });
    }

    Ok(())
}

fn listen_to_redis_response(
    mut microservice_socket: TcpStream,
    connect_node_sender: MpscSender<TcpStream>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>,
    log_path: &str,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(microservice_socket.try_clone()?);
    loop {
        let _ = connect_node_sender.clone();
        let _ = last_command_sent.clone();
        let _ = node_streams.clone();

        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        println!("Respuesta de redis: {}", line);
        logger::log_event(&log_path, &format!("Respuesta de redis: {}", line));

        // client-address qty_users
        if line.starts_with("Client ") && line.contains(" subscribed to ") {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.len() >= 5 {
                let client_addr = parts[1];

                let doc_name = parts[4];

                let bienvenida = format!("Welcome {} {}", doc_name, client_addr);

                let parts: Vec<&str> = bienvenida.split_whitespace().collect();

                let mensaje_final = format_resp_command(&parts);

                if let Err(e) = microservice_socket.write_all(mensaje_final.as_bytes()) {
                    eprintln!("Error al enviar mensaje de bienvenida: {}", e);
                    logger::log_event(&log_path, &format!("Error al enviar mensaje de bienvenida: {}", e));
                }
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn format_resp_command(command_parts: &[&str]) -> String {
    let mut resp_message = format!("*{}\r\n", command_parts.len());

    for part in command_parts {
        resp_message.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
    }

    resp_message
}