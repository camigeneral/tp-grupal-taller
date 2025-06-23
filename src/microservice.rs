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
#[path = "utils/logger.rs"]
mod logger;
#[path = "utils/redis_parser.rs"]
mod redis_parser;

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
    if let Ok(metadata) = fs::metadata(&log_path) {
        if metadata.len() > 0 {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
            {
                let _ = writeln!(file);
            }
        }
    }

    println!("Conectándome al server de redis en {:?}", main_address);
    let mut socket: TcpStream = TcpStream::connect(&main_address)?;
    logger::log_event(
        &log_path,
        &format!(
            "Microservicio conectandose al server de redis en {:?}",
            main_address
        ),
    );
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
                // logger::log_event(&log_path, &format!("Error en la conexión con el nodo: {}", cloned_last_command.lock()));
            }
        });
    }

    {
        match node_streams.lock() {
            Ok(mut map) => {
                map.insert(
                    main_address.clone(),
                    redis_socket_clone_for_hashmap.try_clone()?,
                );
            }
            Err(e) => {
                eprintln!("Error obteniendo lock de node_streams: {}", e);
            }
        }
    }

    let parts: Vec<&str> = command.split_whitespace().collect();
    let resp_command = format_resp_command(&parts);
    println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));
    socket.write_all(resp_command.as_bytes())?;

    connect_node_sender.send(redis_socket)?;

    let otros_puertos = vec![4001, 4002];
    for port in otros_puertos {
        let addr = format!("127.0.0.1:{}", port);
        match TcpStream::connect(&addr) {
            Ok(mut extra_socket) => {
                println!("Microservicio conectado a nodo adicional: {}", addr);

                // Identificarse como microservicio también
                let parts: Vec<&str> = "Microservicio".split_whitespace().collect();
                let resp_command = format_resp_command(&parts);
                extra_socket.write_all(resp_command.as_bytes())?;

                match node_streams.lock() {
                    Ok(mut map) => {
                        map.insert(
                            main_address.clone(),
                            redis_socket_clone_for_hashmap.try_clone()?,
                        );
                    }
                    Err(e) => {
                        eprintln!("Error obteniendo lock de node_streams: {}", e);
                    }
                }

                connect_node_sender.send(extra_socket)?;
            }
            Err(e) => {
                eprintln!("Error al conectar con nodo {}: {}", addr, e);
            }
        }
    }
    {
        let node_streams_clone = Arc::clone(&node_streams);
        let main_address_clone = main_address.clone();
        let last_command_sent_clone = Arc::clone(&last_command_sent);

        thread::spawn(move || loop {
            match node_streams_clone.lock() {
                Ok(streams) => {
                    if let Some(mut stream) = streams.get(&main_address_clone) {
                        let command_parts = vec!["SET", "docprueba.txt", "hola"];
                        let resp_command = format_resp_command(&command_parts);

                        match last_command_sent_clone.lock() {
                            Ok(mut last_command) => {
                                *last_command = resp_command.clone();
                            }
                            Err(e) => {
                                eprintln!("Error obteniendo lock de last_command_sent: {}", e);
                            }
                        }

                        if let Err(e) = stream.write_all(resp_command.as_bytes()) {
                            eprintln!("Error al enviar comando SET docprueba hola: {}", e);
                        } else {
                            println!("Comando automático enviado: SET docprueba hola");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error obteniendo lock de node_streams: {}", e);
                }
            }

            thread::sleep(Duration::from_secs(60));
        });
    }
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
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

        logger::log_event(
            log_path,
            &format!("Respuesta de redis en el microservicio: {}", line),
        );

        let response: Vec<&str> = line.split_whitespace().collect();

        let first = response[0].to_uppercase();
        let first_response = first.as_str();

        match first_response {
            s if s.starts_with("-ERR") => {}
            "CLIENT" => {
                // se va a procesar lo que otro agrego
                let response_client: Vec<&str> = response[1].split('|').collect();
                let client_address = response_client[0];
                let doc_name = response_client[1];

                let bienvenida = format!("Welcome {} {}", doc_name, client_address);

                let parts: Vec<&str> = bienvenida.split_whitespace().collect();

                let mensaje_final = format_resp_command(&parts);

                if let Err(e) = microservice_socket.write_all(mensaje_final.as_bytes()) {
                    eprintln!("Error al enviar mensaje de bienvenida: {}", e);
                    logger::log_event(
                        log_path,
                        &format!("Error al enviar mensaje de bienvenida: {}", e),
                    );
                }
            }
            s if s.starts_with("UPDATE-FILES") => {
                if response.len() >= 2 {
                    let doc_name = response[1];
                    let command_parts = vec!["PUBLISH", doc_name, "UPDATE-FILES-CLIENT"];
                    let resp_command = format_resp_command(&command_parts);
                    if let Err(e) = microservice_socket.write_all(resp_command.as_bytes()) {
                        eprintln!("Error al enviar mensaje de actualizacion de archivo: {}", e);
                        logger::log_event(
                            log_path,
                            &format!("Error al enviar mensaje de actualizacion de archivo: {}", e),
                        );
                    }
                }
            }

            s if s.contains("WRITE|") => {
                let parts: Vec<&str> = if response.len() > 1 {
                    line.trim_end_matches('\n').split('|').collect()
                } else {
                    response[0].trim_end_matches('\n').split('|').collect()
                };

                if parts.len() == 4 {
                    let line_number: &str = parts[1];
                    let text = parts[2];
                    let file_name = parts[3];

                    let command_parts = ["add_content", file_name, line_number, text];

                    let resp_command = format_resp_command(&command_parts);
                    {
                        let mut last_command = last_command_sent.lock().unwrap();
                        *last_command = resp_command.clone();
                    }
                    println!("RESP enviado: {}", resp_command);
                    microservice_socket.write_all(resp_command.as_bytes())?;
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
            "NODEFILES" => {
                let paths = match std::fs::read_dir(".") {
                    Ok(entries) => entries
                        .filter_map(|entry| {
                            let entry = entry.ok()?;
                            let path = entry.path();
                            let fname = path.file_name()?.to_str()?.to_string();
                            if fname.starts_with("redis_node_") && fname.ends_with(".rdb") {
                                Some(fname)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>(),
                    Err(e) => {
                        eprintln!("Error leyendo directorio actual: {}", e);
                        vec![]
                    }
                };

                let response = paths.join(",");
                redis_parser::write_response(
                    &microservice_socket,
                    &redis_parser::CommandResponse::String(response),
                )?;
                continue;
            }
            _ => {}
        }

        /*         let response: Vec<&str> = line.split_whitespace().collect();

               let first = response[0].to_uppercase();
               let _first_response = first.as_str();
        */
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

fn send_command_to_nodes(
    connect_node_sender: MpscSender<TcpStream>,
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>,
    response: Vec<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let last_line_cloned = match last_command_sent.lock() {
        Ok(lock) => lock.clone(),
        Err(e) => {
            eprintln!("Error obteniendo lock de last_command_sent: {}", e);
            return Ok(());
        }
    };

    let mut locked_node_streams = match node_streams.lock() {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("Error obteniendo lock de node_streams: {}", e);
            return Ok(());
        }
    };

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
