use commands::redis;
use std::collections::HashMap;
use std::env::args;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::str;
use std::sync::{Arc, Mutex};
use std::thread;
mod client_info;
mod commands;
mod hashing;
mod local_node;
mod peer_node;
mod utils;

#[derive(Debug)]
pub enum RedisMessage {
    Node,
}

/// Número de argumentos esperados para iniciar el servidor
static REQUIRED_ARGS: usize = 2;

/// Inicia el servidor Redis.
///
/// # Argumentos
/// Espera recibir el puerto en el que escuchará el servidor como argumento
/// en la línea de comandos.
///
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args: Vec<String> = args().collect();
    if cli_args.len() != REQUIRED_ARGS {
        eprintln!("Error: Cantidad de argumentos inválida");
        eprintln!("Uso: {} <puerto>", cli_args[0]);
        return Err("Error: Cantidad de argumentos inválida".into());
    }

    let port = match cli_args[1].parse::<usize>() {
        Ok(n) => n,
        Err(_e) => return Err("Failed to parse arguments".into()),
    };

    let node_address = format!("127.0.0.1:{}", port + 10000);
    let client_address = format!("127.0.0.1:{}", port);

    start_node_connection(port, node_address)?;
    start_server(&client_address)?;

    Ok(())
}

/// Inicia el servidor Redis y maneja las conexiones de clientes.
///
/// Esta función:
/// 1. Carga el estado inicial desde el archivo de persistencia
/// 2. Inicializa las estructuras de datos compartidas
/// 3. Acepta y maneja conexiones de clientes
///
/// # Argumentos
/// * `bind_address` - Dirección IP y puerto donde escuchará el servidor
///
/// # Errores
/// Retorna un error si:
/// - No se puede crear el socket TCP
/// - Hay problemas al leer el archivo de persistencia
fn start_server(bind_address: &str) -> std::io::Result<()> {
    let config_path = "redis.conf";
    let log_path = utils::logger::get_log_path_from_config(config_path);
    
    use std::fs;
    if fs::metadata(&log_path).map(|m| m.len() > 0).unwrap_or(false) {
        let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .and_then(|mut file| writeln!(file, ""));
    }

    let persistence_file = "docs.txt".to_string();
    let stored_documents = match load_persisted_data(&persistence_file) {
        Ok(docs) => docs,
        Err(_) => {
            println!("Iniciando con base de datos vacía");
            HashMap::new()
        }
    };

    // Inicializar estructuras de datos compartidas
    let shared_documents = Arc::new(Mutex::new(stored_documents.clone()));
    let document_subscribers = initialize_document_subscribers(&stored_documents);
    let active_clients = Arc::new(Mutex::new(HashMap::new()));

    // Iniciar servidor TCP
    let tcp_listener = TcpListener::bind(bind_address)?;
    println!("Servidor Redis escuchando en {}", bind_address);

    utils::logger::log_event(&log_path, &format!("Servidor iniciado en {}", bind_address));

    for incoming_connection in tcp_listener.incoming() {
        match incoming_connection {
            Ok(client_stream) => {
                handle_new_microservice_connection(
                    client_stream,
                    &active_clients,
                    &document_subscribers,
                    &shared_documents,
                    &log_path,
                )?;
            }
            Err(e) => {
                eprintln!("Error al aceptar conexión: {}", e);
                utils::logger::log_event(&log_path, &format!("Error al aceptar conexión: {}", e));
            }
        }
    }

    Ok(())
}

/// Inicializa el mapa de suscriptores para cada documento.
///
/// Crea una entrada vacía en el mapa de suscriptores para cada documento
/// existente en la base de datos.
///
/// # Argumentos
/// * `documents` - HashMap con los documentos existentes
///
/// # Retorna
/// Arc<Mutex<HashMap>> con las listas de suscriptores inicializadas
fn initialize_document_subscribers(
    documents: &HashMap<String, Vec<String>>,
) -> Arc<Mutex<HashMap<String, Vec<String>>>> {
    let mut subscriber_map = HashMap::new();

    for document_id in documents.keys() {
        subscriber_map.insert(document_id.clone(), Vec::new());
    }

    Arc::new(Mutex::new(subscriber_map))
}

