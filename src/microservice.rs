use std::collections::HashMap;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::fs;
#[allow(unused_imports)]
use std::net::TcpListener;
use std::net::TcpStream;
#[allow(unused_imports)]
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::{channel, Sender as MpscSender};
use std::sync::{Arc, Mutex};
use std::thread;
#[path = "documento.rs"]
mod documento;
use documento::Documento;
#[allow(unused_imports)]
use std::time::Duration;
#[path = "utils/logger.rs"]
mod logger;
#[path = "utils/redis_parser.rs"]
mod redis_parser;

//
pub struct Microservice {
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    last_command_sent: Arc<Mutex<String>>,
    documents: Arc<Mutex<HashMap<String, Documento>>>,
    log_path: String,

}

impl Microservice {
    pub fn new(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let log_path = logger::get_log_path_from_config(config_path);
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
        Ok(Microservice {
            node_streams: Arc::new(Mutex::new(HashMap::new())),
            last_command_sent: Arc::new(Mutex::new("".to_string())),
            documents: Arc::new(Mutex::new(HashMap::new())),
            log_path,
        })
    }

    pub fn start(&self, redis_port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let main_address = format!("127.0.0.1:{}", redis_port);

        println!("Conectándome al server de redis en {:?}", main_address);
        let mut socket: TcpStream = TcpStream::connect(&main_address)?;
        logger::log_event(
            &self.log_path,
            &format!(
                "Microservicio conectandose al server de redis en {:?}",
                main_address
            ),
        );
        let (connect_node_sender, connect_nodes_receiver) = channel::<TcpStream>();

        let redis_socket = socket.try_clone()?;
        let redis_socket_clone_for_hashmap = socket.try_clone()?;

        let command = "Microservicio\r\n".to_string();

        println!("Enviando: {:?}", command);
        logger::log_event(&self.log_path, &format!("Microservicio envia {:?}", command));

        self.start_node_connection_handler(connect_node_sender.clone(), connect_nodes_receiver);

        {
            match self.node_streams.lock() {
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

                    let parts: Vec<&str> = "Microservicio".split_whitespace().collect();
                    let resp_command = format_resp_command(&parts);
                    extra_socket.write_all(resp_command.as_bytes())?;

                    match self.node_streams.lock() {
                        Ok(mut map) => {
                            map.insert(addr.clone(), extra_socket.try_clone()?);
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
            let node_streams_clone = Arc::clone(&self.node_streams);
            let _main_address_clone = main_address.clone();
            let _last_command_sent_clone = Arc::clone(&self.last_command_sent);

            thread::spawn(move || loop {
                match node_streams_clone.lock() {
                    Ok(_streams) => {
                        /* if let Some(mut stream) = streams.get(&main_address_clone) {
                            let command_parts = vec!["SET", "docprueba.txt", ""];
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
                        } */
                    }
                    Err(e) => {
                        eprintln!("Error obteniendo lock de node_streams: {}", e);
                    }
                }

                thread::sleep(Duration::from_secs(61812100));
            });
        }
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    fn start_node_connection_handler(
        &self,
        connect_node_sender: MpscSender<TcpStream>,
        connect_nodes_receiver: Receiver<TcpStream>,
    ) {
        let cloned_node_streams = Arc::clone(&self.node_streams);
        let cloned_last_command = Arc::clone(&self.last_command_sent);
        let cloned_documents = Arc::clone(&self.documents);
        let log_path = self.log_path.clone();

        thread::spawn(move || {
            if let Err(e) = Self::connect_to_nodes(
                connect_node_sender,
                connect_nodes_receiver,
                cloned_node_streams,
                cloned_last_command,
                cloned_documents,
                &log_path,
            ) {
                eprintln!("Error en la conexión con el nodo: {}", e);
            }
        });
    }

    fn connect_to_nodes(
        sender: MpscSender<TcpStream>,
        reciever: Receiver<TcpStream>,
        node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
        last_command_sent: Arc<Mutex<String>>,
        documents: Arc<Mutex<HashMap<String, Documento>>>,
        log_path: &str,
    ) -> std::io::Result<()> {
        for stream in reciever {
            let cloned_node_streams = Arc::clone(&node_streams);
            let cloned_last_command = Arc::clone(&last_command_sent);
            let cloned_documents = Arc::clone(&documents);
            let cloned_own_sender = sender.clone();
            let log_path_clone = log_path.to_string();

            thread::spawn(move || {
                if let Err(e) = Self::listen_to_redis_response(
                    stream,
                    cloned_own_sender,
                    cloned_node_streams,
                    cloned_last_command,
                    cloned_documents,
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
        documents: Arc<Mutex<HashMap<String, Documento>>>,
        log_path: &str,
    ) -> std::io::Result<()> {
        if let Ok(peer_addr) = microservice_socket.peer_addr() {
            println!("Escuchando respuestas del nodo: {}", peer_addr);
        }

        let mut reader = BufReader::new(microservice_socket.try_clone()?);
        loop {
            let (parts, _) = redis_parser::parse_resp_command(&mut reader)?;
            if parts.is_empty() {
                break;
            }
            println!("partes: {:#?}", parts);
            let first_response = parts[0].to_uppercase();

            match first_response.as_str() {
                "subscribe" => {
                    println!("alguien se suscribio");
                }
                s if s.starts_with("-ERR") => {}
                "DOC" if parts.len() >= 2 => {
                    let doc_name = &parts[1];
                    let content = &parts[2..];

                    println!(
                        "Documento recibido: {} con {} líneas",
                        doc_name,
                        content.len()
                    );
                    logger::log_event(
                        log_path,
                        &format!(
                            "Documento recibido: {} con {} líneas",
                            doc_name,
                            content.len()
                        ),
                    );
                    let is_calc = doc_name.ends_with(".xslx");
                    if let Ok(mut docs) = documents.lock() {
                        let documento = if is_calc {
                            Documento::Calculo(content.to_vec())
                        } else {
                            Documento::Texto(content.to_vec())
                        };
                        docs.insert(doc_name.to_string(), documento);
                        println!("Documento '{}' guardado en el microservicio", doc_name);
                    } else {
                        eprintln!("Error obteniendo lock de documents");
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }


}



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

                let parts: Vec<&str> = "Microservicio".split_whitespace().collect();
                let resp_command = format_resp_command(&parts);
                extra_socket.write_all(resp_command.as_bytes())?;

                match node_streams.lock() {
                    Ok(mut map) => {
                        map.insert(addr.clone(), extra_socket.try_clone()?);
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
        let _main_address_clone = main_address.clone();
        let _last_command_sent_clone = Arc::clone(&last_command_sent);

        thread::spawn(move || loop {
            match node_streams_clone.lock() {
                Ok(_streams) => {
                    /* if let Some(mut stream) = streams.get(&main_address_clone) {
                        let command_parts = vec!["SET", "docprueba.txt", ""];
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
                    } */
                }
                Err(e) => {
                    eprintln!("Error obteniendo lock de node_streams: {}", e);
                }
            }

            thread::sleep(Duration::from_secs(61812100));
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
    if let Ok(peer_addr) = microservice_socket.peer_addr() {
        println!("Escuchando respuestas del nodo: {}", peer_addr);
    }

    let mut reader = BufReader::new(microservice_socket.try_clone()?);
    loop {
        let (parts, _) = redis_parser::parse_resp_command(&mut reader)?;
        if parts.is_empty() {
            break;
        }
        println!("partes: {:#?}", parts);
        let first_response = parts[0].to_uppercase();

        match first_response.as_str() {
            "subscribe" => {
                println!("alguien se suscribio");
            }
            s if s.starts_with("-ERR") => {}
            s if s.starts_with("CLIENT") => {
                /* let response_client: Vec<&str> = response[1].split('|').collect();
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
                } */
            }
            s if s.starts_with("UPDATE-FILES") => {
                /* let parts: Vec<&str> = line_clone.trim_end_matches('\n').split('|').collect();

                let doc_name: &str = parts[1];
                let index = parts[2];
                let text: &str = parts[3];
                let notification = format!("UPDATE-CLIENT|{}|{}|{}", doc_name, index, text);
                let command_parts = vec!["PUBLISH", doc_name, &notification];
                let resp_command = format_resp_command(&command_parts);
                if let Err(e) = microservice_socket.write_all(resp_command.as_bytes()) {
                    eprintln!("Error al enviar mensaje de actualizacion de archivo: {}", e);
                    logger::log_event(
                        log_path,
                        &format!("Error al enviar mensaje de actualizacion de archivo: {}", e),
                    );
                } */
            }
            "DOC" if parts.len() >= 2 => {
                let doc_name = &parts[1];
                let content = &parts[2..];

                println!("Documento recibido: {} con ", doc_name);
                logger::log_event(
                    log_path,
                    &format!(
                        "Documento recibido: {} con {} líneas",
                        doc_name,
                        content.len()
                    ),
                );

                // Aquí puedes procesar el contenido completo del documento
                /* for (i, line) in content.iter().enumerate() {
                    if !line.is_empty() {
                        println!("Línea {}: {}", i, line);
                    }
                } */
            }

            s if s.contains("WRITE|") => {
                /* let parts: Vec<&str> = if response.len() > 1 {
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
                } */
            }
            /* "ASK" => {
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
            } */
            _ => {}
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