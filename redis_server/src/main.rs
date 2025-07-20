extern crate aes;
extern crate rusty_docs;
use commands::redis;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env::args;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::str;
use std::sync::{Arc, Mutex};
use std::thread;
use rusty_docs::resp_parser::{CommandRequest, ValueType, format_resp_command, parse_command, write_response, CommandResponse};
use crate::hashing::get_hash_slots;
use crate::local_node::NodeRole;
use crate::local_node::NodeState;
use rusty_docs::client_info;
mod commands;
mod encryption;
mod hashing;
mod local_node;
mod peer_node;
mod redis_node_handler;
mod server_context;
use crate::server_context::ServerContext;
use client_info::ClientType;
mod types;
use types::*;
use rusty_docs::logger;
use self::logger::*;
use rusty_docs::shared;

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
    let peer_nodes: PeerNodeMap = Arc::new(Mutex::new(HashMap::new()));

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
    peer_nodes: PeerNodeMap,
) -> std::io::Result<()> {
    let config_path = "redis.conf";
    let logger = logger::Logger::init(logger::Logger::get_log_path_from_config(config_path), port);

    let local_node = redis_node_handler::create_local_node(port)?;

    // Leo los archivos de persistencia solo si soy master

    let (file_name, node_role) = {
        let locked_node = match local_node.lock() {
            Ok(lock) => lock,
            Err(poisoned) => {
                eprintln!("Mutex poisoned en local_node.lock(), usando estado interior");
                poisoned.into_inner()
            }
        };
        let file_name = format!(
            "./redis_server/rdb_files/redis_node_{}_{}_{}.rdb",
            locked_node.hash_range.0, locked_node.hash_range.1, locked_node.port
        );
        let node_role = match locked_node.role {
            local_node::NodeRole::Master => local_node::NodeRole::Master,
            local_node::NodeRole::Replica => local_node::NodeRole::Replica,
            local_node::NodeRole::Unknown => local_node::NodeRole::Unknown,
        };
        (file_name, node_role)
    };

    let mut stored_documents = HashMap::new();
    if node_role == local_node::NodeRole::Master {
        stored_documents = match load_persisted_data(&file_name) {
            Ok(docs) => docs,
            Err(_) => {
                logger.log("Iniciando con base de datos vacía");
                HashMap::new()
            }
        };
    }

    // Inicializar estructuras de datos compartidas

    let shared_documents: RedisDocumentsMap = Arc::new(Mutex::new(stored_documents));
    let (document_subscribers, shared_sets) = initialize_datasets(&shared_documents);
    let active_clients = Arc::new(Mutex::new(HashMap::new()));
    let logged_clients: LoggedClientsMap = Arc::new(Mutex::new(HashMap::new()));
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
    logger.log(&format!("Servidor iniciado en {}", bind_address));

    let ctx = Arc::new(ServerContext {
        active_clients: Arc::clone(&active_clients),
        document_subscribers: Arc::clone(&document_subscribers),
        shared_documents: Arc::clone(&shared_documents),
        shared_sets: Arc::clone(&shared_sets),
        local_node: Arc::clone(&local_node),
        peer_nodes: Arc::clone(&peer_nodes),
        logged_clients: Arc::clone(&logged_clients),
        internal_subscription_channel: initialize_subscription_channel(),
        main_addrs: bind_address.to_string(),
    });

    for incoming_connection in tcp_listener.incoming() {
        match incoming_connection {
            Ok(client_stream) => {
                if let Err(e) =
                    handle_new_client_connection(client_stream, Arc::clone(&ctx), logger.clone())
                {
                    logger.log(&format!("Error al manejar nueva conexión: {}", e));
                }
            }
            Err(e) => {
                logger.log(&format!("Error al aceptar conexión: {}", e));
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
fn initialize_datasets(documents: &RedisDocumentsMap) -> (SubscribersMap, SetsMap) {
    // Intentamos obtener la lista de keys; si falla el lock, devolvemos vectores vacíos
    let document_keys: Vec<String> = match documents.lock() {
        Ok(locked_documents) => locked_documents.keys().cloned().collect(),
        Err(poisoned) => {
            eprintln!("Mutex poisoned al intentar inicializar datasets: usando estado interno");
            poisoned.into_inner().keys().cloned().collect()
        }
    };

    let mut subscriber_map: HashMap<String, Vec<_>> = HashMap::new();
    let mut doc_set: HashMap<String, HashSet<String>> = HashMap::new();

    // Inicializar suscriptores para cada documento existente
    for document_id in document_keys {
        subscriber_map.insert(document_id.clone(), Vec::new());
        doc_set.insert(document_id, HashSet::new());
    }

    (
        Arc::new(Mutex::new(subscriber_map)),
        Arc::new(Mutex::new(doc_set)),
    )
}

fn is_client_microservice(ctx: &Arc<ServerContext>, client_id: String) -> bool {
    let clients_result = ctx.active_clients.lock();

    if let Ok(clients_lock) = clients_result {
        if let Some(client) = clients_lock.get(&client_id) {
            return client.client_type == ClientType::Microservice;
        }
    }
    false
}

fn handle_new_client_connection(
    mut client_stream: TcpStream,
    ctx: Arc<ServerContext>,
    logger: logger::Logger,
) -> std::io::Result<()> {
    let client_addr = match client_stream.peer_addr() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("Error obteniendo peer_addr: {}", e);
            return Err(e);
        }
    };

    let stream_clone = match client_stream.try_clone() {
        Ok(clone) => clone,
        Err(e) => {
            eprintln!("Error clonando stream: {}", e);
            return Err(e);
        }
    };
    let mut reader = BufReader::new(stream_clone);

    let command_request = match parse_command(&mut reader) {
        Ok(req) => req,
        Err(e) => {
            println!("Error al parsear comando: {}", e);
            if let Err(write_err) = write_response(
                &client_stream,
                &CommandResponse::Error("Comando inválido".to_string()),
            ) {
                eprintln!("Error al escribir respuesta: {}", write_err);
            }
            return Ok(());
        }
    };

    let client_type = if command_request.command == "microservicio" {
        let client_stream_clone = match client_stream.try_clone() {
            Ok(clone) => clone,
            Err(e) => {
                eprintln!("Error clonando stream para cliente: {}", e);
                return Err(e);
            }
        };

        subscribe_microservice_to_all_docs(
            client_stream_clone,
            client_addr.to_string().clone(),
            Arc::clone(&ctx.shared_documents),
            Arc::clone(&ctx.document_subscribers),
            logger.clone(),
            ctx.main_addrs.clone(),
        );
        ClientType::Microservice
    } else {
        println!("Cliente conectado: {}", client_addr);
        ClientType::Client
    };

    let client_stream_clone = match client_stream.try_clone() {
        Ok(clone) => clone,
        Err(e) => {
            eprintln!("Error clonando stream para cliente: {}", e);
            logger.log(&format!("Error clonando stream para cliente: {}", e));
            return Err(e);
        }
    };

    {
        let client_addr_str = client_addr.to_string();
        let client = client_info::Client {
            stream: Arc::new(Mutex::new(Some(client_stream_clone))),
            client_type: client_type.clone(),
            username: "".to_string(),
        };
        let mut lock_clients = match ctx.active_clients.lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        };
        lock_clients.insert(client_addr_str, client.clone());

        if client_type == ClientType::Microservice {
            subscribe_to_internal_channel(Arc::clone(&ctx), client);
        }
    }

    let client_addr_str = client_addr.to_string();
    let logger_clone = logger.clone();
    let ctx_clone = Arc::clone(&ctx);

    thread::spawn(move || {
        match handle_client(
            &mut client_stream,
            ctx_clone,
            client_addr_str.clone(),
            logger_clone,
        ) {
            Ok(_) => {
                println!("Client {} disconnected.", client_addr);
                logger.log(&format!("Cliente desconectado: {}", client_addr));
            }
            Err(e) => {
                eprintln!("Error in connection with {}: {}", client_addr, e);
                logger.log(&format!("Error en conexión con: {}", client_addr));
            }
        }
    });

    Ok(())
}

/// Suscribe al microservicio al canal de suscripciones
///
/// # Argumentos
/// * `ctx` - Contexto del servidor
/// * `microservice` - Cliente microservicio a suscribir
///
pub fn subscribe_to_internal_channel(ctx: Arc<ServerContext>, microservice: client_info::Client) {
    let mut channels_guard = match ctx.internal_subscription_channel.lock() {
        Ok(lock) => lock,
        Err(poisoned) => poisoned.into_inner(),
    };

    channels_guard.insert("notifications".to_string(), microservice);
    println!("Microservicio suscrito al canal interno subscriptions");
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
    logger: logger::Logger,
) -> std::io::Result<()> {
    let stream_clone_result = stream.try_clone();
    let mut reader = match stream_clone_result {
        Ok(clone) => BufReader::new(clone),
        Err(e) => {
            eprintln!("Error clonando el stream para lectura: {}", e);
            logger.log(&format!("Error clonando el stream para lectura: {}", e));
            return Err(e);
        }
    };

    loop {
        let command_request_result = parse_command(&mut reader);

        let command_request = match command_request_result {
            Ok(req) => req,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                }
                println!("Error al parsear comando: {}", e);
                if let Err(write_err) = write_response(
                    stream,
                    &CommandResponse::Error("Comando inválido".to_string()),
                ) {
                    eprintln!("Error al escribir respuesta: {}", write_err);
                }
                continue;
            }
        };

        let doc = match &command_request.key {
            Some(doc) => doc.clone(),
            _ => "".to_string(),
        };

        let command = command_request.command.clone();

        logger.log(&format!(
            "Comando recibido de {}: {:?}",
            client_id, command_request
        ));

        let response = match execute_command_internal(
            command_request,
            Arc::clone(&ctx),
            client_id.clone(),
            logger.clone(),
        ) {
            Ok(response) => response,
            Err(e) => {
                let error_msg = e.clone();
                println!("Error ejecutando comando: {}", e);
                write_response(stream, &CommandResponse::Error(error_msg))?;
                logger.log(&format!(
                    "Error al ejecutar comando de {}: {}",
                    client_id, e
                ));
                continue;
            }
        };

        if let Err(e) = write_response(stream, &response) {
            println!("Error al escribir respuesta: {}", e);
            logger.log(&format!(
                "Error al escribir respuesta a {}: {}",
                client_id, e
            ));
            break;
        }

        let is_subscribed_command = command == "subscribe";
        if is_subscribed_command && !response.get_resp().contains("ASK") {
            notify_microservice(Arc::clone(&ctx), doc.clone(), client_id.to_string(), false);
        }

        if command.to_lowercase() == "set" {
            if is_client_microservice(&ctx, client_id.clone()) {
                if let Err(e) = persist_documents(&ctx.shared_documents, &ctx.local_node) {
                    eprintln!("Error persistiendo documentos después de SET: {}", e);
                    logger.log(&format!(
                        "Error persistiendo documentos después de SET: {}",
                        e
                    ));
                } else {
                    logger.log("Documents persistidos exitosamente después de comando SET");
                }
            } else {
                notify_microservice(Arc::clone(&ctx), doc.clone(), client_id.to_string(), true);
            }
        }

        logger.log(&format!(
            "Respuesta enviada a {}: {:?}",
            client_id, response
        ));
    }

    cleanup_client_resources(
        &client_id,
        &ctx.active_clients,
        &ctx.document_subscribers,
        logger.clone(),
    );

    Ok(())
}