fn handle_new_microservice_connection(
    mut client_stream: TcpStream,
    active_clients: &Arc<Mutex<HashMap<String, client_info::Client>>>,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_documents: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    log_path: &str,
) -> std::io::Result<()> {
    let client_addr = client_stream.peer_addr()?;
    println!("cliente conectado: {}", client_addr);

    utils::logger::log_event(log_path, &format!("Cliente conectado: {}", client_addr));

    let client_stream_clone = client_stream.try_clone()?;

    {
        let client_addr = client_addr.to_string();
        let client = client_info::Client {
            stream: client_stream_clone,
        };
        let mut lock_clients = active_clients.lock().unwrap();
        lock_clients.insert(client_addr, client);
    }

    let cloned_clients = Arc::clone(active_clients);
    let cloned_clients_on_docs = Arc::clone(document_subscribers);
    let cloned_docs = Arc::clone(shared_documents);
    let client_addr_str = client_addr.to_string();
    let log_path = log_path.to_string();

    thread::spawn(move || {
        match handle_client(
            &mut client_stream,
            cloned_clients,
            cloned_clients_on_docs,
            cloned_docs,
            client_addr_str,
            &log_path,
        ) {
            Ok(_) => {
                println!("Client {} disconnected.", client_addr);

                utils::logger::log_event(
                    &log_path,
                    &format!("Cliente desconectado: {}", client_addr),
                );
            }
            Err(e) => {
                eprintln!("Error in connection with {}: {}", client_addr, e);

                utils::logger::log_event(
                    &log_path,
                    &format!("Error en conexión con: {}", client_addr),
                );
            }
        }
    });

    Ok(())
}

/// Maneja la comunicación con un cliente conectado.
///
/// Esta función:
/// 1. Lee comandos del cliente
/// 2. Procesa los comandos recibidos
/// 3. Envía respuestas al cliente
/// 4. Publica actualizaciones a otros clientes suscritos
///
fn handle_client(
    stream: &mut TcpStream,
    active_clients: Arc<Mutex<HashMap<String, client_info::Client>>>,
    document_subscribers: Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_documents: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_id: String,
    log_path: &str,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    loop {
        let command_request = match utils::redis_parser::parse_command(&mut reader) {
            Ok(req) => req,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                }
                println!("Error al parsear comando: {}", e);
                utils::redis_parser::write_response(
                    stream,
                    &utils::redis_parser::CommandResponse::Error("Comando inválido".to_string()),
                )?;
                utils::logger::log_event(
                    log_path,
                    &format!("Error al parsear comando de {}: {}", client_id, e),
                );
                continue;
            }
        };

        println!("Comando recibido: {:?}", command_request);
        utils::logger::log_event(
            log_path,
            &format!("Comando recibido de {}: {:?}", client_id, command_request),
        );

        let redis_response = redis::execute_command(
            command_request,
            shared_documents.clone(),
            document_subscribers.clone(),
            client_id.clone(),
        );

        if redis_response.publish {
            if let Err(e) = publish_update(
                active_clients.clone(),
                document_subscribers.clone(),
                redis_response.message,
                redis_response.doc,
            ) {
                eprintln!("Error al publicar actualización: {}", e);
            }
        }

        let response = redis_response.response;
        if let Err(e) = utils::redis_parser::write_response(stream, &response) {
            println!("Error al escribir respuesta: {}", e);
            utils::logger::log_event(
                log_path,
                &format!("Error al escribir respuesta a {}: {}", client_id, e),
            );
            break;
        }

        utils::logger::log_event(
            log_path,
            &format!("Respuesta enviada a {}: {:?}", client_id, response),
        );

        if let Err(e) = persist_documents(shared_documents.clone()) {
            eprintln!("Error al persistir documentos: {}", e);
        }
    }

    cleanup_client_resources(&client_id, &active_clients, &document_subscribers);
    // to do: agregar comando para salir, esto nunca se ejecuta porque nunca termina el loop

    Ok(())
}

