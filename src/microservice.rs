use std::collections::HashMap;
use std::io::BufReader;
use std::io::Write;
#[allow(unused_imports)]
use std::net::TcpListener;
use std::net::TcpStream;
#[allow(unused_imports)]
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::{channel, Sender as MpscSender};
use std::sync::{Arc, Mutex};
use std::thread;
#[path = "document.rs"]
mod document;
use document::Documento;
#[allow(unused_imports)]
use std::time::Duration;

#[path = "utils/logger.rs"]
mod logger;
use self::logger::*;
#[path = "utils/redis_parser.rs"]
mod redis_parser;
#[path = "shared.rs"]
mod shared;
use self::shared::MicroserviceMessage;

/// Microservicio que actúa como intermediario entre clientes y nodos Redis.
///
/// Esta estructura maneja las conexiones TCP con múltiples nodos Redis,
/// procesa comandos RESP (Redis Serialization Protocol), y almacena documentos
/// recibidos de los nodos. Proporciona funcionalidad para:
/// - Conectar a múltiples nodos Redis (principal y réplicas)
/// - Escuchar y procesar respuestas de los nodos
/// - Almacenar documentos recibidos en memoria
/// - Registrar eventos en un archivo de log
pub struct Microservice {
    /// Mapa de conexiones TCP activas con los nodos Redis.
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,

    /// El último comando enviado a los nodos Redis.
    /// Se mantiene para referencia y debugging.
    last_command_sent: Arc<Mutex<String>>,

    /// Documentos almacenados en memoria recibidos de los nodos Redis.
    documents: Arc<Mutex<HashMap<String, Documento>>>,

    /// Ruta al archivo de log donde se registran los eventos del microservicio.
    logger: Logger,
}

impl Microservice {
    /// Crea una nueva instancia del microservicio.
    ///
    /// # Argumentos
    ///
    /// * `config_path` - Ruta al archivo de configuración que contiene la configuración del log.
    ///
    /// # Retorna
    ///
    /// * `Ok(Microservice)` - Una nueva instancia del microservicio inicializada.
    /// * `Err(Box<dyn std::error::Error>)` - Error si no se puede leer la configuración o crear el archivo de log.
    ///
    pub fn new(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let logger = logger::Logger::init(
            logger::Logger::get_log_path_from_config(config_path),
            "0000".parse().unwrap(),
        );
        Ok(Microservice {
            node_streams: Arc::new(Mutex::new(HashMap::new())),
            last_command_sent: Arc::new(Mutex::new("".to_string())),
            documents: Arc::new(Mutex::new(HashMap::new())),
            logger,
        })
    }

