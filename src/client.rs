extern crate relm4;
use self::relm4::Sender;
use crate::app::AppMsg;
use std::collections::HashMap;
use std::io::Write;
use std::io::{BufRead, BufReader};
#[allow(unused_imports)]
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
#[allow(unused_imports)]
use std::time::Duration;
use utils::redis_parser::format_resp_command;

pub fn client_run (
    port: u16,
    rx: Receiver<String>,
    ui_sender: Option<Sender<AppMsg>>,
) -> std::io::Result<()> {
    let node_streams: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));
    let last_command_sent: Arc<Mutex<String>> = Arc::new(Mutex::new("".to_string()));

    let address = format!("127.0.0.1:{}", port);
    let cloned_address = address.clone();

    println!("Conectándome al server de redis en {:?}", address);
    let mut socket: TcpStream = TcpStream::connect(address)?;

    let redis_socket = socket.try_clone()?;
    let redis_socket_clone_for_hashmap = socket.try_clone()?;

    {
        let mut locked_node_streams = node_streams.lock().unwrap();
        locked_node_streams.insert(cloned_address, redis_socket_clone_for_hashmap);
    }

    let cloned_node_streams = Arc::clone(&node_streams);
    let cloned_last_command = Arc::clone(&last_command_sent);

    thread::spawn(move || {
        if let Err(e) = listen_to_redis_response(redis_socket, ui_sender, cloned_node_streams, cloned_last_command) {
            eprintln!("Error en la conexión con el nodo: {}", e);
        }
    });

    for command in rx {
        let trimmed_command = command.to_string().trim().to_lowercase();        
        if trimmed_command == "close" {
            println!("Desconectando del servidor");
            break;
        } else {
            println!("Enviando: {:?}", command);

            let parts: Vec<&str> = command.split_whitespace().collect();
            let resp_command = format_resp_command(&parts);

            {
                let mut last_command = last_command_sent.lock().unwrap();
                *last_command = resp_command.clone();
            }

            println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

            socket.write_all(resp_command.as_bytes())?;
        }
    }

    Ok(())
}


fn listen_to_redis_response (
    microservice_socket: TcpStream,
    ui_sender: Option<Sender<AppMsg>>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>
) -> std::io::Result<()> {
    let mut reader = BufReader::new(microservice_socket);
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            break;
        }

        println!("Respuesta de redis: {}", line);

        let response: Vec<&str> = line.split_whitespace().collect();
        if response[0].to_uppercase() == "MOVED" {
            if response.len() < 3 {
                println!("Nodo de redireccion no disponible");
            } else {
                let last_line_cloned = last_command_sent.lock().unwrap().clone();
                let mut locked_node_streams = node_streams.lock().unwrap();
                let new_node_address = response[2].to_string();

                if let Some(stream) = locked_node_streams.get_mut(&new_node_address) {
                    stream.write_all(last_line_cloned.as_bytes())?;
                } else {
                    let stream: TcpStream = TcpStream::connect(new_node_address.clone())?;
                    let mut cloned_stream = stream.try_clone()?;
                    let stream_cloned_for_thread = stream.try_clone()?;
                    locked_node_streams.insert(new_node_address, stream);

                    cloned_stream.write_all(last_line_cloned.as_bytes())?;

                    let cloned_node_streams = Arc::clone(&node_streams);
                    let cloned_last_command = Arc::clone(&last_command_sent);
                    let cloned_sender = ui_sender.clone();

                    thread::spawn(move || {
                        if let Err(e) = listen_to_redis_response(stream_cloned_for_thread, cloned_sender, cloned_node_streams, cloned_last_command) {
                            eprintln!("Error en la conexión con el nodo: {}", e);
                        }
                    });
                }
            }
        }
    }
    Ok(())
}