// Standard library imports
use std::{
    collections::{HashMap, HashSet},
    env,
    io::{BufReader, Write},
    net::TcpStream,
    sync::{
        mpsc::{channel, Receiver, Sender as MpscSender},
        Arc, Mutex,
    },
    thread,
    thread::sleep,
    time::Duration,
};

// External crate imports
extern crate rusty_docs;

// Local imports from rusty_docs
use rusty_docs::{
    document::Document,
    logger::{self, Logger},
    resp_parser::{format_resp_command, format_resp_publish, parse_resp_command},
    shared::MicroserviceMessage,
    vars::DOCKER,
};

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

    /// Ruta al archivo de log donde se registran los eventos del microservicio.
    logger: Logger,

    /// Conjunto de respuestas ya procesadas para evitar duplicados.
    /// Se utiliza para identificar y omitir mensajes duplicados del LLM.
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
            logger::Logger::get_log_path_from_config(config_path, "microservice_log_path="),
            "0000".parse()?,
        );
        Ok(Microservice {
            node_streams: Arc::new(Mutex::new(HashMap::new())),
            last_command_sent: Arc::new(Mutex::new("".to_string())),
            documents: Arc::new(Mutex::new(HashMap::new())),
            logger,
            processed_responses: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Envía datos a un stream y maneja errores de conexión
    ///
    /// Si el write falla con errores de broken pipe (32) o host unreachable (113),
    /// elimina el nodo de node_streams ya que la conexión no es válida.
    ///
    /// # Argumentos
    ///
    /// * `stream` - Stream TCP al que enviar los datos
    /// * `data` - Datos a enviar
    /// * `node_id` - Identificador del nodo para eliminarlo en caso de error
    /// * `node_streams` - Referencia a la colección de streams de nodos
    ///
    /// # Returns
    ///
    /// Result que indica éxito o error en el envío
    fn write_to_stream_with_error_handling(
        stream: &mut TcpStream,
        data: &[u8],
        node_id: &str,
        node_streams: &Arc<Mutex<HashMap<String, TcpStream>>>,
    ) -> std::io::Result<()> {
        match stream.write_all(data) {
            Ok(_) => {
                if let Err(e) = stream.flush() {
                    eprintln!(
                        "Error al hacer flush del stream del nodo {}: {}",
                        node_id, e
                    );
                }
                Ok(())
            }
            Err(e) => {
                // Verificar si es un error de broken pipe (32) o host unreachable (113)
                if e.raw_os_error() == Some(32) || e.raw_os_error() == Some(113) {
                    eprintln!(
                        "Error de conexión con nodo {}: {} (os error {:?})",
                        node_id,
                        e,
                        e.raw_os_error()
                    );

                    // Eliminar el nodo de node_streams
                    if let Ok(mut streams_guard) = node_streams.lock() {
                        streams_guard.remove(node_id);
                        println!(
                            "Nodo {} eliminado de node_streams debido a error de conexión",
                            node_id
                        );
                    } else {
                        eprintln!(
                            "Error obteniendo lock de node_streams para eliminar nodo {}",
                            node_id
                        );
                    }
                } else {
                    eprintln!("Error al escribir al nodo {}: {}", node_id, e);
                }
                Err(e)
            }
        }
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
        let mut socket: TcpStream = Self::connect_to_node_with_retry(&main_address, &self.logger)?;
        self.logger.log(&format!(
            "Microservicio conectandose al server de redis en {:?}",
            main_address
        ));
        let (connect_node_sender, connect_nodes_receiver) = channel::<TcpStream>();
        let redis_socket = socket.try_clone()?;
        let redis_socket_clone_for_hashmap = socket.try_clone()?;

        let command: String = "Microservicio\r\n".to_string();

        println!("Enviando: {:?}", command);
        self.logger
            .log(&format!("Microservicio envia {:?}", command));

        self.start_node_connection_handler(connect_nodes_receiver);

        self.add_node_stream(&main_address, redis_socket_clone_for_hashmap)?;

        let parts: Vec<&str> = command.split_whitespace().collect();
        let resp_command = format_resp_command(&parts);
        println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));
        if let Err(e) = Self::write_to_stream_with_error_handling(
            &mut socket,
            resp_command.as_bytes(),
            &main_address,
            &self.node_streams,
        ) {
            eprintln!(
                "Error al escribir al nodo principal {}: {}",
                main_address, e
            );
            return Err(Box::new(e));
        }

        connect_node_sender.send(redis_socket)?;

        self.connect_to_replica_nodes(&connect_node_sender)?;
        self.start_automatic_commands();

        loop {
            sleep(std::time::Duration::from_secs(1));
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
            match Self::connect_to_node_with_retry(&addr, &self.logger) {
                Ok(mut extra_socket) => {
                    self.logger.log(&format!("Microservicio envia {:?}", addr));
                    println!("Microservicio conectado a nodo adicional: {}", addr);

                    let parts: Vec<&str> = "Microservicio".split_whitespace().collect();
                    let resp_command = format_resp_command(&parts);
                    if let Err(e) = Self::write_to_stream_with_error_handling(
                        &mut extra_socket,
                        resp_command.as_bytes(),
                        &addr,
                        &self.node_streams,
                    ) {
                        eprintln!("Error al escribir al nodo réplica {}: {}", addr, e);
                        continue;
                    }

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

    /// Convierte un documento a formato de string para persistencia
    ///
    /// Esta función toma un documento (texto o spreadsheet) y lo convierte
    /// a un string con formato específico usando "/--/" como separador entre líneas.
    ///
    /// # Argumentos
    ///
    /// * `documento` - Referencia al documento a convertir
    ///
    /// # Returns
    ///
    /// String con el contenido del documento formateado para persistencia
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

                            // logger_clone.log(&format!(
                            //     "Enviando comando SET para persistir documento {} en nodo {}: {}",
                            //     doc_name, stream_id, set_command
                            // ));

                            if let Err(e) = Self::write_to_stream_with_error_handling(
                                stream,
                                set_command.as_bytes(),
                                stream_id,
                                &node_streams_clone,
                            ) {
                                println!("Error enviando comando SET a nodo {}: {}", stream_id, e);
                                logger_clone.log(&format!(
                                    "Error enviando comando SET a nodo {}: {}",
                                    stream_id, e
                                ));
                                continue;
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

    /// Inicia el manejador de conexiones de nodos en un hilo separado
    ///
    /// Esta función crea un hilo que se encarga de procesar las conexiones
    /// entrantes de los nodos Redis. Clona las referencias necesarias y
    /// delega el procesamiento a `connect_to_nodes`.
    ///
    /// # Argumentos
    ///
    /// * `connect_nodes_receiver` - Receiver para recibir streams TCP de nuevos nodos
    fn start_node_connection_handler(&self, connect_nodes_receiver: Receiver<TcpStream>) {
        let cloned_last_command = Arc::clone(&self.last_command_sent);
        let cloned_documents: Arc<Mutex<HashMap<String, Document>>> = Arc::clone(&self.documents);
        let logger = self.logger.clone();
        let proccesed_commands: Arc<Mutex<HashSet<String>>> = Arc::clone(&self.processed_responses);
        let node_streams_clone = Arc::clone(&self.node_streams);

        thread::spawn(move || {
            if let Err(e) = Self::connect_to_nodes(
                connect_nodes_receiver,
                cloned_last_command,
                cloned_documents,
                logger,
                proccesed_commands,
                node_streams_clone,
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
        reciever: Receiver<TcpStream>,
        last_command_sent: Arc<Mutex<String>>,
        documents: Arc<Mutex<HashMap<String, Document>>>,
        logger: Logger,
        processed_responses: Arc<Mutex<HashSet<String>>>,
        node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    ) -> std::io::Result<()> {
        for stream in reciever {
            let cloned_documents = Arc::clone(&documents);
            let cloned_last_command = Arc::clone(&last_command_sent);
            let log_clone = logger.clone();
            let proccesed_commands_clone: Arc<Mutex<HashSet<String>>> =
                Arc::clone(&processed_responses);
            let node_streams_clone = Arc::clone(&node_streams);

            thread::spawn(move || {
                let logger_clone = log_clone.clone();
                if let Err(e) = Self::listen_to_redis_response(
                    stream,
                    cloned_documents,
                    cloned_last_command,
                    log_clone,
                    proccesed_commands_clone,
                    node_streams_clone,
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
        documents: Arc<Mutex<HashMap<String, Document>>>,
        last_command_sent: Arc<Mutex<String>>,
        log_clone: Logger,
        processed_responses: Arc<Mutex<HashSet<String>>>,
        node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    ) -> std::io::Result<()> {
        if let Ok(peer_addr) = microservice_socket.peer_addr() {
            println!("Escuchando respuestas del nodo: {}", peer_addr);
        }

        let mut reader = BufReader::new(microservice_socket.try_clone()?);
        loop {
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
                            if let Err(e) = Self::write_to_stream_with_error_handling(
                                &mut microservice_socket,
                                command_resp.as_bytes(),
                                &document,
                                &node_streams,
                            ) {
                                println!(
                                    "Error al enviar mensaje de actualizacion de archivo: {}",
                                    e
                                );
                                log_clone.log(&format!(
                                    "Error al enviar mensaje de actualizacion de archivo: {}",
                                    e
                                ));
                            }
                        }
                    } else {
                        println!("Error obteniendo lock de documents para client-subscribed");
                        log_clone.log("Error obteniendo lock de documents para client-subscribed");
                    }
                }
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
                }
                MicroserviceMessage::ClientLlmResponse {
                    document,
                    content,
                    selection_mode,
                    line,
                    offset,
                } => {
                    log_clone.log(&format!(
                        "LLMResponse recibido: documento {}, selection_mode {}, línea {:?}, offset {:?}",
                        document, selection_mode, line, offset
                    ));
                    println!(
                        "LLMResponse recibido: documento {}, selection_mode {}, línea {:?}, offset {:?}, contenido: {:?}",
                        document, selection_mode, line, offset, content);
                    let response_id = format!(
                        "{}-{}-{}-{}-{}",
                        document, content, selection_mode, line, offset
                    );
                    if let Ok(mut processed) = processed_responses.lock() {
                        if parts[0].to_uppercase() == "CLIENT-LLM-RESPONSE" {
                            if processed.contains(&response_id) {
                                println!(
                                    "Respuesta duplicada detectada, omitiendo: {}",
                                    parts.join(" ")
                                );
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
                                    let lines: Vec<String> =
                                        content.split("<enter>").map(|s| s.to_string()).collect();
                                    let new_document = Document::Text(lines.clone());
                                    docs.insert(document.clone(), new_document);
                                    log_clone.log(&format!(
                                        "Documento '{}' actualizado (whole-file) con {} líneas",
                                        document,
                                        lines.len()
                                    ));
                                    println!(
                                        "Documento '{}' actualizado (whole-file) con {} líneas",
                                        document,
                                        lines.len()
                                    );
                                }
                                "cursor" => {
                                    let parsed_line = match line.parse::<usize>() {
                                        Ok(idx) => idx,
                                        Err(e) => {
                                            println!("Error parseando índice: {}", e);
                                            log_clone
                                                .log(&format!("Error parseando índice: {}", e));
                                            continue;
                                        }
                                    };

                                    let parsed_offset = match offset.parse::<usize>() {
                                        Ok(idx) => idx,
                                        Err(e) => {
                                            println!("Error parseando índice: {}", e);
                                            log_clone
                                                .log(&format!("Error parseando índice: {}", e));
                                            continue;
                                        }
                                    };

                                    match documento {
                                        Document::Text(doc_lines) => {
                                            let mut new_lines = doc_lines.clone();
                                            if parsed_line < new_lines.len() {
                                                let original_line = &decode_text(
                                                    new_lines[parsed_line].to_string(),
                                                );
                                                let offset = parsed_offset.min(original_line.len());
                                                let mut new_line = String::new();
                                                let parsed_content =
                                                    &decode_text(content.to_string());
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
                                        }
                                        _ => {}
                                    };
                                }
                                _ => {
                                    log_clone.log(&format!(
                                        "Modo de selección desconocido en LLMResponse: {}",
                                        selection_mode
                                    ));
                                }
                            }
                        } else {
                            println!("Documento no encontrado para LLMResponse: {}", document);
                            log_clone.log(&format!(
                                "Documento no encontrado para LLMResponse: {}",
                                document
                            ));
                        }
                    } else {
                        println!("Error obteniendo lock de documents para LLMResponse");
                        log_clone.log("Error obteniendo lock de documents para LLMResponse");
                    }
                }
                MicroserviceMessage::RequestFile { document, prompt } => {
                    if let Ok(mut docs) = documents.lock() {
                        if let Some(documento) = docs.get_mut(&document) {
                            let content = match documento {
                                Document::Text(lines) => lines.join("<enter>").to_string(),
                                _ => String::new(),
                            };
                            let message_parts = &[
                                "requested-file",
                                &document.clone(),
                                &content.clone(),
                                &prompt.clone(),
                            ];
                            let message_resp = format_resp_command(message_parts);
                            let command_resp = format_resp_publish(&"llm_requests", &message_resp);
                            if let Err(e) = Self::write_to_stream_with_error_handling(
                                &mut microservice_socket,
                                command_resp.as_bytes(),
                                &document,
                                &node_streams,
                            ) {
                                println!(
                                    "Error al enviar mensaje de actualizacion de archivo: {}",
                                    e
                                );
                            }
                        }
                    }
                }
                MicroserviceMessage::Error(_) => {}
                _ => {}
            }
        }
        Ok(())
    }

    /// Agrega un nuevo stream de nodo al mapa de conexiones activas
    ///
    /// Esta función agrega un stream TCP de un nodo Redis al mapa de conexiones
    /// activas del microservicio. La dirección del nodo se usa como clave.
    ///
    /// # Argumentos
    ///
    /// * `address` - Dirección del nodo Redis (formato: "host:puerto")
    /// * `stream` - Stream TCP conectado al nodo
    ///
    /// # Returns
    ///
    /// Result que indica éxito o error al agregar el stream
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

    /// Intenta conectarse a un nodo con reintentos y espera entre intentos.
    ///
    /// Esta función intenta conectarse a la dirección especificada usando TCP.
    /// Si la conexión falla, reintenta hasta un máximo de 15 veces, esperando
    /// 10 segundos entre cada intento. Si no logra conectarse, retorna un error.
    ///
    /// # Argumentos
    ///
    /// * `address` - Dirección del nodo Redis (formato: "host:puerto")
    /// * `logger` - Referencia al logger para registrar los eventos
    ///
    /// # Returns
    ///
    /// TcpStream conectado o un error si no se pudo conectar tras los reintentos
    fn connect_to_node_with_retry(address: &str, logger: &Logger) -> std::io::Result<TcpStream> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 15;
        const RETRY_DELAY_SECONDS: u64 = 10;

        loop {
            attempts += 1;
            logger.log(&format!("Intento {} de conectar a {}", attempts, address));
            println!("Intento {} de conectar a {}", attempts, address);

            match TcpStream::connect(address) {
                Ok(socket) => {
                    logger.log(&format!("Conexión exitosa a {}", address));
                    println!("Conexión exitosa a {}", address);
                    return Ok(socket);
                }
                Err(e) => {
                    eprintln!(
                        "Error conectando a {} (intento {}): {}",
                        address, attempts, e
                    );
                    logger.log(&format!(
                        "Error conectando a {} (intento {}): {}",
                        address, attempts, e
                    ));

                    if attempts >= MAX_ATTEMPTS {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::ConnectionRefused,
                            format!(
                                "No se pudo conectar a {} después de {} intentos",
                                address, MAX_ATTEMPTS
                            ),
                        ));
                    }

                    println!("Reintentando en {} segundos...", RETRY_DELAY_SECONDS);
                    logger.log(&format!(
                        "Reintentando en {} segundos...",
                        RETRY_DELAY_SECONDS
                    ));
                    thread::sleep(Duration::from_secs(RETRY_DELAY_SECONDS));
                }
            }
        }
    }
}

/// Convierte texto normal a formato codificado para el sistema
///
/// Esta función toma texto normal y lo convierte al formato interno del sistema,
/// reemplazando espacios con "`<space>`", saltos de línea con "`<enter>`", y
/// strings vacíos con "`<delete>`".
///
/// # Argumentos
///
/// * `value` - String con el texto a codificar
///
/// # Returns
///
/// String con el texto codificado en formato interno
pub fn parse_text(value: String) -> String {
    let val = value.clone();
    let mut value_clone = if value.trim_end_matches('\n').is_empty() {
        "<delete>".to_string()
    } else {
        val.replace('\n', "<enter>")
    };
    value_clone = value_clone.replace(' ', "<space>");
    return value_clone;
}

/// Convierte texto codificado a formato normal
///
/// Esta función toma un string codificado (con "`<space>`", "`<enter>`", "`<delete>`")
/// y lo convierte a un string normal, reemplazando "`<space>`" con espacios,
/// "`<enter>`" con saltos de línea, y "`<delete>`" con strings vacíos.
///
/// # Argumentos
///
/// * `value` - String con el texto codificado
///
/// # Returns
///
/// String con el texto decodificado en formato normal
pub fn decode_text(value: String) -> String {
    let value_clone = value.clone();
    value_clone
        .replace("<space>", " ")
        .replace("<enter>", "\n")
        .replace("<delete>", "")
}

/// Obtiene las direcciones de los nodos Redis desde la variable de entorno
///
/// Lee la variable de entorno `REDIS_NODE_HOSTS` que debe contener las direcciones
/// de los nodos Redis separadas por comas. Si no está en modo Docker, retorna
/// una lista predefinida de direcciones locales.
///
/// # Returns
///
/// Vector de strings con las direcciones de los nodos Redis
///
/// # Ejemplo
///
/// Si `REDIS_NODE_HOSTS=localhost:6379,localhost:6380`, retorna:
/// `["localhost:6379", "localhost:6380"]`
fn get_nodes_addresses() -> Vec<String> {
    if DOCKER {
        match env::var("REDIS_NODE_HOSTS") {
            Ok(val) => val.split(',').map(|s| s.to_string()).collect(),
            Err(_) => {
                eprintln!("REDIS_NODE_HOSTS no está seteada");
                vec![]
            }
        }
    } else {
        return vec![
            "127.0.0.1:4008".to_string(),
            "127.0.0.1:4007".to_string(),
            "127.0.0.1:4006".to_string(),
            "127.0.0.1:4005".to_string(),
            "127.0.0.1:4004".to_string(),
            "127.0.0.1:4003".to_string(),
            "127.0.0.1:4002".to_string(),
            "127.0.0.1:4001".to_string(),
        ];
    }
}

/// Función principal que inicia el microservicio
///
/// Crea una instancia del microservicio con la configuración especificada
/// y lo ejecuta. El archivo de configuración debe estar en "microservice.conf".
///
/// # Returns
///
/// Result que indica éxito o error en la ejecución del programa
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = "microservice.conf";
    let mut microservice = Microservice::new(config_path)?;
    microservice.start()
}
