use std::collections::HashMap;
use std::env;
use std::io::BufRead;
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
use std::collections::HashSet;
use std::thread;
extern crate rusty_docs;
use self::logger::*;
use self::shared::MicroserviceMessage;
use document::Document;
use rusty_docs::document;
use rusty_docs::logger;
use rusty_docs::resp_parser::{format_resp_command, format_resp_publish, parse_resp_command};
use rusty_docs::shared;
use std::thread::sleep;
use std::time::Duration;

/// Microservicio que actúa como intermediario entre clientes y nodos Redis.
///
/// Esta estructura maneja las conexiones TCP con múltiples nodos Redis,
/// procesa comandos RESP (Redis Serialization Protocol), y almacena documentos
/// recibidos de los nodos. Proporciona funcionalidad para:
/// - Conectar a múltiples nodos Redis (principal y réplicas)
/// - Escuchar y procesar respuestas de los nodos
/// - Almacenar documentos recibidos en memoria
/// - Registrar eventos en un archivo de log
#[derive(Debug)]
pub struct Microservice {
    /// Mapa de conexiones TCP activas con los nodos Redis.
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,

    /// El último comando enviado a los nodos Redis.
    /// Se mantiene para referencia y debugging.
    last_command_sent: Arc<Mutex<String>>,

    /// Documents almacenados en memoria recibidos de los nodos Redis.
    documents: Arc<Mutex<HashMap<String, Document>>>,

    /// Mapeo de documentos a stream_ids para saber a qué stream enviar cada documento.
    document_streams: Arc<Mutex<HashMap<String, String>>>,

