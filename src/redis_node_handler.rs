use crate::commands::redis_parser::{parse_replica_command, write_response, CommandResponse};
use crate::documento::Documento;
use commands::redis;
use local_node::{LocalNode, NodeRole, NodeState};
use peer_node;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Cursor;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum RedisMessage {
    Node,
}

pub fn get_config_path(port: usize) -> Result<String, std::io::Error> {
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

    Ok(config_path.to_string())
}

pub fn create_local_node(port: usize) -> Result<Arc<Mutex<LocalNode>>, std::io::Error> {
    let config_path = get_config_path(port)?;

    let local_node = LocalNode::new_from_config(config_path)?;
    Ok(Arc::new(Mutex::new(local_node)))
}

/// Intenta establecer una primera conexion con los otros nodos del servidor
///
/// # Errores
/// Retorna un error si el puerto no corresponde a uno definido en los archivos de configuracion.
pub fn start_node_connection(
    port: usize,
    node_address: String,
    local_node: &Arc<Mutex<LocalNode>>,
    peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_documents: &Arc<Mutex<HashMap<String, Documento>>>,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> Result<(), std::io::Error> {
    let cloned_nodes = Arc::clone(peer_nodes);

    let config_path = match get_config_path(port) {
        Ok(path) => path,
        Err(e) => return Err(e),
    };

    let cloned_local_node = Arc::clone(local_node);
    let cloned_document_subscribers = Arc::clone(document_subscribers);
    let cloned_shared_documents = Arc::clone(shared_documents);
    let cloned_shared_sets = Arc::clone(shared_sets);

    let node_ports = match read_node_ports(config_path) {
        Ok(ports) => ports,
        Err(e) => return Err(e),
    };

    thread::spawn(move || {
        let _ = connect_nodes(
            &node_address,
            cloned_nodes,
            cloned_local_node,
            cloned_document_subscribers,
            cloned_shared_documents,
            cloned_shared_sets,
        );
    });

    let cloned_local_node_for_ping_pong = Arc::clone(local_node);
    let cloned_peer_nodes = Arc::clone(peer_nodes);

    thread::spawn(move || {
        let _ = ping_to_master(cloned_local_node_for_ping_pong, cloned_peer_nodes);
    });

    // Bloque para conexión con otros nodos
    let locked_local_node = match local_node.lock() {
        Ok(node) => node,
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error al bloquear local_node",
            ))
        }
    };

    let mut lock_peer_nodes = match peer_nodes.lock() {
        Ok(lock) => lock,
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error al bloquear peer_nodes",
            ))
        }
    };

    for connection_port in node_ports {
        if connection_port != locked_local_node.port {
            let node_address_to_connect = format!("127.0.0.1:{}", connection_port);
            let peer_addr = node_address_to_connect.clone();

            match TcpStream::connect(&node_address_to_connect) {
                Ok(stream) => {
                    let mut cloned_stream = match stream.try_clone() {
                        Ok(s) => s,
                        Err(e) => return Err(e),
                    };

                    let message = format!(
                        "{:?} {} {:?} {} {} {}\n",
                        RedisMessage::Node,
                        locked_local_node.port,
                        locked_local_node.role,
                        locked_local_node.hash_range.0,
                        locked_local_node.hash_range.1,
                        locked_local_node.priority,
                    );

                    if let Err(e) = cloned_stream.write_all(message.as_bytes()) {
                        return Err(e);
                    }

                    lock_peer_nodes.insert(
                        peer_addr,
                        peer_node::PeerNode::new(
                            stream,
                            connection_port,
                            NodeRole::Unknown,
                            (0, 16383),
                            NodeState::Active,
                            0,
                        ),
                    );
                }
                Err(_) => {
                    // Podés loguear que no se pudo conectar, pero no cortamos toda la ejecución
                    continue;
                }
            }
        }
    }

    Ok(())
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
    local_node: Arc<Mutex<LocalNode>>,
    document_subscribers: Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_documents: Arc<Mutex<HashMap<String, Documento>>>,
    shared_sets: Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(address)?;
    println!("\nServer listening to nodes on: {}", address);

    for stream in listener.incoming() {
        match stream {
            Ok(mut node_stream) => {
                let client_addr = node_stream.peer_addr()?;
                println!("New node connected: {}", client_addr);

                let cloned_nodes = Arc::clone(&nodes);
                let cloned_local_node = Arc::clone(&local_node);
                let cloned_document_subscribers = Arc::clone(&document_subscribers);
                let cloned_shared_documents = Arc::clone(&shared_documents);
                let cloned_shared_sets = Arc::clone(&shared_sets);

                thread::spawn(move || {
                    match handle_node(
                        &mut node_stream,
                        cloned_nodes,
                        &cloned_local_node,
                        cloned_document_subscribers,
                        cloned_shared_documents,
                        cloned_shared_sets,
                    ) {
                        Ok(_) => {
                            println!("Node {} disconnected.", client_addr);
                        }
                        Err(e) => {
                            eprintln!("Error in connection with {}: {}", client_addr, e);
                        }
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
    local_node: &Arc<Mutex<LocalNode>>,
    document_subscribers: Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_documents: Arc<Mutex<HashMap<String, Documento>>>,
    shared_sets: Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> std::io::Result<()> {
    let reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(e) => return Err(e),
    };

    let mut saving_command = false;
    let mut command_string = String::new();
    let mut serialized_hashmap = Vec::new();
    let mut serialized_vec = Vec::new();

    for command in reader.lines().map_while(Result::ok) {
        let input: Vec<String> = command
            .split_whitespace()
            .map(|s| s.to_string().to_lowercase())
            .collect();

        let command = &input[0];
        println!("Recibido: {:?}", input);

        match command.as_str() {
            "node" => {
                if input.len() < 6 {
                    continue;
                }

                let parsed_port = match input[1].trim().parse::<usize>() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let hash_range_start = match input[3].trim().parse::<usize>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let hash_range_end = match input[4].trim().parse::<usize>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let priority = match input[5].trim().parse::<usize>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let node_listening_port = &input[1];
                let node_address = format!("127.0.0.1:{}", node_listening_port);

                let node_role = match input[2].trim().to_lowercase().as_str() {
                    "master" => NodeRole::Master,
                    "replica" => NodeRole::Replica,
                    _ => NodeRole::Unknown,
                };

                let mut local_node_locked = match local_node.lock() {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                let mut lock_nodes = match nodes.lock() {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                if !lock_nodes.contains_key(&node_address) {
                    let node_address_to_connect = format!("127.0.0.1:{}", node_listening_port);
                    let new_stream = match TcpStream::connect(&node_address_to_connect) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                    let mut stream_to_respond = match new_stream.try_clone() {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                    let node_client = peer_node::PeerNode::new(
                        new_stream,
                        parsed_port,
                        node_role.clone(),
                        (hash_range_start, hash_range_end),
                        NodeState::Active,
                        priority,
                    );

                    if hash_range_start == local_node_locked.hash_range.0 {
                        if node_role != local_node_locked.role {
                            if local_node_locked.role == NodeRole::Master {
                                local_node_locked.replica_nodes.push(parsed_port);
                            } else {
                                local_node_locked.master_node = Some(parsed_port);
                                let message = format!("sync_request {}\n", local_node_locked.port);
                                let _ = stream_to_respond.write_all(message.as_bytes());
                            }
                        } else {
                            local_node_locked.replica_nodes.push(parsed_port);
                        }
                    }

                    lock_nodes.insert(node_address.clone(), node_client);

                    let message = format!(
                        "{:?} {} {:?} {} {} {}\n",
                        RedisMessage::Node,
                        local_node_locked.port,
                        local_node_locked.role,
                        local_node_locked.hash_range.0,
                        local_node_locked.hash_range.1,
                        local_node_locked.priority,
                    );

                    let _ = stream_to_respond.write_all(message.as_bytes());
                } else if let Some(peer_node_to_update) = lock_nodes.get_mut(&node_address) {
                    peer_node_to_update.role = node_role.clone();
                    peer_node_to_update.hash_range = (hash_range_start, hash_range_end);

                    if peer_node_to_update.hash_range == local_node_locked.hash_range {
                        if node_role != local_node_locked.role {
                            if local_node_locked.role == NodeRole::Master {
                                local_node_locked.replica_nodes.push(parsed_port);
                            } else {
                                local_node_locked.master_node = Some(parsed_port);
                                let message = format!("sync_request {}\n", local_node_locked.port);
                                let _ = peer_node_to_update.stream.write_all(message.as_bytes());
                            }
                        } else {
                            local_node_locked.replica_nodes.push(parsed_port);
                        }
                    }
                }
            }
            "sync_request" => {
                if input.len() > 1 {
                    let _ = handle_replica_sync(&input[1], &nodes, &shared_sets, &shared_documents);
                }
            }
            "serialize_hashmap" => {
                if input.len() > 1 {
                    serialized_hashmap.push(input[1].clone());
                }
            }
            "serialize_vec" => {
                if input.len() > 1 {
                    serialized_vec.push(input[1].clone());
                }
            }
            "end_serialize_hashmap" => {
                deserialize_hashset_hashmap(&serialized_hashmap, &shared_sets);
                serialized_hashmap.clear();
            }
            "end_serialize_vec" => {
                deserialize_vec_hashmap(&serialized_vec, &shared_documents);
                serialized_vec.clear();
            }
            "start_replica_command" => saving_command = true,
            "end_replica_command" => {
                saving_command = false;
                let cursor = Cursor::new(command_string.clone());
                let mut reader_for_command = BufReader::new(cursor);

                match parse_replica_command(&mut reader_for_command) {
                    Ok(command_request) => {
                        let response = redis::execute_replica_command(
                            command_request,
                            &shared_documents,
                            &document_subscribers,
                            &shared_sets,
                        );
                        println!("Replica response: {:?}", response.response);
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::UnexpectedEof {
                            break;
                        }
                        println!("Error al parsear comando: {}", e);
                        let _ = write_response(
                            stream,
                            &CommandResponse::Error("Comando inválido".to_string()),
                        );
                        continue;
                    }
                }
                command_string.clear();
            }
            "ping" => {
                let message = "pong\n";
                let _ = stream.write_all(message.as_bytes());
            }
            "confirm_master_down" => {
                match confirm_master_state(local_node, &nodes) {
                    Ok(master_state) => {
                        println!("master state: {:?}", master_state);
                        if master_state == NodeState::Inactive {
                            println!("01");

                            let locked_nodes = match nodes.lock() {
                                Ok(n) => n,
                                Err(_) => continue,
                            };
                            let hash_range;

                            println!("02");

                            {
                                let locked_local_node = local_node.lock().unwrap();
                                if locked_local_node.role != NodeRole::Replica {
                                    println!("Master node cannot initiate replica promotion");
                                }

                                hash_range = locked_local_node.hash_range;
                            }

                            println!("03");
                            for (_, peer) in locked_nodes.iter() {
                                if peer.role == NodeRole::Replica && peer.hash_range == hash_range {
                                    if let Ok(mut peer_stream) = peer.stream.try_clone() {
                                        let message = format!("initialize_replica_promotion\n");
                                        let _ = peer_stream.write_all(message.as_bytes());
                                        println!("04");
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => eprintln!("Error confirmando estado del master"),
                };
                println!("05");
            }
            "initialize_replica_promotion" => {
                println!("here");
                initialize_replica_promotion(local_node, &nodes);
            }
            "inactive_node" => {
                if input.len() > 1 {
                    let inactive_node_addr = format!("127.0.0.1:{}", input[1]);
                    if let Ok(mut locked_nodes) = nodes.lock() {
                        if let Some(node) = locked_nodes.get_mut(&inactive_node_addr) {
                            node.state = NodeState::Inactive;
                        }
                    }
                }
            }
            _ => {
                if saving_command {
                    command_string.push_str(&format!("{}\r\n", command));
                } else {
                    let message = "Comando no reconocido\n";
                    let _ = stream.write_all(message.as_bytes());
                }
            }
        }
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

pub fn broadcast_to_replicas(
    local_node: &Arc<Mutex<LocalNode>>,
    peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
    unparsed_command: String,
) -> std::io::Result<()> {
    let locked_local_node = match local_node.lock() {
        Ok(n) => n,
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error al bloquear local_node",
            ));
        }
    };

    let mut locked_peer_nodes = match peer_nodes.lock() {
        Ok(n) => n,
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error al bloquear peer_nodes",
            ));
        }
    };

    let replicas = &locked_local_node.replica_nodes;
    println!("initial command {}", unparsed_command);

    for replica in replicas {
        let key = format!("127.0.0.1:{}", replica);
        if let Some(peer_node) = locked_peer_nodes.get_mut(&key) {
            let stream = &mut peer_node.stream;

            if stream
                .write_all("start_replica_command\n".as_bytes())
                .is_err()
            {
                eprintln!("Error escribiendo start_replica_command a {}", key);
                continue;
            }
            if stream.write_all(unparsed_command.as_bytes()).is_err() {
                eprintln!("Error enviando comando a {}", key);
                continue;
            }
            if stream
                .write_all("end_replica_command\n".as_bytes())
                .is_err()
            {
                eprintln!("Error escribiendo end_replica_command a {}", key);
                continue;
            }
        } else {
            eprintln!("No se encontró nodo réplica para {}", key);
        }
    }

    Ok(())
}

fn handle_replica_sync(
    replica_port: &String,
    peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
    shared_documents: &Arc<Mutex<HashMap<String, Documento>>>,
) -> std::io::Result<()> {
    let replica_addr = format!("127.0.0.1:{}", replica_port);
    // Clonar conjuntos
    let cloned_sets = match shared_sets.lock() {
        Ok(locked_sets) => locked_sets.clone(),
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error al bloquear shared_sets",
            ));
        }
    };

    // Clonar documentos
    let cloned_shared_documents = match shared_documents.lock() {
        Ok(locked_docs) => locked_docs.clone(),
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error al bloquear shared_documents",
            ));
        }
    };

    // Enviar a réplica
    let mut locked_peer_nodes = match peer_nodes.lock() {
        Ok(lock) => lock,
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error al bloquear peer_nodes",
            ));
        }
    };

    if let Some(peer_node) = locked_peer_nodes.get_mut(&replica_addr) {
        match peer_node.stream.try_clone() {
            Ok(peer_stream) => {
                let stream1 = match peer_stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => {
                        eprintln!("Error al clonar stream para conjuntos");
                        return Ok(()); // se ignora el error
                    }
                };

                let stream2 = match peer_stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => {
                        eprintln!("Error al clonar stream para documentos");
                        return Ok(()); // se ignora el error
                    }
                };

                let _ = serialize_hashset_hashmap(&cloned_sets, stream1);
                let _ = serialize_vec_hashmap(&cloned_shared_documents, stream2);
            }
            Err(_) => {
                eprintln!("Error al clonar stream de la réplica");
            }
        }
    } else {
        eprintln!("No se encontró réplica con dirección {}", replica_addr);
    }

    Ok(())
}

