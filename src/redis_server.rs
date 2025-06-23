use commands::redis;
use commands::redis_response::RedisResponse;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env::args;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::str;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::hashing::get_hash_slots;
use crate::local_node::NodeRole;
use crate::local_node::NodeState;
mod client_info;
mod commands;
mod documento;
mod hashing;
mod local_node;
mod peer_node;
mod redis_node_handler;
mod utils;
mod server_context;
use client_info::ClientType;
use documento::Documento;
use crate::server_context::ServerContext;
#[path = "utils/logger.rs"]
mod logger;
use crate::commands::redis_parser::{CommandRequest, CommandResponse, parse_command, write_response};

type SubscribersMap = Arc<Mutex<HashMap<String, Vec<String>>>>;
type SetsMap = Arc<Mutex<HashMap<String, HashSet<String>>>>;


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
    let peer_nodes: Arc<Mutex<HashMap<String, peer_node::PeerNode>>> =
        Arc::new(Mutex::new(HashMap::new()));

    start_server(&client_address, port, node_address, peer_nodes)?;

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
fn start_server(
    bind_address: &str,
    port: usize,
    node_address: String,
    peer_nodes: Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
) -> std::io::Result<()> {
    let config_path = "redis.conf";
    let log_path = logger::get_log_path_from_config(config_path);

    if fs::metadata(&log_path)
        .map(|m| m.len() > 0)
        .unwrap_or(false)
    {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .and_then(|mut file| writeln!(file));
    }

    let local_node = redis_node_handler::create_local_node(port)?;

    // Leo los archivos de persistencia solo si soy master

    let file_name;
    let node_role: local_node::NodeRole;
    let mut stored_documents = HashMap::new();
    {
        let locked_node = local_node.lock().unwrap();
        file_name = format!(
            "redis_node_{}_{}.rdb",
            locked_node.hash_range.0, locked_node.hash_range.1
        );
        node_role = match locked_node.role {
            local_node::NodeRole::Master => local_node::NodeRole::Master,
            local_node::NodeRole::Replica => local_node::NodeRole::Replica,
            local_node::NodeRole::Unknown => local_node::NodeRole::Unknown,
        }
    }
    println!("file name: {}", file_name);

    if node_role == NodeRole::Master {
        stored_documents = match load_persisted_data(&file_name) {
            Ok(docs) => docs,
            Err(_) => {
                println!("Iniciando con base de datos vacía");
                logger::log_event(&log_path, "Iniciando con base de datos vacía");
                HashMap::new()
            }
        };
    }

    // Inicializar estructuras de datos compartidas

    let shared_documents: Arc<Mutex<HashMap<String, Documento>>> =
        Arc::new(Mutex::new(stored_documents));
    let (document_subscribers, shared_sets) = initialize_datasets(&shared_documents);

    let active_clients = Arc::new(Mutex::new(HashMap::new()));
    let logged_clients: Arc<Mutex<HashMap<String, bool>>> = Arc::new(Mutex::new(HashMap::new()));

    redis_node_handler::start_node_connection(
        port,
        node_address,
        &local_node,
        &peer_nodes,
        &document_subscribers,
        &shared_documents,
        &shared_sets,
    )?;
    // Iniciar servidor TCP
    let tcp_listener = TcpListener::bind(bind_address)?;
    println!("Server listening to clients on: {}", bind_address);

    logger::log_event(&log_path, &format!("Servidor iniciado en {}", bind_address));

    let ctx = Arc::new(ServerContext {
        active_clients: Arc::clone(&active_clients),
        document_subscribers: Arc::clone(&document_subscribers),
        shared_documents: Arc::clone(&shared_documents),
        shared_sets: Arc::clone(&shared_sets),
        local_node: Arc::clone(&local_node),
        peer_nodes: Arc::clone(&peer_nodes),
        logged_clients: Arc::clone(&logged_clients),
        log_path: log_path.clone(),
    });

    for incoming_connection in tcp_listener.incoming() {
        match incoming_connection {
            Ok(client_stream) => {
                handle_new_client_connection(
                    client_stream,
                    Arc::clone(&ctx)
                )?;
            }
            Err(e) => {
                eprintln!("Error al aceptar conexión: {}", e);
                logger::log_event(&log_path, &format!("Error al aceptar conexión: {}", e));
            }
        }
    }

    Ok(())
}