/// Ejecuta un comando internamente, manejando la resolución de ubicación de keys y la propagación a réplicas.
///
/// Esta función extrae la lógica de ejecución de comandos de handle_client para poder
/// ser reutilizada por funciones internas como notify_microservice.
///
/// # Argumentos
/// * `command_request` - El comando a ejecutar
/// * `ctx` - Contexto del servidor
/// * `client_id` - ID del cliente que ejecuta el comando (puede ser interno)
///
/// # Retorna
/// Result con la respuesta del comando o un error
fn execute_command_internal(
    command_request: CommandRequest,
    ctx: Arc<ServerContext>,
    client_id: String,
    logger: Logger,
) -> Result<CommandResponse, String> {
    let key = match &command_request.key {
        Some(k) => k.clone(),
        None => {
            return Err("Comando inválido: No se encontró la key".to_string());
        }
    };

    let response = match resolve_key_location(
        key.clone(),
        &ctx.local_node,
        &ctx.peer_nodes,
        logger.clone(),
    ) {
        Ok(()) => {
            let unparsed_command = command_request.unparsed_command.clone();
            // println!("\nunparsed command: {}", unparsed_command);

            let redis_response = redis::execute_command(
                command_request.clone(),
                &ctx.shared_documents,
                &ctx.document_subscribers,
                &ctx.shared_sets,
                client_id.clone(),
                &ctx.active_clients,
                &ctx.logged_clients,
                &ctx.internal_subscription_channel,
            );

            if redis_response.publish {
                if let Err(e) = publish_update(
                    &ctx.active_clients,
                    &ctx.document_subscribers,
                    redis_response.response.get_resp(),
                    redis_response.doc,
                    logger.clone(),
                ) {
                    eprintln!("Error al publicar actualización: {}", e);
                }
            }

            println!(
                "Broadcast_replica: {:?}",
                command_request.command.to_lowercase()
            );
            if let Err(e) = redis_node_handler::broadcast_to_replicas(
                &ctx.local_node,
                &ctx.peer_nodes,
                unparsed_command,
            ) {
                eprintln!("Error al propagar comando a réplicas: {}", e);
            }

            redis_response.response
        }
        Err(response) => response,
    };

    Ok(response)
}