    /// Ruta al archivo de log donde se registran los eventos del microservicio.
    logger: Logger,
    llm_sender: Option<MpscSender<String>>,
    processed_responses: Arc<Mutex<HashSet<String>>>,
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
            "0000".parse()?,
        );
        Ok(Microservice {
            node_streams: Arc::new(Mutex::new(HashMap::new())),
            last_command_sent: Arc::new(Mutex::new("".to_string())),
            documents: Arc::new(Mutex::new(HashMap::new())),
            document_streams: Arc::new(Mutex::new(HashMap::new())),
            logger,
            llm_sender: None,
            processed_responses: Arc::new(Mutex::new(HashSet::new()))

        })
    }

    fn connect_to_llm(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let llm_address = format!("llm_microservice:4030");
        let (tx, rx) = channel::<String>();

        self.llm_sender = Some(tx);
        self.logger.log(&format!(
            "Microservicio conectandose al server de llm en {:?}",
            llm_address
        ));
        let node_streams = Arc::clone(&self.node_streams);

        thread::spawn(move || {
            let mut socket = loop {
                match TcpStream::connect(llm_address.clone()) {
                    Ok(s) => break s,
                    Err(e) => {
                        eprintln!(
                            "No se pudo conectar al LLM: {}. Reintentando en 15 segundos...",
                            e
                        );
                        sleep(Duration::from_secs(15));
                    }
                }
            };
            let mut reader = BufReader::new(socket.try_clone().unwrap());

            for prompt in rx {
                if prompt.trim().is_empty() {
                    break;
                }

                let prompt = format!("{}\n", prompt.trim().trim_end_matches("\n"));
                if let Err(e) = socket.write_all(prompt.as_bytes()) {
                    eprintln!("Error escribiendo al LLM: {}", e);
                    break;
                }
                if let Err(e) = socket.flush() {
                    eprintln!("Error flusheando al LLM: {}", e);
                    break;
                }

                let mut response = String::new();
                if let Err(e) = reader.read_line(&mut response) {
                    eprintln!("Error leyendo del LLM: {}", e);
                    break;
                }

                let parts: Vec<&str> = response
                    .split(' ')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();

                if parts.len() < 2 {
                    eprintln!("Respuesta malformada del LLM: {:?}", response);
                    continue;
                }

                let document = parts[1];
                let message = format_resp_command(&parts);
                let resp = format_resp_publish(document, &message);

                if let Ok(mut streams) = node_streams.lock() {
                    for (id, stream) in streams.iter_mut() {
                        if let Err(e) = stream.write_all(resp.as_bytes()) {
                            eprintln!("Error escribiendo a nodo {}: {}", id, e);
                        } else {
                            println!("Respuesta del LLM enviada a nodo {}: {}", id, resp);
                            let _ = stream.flush();
                        }
                    }
                } else {
                    eprintln!("Error obteniendo lock de node_streams");
                }
            }
        });

        Ok(())
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
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let main_address = format!("node0:4000");

        println!("Conectándome al server de redis en {:?}", main_address);
        let mut socket: TcpStream = TcpStream::connect(&main_address)?;
        self.logger.log(&format!(
            "Microservicio conectandose al server de redis en {:?}",
            main_address
        ));
        let (connect_node_sender, connect_nodes_receiver) = channel::<TcpStream>();
        self.connect_to_llm()?;
        let redis_socket = socket.try_clone()?;
        let redis_socket_clone_for_hashmap = socket.try_clone()?;

        let command: String = "Microservicio\r\n".to_string();

        println!("Enviando: {:?}", command);
        self.logger
            .log(&format!("Microservicio envia {:?}", command));

        self.start_node_connection_handler(connect_node_sender.clone(), connect_nodes_receiver);

        self.add_node_stream(&main_address, redis_socket_clone_for_hashmap)?;

        let parts: Vec<&str> = command.split_whitespace().collect();
        let resp_command = format_resp_command(&parts);
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
        let other_ports = get_nodes_addresses();
        for addr in other_ports {
            // let addr = format!("127.0.0.1:{}", port);
            match TcpStream::connect(&addr) {
                Ok(mut extra_socket) => {
                    self.logger.log(&format!("Microservicio envia {:?}", addr));
                    println!("Microservicio conectado a nodo adicional: {}", addr);

                    let parts: Vec<&str> = "Microservicio".split_whitespace().collect();
                    let resp_command = format_resp_command(&parts);
                    extra_socket.write_all(resp_command.as_bytes())?;

                    self.add_node_stream(&addr, extra_socket.try_clone()?)?;
                    connect_node_sender.send(extra_socket)?;
                }
                Err(e) => {
                    println!("Error al conectar con nodo {}: {}", addr, e);
                }
            }
        }
        Ok(())
    }

    fn get_document_data(documento: &Document) -> String {
        match documento {
            Document::Text(lines) => {
                let mut data: String = format!("");
                for linea in lines {
                    data.push_str(linea);
                    data.push_str("/--/");
                }
                data
            }
            Document::Spreadsheet(lines) => {
                let mut data: String = format!("");
                for linea in lines {
                    data.push_str(linea);
                    data.push_str("/--/");
                }
                data
            }
        }
    }

    /// Inicia el procesamiento automático de comandos en un hilo separado.
    ///
    /// Este método crea un hilo que se ejecuta en segundo plano y realiza
    /// verificaciones periódicas de las conexiones de nodos y persistencia de documentos.       
    fn start_automatic_commands(&self) {
        let node_streams_clone = Arc::clone(&self.node_streams);
        let last_command_sent_clone = Arc::clone(&self.last_command_sent);
        let documents_clone = Arc::clone(&self.documents);
        let logger_clone = self.logger.clone();

        thread::spawn(move || loop {
            match node_streams_clone.lock() {
                Ok(_streams) => {}
                Err(e) => {
                    println!("Error obteniendo lock de node_streams: {}", e);
                    logger_clone.log(&format!("Error obteniendo lock de node_streams: {}", e));
                }
            }

            if let Ok(docs) = documents_clone.lock() {
                if let Ok(mut streams) = node_streams_clone.lock() {
                    logger_clone.log(&format!(
                        "Enviando comandos SET para persistir {} documentos",
                        docs.len()
                    ));

                    for (doc_name, documento) in docs.iter() {
                        let document_data = Self::get_document_data(documento);

                        // Enviar a todos los nodos disponibles
                        for (stream_id, stream) in streams.iter_mut() {
                            let set_parts = vec!["SET", doc_name, &document_data];
                            let set_command = format_resp_command(&set_parts);

                            logger_clone.log(&format!(
                                "Enviando comando SET para persistir documento {} en nodo {}: {}",
                                doc_name, stream_id, set_command
                            ));

                            if let Err(e) = stream.write_all(set_command.as_bytes()) {
                                println!("Error enviando comando SET a nodo {}: {}", stream_id, e);
                                logger_clone.log(&format!(
                                    "Error enviando comando SET a nodo {}: {}",
                                    stream_id, e
                                ));
                                continue;
                            } else {
                                let _ = stream.flush();
                                logger_clone.log(&format!(
                                    "Comando SET enviado exitosamente a nodo {}",
                                    stream_id
                                ));
                            }

                            if let Ok(mut last_command) = last_command_sent_clone.lock() {
                                *last_command = set_command;
                            }
                        }
                    }
                } else {
                    println!("Error obteniendo lock de node_streams para persistencia");
                    logger_clone.log("Error obteniendo lock de node_streams para persistencia");
                }
            } else {
                println!("Error obteniendo lock de documents para persistencia");
                logger_clone.log("Error obteniendo lock de documents para persistencia");
            }

            thread::sleep(Duration::from_secs(3));
        });
    }

    fn start_node_connection_handler(
        &self,
        connect_node_sender: MpscSender<TcpStream>,
        connect_nodes_receiver: Receiver<TcpStream>,
    ) {
        let cloned_node_streams = Arc::clone(&self.node_streams);
        let cloned_last_command = Arc::clone(&self.last_command_sent);
        let cloned_documents: Arc<Mutex<HashMap<String, Document>>> = Arc::clone(&self.documents);
        let cloned_document_streams = Arc::clone(&self.document_streams);
        let logger = self.logger.clone();
        let llm_sender: Option<MpscSender<String>> = self.llm_sender.clone();
        let proccesed_commands: Arc<Mutex<HashSet<String>>> = Arc::clone(&self.processed_responses);

        thread::spawn(move || {
            if let Err(e) = Self::connect_to_nodes(
                connect_node_sender,
                connect_nodes_receiver,
                cloned_node_streams,
                cloned_last_command,
                cloned_documents,
                cloned_document_streams,
                logger,
                llm_sender,
                proccesed_commands
            ) {
                println!("Error en la conexión con el nodo: {}", e);
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
        sender: MpscSender<TcpStream>,
        reciever: Receiver<TcpStream>,
        node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
        last_command_sent: Arc<Mutex<String>>,
        documents: Arc<Mutex<HashMap<String, Document>>>,
        document_streams: Arc<Mutex<HashMap<String, String>>>,
        logger: Logger,
        llm_sender: Option<MpscSender<String>>,
        processed_responses: Arc<Mutex<HashSet<String>>>
    ) -> std::io::Result<()> {
        for stream in reciever {
            let cloned_node_streams = Arc::clone(&node_streams);
            let cloned_documents = Arc::clone(&documents);
            let cloned_document_streams = Arc::clone(&document_streams);
            let cloned_last_command = Arc::clone(&last_command_sent);
            let cloned_own_sender = sender.clone();
            let log_clone = logger.clone();
            let llm_sender: Option<MpscSender<String>> = llm_sender.clone();
            let proccesed_commands_clone: Arc<Mutex<HashSet<String>>> = Arc::clone(&processed_responses);

            thread::spawn(move || {
                let logger_clone = log_clone.clone();
                if let Err(e) = Self::listen_to_redis_response(
                    stream,
                    cloned_own_sender,
                    cloned_node_streams,
                    cloned_documents,
                    cloned_document_streams,
                    cloned_last_command,
                    log_clone,
                    llm_sender,
                    proccesed_commands_clone
                ) {
                    logger_clone.log(&format!("Error en la conexión con el nodo: {}", e));
                    println!("Error en la conexión con el nodo: {}", e);
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
        documents: Arc<Mutex<HashMap<String, Document>>>,
        _document_streams: Arc<Mutex<HashMap<String, String>>>,
        last_command_sent: Arc<Mutex<String>>,
        log_clone: Logger,
        llm_sender: Option<MpscSender<String>>,
        processed_responses: Arc<Mutex<HashSet<String>>>
    ) -> std::io::Result<()> {
        if let Ok(peer_addr) = microservice_socket.peer_addr() {
            println!("Escuchando respuestas del nodo: {}", peer_addr);
        }

        let mut reader = BufReader::new(microservice_socket.try_clone()?);
        loop {
            let llm_sender_clone = llm_sender.clone();
            let (parts, _) = parse_resp_command(&mut reader)?;
            if parts.is_empty() {
                break;
            }
            let message: MicroserviceMessage = MicroserviceMessage::from_parts(&parts);
            match message {
                MicroserviceMessage::ClientSubscribed {
                    document,
                    client_id,
                } => {
                    if let Ok(docs) = documents.lock() {
                        if let Some(documento) = docs.get(&document) {
                            let doc_content = match documento {
                                Document::Text(lines) => lines.join(","),
                                Document::Spreadsheet(lines) => lines.join(","),
                            };
                            let message_parts = &[
                                "status",
                                &document.clone(),
                                &client_id.clone(),
                                &doc_content.clone(),
                            ];
                            let message_resp = format_resp_command(message_parts);
                            let command_resp =
                                format_resp_publish(&document.clone(), &message_resp);
                            println!(
                                "Enviando publish: {}",
                                command_resp.replace("\r\n", "\\r\\n")
                            );
                            log_clone.log(&format!(
                                "Enviando publish para client-subscribed: {}",
                                command_resp
                            ));
                            if let Err(e) = microservice_socket.write_all(command_resp.as_bytes()) {
                                println!(
                                    "Error al enviar mensaje de actualizacion de archivo: {}",
                                    e
                                );
                                log_clone.log(&format!(
                                    "Error al enviar mensaje de actualizacion de archivo: {}",
                                    e
                                ));
                            } else {
                                let _ = microservice_socket.flush();
                                log_clone.log(&format!(
                                    "Enviando publish para client-subscribed: {}",
                                    command_resp
                                ));
                            }
                        }
                    } else {
                        println!("Error obteniendo lock de documents para client-subscribed");
                        log_clone.log("Error obteniendo lock de documents para client-subscribed");
                    }
                },
                MicroserviceMessage::Doc {
                    document,
                    content,
                    stream_id,
                } => {
                    log_clone.log(&format!(
                        "Document recibido: {} con {} líneas del stream {}",
                        document,
                        content.len(),
                        stream_id
                    ));
                    if let Ok(mut docs) = documents.lock() {
                        if document.ends_with(".txt") {
                            let lines: Vec<String> = content
                                .split("/--/")
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string())
                                .collect();
                            docs.insert(document.clone(), Document::Text(lines));
                        } else {
                            let mut rows: Vec<String> = content
                                .split("/--/")
                                .filter(|_| true)
                                .map(|s| s.to_string())
                                .collect();
                            if rows.is_empty() {
                                rows.push("".to_string());
                            }
                            while rows.len() < 100 {
                                rows.push(String::new());
                            }
                            docs.insert(document.clone(), Document::Spreadsheet(rows));
                        }
                    } else {
                        println!("Error obteniendo lock de documents");
                    }
                }
                MicroserviceMessage::Write {
                    index,
                    content,
                    file,
                } => {
                    log_clone.log(&format!(
                        "Write recibido: índice {}, contenido '{}', archivo {}",
                        index, content, file
                    ));
                    if let Ok(mut docs) = documents.lock() {
                        if let Some(documento) = docs.get_mut(&file) {
                            let parsed_index = match index.parse::<usize>() {
                                Ok(idx) => idx,
                                Err(e) => {
                                    println!("Error parseando índice: {}", e);
                                    log_clone.log(&format!("Error parseando índice: {}", e));
                                    continue;
                                }
                            };

                            match documento {
                                Document::Text(lines) => {
                                    if content.contains("<enter>") {
                                        let parts: Vec<&str> = content.split("<enter>").collect();

                                        if parts.len() == 2 {
                                            let before_newline = parts[0];
                                            let after_newline = parts[1];

                                            if parsed_index < lines.len() {
                                                lines[parsed_index] = before_newline.to_string();

                                                lines.insert(
                                                    parsed_index + 1,
                                                    after_newline.to_string(),
                                                );
                                            } else {
                                                while lines.len() < parsed_index {
                                                    lines.push(String::new());
                                                }
                                                lines.push(before_newline.to_string());
                                                lines.push(after_newline.to_string());
                                            }
                                        } else {
                                            log_clone.log(&format!(
                                                "Formato de salto de línea inválido: {}",
                                                content
                                            ));
                                        }
                                    } else if parsed_index < lines.len() {
                                        lines[parsed_index] = content.clone();
                                    } else {
                                        lines.push(content.clone());
                                    }
                                }
                                Document::Spreadsheet(lines) => {
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
                            log_clone.log(&format!("Document no encontrado: {}", file));
                        }
                        {
                            let write_parts = vec!["write", &index, &content, "to", &file];
                            let resp_command = format_resp_command(&write_parts);
                            let mut last_command = last_command_sent.lock().unwrap();
                            *last_command = resp_command.clone();
                        }
                    } else {
                        log_clone.log("Error obteniendo lock de documents para write");
                    }
                },
                 MicroserviceMessage::ClientLlmResponse { document, content, selection_mode, line, offset } => {
                    log_clone.log(&format!(
                        "LLMResponse recibido: documento {}, selection_mode {}, línea {:?}, offset {:?}",
                        document, selection_mode, line, offset
                    ));
                    println!(
                        "LLMResponse recibido: documento {}, selection_mode {}, línea {:?}, offset {:?}, contenido: {:?}",
                        document, selection_mode, line, offset, content);
                    let response_id = format!("{}-{}-{}-{}-{}", document, content, selection_mode, line, offset);
                    if let Ok(mut processed) = processed_responses.lock() {
                        if parts[0].to_uppercase() == "CLIENT-LLM-RESPONSE" {
                            if processed.contains(&response_id) {
                                println!("Respuesta duplicada detectada, omitiendo: {}", parts.join(" "));
                                continue;
                            }
                            processed.insert(response_id);
                        }
        
                        if processed.len() > 1000 {
                            processed.clear();
                        }
                    }
                    if let Ok(mut docs) = documents.lock() {
                        if let Some(documento) = docs.get(&document) {
                            match selection_mode.as_str() {
                                "whole-file" => {
                                    let lines: Vec<String> = content.split("<enter>").map(|s| s.to_string()).collect();
                                    let new_document = Document::Text(lines.clone());
                                    docs.insert(document.clone(), new_document);
                                    log_clone.log(&format!("Documento '{}' actualizado (whole-file) con {} líneas", document, lines.len()));
                                    println!("Documento '{}' actualizado (whole-file) con {} líneas", document, lines.len());
                                },
                                "cursor" => {
                                    let parsed_line = match line.parse::<usize>() {
                                        Ok(idx) => idx,
                                        Err(e) => {
                                            println!("Error parseando índice: {}", e);
                                            log_clone.log(&format!("Error parseando índice: {}", e));
                                            continue;
                                        }
                                    };

                                    let parsed_offset = match offset.parse::<usize>() {
                                        Ok(idx) => idx,
                                        Err(e) => {
                                            println!("Error parseando índice: {}", e);
                                            log_clone.log(&format!("Error parseando índice: {}", e));
                                            continue;
                                        }
                                    };                                    

                                    match documento {
                                        Document::Text(doc_lines) => {
                                            let mut new_lines = doc_lines.clone();
                                            if parsed_line < new_lines.len() {
                                                let original_line = &decode_text(new_lines[parsed_line].to_string());                                                
                                                let offset = parsed_offset.min(original_line.len());
                                                let mut new_line = String::new();
                                                let parsed_content = &decode_text(content.to_string()); 
                                                new_line.push_str(&original_line[..offset]);
                                                new_line.push_str(" ");
                                                new_line.push_str(&parsed_content);
                                                new_line.push_str(" ");
                                                new_line.push_str(&original_line[offset..]);
                                                new_line = parse_text(new_line);
                                                new_lines[parsed_line] = new_line;
                                                let new_document = Document::Text(new_lines);
                                                docs.insert(document.clone(), new_document);
                                                println!("Insertado en documento '{}' en línea {}, offset {}: {}", document, parsed_line, parsed_offset, content);
                                            } else {
                                                //log_clone.log(&format!("Línea {} fuera de rango para documento '{}'", line_num, document));
                                            }
                                        },
                                        _ => {}
                                    };                                                                            
                                },
                                _ => {
                                    log_clone.log(&format!("Modo de selección desconocido en LLMResponse: {}", selection_mode));
                                }
                            }
                        } else {
                            println!("Documento no encontrado para LLMResponse: {}", document);
                            log_clone.log(&format!("Documento no encontrado para LLMResponse: {}", document));
                        }
                    } else {
                        println!("Error obteniendo lock de documents para LLMResponse");
                        log_clone.log("Error obteniendo lock de documents para LLMResponse");
                    }
                },
                MicroserviceMessage::RequestFile { document, prompt } => {                    
                    if let Ok(mut docs) = documents.lock() {
                        if let Some(documento) = docs.get_mut(&document) {
                            let content = match documento {
                                Document::Text(lines) => {

                                    lines.join("<enter>").to_string()
                                },                            
                                _ => String::new()
                            };
                            let message_parts = &[
                                "file-requested",
                                &document.clone(),
                                &content.clone(),
                                &prompt.clone(),                        
                            ];
                            let message_resp = format_resp_command(message_parts);
                            let command_resp = format_resp_publish(&"llm_requests", &message_resp);                            
                            if let Err(e) = microservice_socket.write_all(command_resp.as_bytes()) {
                                println!(
                                    "Error al enviar mensaje de actualizacion de archivo: {}",
                                    e
                                );
                            } else {
                                let _ = microservice_socket.flush();
                                
                            }
                        }
                    }
                },    
                MicroserviceMessage::Error(_) => {}
                _ => {}
            }
        }
        Ok(())
    }

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
                println!("Error obteniendo lock de node_streams: {}", e);
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Error obteniendo lock de node_streams: {}", e),
                )))
            }
        }
    }
}

pub fn parse_text(value: String)-> String {
    let val = value.clone();
    let mut value_clone = if value.trim_end_matches('\n').is_empty() {
        "<delete>".to_string()
    } else {
        val.replace('\n', "<enter>")
    };
    value_clone = value_clone.replace(' ', "<space>");
    return value_clone;
}

pub fn decode_text(value: String)-> String {
    let  value_clone = value.clone();
    value_clone
        .replace("<space>", " ")
        .replace("<enter>", "\n")
        .replace("<delete>", "")
}

fn get_nodes_addresses() -> Vec<String> {
    match env::var("REDIS_NODE_HOSTS") {
        Ok(val) => val.split(',').map(|s| s.to_string()).collect(),
        Err(_) => {
            eprintln!("REDIS_NODE_HOSTS no está seteada");
            vec![]
        }
    }
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = "microservice.conf";
    let mut microservice = Microservice::new(config_path)?;
    microservice.start()
}
