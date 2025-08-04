extern crate reqwest;
extern crate rusty_docs;
extern crate serde_json;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    env,
    io::{BufReader, Error, ErrorKind, Write},
    net::TcpStream,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
mod threadpool;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use rusty_docs::{
    logger::{self, Logger},
    resp_parser,
};
use threadpool::ThreadPool;

type SharedStream = Arc<Mutex<TcpStream>>;
type NodeStreams = Arc<Mutex<HashMap<String, SharedStream>>>;

/// Microservicio LLM que maneja solicitudes de procesamiento de lenguaje natural
///
/// Este microservicio se encarga de:
/// - Escuchar conexiones TCP en el puerto 4030
/// - Procesar solicitudes de texto usando la API de Gemini
/// - Conectar a nodos Redis para comunicación distribuida
/// - Manejar múltiples conexiones concurrentes usando un pool de hilos
pub struct LlmMicroservice {
    /// Pool de hilos para manejar múltiples conexiones concurrentes
    thread_pool: Arc<ThreadPool>,
    /// Streams de conexión a los nodos Redis, indexados por dirección
    node_streams: NodeStreams,
    /// Ruta al archivo de log donde se registran los eventos del microservicio.
    logger: logger::Logger,
    /// Consultas activas en el microservicio, para evitar procesamiento duplicado.
    active_queries: Arc<Mutex<HashSet<String>>>, // <--- NUEVO
}

/// Contexto de una solicitud LLM para procesamiento en el pool de hilos.
///
/// Esta estructura agrupa toda la información necesaria para procesar una solicitud
/// de lenguaje natural, incluyendo referencias a los streams de nodos, el documento,
/// el modo de selección, la línea y offset a modificar, el prompt, el pool de hilos
/// y el logger.
///
/// Se utiliza para pasar datos de manera segura y eficiente entre hilos.
///
/// # Campos
/// - `node_streams`: Referencia compartida a los streams de nodos Redis.
/// - `document`: Nombre del documento a procesar.
/// - `selection_mode`: Modo de selección (por ejemplo, "cursor" o "whole-file").
/// - `line`: Línea relevante para la operación.
/// - `offset`: Offset relevante para la operación.
/// - `prompt`: Instrucción o texto a procesar.
/// - `thread_pool`: Referencia al pool de hilos.
/// - `logger`: Logger para registrar eventos.
#[derive(Clone)]
pub struct RequestContext {
    pub node_streams: NodeStreams,
    pub document: String,
    pub selection_mode: String,
    pub line: String,
    pub offset: String,
    pub prompt: String,
    pub thread_pool: Arc<ThreadPool>,
    pub logger: Logger,
    pub active_queries: Arc<Mutex<HashSet<String>>>,
}

/// Enum que representa los distintos tipos de mensajes que puede procesar el microservicio LLM.
///
/// Cada variante corresponde a una acción o solicitud específica que puede ser recibida
/// desde los nodos Redis o desde otros componentes del sistema.
///
/// - `RequestedFile`: Respuesta con el contenido de un documento y un prompt asociado.
/// - `RequestFile`: Solicitud de un documento específico con un prompt.
/// - `ChangeLine`: Solicitud de modificación de una línea específica en un documento.
/// - `Unknown`: Mensaje desconocido o no reconocido.
/// - `Ignore`: Mensaje que debe ser ignorado.
#[derive(Debug)]
pub enum LlmPromptMessage {
    RequestedFile {
        document: String,
        content: String,
        prompt: String,
    },
    RequestFile {
        document: String,
        prompt: String,
    },
    ChangeLine {
        document: String,
        line: String,
        offset: String,
        prompt: String,
    },
    Unknown(String),
    Ignore,
}