/// Obtiene el microservicio del canal interno de suscripciones y devuelve su dirección peer
///
/// # Argumentos
/// * `ctx` - Contexto del servidor
///
/// # Retorna
/// Option con la dirección peer del microservicio si existe
fn get_microservice_peer_addr(ctx: &Arc<ServerContext>) -> Option<String> {
    let channels_guard = match ctx.internal_subscription_channel.lock() {
        Ok(lock) => lock,
        Err(poisoned) => poisoned.into_inner(),
    };

    if let Some(microservice) = channels_guard.get("notifications") {
        if let Ok(stream_guard) = microservice.stream.lock() {
            if let Some(stream) = stream_guard.as_ref() {
                if let Ok(peer_addr) = stream.peer_addr() {
                    return Some(peer_addr.to_string());
                }
            }
        }
    }

    None
}

pub fn notify_microservice(
    ctx: Arc<ServerContext>,
    doc: String,
    client_id: String,
    create_file: bool,
) {
    let microservice_addr = match get_microservice_peer_addr(&ctx) {
        Some(addr) => addr,
        None => {
            eprintln!("No se pudo obtener la dirección del microservicio del canal interno");
            return;
        }
    };

    let message_enum = if !create_file {
        shared::MicroserviceMessage::ClientSubscribed {
            document: doc.clone(),
            client_id: client_id.clone(),
        }
    } else {
        shared::MicroserviceMessage::Doc {
            document: doc.clone(),
            content: String::new(),
            stream_id: String::new(),
        }
    };

    let message = message_enum.to_string();

    let command_request = CommandRequest {
        command: "publish".to_string(),
        key: Some("notifications".to_string()),
        arguments: vec![ValueType::String(message.to_string())],
        unparsed_command: format!("publish subscriptions {}", message),
    };

    let _ = redis::execute_command(
        command_request,
        &ctx.shared_documents,
        &ctx.document_subscribers,
        &ctx.shared_sets,
        microservice_addr.clone(),
        &ctx.active_clients,
        &ctx.logged_clients,
        &ctx.internal_subscription_channel,
    );
}

