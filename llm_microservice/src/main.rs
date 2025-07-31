extern crate reqwest;
extern crate rusty_docs;
extern crate serde_json;
use serde_json::json;
use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader, Write, Error, ErrorKind},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
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
    /// Listener TCP que acepta conexiones entrantes
    listener: TcpListener,
    /// Streams de conexión a los nodos Redis, indexados por dirección
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
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
        let listener = TcpListener::bind("0.0.0.0:4030").unwrap();

        LlmMicroservice {
            thread_pool,
            listener,
            node_streams: Arc::new(Mutex::new(HashMap::new())),
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

            Usá <space> para representar espacios reales y <enter> para representar saltos de línea. NO uses \n en ningún caso. NO uses espacios literales. NO uses dobles <space>. NO uses texto fuera del bloque generado.

            Insertá texto solo donde se indique.

            IMPORTANTE SOBRE OFFSET: El offset se calcula sobre el contenido DECODIFICADO, es decir, el resultado luego de reemplazar <space> con espacios reales y <enter> con saltos de línea (\n). Ejemplo:

            Contenido codificado: hola<space>mundo  
            Contenido decodificado: hola mundo (10 caracteres)

            - offset 0: antes de 'h'  
            - offset 4: antes del espacio  
            - offset 5: antes de 'm'  
            - offset 10: al final

            FORMATO DEL RESULTADO

            Debe devolverse como una única línea de texto con el siguiente formato:

            llm-response|<nombre_archivo>|linea:<n>|<contenido_codificado>

            Reglas por tipo de modo:

            ▸ Whole-file:
            - Generar todo el contenido del archivo, reemplazándolo por completo.
            - Si el prompt requiere procesar el contenido actual (traducir, corregir, reformatear, etc.), se debe usar como base.
            - Si el prompt indica generar contenido nuevo desde cero, ignorar el contenido original.
            - No incluir `linea:<n>` en este modo.
            - Separar líneas con <enter> y palabras con <space>.

            Ejemplos:
            - prompt: 'traduce al inglés' → traducir contenido original
            - prompt: 'corrige la gramática' → corregir contenido original
            - prompt: 'escribí una receta' → ignorar contenido original
            - prompt: 'dame 5 frutas' → ignorar contenido original

            ▸ Cursor, reemplazo u otros modos con offset:
            - Insertar exactamente en el offset especificado.
            - Si el offset cae en medio de una palabra, dividirla con <space> e insertar el texto entre espacios.
            - Si el offset está entre dos palabras, insertar directamente como <space>NUEVO<space>.
            - El contenido final debe reflejar la inserción aplicada.
            - Incluir etiqueta `linea:<n>`.

            Si el offset está dentro de una palabra:
            ho<space>Siam<space>la  
            → insertar <space>NUEVO<space> entre "ho" y "Siam".

            Si el offset está en un límite claro:
            <space>NUEVO<space>


            REGLAS GENERALES

            - Nunca usar \n. Usar <enter> exclusivamente para saltos de línea.
            - Devolver todo en una sola línea.
            - Nunca usar espacios reales.
            - Nunca agregar texto fuera del contenido requerido.
            - Si el resultado incluye listas o múltiples elementos, devolverlos como una única línea separada por <enter> (no línea por línea).
            - No incluir dobles <space> ni <space> mal ubicados.

            Ejemplo incorrecto:
            <space>Tokio<space> La<space>física<space>es<space>el<space>estudio...

            Ejemplo correcto:
            <space>Tokio<space>La<space>física<space>es<space>el<space>estudio...


            Todas las palabras deben estar separadas por <space>. No usar ningún delimitador adicional.
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
        let api_key = "AIzaSyDSyVJnHxJnUXDRnM7SxphBTwEPGtOjMEI";

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
        headers.insert("X-goog-api-key", HeaderValue::from_str(api_key).unwrap());

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
    fn handle_requests(stream: TcpStream, thread_pool: Arc<ThreadPool>) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        loop {
            let mut input_prompt = String::new();
            match reader.read_line(&mut input_prompt) {
                Ok(0) => {
                    println!("Conexión cerrada por el cliente");
                    break;
                }
                Ok(_) => {
                    let prompt = input_prompt.trim().to_string();

                    if prompt.is_empty() {
                        break;
                    }

                    let mut stream_clone = stream.try_clone().unwrap();
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
                                    let mut resp_parts = resp
                                        .split("|")
                                        .map(|s| s.trim().to_string())
                                        .collect::<Vec<String>>();
                                    if let Some(last) = resp_parts.last_mut() {
                                        *last = last.replace(" ", "");
                                    }
                                    let final_resp = resp_parts.join(" ");
                                    if let Err(e) =
                                        stream_clone.write_all(format!("{final_resp}\n").as_bytes())
                                    {
                                        eprintln!("Error escribiendo al cliente: {}", e);
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
                Err(e) => {
                    eprintln!("Error leyendo del stream: {}", e);
                    break;
                }
            }
        }
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
            if let Err(e) = stream.write_all(resp_command.as_bytes()) {
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
        self.send_initial_command();
        //self.connect_to_replica_nodes(&connect_node_sender)?;

        for stream in self.listener.incoming() {
            let stream = stream?;
            println!("Se conecto el microservicio");
            let pool = Arc::clone(&self.thread_pool);
            thread::spawn(move || { //Esto va a ser el listen_to_redis_response
                Self::handle_requests(stream, pool);
            });
        }
        Ok(())
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