impl LlmPromptMessage {
    /// Construye un mensaje LlmPromptMessage a partir de una lista de partes (strings).
    ///
    /// # Argumentos
    /// * `parts` - Vector de strings que representan los campos del mensaje.
    ///
    /// # Returns
    /// Un valor de tipo `LlmPromptMessage` correspondiente al mensaje recibido.
    pub fn from_parts(parts: &[String]) -> Self {
        if parts.is_empty() {
            return LlmPromptMessage::Unknown("Empty message".to_string());
        }

        match parts[0].as_str() {
            "request-file" => {
                return LlmPromptMessage::RequestFile {
                    document: parts[1].clone(),
                    prompt: parts[2].clone(),
                }
            }
            "change-line" => LlmPromptMessage::ChangeLine {
                document: parts[1].clone(),
                line: parts[2].clone(),
                offset: parts[3].clone(),
                prompt: parts[4].clone(),
            },
            "requested-file" => LlmPromptMessage::RequestedFile {
                document: parts[1].clone(),
                content: parts[2].clone(),
                prompt: parts[3].clone(),
            },
            _ => LlmPromptMessage::Ignore,
        }
    }
}

impl LlmMicroservice {
    /// Crea una nueva instancia del microservicio LLM
    ///
    /// # Argumentos
    ///
    /// * `n_threads` - Número de hilos en el pool para manejar conexiones concurrentes
    ///
    /// # Ejemplo
    ///
    /// ```rust
    /// let microservice = LlmMicroservice::new(4);
    /// ```
    pub fn new(n_threads: usize) -> Self {
        let thread_pool = Arc::new(ThreadPool::new(n_threads));
        let logger = logger::Logger::init(
            logger::Logger::get_log_path_from_config(
                "llm_microservice.conf",
                "llm_microservice_log_path=",
            ),
            4030,
        );
        LlmMicroservice {
            thread_pool,
            node_streams: Arc::new(Mutex::new(HashMap::new())),
            logger,
            active_queries: Arc::new(Mutex::new(HashSet::new())), // <--- NUEVO
        }
    }