/// Inicializa el mapa de suscriptores para cada documento y los sets para cada documento.
///
/// Crea una entrada vacía en el mapa y en el set de suscriptores para cada documento
/// existente en la base de datos
///
/// # Argumentos
/// * `documents` - HashMap con los documentos existentes
///
/// # Retorna
/// (Arc::new(Mutex::new(subscriber_map)), Arc::new(Mutex::new(doc_set))) con las listas de suscriptores
/// inicializadas y los sets iniciales
fn initialize_datasets(
    documents: &Arc<Mutex<HashMap<String, Documento>>>,
) -> (
    SubscribersMap,
    SetsMap,
) {
    let document_keys: Vec<String>;
    {
        let locked_documents = documents.lock().unwrap();
        document_keys = locked_documents.keys().cloned().collect();
    }
    let mut subscriber_map: HashMap<String, Vec<_>> = HashMap::new();
    let mut doc_set: HashMap<String, HashSet<String>> = HashMap::new();

    for document_id in document_keys {
        subscriber_map.insert(document_id.clone(), Vec::new());
        doc_set.insert(document_id, HashSet::new());
    }

    (
        Arc::new(Mutex::new(subscriber_map)),
        Arc::new(Mutex::new(doc_set)),
    )
}

fn handle_new_client_connection(
    mut client_stream: TcpStream,
    ctx: Arc<ServerContext>,
) -> std::io::Result<()> {
    let client_addr = client_stream.peer_addr()?;
    let mut reader = BufReader::new(client_stream.try_clone()?);
    let command_request = match parse_command(&mut reader) {
        Ok(req) => req,
        Err(e) => {
            println!("Error al parsear comando: {}", e);
            write_response(
                &client_stream,
                &CommandResponse::Error("Comando inválido".to_string()),
            )?;
            return Ok(()); // Salir anticipadamente
        }
    };

    let client_type = if command_request.command == "microservicio" {
        subscribe_microservice_to_all_docs(
            client_addr.to_string(),
            Arc::clone(&ctx.shared_documents),
            Arc::clone(&ctx.document_subscribers),
        );
        println!("Microservicio conectado: {}", client_addr);
        ClientType::Microservicio
    } else {
        println!("Cliente conectado: {}", client_addr);
        ClientType::Cliente
    };

    logger::log_event(&ctx.log_path, &format!("Cliente conectado: {}", client_addr));

    let client_stream_clone = client_stream.try_clone()?;
    {
        let client_addr = client_addr.to_string();
        let client = client_info::Client {
            stream: client_stream_clone,
            client_type,
            username: "".to_string(),
        };
        let mut lock_clients = ctx.active_clients.lock().unwrap();
        lock_clients.insert(client_addr, client);
    }

    let client_addr_str = client_addr.to_string();
    let log_path = ctx.log_path.clone();

    // Ya NO necesitas crear un nuevo ctx aquí

    logger::log_event(&log_path, &format!("Cliente conectado: {}", client_addr));


    let ctx = Arc::clone(&ctx);
    thread::spawn(move || {
        match handle_client(
            &mut client_stream,
            ctx,
            client_addr_str,
        ) {
            Ok(_) => {
                println!("Client {} disconnected.", client_addr);

                logger::log_event(
                    &log_path,
                    &format!("Cliente desconectado: {}", client_addr),
                );
            }
            Err(e) => {
                eprintln!("Error in connection with {}: {}", client_addr, e);

                logger::log_event(
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
    ctx: Arc<ServerContext>,
    client_id: String,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    loop {
        let command_request: CommandRequest =
            match parse_command(&mut reader) {
                Ok(req) => req,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    println!("Error al parsear comando: {}", e);
                    write_response(
                        stream,
                        &CommandResponse::Error(
                            "Comando inválido".to_string(),
                        ),
                    )?;
                    continue;
                }
            };

        println!("Comando recibido: {:?}", command_request);
        logger::log_event(
            &ctx.log_path,
            &format!("Comando recibido de {}: {:?}", client_id, command_request),
        );

        //TODO esto solo se puede resolver con la replicacion, ya que hay informacion que tienen algunos nodos y otros que no
        // Entonces el estado de las variables no es el mismo en cada instancia
        /* if command_request.command != "auth" && command_request.command != "connect" {
            println!("client_id {}", client_id);
            println!("Usuarios logueados, {:#?}", ctx.logged_clients);
            println!("Verificando autorización para client_id: {}", client_id);
            let logged_clients_clone = ctx.logged_clients.clone();
            if !is_authorized_client(ctx.logged_clients_clone, client_id.clone()) {
                println!("Cliente no autorizado: {}", client_id);
                utils::write_response(
                    stream,
                    &utils::CommandResponse::Error("Cliente sin autorizacion".to_string()),
                )?;
                logger::log_event(
                    ctx.log_path,
                    &format!("Cliente {} sin autorizacion ", client_id),
                );
                continue;
            }
        } */

        let key = match &command_request.key {
            Some(k) => k.clone(),
            None => {
                println!("No key found");
                write_response(
                    stream,
                    &CommandResponse::Error("Comando inválido".to_string()),
                )?;
                logger::log_event(
                    &ctx.log_path,
                    &format!(
                        "Error al parsear comando de {}: No se encontro la key",
                        client_id
                    ),
                );
                continue;
            }
        };

        let response = match resolve_key_location(key, &ctx.local_node, &ctx.peer_nodes) {
            Ok(()) => {
                let unparsed_command = command_request.unparsed_command.clone();
                let redis_response = redis::execute_command(
                    command_request,
                    &ctx.shared_documents,
                    &ctx.document_subscribers,
                    &ctx.shared_sets,
                    client_id.clone(),
                    &ctx.active_clients,
                    &ctx.logged_clients,
                );

                if redis_response.publish {
                    if let Err(e) = publish_update(
                        &ctx.active_clients,
                        &ctx.document_subscribers,
                        redis_response.message,
                        redis_response.doc,
                    ) {
                        eprintln!("Error al publicar actualización: {}", e);
                    }
                }

                let _ = redis_node_handler::broadcast_to_replicas(
                    &ctx.local_node,
                    &ctx.peer_nodes,
                    unparsed_command,
                );

                redis_response.response
            }
            Err(response) => response,
        };

        if let Err(e) = write_response(stream, &response) {
            println!("Error al escribir respuesta: {}", e);
            logger::log_event(
                &ctx.log_path,
                &format!("Error al escribir respuesta a {}: {}", client_id, e),
            );
            break;
        }

        logger::log_event(
            &ctx.log_path,
            &format!("Respuesta enviada a {}: {:?}", client_id, response),
        );

        if let Err(e) = persist_documents(&ctx.shared_documents, &ctx.local_node) {
            eprintln!("Error al persistir documentos: {}", e);
        }
    }

    cleanup_client_resources(&client_id, &ctx.active_clients, &ctx.document_subscribers);
    // to do: agregar comando para salir, esto nunca se ejecuta porque nunca termina el loop

    Ok(())
}

fn _is_authorized_client(
    logged_clients: Arc<Mutex<HashMap<String, bool>>>,
    client_id: String,
) -> bool {
    println!("Verificando autorización para client_id: {}", client_id);
    println!("Formato exacto del client_id: {:?}", client_id);
    let locked = logged_clients.lock().unwrap();
    println!("Estado actual del HashMap: {:#?}", *locked);
    println!(
        "Claves en el HashMap: {:?}",
        locked.keys().collect::<Vec<_>>()
    );
    match locked.get(&client_id) {
        Some(&true) => {
            println!("Cliente {} autorizado", client_id);
            true
        }
        Some(&false) => {
            println!("Cliente {} no autorizado", client_id);
            false
        }
        None => {
            println!("Cliente {} no encontrado en el HashMap", client_id);
            false
        }
    }
}

/// Determina si la key recibida corresponde al nodo actual o si debe ser redirigida a otro nodo,
/// a traves del mensaje "ASK *key hasheada* *ip del nodo correspondiente*". En el caso de que
/// no se encuentre el nodo correspondiente, se manda el mensaje sin ip.
///
/// # Devuelve
/// - "Ok(())" si la key corresponde al nodo actual
/// - "Err(CommandResponse)" con el mensaje "ASK" si corresponde a otro nodo
pub fn resolve_key_location(
    key: String,
    local_node: &Arc<Mutex<local_node::LocalNode>>,
    peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
) -> Result<(), CommandResponse> {
    let hashed_key = get_hash_slots(key);

    {
        let locked_node = local_node.lock().unwrap();
        let locked_peer_nodes = peer_nodes.lock().unwrap();
        let lower_hash_bound = locked_node.hash_range.0;
        let upper_hash_bound = locked_node.hash_range.1;

        println!("Hash: {}", hashed_key);

        if hashed_key < lower_hash_bound || hashed_key >= upper_hash_bound {
            if let Some(peer_node) = locked_peer_nodes.values().find(|p| {
                p.role == NodeRole::Master
                    && p.state == NodeState::Active
                    && p.hash_range.0 <= hashed_key
                    && p.hash_range.1 > hashed_key
            }) {
                let response_string =
                    format!("ASK {} 127.0.0.1:{}", hashed_key, peer_node.port - 10000);
                let redis_redirect_response = CommandResponse::String(response_string.clone());

                println!("Hashing para otro nodo: {:?}", response_string.clone());

                return Err(redis_redirect_response);
            } else {
                let response_string = format!("ASK {}", hashed_key);
                let redis_redirect_response = CommandResponse::String(response_string.clone());

                println!(
                    "Hashing para nodo indefinido: {:?}",
                    response_string.clone()
                );

                return Err(redis_redirect_response);
            }
        }
    }

    Ok(())
}

/// Publica una actualización a todos los clientes suscritos a un documento.
///
/// # Errores
/// Retorna un error si hay problemas al escribir en algún stream de cliente
pub fn publish_update(
    active_clients: &Arc<Mutex<HashMap<String, client_info::Client>>>,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
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
pub fn persist_documents(
    documents: &Arc<Mutex<HashMap<String, Documento>>>,
    local_node: &Arc<Mutex<local_node::LocalNode>>,
) -> io::Result<()> {
    let file_name;
    {
        let locked_node = local_node.lock().unwrap();
        file_name = format!(
            "redis_node_{}_{}.rdb",
            locked_node.hash_range.0, locked_node.hash_range.1
        );
    }
    let mut persistence_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(file_name)?;

    let documents_guard = documents.lock().unwrap();
    let document_ids: Vec<&String> = documents_guard.keys().collect();

    for document_id in document_ids {
        if let Some(doc) = documents_guard.get(document_id) {
            match doc {
                Documento::Texto(lineas) => {
                    let mut document_data = format!("{}/++/", document_id);
                    for linea in lineas {
                        document_data.push_str(linea);
                        document_data.push_str("/--/");
                    }
                    writeln!(persistence_file, "{}", document_data)?;
                }
                Documento::Calculo(filas) => {
                    let mut document_data = format!("{}/++/", document_id);
                    if !filas.is_empty() {
                        for fila in filas {
                            document_data.push_str(&fila.join(","));
                            document_data.push_str("/--/");
                        }
                    }
                    writeln!(persistence_file, "{}", document_data)?;
                }
            }
        }
    }

    Ok(())
}

/// Carga los documentos persistidos desde el archivo.
///
/// # Retorna
/// HashMap con los documentos y sus mensajes, o un error si hay problemas
/// al leer el archivo
pub fn load_persisted_data(file_path: &String) -> Result<HashMap<String, Documento>, String> {
    let mut documents = HashMap::new();
    let file = File::open(file_path).map_err(|_| "archivo-no-encontrado".to_string())?;
    let reader = BufReader::new(file);
    let lines = reader.lines();

    for line in lines {
        let content = line.map_err(|e| e.to_string())?;
        let parts: Vec<&str> = content.split("/++/").collect();
        if parts.len() != 2 {
            continue;
        }
        let document_id = parts[0].to_string();
        let messages_data = parts[1];

        if document_id.ends_with(".txt") {
            let messages: Vec<String> = messages_data
                .split("/--/")
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            documents.insert(document_id, Documento::Texto(messages));
        } else if document_id.ends_with(".xlsx") {
            let filas: Vec<Vec<String>> = messages_data
                .split("/--/")
                .filter(|s| !s.is_empty())
                .map(|fila| fila.split(',').map(|c| c.to_string()).collect())
                .collect();
            documents.insert(document_id, Documento::Calculo(filas));
        }
    }
    Ok(documents)
}

pub fn subscribe_microservice_to_all_docs(
    addr: String,
    docs: Arc<Mutex<HashMap<String, Documento>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) {
    let docs_lock = docs.lock().unwrap();
    let mut map = clients_on_docs.lock().unwrap();

    for doc_name in docs_lock.keys() {
        if let Some(list) = map.get_mut(doc_name) {
            list.push(addr.clone());
            RedisResponse::new(
                CommandResponse::String(format!("Subscribed to {}", doc_name)),
                false,
                "".to_string(),
                "".to_string(),
            );
            println!(
                "Microservicio {} suscripto automáticamente a {}",
                addr, doc_name
            );

            // let notification = format!("Client {} subscribed to {}", client_addr, doc_name);

            // RedisResponse::new(CommandResponse::Null, true, notification, doc_name.to_string())
        }
        // let subscribers = clients_on_docs_lock
        //     .entry(doc_name.clone())
        //     .or_insert_with(Vec::new);

        // if !subscribers.contains(&addr) {
        //     subscribers.push(addr.clone());
        //     println!(
        //         "Microservicio {} suscripto automáticamente a {}",
        //         addr, doc_name
        //     );
        // }
    }
}
