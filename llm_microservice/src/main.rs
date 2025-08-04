extern crate reqwest;
extern crate rusty_docs;
extern crate serde_json;
use serde_json::json;
use std::{
    collections::HashMap,
    env,
    io::{BufReader, Write, Error, ErrorKind},
    net::{TcpStream},
    sync::{Arc, Mutex, mpsc::{channel, Receiver, Sender}},
    thread,
    time::Duration
};
mod threadpool;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use rusty_docs::{logger, resp_parser};
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
}

/// Mensajes que procesa el microservicio
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
        prompt: String  
    },    
    Unknown(String),
    Ignore
}

impl LlmPromptMessage {
    pub fn from_parts(parts: &[String]) -> Self {
        if parts.is_empty() {
            return LlmPromptMessage::Unknown("Empty message".to_string());
        }

        match parts[0].as_str() {
            "request-file" => return LlmPromptMessage::RequestFile { document: parts[1].clone(), prompt: parts[2].clone()},
            "change-line" => LlmPromptMessage::ChangeLine { document: parts[1].clone(), line: parts[2].clone(), offset: parts[3].clone(), prompt: parts[4].clone()},
            "requested-file" => LlmPromptMessage::RequestedFile { document: parts[1].clone(), content: parts[2].clone(), prompt: parts[3].clone()},
            _ => LlmPromptMessage::Ignore
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

        LlmMicroservice {
            thread_pool,        
            node_streams: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn send_to_node(
        node_streams: &NodeStreams,
        node_id: &str,
        data: &[u8],
    ) {
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
                    eprintln!("[ERROR] No se pudo obtener lock de node_streams para {}: {}", node_id, e);
                    return;
                }
            }
        };
    
        if let Some(stream_arc) = stream_arc {
            let node_id_clone = node_id.to_string();
            let data_clone = data.to_vec();
            let node_streams_clone = Arc::clone(node_streams);
    
            println!("[DEBUG] Stream encontrado, intentando obtener lock del stream del nodo: {}", node_id_clone);
    
            match stream_arc.lock() {
                Ok(mut stream) => {
    
                    match stream.write_all(&data_clone) {
                        Ok(_) => {
                            match stream.flush() {
                                Ok(_) => {
                                    println!("[DEBUG] Flush exitoso para el nodo {}", node_id_clone);
                                }
                                Err(e) => {
                                    eprintln!("[ERROR] Error al hacer flush del stream del nodo {}: {}", node_id_clone, e);
                                }
                            }
                        }
                        Err(e) => {
                            if let Some(os_err) = e.raw_os_error() {
                                if os_err == 32 || os_err == 113 {
                                    eprintln!("[ERROR] Error de conexión con nodo {}: {} (os error {:?})", node_id_clone, e, os_err);
                                    Self::remove_failed_node(&node_streams_clone, &node_id_clone);
                                } else {
                                    eprintln!("[ERROR] Error al escribir al nodo {}: {} (os error {:?})", node_id_clone, e, os_err);
                                }
                            } else {
                                eprintln!("[ERROR] Error al escribir al nodo {}: {}", node_id_clone, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[ERROR] No se pudo obtener lock del stream para nodo {}: {}", node_id_clone, e);
                }
            }
        } else {
            eprintln!("[ERROR] No se encontró stream para el nodo: {}", node_id);
        }
    }
    

    fn send_to_all_nodes(
        node_streams: &NodeStreams,
        data: &[u8], 
    ) {
        let node_ids: Vec<String> = {
            if let Ok(streams_guard) = node_streams.lock() {
                streams_guard.keys().cloned().collect()
            } else {
                eprintln!("Error obteniendo lock de node_streams para envío masivo");
                return;
            }
        }; 
        
        for node_id in node_ids {
            Self::send_to_node(node_streams, &node_id, data);
        }
    }

    fn remove_failed_node(node_streams: &NodeStreams, node_id: &str) {
        match node_streams.lock() {
            Ok(mut streams_guard) => {
                if streams_guard.remove(node_id).is_some() {
                    println!("Nodo {} eliminado exitosamente de node_streams", node_id);
                } else {
                    println!("Nodo {} no encontrado en node_streams", node_id);
                }
            }
            Err(_) => {
                eprintln!("Error obteniendo lock de node_streams para eliminar nodo {}", node_id);
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

    fn handle_requests(
        node_streams: NodeStreams,
        document: String,
        selection_mode: String,
        line: String,
        offset: String,
        prompt: String,
        thread_pool: Arc<ThreadPool>,
    ) {
        let prompt_clone = prompt.clone();
        if prompt_clone.is_empty() {
            return;
        }
        thread_pool.execute(move || {
            let gemini_resp = Self::get_gemini_respond(&prompt_clone);
    
            let response_str = match gemini_resp {
                Ok(resp) => String::from_utf8_lossy(&resp).into_owned(),
                Err(e) => {
                    eprintln!("Error en get_gemini_respond: {}", e);
                    return;
                }
            };
    
            match serde_json::from_str::<serde_json::Value>(&response_str) {
                Ok(parsed) => {
                    if let Some(text) = parsed["candidates"]
                        .get(0)
                        .and_then(|c| c["content"]["parts"].get(0))
                        .and_then(|p| p["text"].as_str())
                    {
                        let resp = text.trim().trim_end_matches("\n");
                        let resp_parts = resp.replace(" ", "");  
                        let message_parts = &[
                            "llm-response", 
                            &resp_parts.clone(),
                            &document.clone(),
                            &selection_mode.clone(),
                            &line.clone(),
                            &offset.clone(),                
                        ];
                            
                        let message_resp = resp_parser::format_resp_command(message_parts);
                        let command_resp = resp_parser::format_resp_publish(&document, &message_resp);
                        println!("GEMINI RESPONSE: {command_resp}");
                        
                        Self::send_to_all_nodes(&node_streams, command_resp.as_bytes());
                    } else {
                        println!("Error: no se pudo extraer texto de Gemini");
                    }
                }
                Err(e) => {
                    println!("Error parseando JSON: {}", e);
                }
            }
        });
    }

    /// Conecta a todos los nodos Redis configurados en la variable de entorno
    fn connect_to_redis_nodes(&mut self) -> std::io::Result<()> {
        let node_addresses = get_nodes_addresses();

        if node_addresses.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "No hay nodos configurados en REDIS_NODE_HOSTS",
            ));
        }

        println!("Intentando conectar a {} nodos", node_addresses.len());

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
            return Err(Error::new(ErrorKind::Other, "Error obteniendo lock para actualizar node_streams"));
        }
        Ok(())
    }

    fn send_initial_command(&self) {
        let resp_command = resp_parser::format_resp_command(&["llm_microservice"]);
        Self::send_to_all_nodes(&self.node_streams, resp_command.as_bytes());
    }


    fn connect_to_node_with_retry(&self, address: &str) -> std::io::Result<TcpStream> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 15;
        const RETRY_DELAY_SECONDS: u64 = 10;

        loop {
            attempts += 1;
            println!("Intento {} de conectar a {}", attempts, address);

            match TcpStream::connect(address) {
                Ok(socket) => {
                    println!("Conexión exitosa a {}", address);
                    return Ok(socket);
                }
                Err(e) => {
                    eprintln!(
                        "Error conectando a {} (intento {}): {}",
                        address, attempts, e
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
                    thread::sleep(Duration::from_secs(RETRY_DELAY_SECONDS));
                }
            }
        }
    }

     pub fn listen_node_responses(
        node_socket: TcpStream,
        thread_pool: Arc<ThreadPool>,
        node_streams: NodeStreams,
    ) -> std::io::Result<()> {

        let peer_addr = match node_socket.peer_addr() {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("Error obteniendo peer_addr: {}", e);
                return Err(e);
            }
        };
        let mut correct_port = String::new();
        if let Some((_, port)) = peer_addr.to_string().split_once(':') {
            if let Some(last_char) = port.chars().last() {
                correct_port = format!("node{}:{}", last_char, port);
            } else {
                eprintln!("[ERROR] No se pudo obtener el último carácter del puerto: {}", port);
            }
        } else {
            eprintln!("[ERROR] Dirección inválida (no tiene ':'): {}", peer_addr);
        }
        
        
        let mut reader = BufReader::new(node_socket.try_clone()?);
        
        loop {
            let thread_pool_clone = Arc::clone(&thread_pool);
            
            let (parts, _) = resp_parser::parse_resp_command(&mut reader)?;
            if parts.is_empty() {
                break;
            }           
            let correct_addr_clone = correct_port.clone();
            let llm_message = LlmPromptMessage::from_parts(&parts);
            match llm_message {
                LlmPromptMessage::ChangeLine { document, line, offset, prompt } => {
                    println!("change line {document}, {line}, {offset}, {prompt}");
                    let node_streams_clone = Arc::clone(&node_streams);

                    let final_prompt = format!("{prompt}");
                    Self::handle_requests(node_streams_clone, document, "cursor".to_string(), line, offset, final_prompt, thread_pool_clone);
                },
                LlmPromptMessage::RequestFile { document, prompt } => {
                    let message_parts = &[
                        "microservice-request-file",
                        &document.clone(),
                        &prompt.clone(),                        
                    ];
                    let message_resp = resp_parser::format_resp_command(message_parts);
                    let command_resp = resp_parser::format_resp_publish(&"llm_requests", &message_resp);
                    println!("Enviando publish: {}", command_resp.replace("\r\n", "\\r\\n"));
                    
                    
                    Self::send_to_node(&node_streams, &correct_addr_clone.clone().to_string(), command_resp.as_bytes());
                }
                LlmPromptMessage::RequestedFile { document, content, prompt } => {
                    println!("Documento: {document}, content: {content}, prompt {prompt}");                    
                    let node_streams_clone = Arc::clone(&node_streams);

                    let final_prompt = format!("content-to-change:{content}, user-prompt:{prompt}");
                    Self::handle_requests(node_streams_clone, document, "whole-file".to_string(), "0".to_string(), "0".to_string(), final_prompt, thread_pool_clone);
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
    ) -> std::io::Result<()> {        
        for stream in receiver {
            let thread_pool_clone = Arc::clone(&thread_pool);
            let cloned_node_streams = Arc::clone(&node_streams);
            
            thread::spawn(move || {
                if let Err(e) = Self::listen_node_responses(
                    stream,
                    thread_pool_clone,
                    cloned_node_streams
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

        thread::spawn(move || {
            if let Err(e) = Self::handle_node_connections(receiver, thread_pool_clone, cloned_node_streams) {
                println!("Error en la conexión con el nodo: {}", e);
            }
        });
    }

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
                                eprintln!("Error al enviar el stream del nodo {} al handler: {}", node_id, e);
                                return Err(Error::new(ErrorKind::Other, "Error al enviar stream al handler"));
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