use std::collections::HashMap;
use std::io::Write;
use std::io::{BufReader};
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

        self.add_node_stream(&main_address, redis_socket_clone_for_hashmap)?;

        let parts: Vec<&str> = command.split_whitespace().collect();
        let resp_command = redis_parser::format_resp_command(&parts);
        println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));
        socket.write_all(resp_command.as_bytes())?;

        connect_node_sender.send(redis_socket)?;

        self.connect_to_replica_nodes(&connect_node_sender)?;

        self.start_automatic_commands();
        
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    fn connect_to_replica_nodes(&self,
        connect_node_sender: &MpscSender<TcpStream>,) -> Result<(), Box<dyn std::error::Error>> {
            let otros_puertos = vec![4001, 4002];
            for port in otros_puertos {
                let addr = format!("127.0.0.1:{}", port);
                match TcpStream::connect(&addr) {
                    Ok(mut extra_socket) => {
                        println!("Microservicio conectado a nodo adicional: {}", addr);
    
                        let parts: Vec<&str> = "Microservicio".split_whitespace().collect();
                        let resp_command = redis_parser::format_resp_command(&parts);
                        extra_socket.write_all(resp_command.as_bytes())?;
    
                        self.add_node_stream(&addr, extra_socket.try_clone()?)?;
                        connect_node_sender.send(extra_socket)?;
                    }
                    Err(e) => {
                        eprintln!("Error al conectar con nodo {}: {}", addr, e);
                    }
                }
            }
            Ok(())
        }

    fn start_automatic_commands(&self) {
        let node_streams_clone = Arc::clone(&self.node_streams);
        let _last_command_sent_clone = Arc::clone(&self.last_command_sent);

        thread::spawn(move || loop {
            match node_streams_clone.lock() {
                Ok(_streams) => {
                }
                Err(e) => {
                    eprintln!("Error obteniendo lock de node_streams: {}", e);
                }
            }

            thread::sleep(Duration::from_secs(61812100));
        });
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
    
    fn add_node_stream(
        &self,
        address: &str,
        stream: TcpStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.node_streams.lock() {
            Ok(mut map) => {
                map.insert(address.to_string(), stream);
                Ok(())
            }
            Err(e) => {
                eprintln!("Error obteniendo lock de node_streams: {}", e);
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Error obteniendo lock de node_streams: {}", e),
                )))
            }
        }
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
    let config_path = "redis.conf";
    let microservice = Microservice::new(config_path)?;
    microservice.start(4000)
}