fn _is_authorized_client(logged_clients: LoggedClientsMap, client_id: String) -> bool {
    let locked = match logged_clients.lock() {
        Ok(lock) => lock,
        Err(poisoned) => {
            eprintln!("Mutex poisoned al obtener logged_clients");
            poisoned.into_inner()
        }
    };

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
    local_node: &LocalNodeMap,
    peer_nodes: &PeerNodeMap,
    logger: Logger,
) -> Result<(), CommandResponse> {
    let hashed_key = get_hash_slots(key);

    let (lower_hash_bound, upper_hash_bound, locked_peer_nodes, node_role) = {
        let locked_node = match local_node.lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        };
        let locked_peers = match peer_nodes.lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        };
        let node_role = match locked_node.role {
            NodeRole::Master => NodeRole::Master,
            NodeRole::Replica => NodeRole::Replica,
            NodeRole::Unknown => NodeRole::Unknown,
        };

        (
            locked_node.hash_range.0,
            locked_node.hash_range.1,
            locked_peers,
            node_role,
        )
    };

    // println!("Hash: {}", hashed_key);
    logger.log(&format!("Hash: {}", hashed_key));

    if node_role != NodeRole::Master
        || hashed_key < lower_hash_bound
        || hashed_key >= upper_hash_bound
    {
        if let Some(peer_node) = locked_peer_nodes.values().find(|p| {
            p.role == NodeRole::Master
                && p.state == NodeState::Active
                && p.hash_range.0 <= hashed_key
                && p.hash_range.1 > hashed_key
        }) {
            let response_string =
                format!("ASK {} 127.0.0.1:{}", hashed_key, peer_node.port - 10000);
            let redis_redirect_response = CommandResponse::Array(vec![
                CommandResponse::String("ASK".to_string()),
                CommandResponse::String(hashed_key.clone().to_string()),
                CommandResponse::String(format!("127.0.0.1:{}", peer_node.port - 10000)),
            ]);

            println!("\n\nHashing para otro nodo: {:?}", response_string.clone());
            logger.log(&format!(
                "Hashing para otro nodo: {:?}",
                response_string.clone()
            ));

            return Err(redis_redirect_response);
        } else {
            let response_string = format!("ASK {}", hashed_key);
            let redis_redirect_response = CommandResponse::Array(vec![
                CommandResponse::String("ASK".to_string()),
                CommandResponse::String(hashed_key.clone().to_string()),
            ]);

            // println!(
            //     "Hashing para nodo indefinido: {:?}",
            //     response_string.clone()
            // );
            logger.log(&format!(
                "Hashing para nodo indefinido: {:?}",
                response_string.clone()
            ));

            return Err(redis_redirect_response);
        }
    }

    Ok(())
}