fn serialize_vec_hashmap(
    map: &HashMap<String, Documento>,
    mut stream: TcpStream,
) -> std::io::Result<()> {
    for (key, doc) in map {
        let line = match doc {
            Documento::Texto(vec) => {
                let joined = vec.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",");

                format!("serialize_vec {}:{}\n", key, joined)
            }
            _ => continue,
        };
        stream.write_all(line.as_bytes())?;
    }
    let message = "end_serialize_vec\n";
    stream.write_all(message.as_bytes())?;
    Ok(())
}

fn serialize_hashset_hashmap(
    map: &HashMap<String, HashSet<String>>,
    mut stream: TcpStream,
) -> std::io::Result<()> {
    for (key, set) in map {
        let values: Vec<String> = set.iter().cloned().collect();
        let line = format!("serialize_hashmap {}:{}\n", key, values.join(","));
        stream.write_all(line.as_bytes())?;
        println!("se mando start");
    }
    let message = "end_serialize_hashmap\n";
    stream.write_all(message.as_bytes())?;
    println!("se mando end");
    Ok(())
}

fn deserialize_hashset_hashmap(
    lines: &Vec<String>,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) {
    if let Ok(mut locked_sets) = shared_sets.lock() {
        for line in lines {
            if let Some((key, values_str)) = line.split_once(':') {
                let values: HashSet<String> = values_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                locked_sets.insert(key.to_string(), values);
            }
        }
    } else {
        eprintln!("No se pudo bloquear shared_sets en deserialize_hashset_hashmap");
    }
}