    /// Inicia el microservicio y establece las conexiones con los nodos Redis.
    ///
    /// Este método realiza las siguientes operaciones:
    /// 1. Se conecta al nodo Redis principal en el puerto especificado.
    /// 2. Envía el comando de identificación "Microservicio" al nodo.
    /// 3. Inicia el manejador de conexiones de nodos en un hilo separado.
    /// 4. Se conecta a nodos réplica.
    /// 5. Inicia el procesamiento automático de comandos.
    /// 6. Entra en un bucle infinito para mantener el microservicio activo.
    ///
    /// # Argumentos
    ///
    /// * `redis_port` - Puerto del nodo Redis principal al cual conectarse.
    ///
    /// # Retorna
    ///
    /// * `Ok(())` - El microservicio se inició correctamente.
    /// * `Err(Box<dyn std::error::Error>)` - Error si no se puede conectar al nodo Redis o establecer las conexiones.
    pub fn start(&self, redis_port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let main_address = format!("127.0.0.1:{}", redis_port);

        println!("Conectándome al server de redis en {:?}", main_address);
        let mut socket: TcpStream = TcpStream::connect(&main_address)?;
        self.logger.log(&format!(
            "Microservicio conectandose al server de redis en {:?}",
            main_address
        ));
        let (connect_node_sender, connect_nodes_receiver) = channel::<TcpStream>();

        let redis_socket = socket.try_clone()?;
        let redis_socket_clone_for_hashmap = socket.try_clone()?;

        let command = "Microservicio\r\n".to_string();

        println!("Enviando: {:?}", command);
        self.logger
            .log(&format!("Microservicio envia {:?}", command));

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

    /// Conecta el microservicio a nodos Redis réplica adicionales.
    ///
    /// Este método intenta conectarse a nodos Redis en los puertos 4001 y 4002.
    /// Para cada conexión exitosa:
    /// - Envía el comando de identificación "Microservicio"
    /// - Agrega el stream TCP al mapa de conexiones
    /// - Envía el stream al manejador de conexiones
    ///
    /// Si una conexión falla, se registra el error pero el proceso continúa
    /// con los demás nodos.
    ///
    /// # Argumentos
    ///
    /// * `connect_node_sender` - Sender para enviar streams TCP al manejador de conexiones.
    ///
    /// # Retorna
    ///
    /// * `Ok(())` - Las conexiones se establecieron correctamente (aunque algunas puedan haber fallado).
    /// * `Err(Box<dyn std::error::Error>)` - Error si no se puede escribir en algún stream TCP.
    fn connect_to_replica_nodes(
        &self,
        connect_node_sender: &MpscSender<TcpStream>,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    /// Inicia el procesamiento automático de comandos en un hilo separado.
    ///
    /// Este método crea un hilo que se ejecuta en segundo plano y realiza
    /// verificaciones periódicas de las conexiones de nodos.       
    fn start_automatic_commands(&self) {
        let node_streams_clone = Arc::clone(&self.node_streams);
        let _last_command_sent_clone = Arc::clone(&self.last_command_sent);
        let logger_clone = self.logger.clone();

        thread::spawn(move || loop {
            match node_streams_clone.lock() {
                Ok(_streams) => {}
                Err(e) => {
                    eprintln!("Error obteniendo lock de node_streams: {}", e);
                    logger_clone.log(&format!("Error obteniendo lock de node_streams: {}", e));
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
        let cloned_documents: Arc<Mutex<HashMap<String, Documento>>> = Arc::clone(&self.documents);
        let logger = self.logger.clone();

        thread::spawn(move || {
            if let Err(e) = Self::connect_to_nodes(
                connect_node_sender,
                connect_nodes_receiver,
                cloned_node_streams,
                cloned_last_command,
                cloned_documents,
                logger,
            ) {
                eprintln!("Error en la conexión con el nodo: {}", e);
            }
        });
    }

    /// Inicia el manejador de conexiones de nodos en un hilo separado.
    ///
    /// Este método crea un hilo que se encarga de procesar las conexiones
    /// entrantes de los nodos Redis. Utiliza un canal de comunicación
    /// para recibir streams TCP de nuevos nodos y los procesa en paralelo.
    ///
    /// # Argumentos
    ///
    /// * `connect_node_sender` - Sender para enviar streams TCP al manejador.
    /// * `connect_nodes_receiver` - Receiver para recibir streams TCP de nuevos nodos.
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

    /// Procesa las conexiones entrantes de los nodos Redis.
    ///
    /// Esta función recibe streams TCP de nuevos nodos a través de un canal y
    /// lanza un hilo por cada conexión para escuchar las respuestas de cada nodo.
    ///
    /// # Argumentos
    ///
    /// * `sender` - Canal para enviar streams TCP a otros manejadores si es necesario.
    /// * `reciever` - Canal para recibir streams TCP de nuevos nodos.
    /// * `node_streams` - Referencia compartida al mapa de streams de nodos.
    /// * `last_command_sent` - Referencia compartida al último comando enviado.
    /// * `documents` - Referencia compartida a los documentos almacenados.
    /// * `log_path` - Ruta al archivo de log.
    ///
    /// # Retorna
    ///
    /// * `Ok(())` si todas las conexiones se procesaron correctamente.
    /// * `Err(std::io::Error)` si ocurre un error en algún hilo.
    fn connect_to_nodes(
        _sender: MpscSender<TcpStream>,
        reciever: Receiver<TcpStream>,
        _node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
        _last_command_sent: Arc<Mutex<String>>,
        documents: Arc<Mutex<HashMap<String, Documento>>>,
        logger: Logger,
    ) -> std::io::Result<()> {
        for stream in reciever {
            let cloned_node_streams = Arc::clone(&_node_streams);
            let cloned_documents = Arc::clone(&documents);
            let cloned_own_sender = _sender.clone();
            let log_clone = logger.clone();

            thread::spawn(move || {
                if let Err(e) = Self::listen_to_redis_response(
                    stream,
                    cloned_own_sender,
                    cloned_node_streams,
                    cloned_documents,
                    log_clone,
                ) {
                    eprintln!("Error en la conexión con el nodo: {}", e);
                }
            });
        }

        Ok(())
    }
    /// Escucha y procesa las respuestas recibidas de un nodo Redis.
    ///
    /// Esta función se ejecuta en un hilo separado para cada conexión de nodo.
    /// Lee comandos RESP del nodo, procesa documentos recibidos y registra eventos.
    ///
    /// # Argumentos
    ///
    /// * `microservice_socket` - Stream TCP con el nodo Redis.
    /// * `connect_node_sender` - Canal para enviar streams TCP a otros manejadores si es necesario.
    /// * `node_streams` - Referencia compartida al mapa de streams de nodos.
    /// * `last_command_sent` - Referencia compartida al último comando enviado.
    /// * `documents` - Referencia compartida a los documentos almacenados.
    /// * `log_path` - Ruta al archivo de log.
    ///
    /// # Retorna
    ///
    /// * `Ok(())` si la escucha y el procesamiento fueron exitosos.
    /// * `Err(std::io::Error)` si ocurre un error de IO.
    fn listen_to_redis_response(
        mut microservice_socket: TcpStream,
        _connect_node_sender: MpscSender<TcpStream>,
        _node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
        documents: Arc<Mutex<HashMap<String, Documento>>>,
        log_clone: Logger,
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
            let message = MicroserviceMessage::from_parts(&parts);
            println!("message: {:#?}", message);
            match message {
                MicroserviceMessage::ClientSubscribed {
                    document,
                    client_id,
                } => {
                    if let Ok(docs) = documents.lock() {
                        if let Some(documento) = docs.get(&document) {
                            let doc_content = match documento {
                                Documento::Texto(lines) => lines.join(","),
                                Documento::Calculo(lines) => lines.join(","),
                            };
                            let message_parts = &[
                                "status",
                                &document.clone(),
                                &client_id.clone(),
                                &doc_content.clone(),
                            ];
                            let message_resp = redis_parser::format_resp_command(message_parts);
                            let command_resp =
                                redis_parser::format_resp_publish(&document.clone(), &message_resp);
                            println!(
                                "Enviando publish: {}",
                                command_resp.replace("\r\n", "\\r\\n")
                            );
                            log_clone.log(&format!(
                                "Enviando publish para client-subscribed: {}",
                                command_resp
                            ));
                            if let Err(e) = microservice_socket.write_all(command_resp.as_bytes()) {
                                eprintln!(
                                    "Error al enviar mensaje de actualizacion de archivo: {}",
                                    e
                                );
                                log_clone.log(&format!(
                                    "Error al enviar mensaje de actualizacion de archivo: {}",
                                    e
                                ));
                            }
                        } else {
                            eprintln!("Documento no encontrado: {}", document);
                            log_clone.log(&format!("Documento no encontrado: {}", document));
                        }
                    } else {
                        eprintln!("Error obteniendo lock de documents para client-subscribed");
                        log_clone.log("Error obteniendo lock de documents para client-subscribed");
                    }
                }
                MicroserviceMessage::Doc { document, content } => {
                    println!(
                        "Documento recibido: {} con {} líneas",
                        document,
                        content.len()
                    );
                    log_clone.log(&format!(
                        "Documento recibido: {} con {} líneas",
                        document,
                        content.len()
                    ));                    
                    if let Ok(mut docs) = documents.lock() {                        
                        if document.ends_with(".txt") {
                            let messages: Vec<String> = content
                                .split("/--/")
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string())
                                .collect();
                            docs.insert(document.clone(), Documento::Texto(messages));
                        } else {
                            let mut rows: Vec<String> =
                                content.split("/--/").map(|s| s.to_string()).collect();
                
                            while rows.len() < 100 {
                                rows.push(String::new());
                            }
                
                            docs.insert(document.clone(), Documento::Calculo(rows));
                        }            
                        println!("docs: {:#?}", docs);
                    } else {
                        eprintln!("Error obteniendo lock de documents");
                    }
                }
                MicroserviceMessage::Write { index, content, file } => {
                    log_clone.log(&format!(
                        "Write recibido: índice {}, contenido '{}', archivo {}",
                        index, content, file
                    ));
                    if let Ok(mut docs) = documents.lock() {
                        if let Some(documento) = docs.get_mut(&file) {
                            let parsed_index = match index.parse::<usize>() {
                                Ok(idx) => idx,
                                Err(e) => {
                                    eprintln!("Error parseando índice: {}", e);
                                    log_clone.log(&format!("Error parseando índice: {}", e));
                                    continue;
                                }
                            };

                            match documento {
                                Documento::Texto(lines) => {
                                    if content.contains("<enter>") {
                                        let parts: Vec<&str> = content.split("<enter>").collect();
                                        
                                        if parts.len() == 2 {
                                            let before_newline = parts[0];
                                            let after_newline = parts[1];
                                            
                                            if parsed_index < lines.len() {
                                                lines[parsed_index] = before_newline.to_string();
                                                
                                                lines.insert(parsed_index + 1, after_newline.to_string());
                                            } else {
                                                while lines.len() < parsed_index {
                                                    lines.push(String::new());
                                                }
                                                lines.push(before_newline.to_string());
                                                lines.push(after_newline.to_string());
                                            }
                                        } else {
                                            log_clone.log(&format!("Formato de salto de línea inválido: {}", content));
                                        }
                                    } else if parsed_index < lines.len() {
                                        lines[parsed_index] = content.clone();
                                    } else {
                                        lines.push(content.clone());
                                    }
                                }
                                Documento::Calculo(lines) => {
                                    if parsed_index < lines.len() {
                                        lines[parsed_index] = content.clone();
                                    } else {
                                        while lines.len() <= parsed_index {
                                            lines.push(String::new());
                                        }
                                        lines[parsed_index] = content.clone();
                                    }                                  
                                }
                            }
                        } else {
                            log_clone.log(&format!("Documento no encontrado: {}", file));
                        }
                    } else {
                        log_clone.log("Error obteniendo lock de documents para write");
                    }
                },
                MicroserviceMessage::Error(_) => {}
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