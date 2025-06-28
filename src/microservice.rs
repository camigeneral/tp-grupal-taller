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
    /// La clave es la dirección del nodo (ej: "127.0.0.1:4000") y el valor es el stream TCP.
    node_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
    
    /// El último comando enviado a los nodos Redis.
    /// Se mantiene para referencia y debugging.
    last_command_sent: Arc<Mutex<String>>,
    
    /// Documentos almacenados en memoria recibidos de los nodos Redis.
    /// La clave es el nombre del documento y el valor es el contenido.
    documents: Arc<Mutex<HashMap<String, Documento>>>,
    
    /// Ruta al archivo de log donde se registran los eventos del microservicio.
    log_path: String,
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

    /// Inicia el procesamiento automático de comandos en un hilo separado.
    /// 
    /// Este método crea un hilo que se ejecuta en segundo plano y realiza
    /// verificaciones periódicas de las conexiones de nodos. Actualmente
    /// solo verifica que se pueda obtener el lock de node_streams, pero
    /// está diseñado para expandirse con funcionalidad adicional.
    /// 
    /// El hilo se ejecuta indefinidamente con un intervalo de sueño muy largo
    /// (aproximadamente 2 años) para mantener la funcionalidad activa.
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
            //println!("partes: {:#?}", parts);
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

                        println!("docs: {:#?}", docs);
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