fn deserialize_vec_hashmap(
    lines: &Vec<String>,
    shared_documents: &Arc<Mutex<HashMap<String, Documento>>>,
) {
    if let Ok(mut locked_documents) = shared_documents.lock() {
        for line in lines {
            if let Some((key, values_str)) = line.split_once(':') {
                let values: Vec<String> = values_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                locked_documents.insert(key.to_string(), Documento::Texto(values));
            }
        }
    } else {
        eprintln!("No se pudo bloquear shared_documents en deserialize_vec_hashmap");
    }
}

fn ping_to_master(
    local_node: Arc<Mutex<LocalNode>>,
    peer_nodes: Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
) -> std::io::Result<()> {
    let ping_interval = Duration::from_secs(5);
    let error_interval = Duration::from_secs(50);
    let mut last_sent = Instant::now();

    let mut stream_to_ping = None;

    loop {
        let mut now = Instant::now();
        if now.duration_since(last_sent) >= ping_interval {
            {
                if stream_to_ping.is_none() {
                    let locked_local_node = local_node.lock().unwrap();
                    let mut locked_peer_nodes = peer_nodes.lock().unwrap();
                    if let Some(port) = locked_local_node.master_node {
                        let key = format!("127.0.0.1:{}", port);
                        if let Some(peer_node) = locked_peer_nodes.get_mut(&key) {
                            if peer_node.state == NodeState::Active {
                                let cloned_stream = peer_node.stream.try_clone().ok();
                                stream_to_ping = cloned_stream;
                            }
                        }
                    }
                }
            }

            match stream_to_ping.as_ref() {
                Some(mut stream) => {
                    stream.set_read_timeout(Some(error_interval))?;
                    let mut reader = BufReader::new(stream.try_clone()?);
                    stream.write_all("ping\n".to_string().as_bytes())?;
                    now = Instant::now();

                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => {
                            println!("Connection closed by peer");
                            request_master_state_confirmation(&local_node, &peer_nodes);
                            stream_to_ping = None;
                            std::hint::spin_loop();
                        }
                        Ok(_) => {
                            println!("Received response: {}", line.trim());
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                            println!("Timeout: no response within {:?}", error_interval);
                            request_master_state_confirmation(&local_node, &peer_nodes);
                            stream_to_ping = None;
                            std::hint::spin_loop();
                        }
                        Err(e) => {
                            println!("Unexpected error: {}", e);
                            request_master_state_confirmation(&local_node, &peer_nodes);
                            stream_to_ping = None;
                            std::hint::spin_loop();
                        }
                    }

                    last_sent = now;
                }
                None => {
                    std::hint::spin_loop();
                }
            }
        }
        std::hint::spin_loop();
    }
}