/// Publica una actualización a todos los clientes suscritos a un documento.
///
/// # Errores
/// Retorna un error si hay problemas al escribir en algún stream de cliente
pub fn publish_update(
    active_clients: &ClientsMap,
    document_subscribers: &SubscribersMap,
    update_message: String,
    document_id: String,
    logger: logger::Logger,
) -> std::io::Result<()> {
    let mut clients_guard = match active_clients.lock() {
        Ok(lock) => lock,
        Err(poisoned) => poisoned.into_inner(),
    };
    let subscribers_guard = match document_subscribers.lock() {
        Ok(lock) => lock,
        Err(poisoned) => poisoned.into_inner(),
    };

    if let Some(document_subscribers) = subscribers_guard.get(&document_id) {
        for subscriber_id in document_subscribers {
            if let Some(client) = clients_guard.get_mut(subscriber_id) {
                if let Err(e) = write!(client, "{}", update_message.trim()) {
                    eprintln!("Error enviando actualización a {}: {}", subscriber_id, e);
                    logger.log(&format!(
                        "Error enviando actualización a {}: {}",
                        subscriber_id, e
                    ));
                    return Err(e);
                } else {
                    client.flush()?;
                    logger.log(&format!(
                        "Actualización enviada a {} sobre documento {}",
                        subscriber_id, document_id
                    ));
                }
            } else {
                println!("Cliente no encontrado: {}", subscriber_id);
                logger.log(&format!(
                    "Cliente {} no encontrado al intentar enviar actualización de {}",
                    subscriber_id, document_id
                ));
            }
        }
    } else {
        println!("Document no encontrado: {}", document_id);
        logger.log(&format!(
            "Document {} no encontrado al intentar publicar actualización",
            document_id
        ));
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
    active_clients: &ClientsMap,
    document_subscribers: &SubscribersMap,
    logger: Logger,
) {
    if let Ok(mut lock) = active_clients.lock() {
        if lock.remove(client_id).is_some() {
            logger.log(&format!(
                "Cliente {} eliminado de la lista de activos",
                client_id
            ));
        }
    } else {
        eprintln!("Mutex poisoned al limpiar active_clients");
        logger.log("Mutex poisoned al limpiar active_clients");
    }

    if let Ok(mut subscribers_guard) = document_subscribers.lock() {
        let mut eliminado = false;
        for subscriber_list in subscribers_guard.values_mut() {
            let antes = subscriber_list.len();
            subscriber_list.retain(|id| id != client_id);
            if subscriber_list.len() < antes {
                eliminado = true;
            }
        }
        if eliminado {
            logger.log(&format!(
                "Cliente {} eliminado de listas de suscriptores",
                client_id
            ));
        }
    } else {
        eprintln!("Mutex poisoned al limpiar document_subscribers");
        logger.log("Mutex poisoned al limpiar document_subscribers");
    }
}

/// Persiste el estado actual de los documentos en el archivo.
///
/// # Errores
/// Retorna un error si hay problemas al escribir en el archivo
pub fn persist_documents(
    documents: &RedisDocumentsMap,
    local_node: &LocalNodeMap,
) -> std::io::Result<()> {
    let file_name = match local_node.lock() {
        Ok(locked_node) => {
            format!(
                "./redis_server/rdb_files/redis_node_{}_{}_{}.rdb",
                locked_node.hash_range.0, locked_node.hash_range.1, locked_node.port
            )
        }
        Err(poisoned) => {
            let locked_node = poisoned.into_inner();
            format!(
                "./redis_server/rdb_files/redis_node_{}_{}_{}.rdb",
                locked_node.hash_range.0, locked_node.hash_range.1, locked_node.port
            )
        }
    };

    let mut persistence_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&file_name)?;

    let documents_guard = match documents.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    for (doc_name, doc) in documents_guard.iter() {
        let data = doc.to_string();
        // Si el contenido ya empieza con el nombre, no lo agregues de nuevo
        if data.starts_with(&format!("{}/++/", doc_name)) {
            writeln!(persistence_file, "{}", data)?;
        } else {
            writeln!(persistence_file, "{}/++/{}/--/", doc_name, data)?;
        }
    }

    Ok(())
}