/// Publica una actualización a todos los clientes suscritos a un documento.
///
/// # Errores
/// Retorna un error si hay problemas al escribir en algún stream de cliente
pub fn publish_update(
    active_clients: Arc<Mutex<HashMap<String, client_info::Client>>>,
    document_subscribers: Arc<Mutex<HashMap<String, Vec<String>>>>,
    update_message: String,
    document_id: String,
) -> std::io::Result<()> {
    let mut clients_guard = active_clients.lock().unwrap();
    let subscribers_guard = document_subscribers.lock().unwrap();

    if let Some(document_subscribers) = subscribers_guard.get(&document_id) {
        for subscriber_id in document_subscribers {
            if let Some(client) = clients_guard.get_mut(subscriber_id) {
                writeln!(client.stream, "{}", update_message.trim())?;
            } else {
                println!("Cliente no encontrado: {}", subscriber_id);
            }
        }
    } else {
        println!("Documento no encontrado: {}", document_id);
    }

    Ok(())
}

/// Limpia los recursos asociados a un cliente cuando se desconecta.
///
/// Elimina al cliente de:
/// - La lista de clientes activos
/// - Las listas de suscriptores de documentos
///
fn cleanup_client_resources(
    client_id: &str,
    active_clients: &Arc<Mutex<HashMap<String, client_info::Client>>>,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
) {
    active_clients.lock().unwrap().remove(client_id);

    let mut subscribers_guard = document_subscribers.lock().unwrap();
    for subscriber_list in subscribers_guard.values_mut() {
        subscriber_list.retain(|id| id != client_id);
    }
}

/// Persiste el estado actual de los documentos en el archivo.
///
/// # Errores
/// Retorna un error si hay problemas al escribir en el archivo
pub fn persist_documents(documents: Arc<Mutex<HashMap<String, Vec<String>>>>) -> io::Result<()> {
    let mut persistence_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open("docs.txt")?;

    let documents_guard = documents.lock().unwrap();
    let document_ids: Vec<&String> = documents_guard.keys().collect();

    for document_id in document_ids {
        let mut document_data = document_id.to_string();
        document_data.push_str("/++/");

        if let Some(messages) = documents_guard.get(document_id) {
            for message in messages {
                document_data.push_str(message);
                document_data.push_str("/--/");
            }
        }

        writeln!(persistence_file, "{}", document_data)?;
    }

    Ok(())
}

/// Carga los documentos persistidos desde el archivo.
///
/// # Retorna
/// HashMap con los documentos y sus mensajes, o un error si hay problemas
/// al leer el archivo
pub fn load_persisted_data(file_path: &String) -> Result<HashMap<String, Vec<String>>, String> {
    let file = File::open(file_path).map_err(|_| "archivo-no-encontrado".to_string())?;
    let reader = BufReader::new(file);
    let lines = reader.lines();

    let mut documents: HashMap<String, Vec<String>> = HashMap::new();

    for line in lines {
        match line {
            Ok(content) => {
                let parts: Vec<&str> = content.split("/++/").collect();
                if parts.len() != 2 {
                    continue;
                }

                let document_id = parts[0].to_string();
                let messages_data = parts[1];

                let messages: Vec<String> = messages_data
                    .split("/--/")
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();

                documents.insert(document_id, messages);
            }
            Err(_) => return Err("error-al-leer-archivo".to_string()),
        }
    }

    Ok(documents)
}

/// Intenta establecer una primera conexion con los otros nodos del servidor
///
/// # Errores
/// Retorna un error si el puerto no corresponde a uno definido en los archivos de configuracion.
pub fn start_node_connection(port: usize, node_address: String) -> Result<(), std::io::Error> {
    let peer_nodes: Arc<Mutex<HashMap<String, peer_node::PeerNode>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let cloned_nodes = Arc::clone(&peer_nodes);

    thread::spawn(move || match connect_nodes(&node_address, cloned_nodes) {
        Ok(_) => {}
        Err(_e) => {}
    });

    let config_path = match port {
        4000 => "redis0.conf",
        4001 => "redis1.conf",
        4002 => "redis2.conf",
        4003 => "redis3.conf",
        4004 => "redis4.conf",
        4005 => "redis5.conf",
        4006 => "redis6.conf",
        4007 => "redis7.conf",
        4008 => "redis8.conf",
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Port not recognized",
            ))
        }
    };

    let local_node = local_node::LocalNode::new_from_config(config_path)?;
    let node_ports = read_node_ports(config_path)?;

    {
        let mut lock_peer_nodes: std::sync::MutexGuard<'_, HashMap<String, peer_node::PeerNode>> =
            peer_nodes.lock().unwrap();

        for connection_port in node_ports {
            if connection_port != port + 10000 {
                let node_address_to_connect = format!("127.0.0.1:{}", connection_port);
                let peer_addr = format!("127.0.0.1:{}", connection_port);
                if let Ok(stream) = TcpStream::connect(node_address_to_connect) {
                    let mut cloned_stream = stream.try_clone()?;

                    let message = format!(
                        "{:?} {} {:?} {} {}\n",
                        RedisMessage::Node,
                        local_node.port,
                        local_node.role,
                        local_node.hash_range.0,
                        local_node.hash_range.1
                    );

                    cloned_stream.write_all(message.as_bytes())?;

                    lock_peer_nodes.insert(
                        peer_addr,
                        peer_node::PeerNode::new(
                            stream,
                            connection_port,
                            local_node::NodeRole::Unknown,
                            None,
                        ),
                    );

                    
                };
            }
        }

        Ok(())
    }
}