fn request_master_state_confirmation(
    local_node: &Arc<Mutex<LocalNode>>,
    peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
) {
    let master_port_option;
    let master_port;
    let hash_range;

    {
        // Bloqueo local_node
        match local_node.lock() {
            Ok(locked_local_node) => {
                if locked_local_node.role != NodeRole::Replica {
                    println!("Master node cannot initiate replica promotion");
                    return;
                }

                master_port_option = locked_local_node.master_node;
                hash_range = locked_local_node.hash_range;
            }
            Err(_) => {
                eprintln!("Error al bloquear local_node");
                return;
            }
        }

        master_port = match master_port_option {
            Some(p) => p,
            None => {
                println!("No master node");
                return;
            }
        };
    }

    let mut contacted_replica = false;

    {
        // Bloqueo peer_nodes
        let mut locked_peer_nodes = match peer_nodes.lock() {
            Ok(n) => n,
            Err(_) => {
                eprintln!("Error al bloquear peer_nodes");
                return;
            }
        };

        // Marcar master como inactivo
        for peer in locked_peer_nodes.values_mut() {
            if peer.port == master_port {
                peer.state = NodeState::Inactive;
            }
        }

        // Intentar contactar a otra réplica
        for (_, peer) in locked_peer_nodes.iter() {
            if peer.role == NodeRole::Replica
                && peer.hash_range == hash_range
                && peer.port != master_port
            {
                let message = format!("confirm_master_down {}\n", master_port);
                match peer.stream.try_clone() {
                    Ok(mut peer_stream) => {
                        if peer_stream.write_all(message.as_bytes()).is_ok() {
                            contacted_replica = true;
                        } else {
                            eprintln!("Error escribiendo a la réplica");
                        }
                    }
                    Err(_) => {
                        eprintln!("Error clonando stream para réplica");
                    }
                }
            }
        }
    }

    if !contacted_replica {
        initialize_replica_promotion(local_node, peer_nodes);
    }
}