/// Carga los documentos persistidos desde el archivo.
///
/// # Retorna
/// HashMap con los documentos y sus mensajes, o un error si hay problemas
/// al leer el archivo
pub fn load_persisted_data(file_path: &String) -> Result<HashMap<String, String>, String> {
    let mut documents = HashMap::new();

    let file = match File::open(file_path) {
        Ok(f) => f,
        Err(_) => return Err("archivo-no-encontrado".to_string()),
    };

    let reader = BufReader::new(file);
    let lines = reader.lines();

    for line_result in lines {
        let content = match line_result {
            Ok(l) => l,
            Err(e) => return Err(e.to_string()),
        };

        let parts: Vec<&str> = content.split("/++/").collect();
        if parts.len() != 2 {
            continue;
        }

        let document_id = parts[0].to_string();
        let messages_data = parts[1];
        documents.insert(document_id, messages_data.to_string());
    }
    // println!("documentos: {:#?}", documents);
    Ok(documents)
}

pub fn subscribe_microservice_to_all_docs(
    mut client_stream: TcpStream,
    addr: String,
    docs: RedisDocumentsMap,
    clients_on_docs: SubscribersMap,
    logger: Logger,
    main_addrs: String,
) {
    let docs_lock = match docs.lock() {
        Ok(lock) => lock,
        Err(poisoned) => {
            eprintln!("Error al bloquear docs mutex: {:?}", poisoned);
            logger.log(&format!("Error al bloquear docs mutex: {:?}", poisoned));
            return;
        }
    };
    let mut map = match clients_on_docs.lock() {
        Ok(lock) => lock,
        Err(poisoned) => {
            eprintln!("Error al bloquear clients_on_docs mutex: {:?}", poisoned);
            logger.log(&format!(
                "Error al bloquear clients_on_docs mutex: {:?}",
                poisoned
            ));

            return;
        }
    };

    for (doc_name, document) in docs_lock.iter() {
        let subscribers = map.entry(doc_name.clone()).or_insert_with(Vec::new);
        if !subscribers.contains(&addr) {
            subscribers.push(addr.clone());
            println!(
                "Microservicio {} suscripto automáticamente a {}",
                addr, doc_name
            );
            let document_data = document.to_string().clone();

            let command_parts = vec!["DOC", doc_name, &document_data, &main_addrs];

            let message = format_resp_command(&command_parts.clone());
            if let Err(e) = client_stream.write_all(message.as_bytes()) {
                eprintln!("Error enviando notificación DOC al microservicio: {}", e);
            } else {
                let _ = client_stream.flush();
            }
        }
    }

    if let Err(e) = client_stream.flush() {
        eprintln!("Error al hacer flush del stream: {}", e);
    }
}

/// Inicializa los canales de comunicación internos del sistema
///
/// # Retorna
/// InternalChannelsMap con los canales internos inicializados
fn initialize_subscription_channel() -> ClientsMap {
    let mut internal_channels: HashMap<String, client_info::Client> = HashMap::new();
    internal_channels.insert(
        "notifications".to_string(),
        client_info::Client {
            stream: Arc::new(Mutex::new(None)),
            client_type: ClientType::Microservice,
            username: "Microservice".to_string(),
        },
    );
    Arc::new(Mutex::new(internal_channels))
}