/// Permite que un nodo esuche mensajes, y maneja las conexiones con los otros nodos.
///
/// # Argumentos
/// * `address` - Dirección IP y puerto donde escuchará el servidor
/// * `nodes` - HashMap que guarda la informacion de los nodos usando el struct 'PeerNode'
///
/// # Errores
/// Retorna un error si no se puede crear el socket TCP
fn connect_nodes(
    address: &str,
    nodes: Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(address)?;
    println!("Server listening nodes on {}", address);

    for stream in listener.incoming() {
        match stream {
            Ok(mut node_stream) => {
                let client_addr = node_stream.peer_addr()?;
                println!("New node connected: {}", client_addr);

                let cloned_nodes = Arc::clone(&nodes);

                thread::spawn(move || match handle_node(&mut node_stream, cloned_nodes) {
                    Ok(_) => {
                        println!("Node {} disconnected.", client_addr);
                    }
                    Err(e) => {
                        eprintln!("Error in connection with {}: {}", client_addr, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}

/// Maneja la comunicación con otro nodo.
///
/// Por el momento solo lee el comando "node", y con eso se guarda la informacion del nodo.
fn handle_node(
    stream: &mut TcpStream,
    nodes: Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
) -> std::io::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);

    for command in reader.lines().map_while(Result::ok) {
        let input: Vec<String> = command
            .split_whitespace()
            .map(|s| s.to_string().to_lowercase())
            .collect();
        let command = &input[0];
        println!("Recibido: {:?}", input);

        match command.as_str() {
            "node" => {
                let node_listening_port = &input[1];
                let parsed_port = &input[1].trim().parse::<usize>().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid end range")
                })?;

                let node_role = match input[2].trim().to_lowercase().as_str() {
                    "master" => local_node::NodeRole::Master,
                    "replica" => local_node::NodeRole::Replica,
                    _ => local_node::NodeRole::Unknown,
                };

                let hash_range_start = &input[3].trim().parse::<usize>().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid end range")
                })?;
                let hash_range_end = &input[4].trim().parse::<usize>().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid end range")
                })?;

                {
                    let mut lock_nodes = nodes.lock().unwrap();
                    if !lock_nodes.contains_key(&node_listening_port.to_string()) {
                        let node_address_to_connect = format!("127.0.0.1:{}", node_listening_port);
                        let new_stream = TcpStream::connect(node_address_to_connect)?;

                        let node_client = peer_node::PeerNode::new(
                            new_stream,
                            *parsed_port,
                            node_role,
                            Some((*hash_range_start, *hash_range_end)),
                        );

                        let node_address_to_connect = format!("127.0.0.1:{}", node_listening_port);

                        lock_nodes.insert(node_address_to_connect.to_string(), node_client);
                    }
                }
            }
            _ => {
                writeln!(stream, "Comando no reconocido")?;
            }
        };
    }

    Ok(())
}

/// Lee un archivo de configuracion y genera una lista de los puertos a los que un nodo se debe conectar
///
/// # Errores
/// Retorna un error si alguna linea no corresponde a un puerto
fn read_node_ports<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<usize>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut ports = Vec::new();

    for line_result in reader.lines() {
        let line = line_result?;
        let split_line: Vec<&str> = line.split(",").collect();
        if let Ok(port) = split_line[0].trim().parse::<usize>() {
            ports.push(port);
        } else {
            println!("Invalid port number: {}", line);
        }
    }

    Ok(ports)
}
