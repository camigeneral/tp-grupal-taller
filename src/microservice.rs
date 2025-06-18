extern crate relm4;
use self::relm4::Sender;
use std::collections::HashMap;
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
use std::env::args;
#[path = "utils/logger.rs"]
mod logger;

static REQUIRED_ARGS: usize = 2;


pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args: Vec<String> = args().collect();
    if cli_args.len() != REQUIRED_ARGS {
        eprintln!("Error: Cantidad de argumentos inválida");
        eprintln!("Uso: {} <puerto>", cli_args[0]);
        return Err("Error: Cantidad de argumentos inválida".into());
    }

    let redis_port = match cli_args[1].parse::<usize>() {
        Ok(n) => n,
        Err(_e) => return Err("Failed to parse arguments".into()),
    };

    let node_streams: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));
    let last_command_sent: Arc<Mutex<String>> = Arc::new(Mutex::new("".to_string()));

    let address = format!("127.0.0.1:{}", redis_port);
    let cloned_address = address.clone();

    let config_path = "redis.conf";
    let log_path = logger::get_log_path_from_config(config_path);
    
    use std::fs;
    if fs::metadata(&log_path).map(|m| m.len() > 0).unwrap_or(false) {
        let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .and_then(|mut file| writeln!(file, ""));
    }

    println!("Conectándome al server de redis en {:?}", address);
    logger::log_event(&log_path, &format!("Microservicio conectandose al server de redis en {:?}", address));
    let mut socket: TcpStream = TcpStream::connect(address)?;

    let command = "Microservicio\r\n".to_string();

    println!("Enviando: {:?}", command);
    logger::log_event(&log_path, &format!("Microservicio envia {:?}", command));
    let parts: Vec<&str> = command.split_whitespace().collect();
    let resp_command = format_resp_command(&parts);

    println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

    socket.write_all(resp_command.as_bytes())?;

    let redis_socket = socket.try_clone()?;
    let redis_socket_clone_for_hashmap = socket.try_clone()?;

    {
        let mut locked_node_streams = node_streams.lock().unwrap();
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
            cloned_node_streams,
            cloned_last_command,
            &log_path,
        ) {
            eprintln!("Error en la conexión con el nodo: {}", e);
            logger::log_event(&log_path, &format!("Error en la conexión con el nodo: {}", command));
        }
    });

    let _ = connect_node_sender.send(redis_socket);

    loop{
        
    }
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
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        println!("Respuesta de redis: {}", line);
        logger::log_event(&log_path, &format!("Respuesta de redis: {}", line));


        if line.starts_with("Client ") && line.contains(" subscribed to ") {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.len() >= 5 {
                let client_addr = parts[1];

                let doc_name = parts[4];

                let bienvenida = format!("Welcome {} {}",doc_name, client_addr);
                

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

pub fn format_resp_command(command_parts: &[&str]) -> String {
    let mut resp_message = format!("*{}\r\n", command_parts.len());

    for part in command_parts {
        resp_message.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
    }

    resp_message
}