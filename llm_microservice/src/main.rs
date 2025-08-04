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
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
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
                    eprintln!("Error al hacer flush del stream del nodo {}: {}", node_id, e);
                }
                Ok(())
            }
            Err(e) => {
                // Verificar si es un error de broken pipe (32) o host unreachable (113)
                if e.raw_os_error() == Some(32) || e.raw_os_error() == Some(113) {
                    eprintln!("Error de conexión con nodo {}: {} (os error {:?})", node_id, e, e.raw_os_error());
                    
                    // Eliminar el nodo de node_streams
                    if let Ok(mut streams_guard) = node_streams.lock() {
                        streams_guard.remove(node_id);
                        println!("Nodo {} eliminado de node_streams debido a error de conexión", node_id);
                    } else {
                        eprintln!("Error obteniendo lock de node_streams para eliminar nodo {}", node_id);
                    }
                } else {
                    eprintln!("Error al escribir al nodo {}: {}", node_id, e);
                }
                Err(e)
            }
        }
    }

    /// Obtiene las instrucciones del sistema para el modelo LLM
    /// 
    /// Estas instrucciones definen el formato de respuesta esperado y las reglas
    /// para el procesamiento de texto, incluyendo el manejo de espacios y saltos de línea.
    /// 
    /// # Returns
    /// 
    /// String con las instrucciones del sistema para el modelo LLM
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
    /// 
    /// # Argumentos
    /// 
    /// * `prompt` - El texto de entrada que se enviará al modelo LLM
    /// 
    /// # Returns
    /// 
    /// Result que contiene los bytes de la respuesta de Gemini o un error de reqwest
    /// 
    /// # Errores
    /// 
    /// Esta función puede fallar si:
    /// - La API key no es válida
    /// - Hay problemas de conectividad con la API de Gemini
    /// - La respuesta no es válida
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

    /// Maneja las solicitudes entrantes de un stream TCP
    /// 
    /// Esta función lee líneas del stream, las procesa con el modelo LLM,
    /// y envía las respuestas de vuelta al cliente.
    /// 
    /// # Argumentos
    /// 
    /// * `stream` - Stream TCP conectado al cliente
    /// * `thread_pool` - Pool de hilos para procesar solicitudes de forma asíncrona
    fn handle_requests(
        node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
        document: String,
        selection_mode: String,
        line: String,
        offset: String,
        prompt: String,
        thread_pool: Arc<ThreadPool>,
    ) {
        let prompt_clone = prompt.clone();
    
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
                        
                        if let Ok(mut streams_guard) = node_streams.lock() {
                            for (node_id, stream) in streams_guard.iter_mut() {
                                if let Err(e) = Self::write_to_stream_with_error_handling(stream, command_resp.as_bytes(), node_id, &node_streams) {
                                    eprintln!("Error enviando publish al nodo {}: {}", node_id, e);
                                } else {
                                    println!("Comando enviado exitosamente al nodo {}", node_id);
                                }
                            }
                        } else {
                            eprintln!("Error obteniendo lock de node_streams");
                        }
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
    /// 
    /// Lee la variable de entorno `REDIS_NODE_HOSTS` que debe contener las direcciones
    /// de los nodos Redis separadas por comas.
    /// 
    /// # Returns
    /// 
    /// Result que indica éxito o error en la conexión
    /// 
    /// # Errores
    /// 
    /// Esta función puede fallar si:
    /// - La variable `REDIS_NODE_HOSTS` no está configurada
    /// - No se puede conectar a alguno de los nodos después de múltiples intentos
    fn connect_to_redis_nodes(&mut self) -> std::io::Result<()> {
        let node_addresses = get_nodes_addresses();

        if node_addresses.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "No hay nodos configurados en REDIS_NODE_HOSTS",
            ));
        }

        println!("Intentando conectar a {} nodos", node_addresses.len());

        for address in node_addresses {
            self.connect_to_node_with_retry(&address)?;
        }

        Ok(())
    }

    /// Envía el comando inicial de registro a todos los nodos Redis conectados
    /// 
    /// Este comando informa a los nodos Redis que este microservicio está disponible
    /// y listo para recibir solicitudes.
    fn send_initial_command(&mut self) {
        let mut streams = self.node_streams.lock().unwrap();

        for (node_id, stream) in streams.iter_mut() {
            let resp_command = resp_parser::format_resp_command(&["llm_microservice"]);
            if let Err(e) = Self::write_to_stream_with_error_handling(stream, resp_command.as_bytes(), node_id, &self.node_streams) {
                eprintln!("Error al escribir al nodo {}: {}", node_id, e);
            }
        }
    }

    /// Intenta conectar a un nodo Redis específico con reintentos automáticos
    /// 
    /// Realiza hasta 15 intentos de conexión con un delay de 10 segundos entre intentos.
    /// 
    /// # Argumentos
    /// 
    /// * `address` - Dirección del nodo Redis (formato: "host:puerto")
    /// 
    /// # Returns
    /// 
    /// Result que indica éxito o error en la conexión
    /// 
    /// # Errores
    /// 
    /// Esta función puede fallar si:
    /// - No se puede conectar después de 15 intentos
    /// - El nodo no está disponible
    fn connect_to_node_with_retry(&mut self, address: &str) -> std::io::Result<()> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 15;
        const RETRY_DELAY_SECONDS: u64 = 10;

        loop {
            attempts += 1;
            println!("Intento {} de conectar a {}", attempts, address);

            match TcpStream::connect(address) {
                Ok(socket) => {
                    println!("Conexión exitosa a {}", address);
                    let mut streams = self.node_streams.lock().unwrap();
                    streams.insert(address.to_string(), socket);
                    return Ok(());
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
        node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    ) -> std::io::Result<()> {
        if let Ok(peer_addr) = node_socket.peer_addr() {
            println!("Escuchando respuestas del nodo: {}", peer_addr);
        }
        
        let mut reader = BufReader::new(node_socket.try_clone()?);
        loop {
            let thread_pool_clone: Arc<ThreadPool> = thread_pool.clone();
            let mut node_socket_clone = node_socket.try_clone()?;
            
            let (parts, _) = resp_parser::parse_resp_command(&mut reader)?;
            if parts.is_empty() {
                break;
            }           
            println!("partes de llm_requests: {:#?}", parts);
            

            let llm_message = LlmPromptMessage::from_parts(&parts);
            println!("llm_message: {:#?}", llm_message);
            

            match llm_message {
                LlmPromptMessage::ChangeLine { document, line, offset, prompt } => {
                        println!("change linge {document}, {line}, {offset}, {prompt}");
                        let node_streams_clone: Arc<Mutex<HashMap<String, TcpStream>>>= Arc::clone(&node_streams);

                          let final_prompt =  format!(
                            "{prompt}",                        
                        );
                        Self::handle_requests(node_streams_clone, document, "cursor".to_string(), line, offset, final_prompt, thread_pool_clone);
                },
                LlmPromptMessage::RequestFile { document, prompt } => {
                    let message_parts = &[
                        "microservice-request-file",
                        &document.clone(),
                        &prompt.clone(),                        
                    ];
                    let message_resp = resp_parser::format_resp_command(message_parts);
                    let command_resp =
                    resp_parser::format_resp_publish(&"llm_requests", &message_resp);
                    println!(
                        "Enviando publish: {}",
                        command_resp.replace("\r\n", "\\r\\n")
                    );
                    if let Err(e) = Self::write_to_stream_with_error_handling(&mut node_socket_clone, command_resp.as_bytes(), "llm_requests", &node_streams) {
                        println!(
                            "Error al enviar mensaje de actualizacion de archivo: {}",
                            e
                        );
                    }
                }
                LlmPromptMessage::RequestedFile { document, content, prompt } => {
                    println!("Documetno: {document}, content: {content}, prompt {prompt}");                    
                    let node_streams_clone: Arc<Mutex<HashMap<String, TcpStream>>>= Arc::clone(&node_streams);

                    let final_prompt =  format!(
                        "content-to-change:{content}, user-prompt:{prompt}",                        
                    );

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
        node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,

    ) -> std::io::Result<()> {        
        for stream in receiver {
            let thread_pool_clone: Arc<ThreadPool> = thread_pool.clone();
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


    fn start_node_connection_handler(&mut self, receiver: Receiver<TcpStream>) {

        let thread_pool_clone: Arc<ThreadPool> = self.thread_pool.clone();
        let cloned_node_streams = Arc::clone(&self.node_streams);

        thread::spawn(move || {
            if let Err(e) = Self::handle_node_connections(receiver, thread_pool_clone, cloned_node_streams) {
                println!("Error en la conexión con el nodo: {}", e);
            }
        });
    }

    /// Envia todos los nodos conectados al handler de conexiones de nodos
    fn send_connected_nodes_to_handler(
        &mut self,
        connect_node_sender: &Sender<TcpStream>,
    ) -> std::io::Result<()> {
        let streams = self.node_streams.lock().unwrap();
        for (node_id, stream) in streams.iter() {
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
        }
        Ok(())
    }
    
    /// Ejecuta el microservicio LLM
    /// 
    /// Esta función:
    /// 1. Conecta a todos los nodos Redis configurados
    /// 2. Envía el comando inicial de registro
    /// 3. Comienza a escuchar conexiones TCP entrantes
    /// 4. Maneja cada conexión en un hilo separado
    /// 
    /// # Returns
    /// 
    /// Result que indica éxito o error en la ejecución
    /// 
    /// # Errores
    /// 
    /// Esta función puede fallar si:
    /// - No se puede conectar a los nodos Redis
    /// - Hay problemas con el listener TCP
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
/// 
/// Crea una instancia del microservicio con 4 hilos y lo ejecuta.
/// 
/// # Returns
/// 
/// Result que indica éxito o error en la ejecución del programa
fn main() -> std::io::Result<()> {
    let mut llm_microservice = LlmMicroservice::new(4);

    llm_microservice.run()?;
    Ok(())
}

/// Obtiene las direcciones de los nodos Redis desde la variable de entorno
/// 
/// Lee la variable de entorno `REDIS_NODE_HOSTS` que debe contener las direcciones
/// de los nodos Redis separadas por comas.
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
    match env::var("REDIS_NODE_HOSTS") {
        Ok(val) => val.split(',').map(|s| s.to_string()).collect(),
        Err(_) => {
            eprintln!("REDIS_NODE_HOSTS no está seteada");
            vec![]
        }
    }
}
