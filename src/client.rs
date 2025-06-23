extern crate relm4;
use self::relm4::Sender;
use crate::app::AppMsg;
use std::collections::HashMap;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::{channel, Sender as MpscSender};
use std::sync::{Arc, Mutex};
use std::thread;
use utils::redis_parser::{format_resp_publish, format_resp_command};

pub fn client_run(
    port: u16,
    rx: Receiver<String>,
    ui_sender: Option<Sender<AppMsg>>,
) -> std::io::Result<()> {
    let node_streams: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));
    let last_command_sent: Arc<Mutex<String>> = Arc::new(Mutex::new("".to_string()));

    let address = format!("127.0.0.1:{}", port);
    let cloned_address = address.clone();

    //println!("Conectándome al server de redis en {:?}", address);
    let mut socket: TcpStream = TcpStream::connect(address.clone())?;

    let command = "Cliente\r\n".to_string();

    println!("Enviando: {:?}", command);
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
            let resp_command = format_resp_publish(parts[0], parts[1]);

            println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

            socket.write_all(resp_command.as_bytes())?;
            break;
        } else {

            
            println!("Enviando: {:?}", command);

            let parts: Vec<&str> = command.split_whitespace().collect();
            let resp_command =
             if parts[0] == "AUTH" || parts[0] == "subscribe"  || parts[0] == "unsubscribe" {
                    format_resp_command(&parts)
                }else{
                    if parts[0].contains("WRITE") {
                        let splited_command: Vec<&str> = command.split("|").collect();
                        format_resp_publish(splited_command[3], &command)
                    } else {
                        format_resp_publish(parts[1], &command)
                    }
                };

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

fn listen_to_redis_response(
    client_socket: TcpStream,
    ui_sender: Option<Sender<AppMsg>>,
    connect_node_sender: MpscSender<TcpStream>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>,
) -> std::io::Result<()> {
    let client_socket_cloned = client_socket.try_clone()?;
    let mut reader = BufReader::new(client_socket);

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            break;
        }

        println!("Respuesta de redis: {}", line);

        let response: Vec<&str> = line.split_whitespace().collect();

        let first = response[0].to_uppercase();
        let first_response = first.as_str();
        let local_addr: std::net::SocketAddr = client_socket_cloned.local_addr()?;

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
                
                if socket != local_addr.to_string() {
                    continue;
                }

                if let Some(sender) = &ui_sender {
                    let _ = sender.send(AppMsg::ManageSubscribeResponse(
                        response_status[1].to_string(),
                    ));
                }
            }

            s if s.starts_with("UPDATE-FILES-CLIENT") => {
                if let Some(sender) = &ui_sender {
                    println!("Recargar");
                    let _ = sender.send(AppMsg::RefreshData);
                }
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
    _ui_sender: Option<Sender<AppMsg>>,
    connect_node_sender: MpscSender<TcpStream>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>,
    response: Vec<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let last_line_cloned = last_command_sent.lock().unwrap().clone();
    let mut locked_node_streams = node_streams.lock().unwrap();
    let new_node_address = response[2].to_string();

    println!("Ultimo comando ejecutado: {:#?}", last_line_cloned);
    println!("Redirigiendo a nodo: {}", new_node_address);

    if let Some(stream) = locked_node_streams.get_mut(&new_node_address) {
        println!("Usando conexión existente al nodo {}", new_node_address);
        stream.write_all(last_line_cloned.as_bytes())?;
    } else {
        println!("Creando nueva conexión al nodo {}", new_node_address);
        let parts: Vec<&str> = "connect".split_whitespace().collect();
        let resp_command = format_resp_command(&parts);
        let mut final_stream = TcpStream::connect(new_node_address.clone())?;
        final_stream.write_all(resp_command.as_bytes())?;

        let mut cloned_stream_to_connect = final_stream.try_clone()?;
        locked_node_streams.insert(new_node_address, final_stream);

        let _ = connect_node_sender.send(cloned_stream_to_connect.try_clone()?);
        std::thread::sleep(std::time::Duration::from_millis(2));

        if let Err(e) = cloned_stream_to_connect.write_all(last_line_cloned.as_bytes()) {
            eprintln!("Error al reenviar el último comando: {}", e);
        }
    }
    Ok(())
}

fn connect_to_nodes(
    sender: MpscSender<TcpStream>,
    reciever: Receiver<TcpStream>,
    ui_sender: Option<Sender<AppMsg>>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>,
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
