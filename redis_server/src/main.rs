extern crate aes;
extern crate rusty_docs;
use crate::hashing::get_hash_slots;
use crate::local_node::NodeRole;
use crate::local_node::NodeState;
use commands::redis;
use rusty_docs::client_info;
use rusty_docs::resp_parser::{
    format_resp_command, parse_command, write_response, CommandRequest, CommandResponse, ValueType,
};
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
mod commands;
mod encryption;
mod execute_command_params;
mod hashing;
mod local_node;
mod peer_node;
mod redis_node_handler;
mod server_context;
mod utils;
use crate::execute_command_params::ExecuteCommandParams;
use crate::server_context::ServerContext;
use client_info::ClientType;
mod types;
use self::logger::*;
use rusty_docs::logger;
use rusty_docs::shared;
use types::*;

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

    let node_address = format!("0.0.0.0:{}", port + 10000);
    let client_address = format!("0.0.0.0:{}", port);
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
    let logger = logger::Logger::init(
        logger::Logger::get_log_path_from_config(config_path, ""),
        port,
    );

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
        let node_role = locked_node.role.clone();
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
    println!("Servidor iniciado en {}", bind_address);

    let ctx = Arc::new(ServerContext {
        active_clients: Arc::clone(&active_clients),
        document_subscribers: Arc::clone(&document_subscribers),
        shared_documents: Arc::clone(&shared_documents),
        shared_sets: Arc::clone(&shared_sets),
        local_node: Arc::clone(&local_node),
        peer_nodes: Arc::clone(&peer_nodes),
        logged_clients: Arc::clone(&logged_clients),
        internal_subscription_channel: initialize_subscription_channel(),
        llm_channel: initialize_llm_request_channel(),
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

/// Inicializa los mapas de suscriptores y sets para cada documento existente.
///
/// Esta función recorre todos los documentos actuales y crea una entrada vacía
/// en el mapa de suscriptores y en el set de suscriptores para cada documento.
///
/// # Argumentos
/// * `documents` - Mapa compartido de documentos existentes (`RedisDocumentsMap`)
///
/// # Retorna
/// Una tupla con:
/// - `SubscribersMap`: Mapa compartido de listas de suscriptores por documento.
/// - `SetsMap`: Mapa compartido de sets de suscriptores por documento.
///
/// # Ejemplo
/// ```rust
/// let (subs, sets) = initialize_datasets(&shared_documents);
/// ```
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

/// Determina si un cliente es un microservicio a partir de su ID.
///
/// Consulta el mapa de clientes activos y verifica si el tipo de cliente es `Microservice`.
///
/// # Argumentos
/// * `ctx` - Contexto compartido del servidor.
/// * `client_id` - Identificador del cliente a consultar.
///
/// # Retorna
/// `true` si el cliente es un microservicio, `false` en caso contrario.
fn is_client_microservice(ctx: &Arc<ServerContext>, client_id: String) -> bool {
    let clients_result = ctx.active_clients.lock();

    if let Ok(clients_lock) = clients_result {
        if let Some(client) = clients_lock.get(&client_id) {
            return client.client_type == ClientType::Microservice;
        }
    }
    false
}

/// Maneja la conexión de un nuevo cliente al servidor.
///
/// Realiza el handshake inicial, determina el tipo de cliente (normal, microservicio o LLM),
/// lo registra en el contexto, y lanza un hilo para manejar su comunicación.
///
/// # Argumentos
/// * `client_stream` - Stream TCP del cliente.
/// * `ctx` - Contexto compartido del servidor.
/// * `logger` - Logger para registrar eventos.
///
/// # Retorna
/// `Ok(())` si la conexión se maneja correctamente, o un error de IO en caso contrario.
///
/// # Detalles
/// - Si el cliente es un microservicio, lo suscribe automáticamente a todos los documentos.
/// - Si es LLM o microservicio, lo suscribe a los canales internos correspondientes.
/// - Lanza un hilo para manejar la comunicación con el cliente.
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

    let client_type = if command_request.command == "microservicio"
        || command_request.command == "llm_microservice"
    {
        let client_stream_clone = match client_stream.try_clone() {
            Ok(clone) => clone,
            Err(e) => {
                eprintln!("Error clonando stream para cliente: {}", e);
                return Err(e);
            }
        };
        let client_type = if command_request.command == "microservicio" {
            ClientType::Microservice
        } else {
            ClientType::LlmMicroservice
        };

        if client_type == ClientType::Microservice {
            subscribe_microservice_to_all_docs(
                client_stream_clone,
                client_addr.to_string().clone(),
                Arc::clone(&ctx.shared_documents),
                Arc::clone(&ctx.document_subscribers),
                logger.clone(),
                ctx.main_addrs.clone(),
                client_type.clone(),
            );
        }

        client_type
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
        let client_addr_str: String = client_addr.to_string();
        let client = client_info::Client {
            stream: Arc::new(Mutex::new(Some(client_stream_clone))),
            client_type: client_type.clone(),
            username: "".to_string(),
        };
        let mut lock_clients = match ctx.active_clients.lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        };
        lock_clients.insert(client_addr_str.clone(), client.clone());

        if client_type != ClientType::Client {
            if client_type == ClientType::Microservice {
                subscribe_to_internal_channel(Arc::clone(&ctx), client.clone())
            }
            if client_type == ClientType::LlmMicroservice {
                subscribe_to_llm_request_channel(Arc::clone(&ctx), client.clone());
            }
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

/// Suscribe un cliente LLM-microservicio al canal de solicitudes LLM (`llm_requests`).
///
/// # Argumentos
/// * `ctx` - Contexto compartido del servidor.
/// * `client` - Estructura del cliente a suscribir.
///
/// # Detalles
/// Agrega el cliente al vector de suscriptores del canal `llm_requests`.
pub fn subscribe_to_llm_request_channel(ctx: Arc<ServerContext>, client: client_info::Client) {
    let mut channels_guard = match ctx.llm_channel.lock() {
        Ok(lock) => lock,
        Err(poisoned) => poisoned.into_inner(),
    };
    println!("Se suscribio a llm_channesl : {:#?}", client);
    channels_guard
        .entry("llm_requests".to_string())
        .or_insert_with(Vec::new)
        .push(client);
}

/// Suscribe un microservicio al canal interno de notificaciones (`notifications`).
///
/// # Argumentos
/// * `ctx` - Contexto compartido del servidor.
/// * `client` - Estructura del cliente microservicio a suscribir.
///
/// # Detalles
/// Inserta el cliente como único suscriptor del canal `notifications`.
pub fn subscribe_to_internal_channel(ctx: Arc<ServerContext>, client: client_info::Client) {
    let mut channels_guard = match ctx.internal_subscription_channel.lock() {
        Ok(lock) => lock,
        Err(poisoned) => poisoned.into_inner(),
    };

    channels_guard.insert("notifications".to_string(), client);
    println!("Microservicio suscrito al canal interno subscriptions");
}

/// Maneja la comunicación con un cliente conectado.
///
/// Lee comandos del cliente, los procesa, envía respuestas y publica actualizaciones
/// a otros clientes suscritos si corresponde. También persiste documentos tras un `SET`.
///
/// # Argumentos
/// * `stream` - Stream TCP del cliente.
/// * `ctx` - Contexto compartido del servidor.
/// * `client_id` - Identificador del cliente.
/// * `logger` - Logger para registrar eventos.
///
/// # Retorna
/// `Ok(())` si la comunicación finaliza correctamente, o un error de IO en caso contrario.
///
/// # Detalles
/// - Si el cliente se desconecta, limpia sus recursos.
/// - Si el comando es `subscribe`, notifica al microservicio.
/// - Si el comando es `set`, persiste los documentos o notifica al microservicio según el tipo de cliente.
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

/// Ejecuta un comando internamente, resolviendo la ubicación de la key y propagando a réplicas si es necesario.
///
/// # Argumentos
/// * `command_request` - Comando a ejecutar.
/// * `ctx` - Contexto compartido del servidor.
/// * `client_id` - Identificador del cliente que ejecuta el comando.
/// * `logger` - Logger para registrar eventos.
///
/// # Retorna
/// `Ok(CommandResponse)` con la respuesta del comando, o `Err(String)` si ocurre un error.
///
/// # Detalles
/// - Si la key no corresponde al nodo actual, retorna una respuesta `ASK` para redirección.
/// - Propaga el comando a réplicas si corresponde.
/// - Publica actualizaciones si el comando lo requiere.
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
            let execute_command_params = ExecuteCommandParams {
                request: command_request.clone(),
                docs: &ctx.shared_documents,
                document_subscribers: &ctx.document_subscribers,
                shared_sets: &ctx.shared_sets,
                client_addr: client_id.clone(),
                active_clients: &ctx.active_clients,
                logged_clients: &ctx.logged_clients,
                suscription_channel: &ctx.internal_subscription_channel,
                llm_channel: &ctx.llm_channel,
            };
            let redis_response = redis::execute_command(execute_command_params);

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

/// Obtiene la dirección peer del microservicio suscrito al canal interno.
///
/// # Argumentos
/// * `ctx` - Contexto compartido del servidor.
///
/// # Retorna
/// `Some(String)` con la dirección peer si existe, o `None` si no se encuentra.
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

/// Notifica al microservicio sobre una suscripción o creación de archivo.
///
/// Envía un mensaje al microservicio a través del canal interno de notificaciones,
/// indicando que un cliente se suscribió o que se creó un archivo.
///
/// # Argumentos
/// * `ctx` - Contexto compartido del servidor.
/// * `doc` - Nombre del documento.
/// * `client_id` - Identificador del cliente.
/// * `create_file` - Si es `true`, notifica creación de archivo; si es `false`, suscripción.
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
    let execute_command_params = ExecuteCommandParams {
        request: command_request.clone(),
        docs: &ctx.shared_documents,
        document_subscribers: &ctx.document_subscribers,
        shared_sets: &ctx.shared_sets,
        client_addr: microservice_addr.clone(),
        active_clients: &ctx.active_clients,
        logged_clients: &ctx.logged_clients,
        suscription_channel: &ctx.internal_subscription_channel,
        llm_channel: &ctx.llm_channel,
    };
    let _ = redis::execute_command(execute_command_params);
}

/// Verifica si un cliente está autorizado (logueado) en el sistema.
///
/// # Argumentos
/// * `logged_clients` - Mapa compartido de clientes logueados.
/// * `client_id` - Identificador del cliente.
///
/// # Retorna
/// `true` si el cliente está autorizado, `false` en caso contrario.
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

/// Determina si la key recibida corresponde al nodo actual o debe ser redirigida.
///
/// Si la key no corresponde al nodo actual, retorna una respuesta `ASK` con la dirección
/// del nodo correspondiente o sin dirección si no se encuentra.
///
/// # Argumentos
/// * `key` - Key a resolver.
/// * `local_node` - Nodo local.
/// * `peer_nodes` - Mapa de nodos pares.
/// * `logger` - Logger para registrar eventos.
///
/// # Retorna
/// - `Ok(())` si la key corresponde al nodo actual.
/// - `Err(CommandResponse)` con el mensaje `ASK` si corresponde a otro nodo.
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
        let node_role = locked_node.role.clone();

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
            let ask_address = utils::get_node_address(peer_node.port - 10000);
            let response_string = format!("ASK {} {}", hashed_key, ask_address);
            // let response_string =
            //     format!("ASK {} 127.0.0.1:{}", hashed_key, peer_node.port - 10000);
            let redis_redirect_response = CommandResponse::Array(vec![
                CommandResponse::String("ASK".to_string()),
                CommandResponse::String(hashed_key.clone().to_string()),
                CommandResponse::String(ask_address),
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
/// # Argumentos
/// * `active_clients` - Mapa compartido de clientes activos.
/// * `document_subscribers` - Mapa compartido de suscriptores por documento.
/// * `update_message` - Mensaje a publicar.
/// * `document_id` - ID del documento.
/// * `logger` - Logger para registrar eventos.
///
/// # Retorna
/// `Ok(())` si la publicación fue exitosa, o un error de IO si falla.
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

/// Limpia los recursos asociados a un cliente desconectado.
///
/// Elimina al cliente de la lista de clientes activos y de todas las listas de suscriptores.
///
/// # Argumentos
/// * `client_id` - Identificador del cliente.
/// * `active_clients` - Mapa compartido de clientes activos.
/// * `document_subscribers` - Mapa compartido de suscriptores por documento.
/// * `logger` - Logger para registrar eventos.
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

/// Persiste el estado actual de los documentos en un archivo RDB.
///
/// Guarda el contenido de todos los documentos en un archivo específico del nodo.
///
/// # Argumentos
/// * `documents` - Mapa compartido de documentos.
/// * `local_node` - Nodo local.
///
/// # Retorna
/// `Ok(())` si la persistencia fue exitosa, o un error de IO si falla.
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

/// Carga los documentos persistidos desde un archivo RDB.
///
/// # Argumentos
/// * `file_path` - Ruta al archivo de persistencia.
///
/// # Retorna
/// `Ok(HashMap<String, String>)` con los documentos cargados, o un error si falla la lectura.
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
    //println!("documentos: {:#?} file_path_ {file_path}", documents);
    Ok(documents)
}

/// Suscribe automáticamente al microservicio a todos los documentos existentes.
///
/// Envía al microservicio los datos de cada documento al que se suscribe.
///
/// # Argumentos
/// * `client_stream` - Stream TCP del microservicio.
/// * `addr` - Dirección del microservicio.
/// * `docs` - Mapa compartido de documentos.
/// * `clients_on_docs` - Mapa compartido de suscriptores por documento.
/// * `logger` - Logger para registrar eventos.
/// * `main_addrs` - Dirección principal del servidor.
/// * `client_type` - Tipo de cliente (Microservice o LlmMicroservice).
pub fn subscribe_microservice_to_all_docs(
    mut client_stream: TcpStream,
    addr: String,
    docs: RedisDocumentsMap,
    clients_on_docs: SubscribersMap,
    logger: Logger,
    main_addrs: String,
    client_type: ClientType,
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

            if client_type == ClientType::Microservice {
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
            } else {
                println!(
                    "LLM-Microservicio {} suscripto automáticamente a {}",
                    addr, doc_name
                );
            }
        }
    }

    if let Err(e) = client_stream.flush() {
        eprintln!("Error al hacer flush del stream: {}", e);
    }
}

/// Inicializa el canal interno de notificaciones del sistema.
///
/// Crea un canal con una entrada para el microservicio.
///
/// # Retorna
/// Un `ClientsMap` con el canal `notifications` inicializado.
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

/// Inicializa el canal interno de solicitudes LLM del sistema.
///
/// Crea un canal con una entrada para el LLM-microservicio.
///
/// # Retorna
/// Un `LlmNodesMap` con el canal `llm_request` inicializado.
fn initialize_llm_request_channel() -> LlmNodesMap {
    let mut internal_channels: HashMap<String, Vec<client_info::Client>> = HashMap::new();

    let mut vector = Vec::new();
    let client = client_info::Client {
        stream: Arc::new(Mutex::new(None)),
        client_type: ClientType::LlmMicroservice,
        username: "llm_microservice".to_string(),
    };

    vector.push(client);
    internal_channels.insert("llm_request".to_string(), vector);
    Arc::new(Mutex::new(internal_channels))
}