fn confirm_master_state(
    local_node: &Arc<Mutex<LocalNode>>,
    peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
) -> std::io::Result<NodeState> {
    let master_port_option;
    let mut master_state = NodeState::Active;

    {
        master_port_option = match local_node.lock() {
            Ok(locked_local_node) => locked_local_node.master_node,
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Error locking local_node",
                ));
            }
        };
    }

    let master_port = match master_port_option {
        Some(p) => p,
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Master node not set",
            ));
        }
    };

    {
        let peer_nodes_locked = match peer_nodes.lock() {
            Ok(lock) => lock,
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Error locking peer_nodes",
                ));
            }
        };

        for (_addr, peer_node) in peer_nodes_locked.iter() {
            if peer_node.port == master_port && peer_node.state == NodeState::Inactive {
                master_state = NodeState::Inactive;
            }
        }
    }

    Ok(master_state)
}

fn initialize_replica_promotion(
    local_node: &Arc<Mutex<LocalNode>>,
    peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
) {
    println!("initializing replica promotion");

    let (mut locked_local_node, locked_peer_nodes) = match (local_node.lock(), peer_nodes.lock()) {
        (Ok(local), Ok(peers)) => (local, peers),
        _ => {
            eprintln!("Error locking local_node or peer_nodes");
            return;
        }
    };

    let inactive_port = match locked_local_node.master_node {
        Some(port) => port,
        None => {
            eprintln!("No master node set in local_node");
            return;
        }
    };

    println!("inactive port: {}", inactive_port);

    locked_local_node.role = NodeRole::Master;
    locked_local_node.master_node = None;

    let node_info_message = format!(
        "{:?} {} {:?} {} {}\n",
        RedisMessage::Node,
        locked_local_node.port,
        locked_local_node.role,
        locked_local_node.hash_range.0,
        locked_local_node.hash_range.1
    );

    let inactive_node_message = format!("inactive_node {}\n", inactive_port);

    for (_, peer) in locked_peer_nodes.iter() {
        if peer.state == NodeState::Active {
            println!("sending to: {}", peer.port);
            match peer.stream.try_clone() {
                Ok(mut peer_stream) => {
                    if let Err(e) = peer_stream.write_all(node_info_message.as_bytes()) {
                        eprintln!("Error writing node_info_message: {}", e);
                    }
                    if let Err(e) = peer_stream.write_all(inactive_node_message.as_bytes()) {
                        eprintln!("Error writing inactive_node_message: {}", e);
                    }
                    println!("sent");
                }
                Err(_) => {
                    eprintln!("Error cloning stream");
                }
            }
        }
    }
}