    /// Envía datos a un nodo específico de Redis.
    ///
    /// Esta función busca el stream correspondiente al nodo, obtiene el lock y
    /// escribe los datos. Si ocurre un error de conexión (broken pipe o host unreachable),
    /// elimina el nodo del mapa de conexiones activas.
    ///
    /// # Argumentos
    /// * `node_streams` - Referencia al mapa compartido de streams de nodos.
    /// * `node_id` - Identificador del nodo destino.
    /// * `data` - Datos a enviar.
    /// * `logger` - Logger para registrar eventos y errores.
    fn send_to_node(node_streams: &NodeStreams, node_id: &str, data: &[u8], logger: Logger) {
        println!("[DEBUG] Intentando enviar datos al nodo: {}", node_id);

        let stream_arc = {
            match node_streams.lock() {
                Ok(streams_guard) => {
                    let maybe_stream = streams_guard.get(node_id).cloned();
                    if maybe_stream.is_none() {
                        eprintln!("[DEBUG] No se encontró el stream para el nodo: {}", node_id);
                    }
                    maybe_stream
                }
                Err(e) => {
                    eprintln!(
                        "[ERROR] No se pudo obtener lock de node_streams para {}: {}",
                        node_id, e
                    );
                    return;
                }
            }
        };

        if let Some(stream_arc) = stream_arc {
            let node_id_clone = node_id.to_string();
            let data_clone = data.to_vec();
            let node_streams_clone = Arc::clone(node_streams);

            match stream_arc.lock() {
                Ok(mut stream) => match stream.write_all(&data_clone) {
                    Ok(_) => match stream.flush() {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!(
                                "[ERROR] Error al hacer flush del stream del nodo {}: {}",
                                node_id_clone, e
                            );
                        }
                    },
                    Err(e) => {
                        if let Some(os_err) = e.raw_os_error() {
                            if os_err == 32 || os_err == 113 {
                                eprintln!(
                                    "[ERROR] Error de conexión con nodo {}: {} (os error {:?})",
                                    node_id_clone, e, os_err
                                );
                                Self::remove_failed_node(
                                    &node_streams_clone,
                                    &node_id_clone,
                                    logger,
                                );
                            } else {
                                eprintln!(
                                    "[ERROR] Error al escribir al nodo {}: {} (os error {:?})",
                                    node_id_clone, e, os_err
                                );
                            }
                        } else {
                            eprintln!("[ERROR] Error al escribir al nodo {}: {}", node_id_clone, e);
                        }
                    }
                },
                Err(e) => {
                    eprintln!(
                        "[ERROR] No se pudo obtener lock del stream para nodo {}: {}",
                        node_id_clone, e
                    );
                }
            }
        } else {
            eprintln!("[ERROR] No se encontró stream para el nodo: {}", node_id);
        }
    }

    /// Envía datos a todos los nodos Redis conectados.
    ///
    /// Itera sobre todos los nodos en el mapa de streams y utiliza `send_to_node`
    /// para enviar los datos a cada uno.
    ///
    /// # Argumentos
    /// * `node_streams` - Referencia al mapa compartido de streams de nodos.
    /// * `data` - Datos a enviar.
    /// * `logger` - Logger para registrar eventos y errores.
    fn send_to_all_nodes(node_streams: &NodeStreams, data: &[u8], logger: Logger) {
        let node_ids: Vec<String> = {
            if let Ok(streams_guard) = node_streams.lock() {
                streams_guard.keys().cloned().collect()
            } else {
                eprintln!("Error obteniendo lock de node_streams para envío masivo");
                return;
            }
        };

        for node_id in node_ids {
            let logger_clone = logger.clone();
            Self::send_to_node(node_streams, &node_id, data, logger_clone);
        }
    }

    /// Elimina un nodo del mapa de conexiones activas.
    ///
    /// Si el nodo está presente en el mapa, lo elimina y registra el evento.
    /// Si no está, registra que no se encontró el nodo.
    ///
    /// # Argumentos
    /// * `node_streams` - Referencia al mapa compartido de streams de nodos.
    /// * `node_id` - Identificador del nodo a eliminar.
    /// * `logger` - Logger para registrar eventos y errores.
    fn remove_failed_node(node_streams: &NodeStreams, node_id: &str, logger: Logger) {
        match node_streams.lock() {
            Ok(mut streams_guard) => {
                if streams_guard.remove(node_id).is_some() {
                    logger.log(
                        format!("Nodo {} eliminado exitosamente de node_streams", node_id).as_str(),
                    );
                    println!("Nodo {} eliminado exitosamente de node_streams", node_id);
                } else {
                    logger.log(format!("Nodo {} no encontrado en node_streams", node_id).as_str());
                    println!("Nodo {} no encontrado en node_streams", node_id);
                }
            }
            Err(_) => {
                logger.log(
                    format!(
                        "Error obteniendo lock de node_streams para eliminar nodo {}",
                        node_id
                    )
                    .as_str(),
                );
                eprintln!(
                    "Error obteniendo lock de node_streams para eliminar nodo {}",
                    node_id
                );
            }
        }
    }

    /// Obtiene las instrucciones del sistema para el modelo LLM
    fn get_llm_instruction() -> String {
        return r#"INSTRUCCIONES
        Respondé únicamente con la respuesta solicitada. No agregues introducciones, explicaciones, comentarios, aclaraciones ni conclusiones. No uses frases como 'Claro', 'Aquí está', 'Como modelo de lenguaje', etc. Respondé únicamente con el texto generado.
        
        FORMATO DE RESPUESTA:
        -  Ese contenido es tu única respuesta, y debe estar codificado usando los siguientes tags:
        - Usá <space> para representar espacios reales.
        - Usá <enter> para representar saltos de línea.
        - NO uses \n. NO uses espacios literales. NO uses dobles <space>. NO agregues texto fuera del bloque de salida.
        - El bloque NO debe empezar ni terminar con <enter>. Los tags <space> y <enter> deben estar en inglés.
        - Si te piden algo extenso con parrafos, o devolves parrafos, NO uses \n, usa <enter>
        
        CRITERIO DE RESPUESTA:
        - Siempre asumí que el input es una instrucción implícita, incluso si no hay verbos. Por ejemplo:
        - Entrada: "una planta" → Salida: Rose
        - Entrada: "un planeta" → Salida: Mars
        - Entrada: "un perro" → Salida: Golden<space>Retriever
        
        - Si el input tiene forma:  
        `content-to-change:{contenido}, user-prompt:{instrucción}`  
        Entonces:
        - Si se pide traducir o modificar el contenido, trabajá sobre él.
        - Pero si se entiende que se está pidiendo algo nuevo (generación desde cero), ignorá `content-to-change` y generá nuevo contenido.
        - En ese caso, si producís varios párrafos, separalos con <enter> (uno solo entre párrafos).
        - NO uses <enter> al inicio ni al final del bloque, incluso si hay varios párrafos.
        
        EJEMPLOS:
        - Entrada: 'traduci hola<enter>como a frances'  
        → Salida: Bonjour<enter>comme
        - Entrada: 'content-to-change:{lorem ipsum}, user-prompt:{Generá un ensayo sobre Marte}'  
        → Salida: Marte<space>es<space>el<space>cuarto<space>planeta<space>del<space>sistema<space>solar.<enter>Es<space>conocido<space>por<space>su<space>color<space>rojo...
        "#.to_string();
    }

    /// Envía una solicitud al modelo Gemini y obtiene la respuesta
    fn get_gemini_respond(prompt: &str) -> Result<Vec<u8>, reqwest::Error> {
        let api_key = env::var("GEMINI_API_KEY").unwrap_or_else(|_| {
            eprintln!("GEMINI_API_KEY no está configurada, usando API key por defecto");
            "AIzaSyDSyVJnHxJnUXDRnM7SxphBTwEPGtOjMEI".to_string()
        });

        let body = json!({
            "system_instruction": {
                "parts": [{
                    "text": Self::get_llm_instruction(),
                }]
            },
            "contents": [{
                "parts": [{
                    "text": prompt
                }]
            }]
        });

        let client = reqwest::blocking::Client::new();
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("X-goog-api-key", HeaderValue::from_str(&api_key).unwrap());

        let res = client
            .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent")
            .headers(headers)
            .json(&body)
            .send()?;

        Ok(res.bytes()?.to_vec())
    }

    /// Procesa una solicitud LLM ejecutando la tarea en un hilo del pool.
    ///
    /// # Descripción
    /// Esta función recibe un contexto de solicitud (`RequestContext`) y delega el procesamiento
    /// de la misma a un hilo del pool de hilos (`ThreadPool`). Esto permite que múltiples
    /// solicitudes sean procesadas en paralelo, sin bloquear el hilo principal.
    ///
    /// # Funcionamiento
    /// - Si el prompt está vacío, la función retorna inmediatamente.
    /// - Clona los datos necesarios del contexto para moverlos al closure.
    /// - Llama a `thread_pool.execute`, que envía el closure al canal del pool.
    /// - Un worker del pool toma la tarea y ejecuta:
    ///     - Llama a la API de Gemini con el prompt.
    ///     - Procesa la respuesta y la formatea.
    ///     - Publica la respuesta a los nodos Redis usando `send_to_all_nodes`.
    ///
    /// # Diagrama de flujo simplificado
    /// ```text
    /// handle_requests(ctx)
    ///        |
    ///        v
    /// thread_pool.execute(|| {
    ///     // Código de procesamiento de la solicitud
    /// })
    ///        |
    ///        v
    /// [Worker disponible del pool]
    ///        |
    ///        v
    /// Ejecuta la tarea: llama a Gemini, procesa respuesta, publica resultado
    /// ```
    ///
    /// # Ejemplo de uso
    /// ```text
    /// let ctx = RequestContext { ... };
    /// LlmMicroservice::handle_requests(ctx);
    /// ```
    fn handle_requests(ctx: RequestContext) {
        if ctx.prompt.is_empty() {
            return;
        }

        let query_key = format!(
            "{}|{}|{}|{}|{}",
            ctx.document, ctx.selection_mode, ctx.line, ctx.offset, ctx.prompt
        );

        {
            let mut active = ctx.active_queries.lock().unwrap();
            if active.contains(&query_key) {
                return;
            }
            active.insert(query_key.clone());
        }

        let prompt_clone = ctx.prompt.clone();
        let thread_pool = Arc::clone(&ctx.thread_pool);
        let logger = ctx.logger.clone();
        let node_streams = Arc::clone(&ctx.node_streams);
        let document = ctx.document.clone();
        let selection_mode = ctx.selection_mode.clone();
        let line = ctx.line.clone();
        let offset = ctx.offset.clone();
        let active_queries = Arc::clone(&ctx.active_queries);

        thread_pool.execute(move || {
            let gemini_resp = Self::get_gemini_respond(&prompt_clone);

            let response_str = match gemini_resp {
                Ok(resp) => String::from_utf8_lossy(&resp).into_owned(),
                Err(e) => {
                    logger.log(format!("Error en get_gemini_respond: {}", e).as_str());
                    eprintln!("Error en get_gemini_respond: {}", e);
                    return;
                }
            };

            match serde_json::from_str::<serde_json::Value>(&response_str) {
                Ok(parsed) => {
                    // Manejo explícito de errores de Gemini
                    if let Some(error) = parsed.get("error") {
                        let code = error.get("code").and_then(|c| c.as_u64());
                        if code == Some(503) {
                            let message_parts = &[
                                "llm-response-error",
                                "ups, la ia esta sobresaturada. Intente mas tarde",
                                &document,
                            ];

                            let message_resp = resp_parser::format_resp_command(message_parts);
                            let command_resp =
                                resp_parser::format_resp_publish(&document, &message_resp);

                            println!("Error 503 de Gemini: servicio sobresaturado");
                            logger.log("Error 503 de Gemini: servicio sobresaturado");

                            Self::send_to_all_nodes(&node_streams, command_resp.as_bytes(), logger);
                            return;
                        } else {
                            println!("Error inesperado de Gemini: {:?}", error);
                            logger.log(format!("Gemini API error: {:?}", error).as_str());
                            return;
                        }
                    }

                    // Caso exitoso: extracción del texto generado
                    if let Some(text) = parsed["candidates"]
                        .get(0)
                        .and_then(|c| c["content"]["parts"].get(0))
                        .and_then(|p| p["text"].as_str())
                    {
                        let resp = text.trim().trim_end_matches('\n');
                        let resp_parts = resp.replace(' ', "");
                        let message_parts = &[
                            "llm-response",
                            &resp_parts,
                            &document,
                            &selection_mode,
                            &line,
                            &offset,
                        ];

                        let message_resp = resp_parser::format_resp_command(message_parts);
                        let command_resp =
                            resp_parser::format_resp_publish(&document, &message_resp);

                        println!("Respuesta gemini: {command_resp}");
                        logger.log(format!("Respuesta gemini: {command_resp}").as_str());

                        Self::send_to_all_nodes(&node_streams, command_resp.as_bytes(), logger);
                    } else {
                        println!("Error: no se pudo extraer texto de Gemini {:#?}", parsed);
                        logger.log("Error: formato inesperado en respuesta de Gemini");
                    }
                }
                Err(e) => {
                    println!("Error al parsear JSON de Gemini: {:?}", e);
                    logger.log(format!("Error al parsear JSON de Gemini: {:?}", e).as_str());
                }
            }

            let mut active = active_queries.lock().unwrap();
            active.remove(&query_key);
        });
    }

    /// Conecta a todos los nodos Redis configurados en la variable de entorno
    fn connect_to_redis_nodes(&mut self) -> std::io::Result<()> {
        let node_addresses = get_nodes_addresses();

        if node_addresses.is_empty() {
            self.logger.log(format!("REDIS_NODE_HOSTS").as_str());
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "No hay nodos configurados en REDIS_NODE_HOSTS",
            ));
        }

        println!("Intentando conectar a {} nodos", node_addresses.len());
        self.logger
            .log(format!("Intentando conectar a {} nodos", node_addresses.len()).as_str());

        let mut new_streams = HashMap::new();

        for address in node_addresses {
            match self.connect_to_node_with_retry(&address) {
                Ok(stream) => {
                    new_streams.insert(address.clone(), Arc::new(Mutex::new(stream)));
                }
                Err(e) => {
                    eprintln!("Error conectando a {}: {}", address, e);
                    return Err(e);
                }
            }
        }

        if let Ok(mut streams_guard) = self.node_streams.lock() {
            *streams_guard = new_streams;
        } else {
            return Err(Error::new(
                ErrorKind::Other,
                "Error obteniendo lock para actualizar node_streams",
            ));
        }
        Ok(())
    }

    /// Envía el comando inicial de identificación a todos los nodos Redis.
    ///
    /// Utiliza el formato RESP para enviar el mensaje "llm_microservice" a todos los nodos.
    fn send_initial_command(&self) {
        let resp_command = resp_parser::format_resp_command(&["llm_microservice"]);
        Self::send_to_all_nodes(
            &self.node_streams,
            resp_command.as_bytes(),
            self.logger.clone(),
        );
    }

    /// Intenta conectarse a un nodo Redis con reintentos.
    ///
    /// Realiza hasta 15 intentos de conexión, esperando 10 segundos entre cada uno.
    /// Si no logra conectarse, retorna un error.
    ///
    /// # Argumentos
    /// * `address` - Dirección del nodo Redis.
    ///
    /// # Returns
    /// * `Ok(TcpStream)` si la conexión fue exitosa.
    /// * `Err(std::io::Error)` si falla tras los reintentos.
    fn connect_to_node_with_retry(&self, address: &str) -> std::io::Result<TcpStream> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 15;
        const RETRY_DELAY_SECONDS: u64 = 10;

        loop {
            attempts += 1;
            self.logger
                .log(format!("Intento {} de conectar a {}", attempts, address).as_str());
            println!("Intento {} de conectar a {}", attempts, address);

            match TcpStream::connect(address) {
                Ok(socket) => {
                    self.logger
                        .log(format!("Conexión exitosa a {}", address).as_str());
                    println!("Conexión exitosa a {}", address);
                    return Ok(socket);
                }
                Err(e) => {
                    eprintln!(
                        "Error conectando a {} (intento {}): {}",
                        address, attempts, e
                    );
                    self.logger.log(
                        format!(
                            "Error conectando a {} (intento {}): {}",
                            address, attempts, e
                        )
                        .as_str(),
                    );

                    if attempts >= MAX_ATTEMPTS {
                        return Err(Error::new(
                            ErrorKind::ConnectionRefused,
                            format!(
                                "No se pudo conectar a {} después de {} intentos",
                                address, MAX_ATTEMPTS
                            ),
                        ));
                    }

                    println!("Reintentando en {} segundos...", RETRY_DELAY_SECONDS);
                    self.logger.log(
                        format!("Reintentando en {} segundos...", RETRY_DELAY_SECONDS).as_str(),
                    );
                    thread::sleep(Duration::from_secs(RETRY_DELAY_SECONDS));
                }
            }
        }
    }

    /// Escucha y procesa las respuestas recibidas de un nodo Redis.
    ///
    /// Lee mensajes RESP del nodo, los interpreta y delega el procesamiento
    /// a la función correspondiente según el tipo de mensaje recibido.
    ///
    /// # Argumentos
    /// * `node_socket` - Stream TCP conectado al nodo.
    /// * `thread_pool` - Referencia al pool de hilos.
    /// * `node_streams` - Referencia al mapa compartido de streams de nodos.
    /// * `logger` - Logger para registrar eventos y errores.
    ///
    /// # Returns
    /// * `Ok(())` si la escucha y el procesamiento fueron exitosos.
    /// * `Err(std::io::Error)` si ocurre un error de IO.
    pub fn listen_node_responses(
        node_socket: TcpStream,
        thread_pool: Arc<ThreadPool>,
        node_streams: NodeStreams,
        logger: Logger,
        active_queries: Arc<Mutex<HashSet<String>>>, // <--- AGREGADO
    ) -> std::io::Result<()> {
        
        let mut reader = BufReader::new(node_socket.try_clone()?);

        loop {
            let thread_pool_clone = Arc::clone(&thread_pool);
            let logger_clone = logger.clone();
            let active_queries_clone = Arc::clone(&active_queries);
            let (parts, _) = resp_parser::parse_resp_command(&mut reader)?;
            if parts.is_empty() {
                break;
            }
            let llm_message = LlmPromptMessage::from_parts(&parts);
            match llm_message {
                LlmPromptMessage::ChangeLine {
                    document,
                    line,
                    offset,
                    prompt,
                } => {
                    logger_clone.log(format!(
                        "Se solicito cambio de linea por la IA en el documento: {}, linea: {line}, offset: {offset}, prompt:{prompt}", 
                        document
                    ).as_str());

                    println!("change line {document}, {line}, {offset}, {prompt}");

                    let ctx = RequestContext {
                        node_streams: Arc::clone(&node_streams),
                        document,
                        selection_mode: "cursor".to_string(),
                        line,
                        offset,
                        prompt,
                        thread_pool: Arc::clone(&thread_pool_clone),
                        logger: logger_clone.clone(),
                        active_queries: Arc::clone(&active_queries_clone), 
                    };

                    Self::handle_requests(ctx);
                }

                LlmPromptMessage::RequestFile { document, prompt } => {
                    logger_clone.log(
                        format!("La IA solicito el documento: {document}, prompt:{prompt}")
                            .as_str(),
                    );

                    let message_parts = &["microservice-request-file", &document, &prompt];

                    let message_resp = resp_parser::format_resp_command(message_parts);
                    let command_resp =
                        resp_parser::format_resp_publish(&"notifications", &message_resp);

                    println!(
                        "Enviando publish: {}",
                        command_resp.replace("\r\n", "\\r\\n")
                    );
                    logger_clone.log(
                        format!(
                            "Enviando publish: {}",
                            command_resp.replace("\r\n", "\\r\\n")
                        )
                        .as_str(),
                    );
                    Self::send_to_all_nodes(&node_streams, command_resp.as_bytes(), logger_clone);
                }

                LlmPromptMessage::RequestedFile {
                    document,
                    content,
                    prompt,
                } => {
                    logger_clone
                        .log(format!("Documento solicitado: {document} content{content}").as_str());
                    println!("Documento: {document}, content: {content}, prompt {prompt}");

                    let final_prompt = format!("content-to-change:{content}, user-prompt:{prompt}");

                    let ctx = RequestContext {
                        node_streams: Arc::clone(&node_streams),
                        document,
                        selection_mode: "whole-file".to_string(),
                        line: "0".to_string(),
                        offset: "0".to_string(),
                        prompt: final_prompt,
                        thread_pool: Arc::clone(&thread_pool_clone),
                        logger: logger_clone.clone(),
                        active_queries: Arc::clone(&active_queries_clone), 
                    };

                    Self::handle_requests(ctx);
                }

                _ => {}
            }
        }
        Ok(())
    }

    fn handle_node_connections(
        receiver: Receiver<TcpStream>,
        thread_pool: Arc<ThreadPool>,
        node_streams: NodeStreams,
        logger: Logger,
        active_queries: Arc<Mutex<HashSet<String>>>, 
    ) -> std::io::Result<()> {
        for stream in receiver {
            let thread_pool_clone = Arc::clone(&thread_pool);
            let cloned_node_streams = Arc::clone(&node_streams);
            let logger_clone = logger.clone();
            let active_queries_clone = Arc::clone(&active_queries); 

            thread::spawn(move || {
                if let Err(e) = Self::listen_node_responses(
                    stream,
                    thread_pool_clone,
                    cloned_node_streams,
                    logger_clone,
                    active_queries_clone, 
                ) {
                    println!("Error en la conexión con el nodo: {}", e);
                }
            });
        }
        Ok(())
    }

    fn start_node_connection_handler(&self, receiver: Receiver<TcpStream>) {
        let thread_pool_clone = Arc::clone(&self.thread_pool);
        let cloned_node_streams = Arc::clone(&self.node_streams);
        let logger_clone = self.logger.clone();
        let active_queries_clone = Arc::clone(&self.active_queries); 
        thread::spawn(move || {
            if let Err(e) = Self::handle_node_connections(
                receiver,
                thread_pool_clone,
                cloned_node_streams,
                logger_clone,
                active_queries_clone, 
            ) {
                println!("Error en la conexión con el nodo: {}", e);
            }
        });
    }

    /// Envía los streams de los nodos ya conectados al handler de conexiones.
    ///
    /// Clona los streams TCP de los nodos y los envía por el canal al manejador
    /// de conexiones, para que puedan ser procesados en paralelo.
    ///
    /// # Argumentos
    /// * `connect_node_sender` - Canal para enviar streams TCP al handler.
    ///
    /// # Returns
    /// * `Ok(())` si todos los streams se enviaron correctamente.
    /// * `Err(std::io::Error)` si ocurre un error al enviar algún stream.
    fn send_connected_nodes_to_handler(
        &self,
        connect_node_sender: &Sender<TcpStream>,
    ) -> std::io::Result<()> {
        if let Ok(streams_guard) = self.node_streams.lock() {
            for (node_id, stream_arc) in streams_guard.iter() {
                if let Ok(stream) = stream_arc.lock() {
                    match stream.try_clone() {
                        Ok(clone) => {
                            if let Err(e) = connect_node_sender.send(clone) {
                                eprintln!(
                                    "Error al enviar el stream del nodo {} al handler: {}",
                                    node_id, e
                                );
                                return Err(Error::new(
                                    ErrorKind::Other,
                                    "Error al enviar stream al handler",
                                ));
                            }
                        }
                        Err(e) => {
                            eprintln!("Error al clonar el stream del nodo {}: {}", node_id, e);
                            return Err(e);
                        }
                    }
                } else {
                    eprintln!("Error obteniendo lock del stream del nodo {}", node_id);
                }
            }
        } else {
            eprintln!("Error obteniendo lock de node_streams");
        }
        Ok(())
    }

    /// Ejecuta el microservicio LLM
    pub fn run(&mut self) -> std::io::Result<()> {
        self.connect_to_redis_nodes()?;
        let (connect_node_sender, connect_nodes_receiver) = channel::<TcpStream>();
        self.start_node_connection_handler(connect_nodes_receiver);
        self.send_connected_nodes_to_handler(&connect_node_sender)?;
        self.send_initial_command();

        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

/// Función principal que inicia el microservicio LLM
fn main() -> std::io::Result<()> {
    let mut llm_microservice = LlmMicroservice::new(4);
    llm_microservice.run()?;
    Ok(())
}

/// Obtiene las direcciones de los nodos Redis desde la variable de entorno
fn get_nodes_addresses() -> Vec<String> {
    match env::var("REDIS_NODE_HOSTS") {
        Ok(val) => val.split(',').map(|s| s.to_string()).collect(),
        Err(_) => {
            eprintln!("REDIS_NODE_HOSTS no está seteada");
            vec![]
        }
    }
}
