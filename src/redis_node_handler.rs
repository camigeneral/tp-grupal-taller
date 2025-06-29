use crate::commands::redis_parser::{parse_replica_command, write_response, CommandResponse};
use crate::redis_node_handler::redis_types::SetsMap;
use commands::redis;
use local_node::{LocalNode, NodeRole, NodeState};
use peer_node;
use encryption::{encrypt_message, KEY};
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

const PRINT_PINGS: bool = true;
extern crate base64;
use aes::Aes128;
use aes::cipher::{
    BlockDecrypt, KeyInit,
    generic_array::GenericArray,
};
use self::base64::{engine::general_purpose, Engine as _};
#[path = "redis_types.rs"]
mod redis_types;
use redis_types::*;

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
    local_node: &LocalNodeMap,
    peer_nodes: &PeerNodeMap,
    document_subscribers: &SubscribersMap,
    shared_documents: &RedisDocumentsMap,
    shared_sets: &SetsMap,
) -> Result<(), std::io::Error> {
    let key = GenericArray::from_slice(&KEY);
    let cipher = Aes128::new(&key);
    
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

    // let cloned_local_node_for_ping_pong = Arc::clone(local_node);
    // let cloned_peer_nodes = Arc::clone(peer_nodes);

    // thread::spawn(move || {
    //     let _ = ping_to_master(cloned_local_node_for_ping_pong, cloned_peer_nodes);
    // });

    let cloned_local_node_for_ping_pong_node = Arc::clone(local_node);
    let cloned_peer_nodes_for_ping_pong = Arc::clone(peer_nodes);

    thread::spawn(move || {
        let _ = ping_to_node(cloned_local_node_for_ping_pong_node, cloned_peer_nodes_for_ping_pong);
    });

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

                    println!("Recibido comando: {}", message);


                    let encrypted_b64 = encrypt_message(&cipher, &message);

                    if let Err(e) = cloned_stream.write_all(encrypted_b64.as_bytes()) {
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
                            0
                        ),
                    );
                }
                Err(_) => {
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
    nodes: PeerNodeMap,
    local_node: LocalNodeMap,
    document_subscribers: SubscribersMap,
    shared_documents: RedisDocumentsMap,
    shared_sets: SetsMap,
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
    nodes: PeerNodeMap,
    local_node: &LocalNodeMap,
    document_subscribers: SubscribersMap,
    shared_documents: RedisDocumentsMap,
    shared_sets: SetsMap,
) -> std::io::Result<()> {
    let key = GenericArray::from_slice(&KEY);
    let cipher = Aes128::new(&key);

    let reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(e) => return Err(e),
    };

    let mut saving_command = false;
    let mut command_string = String::new();
    let mut serialized_hashmap = Vec::new();
    let mut serialized_vec = Vec::new();
        
    for command in reader.lines().map_while(Result::ok) {
        let message;

        // Decodifica base64
        let encoded_bytes = match general_purpose::STANDARD.decode(&command) {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("Error decodificando base64");
                continue;
            }
        };

        // Descifra como antes
        let mut decrypted = Vec::new();
        for chunk in encoded_bytes.chunks(16) {
            let mut block = GenericArray::clone_from_slice(chunk);
            cipher.decrypt_block(&mut block);
            decrypted.extend_from_slice(&block);
        }

        // Padding seguro
        if !decrypted.is_empty() {
            let pad = *decrypted.last().unwrap() as usize;
            if pad > 0 && pad <= decrypted.len() {
                decrypted.truncate(decrypted.len() - pad);
            } else {
                eprintln!("Padding inválido al descifrar mensaje de nodo");
                continue;
            }
        } else {
            continue;
        }

        message = String::from_utf8(decrypted).expect("UTF-8 inválido");
        

        let input: Vec<String> = message
            .split_whitespace()
            .map(|s| s.to_string().to_lowercase())
            .collect();

        let command = &input[0];

        if command != "pong"  && command != "ping" || PRINT_PINGS {
            println!("Recibido: {:?}", input);
        }

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
                        0
                    );

                    if hash_range_start == local_node_locked.hash_range.0 {
                        if node_role != local_node_locked.role {
                            if local_node_locked.role == NodeRole::Master {
                                local_node_locked.replica_nodes.push(parsed_port);
                            } else {
                                local_node_locked.master_node = Some(parsed_port);
                                let message = format!("sync_request {}\n", local_node_locked.port);

                                let encrypted_b64 = encrypt_message(&cipher, &message);
                                
                                let _ = stream_to_respond.write_all(encrypted_b64.as_bytes());
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
                    let encrypted_b64 = encrypt_message(&cipher, &message);

                    let _ = stream_to_respond.write_all(encrypted_b64.as_bytes());
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

                                let encrypted_b64 = encrypt_message(&cipher, &message);
                                let _ = peer_node_to_update.stream.write_all(encrypted_b64.as_bytes());
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
                {
                    let mut locked_local_node = local_node.lock().unwrap();
                    let mut locked_peer_nodes = nodes.lock().unwrap();
                    let updated_epoch = locked_local_node.epoch + 1;
                    locked_local_node.epoch = updated_epoch;
                    for replica in locked_local_node.replica_nodes.clone() {
                        let replica_address = format!("127.0.0.1:{}", replica);
                        if let Some(replica_node) = locked_peer_nodes.get_mut(&replica_address) {
                            let message = format!("update_epoch {} {}\n", locked_local_node.port, updated_epoch);
                            let _ = replica_node.stream.write_all(message.as_bytes());
                        }
                    }
                }
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
                let encrypted_b64 = encrypt_message(&cipher, &message);
                let _ = stream.write_all(encrypted_b64.as_bytes());
            }
            "node_status" => {
                // ("node_status {} {:?}\n", inactive_port, NodeState::Fail);

                let inactive_port = match input[1].trim().parse::<usize>() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let inactive_state: NodeState = match input[2].trim().to_lowercase().as_str() {
                    "fail" => NodeState::Fail,
                    "pfail" => NodeState::PFail,
                    _ => NodeState::Active,
                };
                let promote_replica = set_failed_node(local_node, &nodes, inactive_port, inactive_state);
                if promote_replica {
                    initialize_replica_promotion(local_node, &nodes);
                }
            }
            "update_epoch" => {
                let port = match input[1].trim().parse::<usize>() {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                let epoch = match input[2].trim().parse::<usize>() {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                {
                    let mut locked_peer_nodes = nodes.lock().unwrap();
                    let address = format!("127.0.0.1:{}", port);

                    if let Some(replica_node) = locked_peer_nodes.get_mut(&address) {
                        replica_node.epoch = epoch;
                    }
                }

            }
            _ => {
                if saving_command {
                    command_string.push_str(&format!("{}\r\n", command));
                } else {
                    let message = "Comando no reconocido\n";
                    let encrypted_b64 = encrypt_message(&cipher, &message);
                    let _ = stream.write_all(encrypted_b64.as_bytes());
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
    let key = GenericArray::from_slice(&KEY);
    let cipher = Aes128::new(key);

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

            let start_cmd = encrypt_message(&cipher, "start_replica_command\n");
            if stream.write_all(start_cmd.as_bytes()).is_err() {
                eprintln!("Error escribiendo start_replica_command a {}", key);
                continue;
            }

            let encrypted_cmd = encrypt_message(&cipher, &unparsed_command);
            if stream.write_all(encrypted_cmd.as_bytes()).is_err() {
                eprintln!("Error enviando comando a {}", key);
                continue;
            }

            let end_cmd = encrypt_message(&cipher, "end_replica_command\n");
            if stream.write_all(end_cmd.as_bytes()).is_err() {
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
    peer_nodes: &PeerNodeMap,
    shared_sets: &SetsMap,
    shared_documents: &RedisDocumentsMap,
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
    map: &HashMap<String, String>,
    mut stream: TcpStream,
) -> std::io::Result<()> {
    let key = GenericArray::from_slice(&KEY);
    let cipher = Aes128::new(key);

    for (key, line) in map {
        let message = format!("serialize_vec {}:{}\n", key, line);
        let encrypted_b64 = encrypt_message(&cipher, &message);
        stream.write_all(encrypted_b64.as_bytes())?;
    }

    let message = "end_serialize_vec\n";
    let encrypted_b64 = encrypt_message(&cipher, &message);

    stream.write_all(encrypted_b64.as_bytes())?;
    Ok(())
}


fn serialize_hashset_hashmap(
    map: &HashMap<String, HashSet<String>>,
    mut stream: TcpStream,
) -> std::io::Result<()> {
    let key = GenericArray::from_slice(&KEY);
    let cipher = Aes128::new(key);

    for (key, set) in map {
        let values: Vec<String> = set.iter().cloned().collect();
        let line = format!("serialize_hashmap {}:{}\n", key, values.join(","));
        let encrypted_b64 = encrypt_message(&cipher, &line);
        stream.write_all(encrypted_b64.as_bytes())?;
        println!("se mando start");
    }
    let message = "end_serialize_hashmap\n";
    let encrypted_b64 = encrypt_message(&cipher, &message);
    stream.write_all(encrypted_b64.as_bytes())?;
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

fn deserialize_vec_hashmap(lines: &Vec<String>, shared_documents: &RedisDocumentsMap) {
    if let Ok(mut locked_documents) = shared_documents.lock() {
        for line in lines {
            if let Some((key, values_str)) = line.split_once(':') {
                locked_documents.insert(key.to_string(), values_str.to_string());
            }
        }
    } else {
        eprintln!("No se pudo bloquear shared_documents en deserialize_vec_hashmap");
    }
}


fn ping_to_node(
    local_node: Arc<Mutex<LocalNode>>,
    peer_nodes: Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
) -> std::io::Result<()> {
    let key = GenericArray::from_slice(&KEY);
    let cipher = Aes128::new(key);
    let ping_interval = Duration::from_secs(5);
    let error_interval = Duration::from_secs(5);
    let mut last_sent = Instant::now();

    loop {
        let mut now = Instant::now();
        if now.duration_since(last_sent) >= ping_interval {
            let mut failed_port = 0;

            {
                let mut locked_peer_nodes = peer_nodes.lock().unwrap();

                for (_, peer) in locked_peer_nodes.iter_mut() {
                    if peer.state != NodeState::Fail {
                        if PRINT_PINGS { println!("sending to: {} {:?}", peer.port, peer.state) }
                        match peer.stream.try_clone() {
                            Ok(mut peer_stream) => {
                                peer_stream.set_read_timeout(Some(error_interval))?;
                                let mut reader = BufReader::new(peer_stream.try_clone()?);
                                peer_stream.write_all("ping\n".to_string().as_bytes())?;
                                now = Instant::now();
    
                                let mut line = String::new();
                                match reader.read_line(&mut line) {
                                    Ok(0) => {
                                        println!("Connection closed by peer");
                                        failed_port = peer.port.clone();
                                        break;
                                    }
                                    Ok(_) => {
                                        if PRINT_PINGS { println!("Received response: {}", line.trim()) }
                                    }
                                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                                        println!("Timeout: no response within {:?}", error_interval);
                                        failed_port = peer.port.clone();
                                        break;
                                    }
                                    Err(e) => {
                                        println!("Unexpected error: {}", e);
                                        failed_port = peer.port.clone();
                                        break;
                                    }
                                }
                            }
                            Err(_) => {
                                std::hint::spin_loop();
                            }
                        }
                    }
                }
            }

            if failed_port != 0 {
                let promote_replica: bool = detect_failed_node(&local_node, &peer_nodes, failed_port);
                if promote_replica {
                    initialize_replica_promotion(&local_node, &peer_nodes);
                }
            }

            

            last_sent = now;
        }
    }
}




fn detect_failed_node(local_node: &Arc<Mutex<LocalNode>>, peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>, inactive_port: usize) -> bool {
    let mut locked_peer_nodes = peer_nodes.lock().unwrap();
    let locked_local_node = local_node.lock().unwrap();
    let mut peer_state = NodeState::Fail;
    let mut promote_replica = false;
    
    let inactive_address = format!("127.0.0.1:{}", inactive_port);
    if let Some(inactive_node) = locked_peer_nodes.get_mut(&inactive_address) {
        match inactive_node.state {
            NodeState::Active => {
                inactive_node.state = NodeState::PFail;
                peer_state = NodeState::PFail;
            }
            NodeState::PFail => {
                inactive_node.state = NodeState::Fail;
                peer_state = NodeState::Fail;
                if inactive_node.role == NodeRole::Master && inactive_node.hash_range.0 == locked_local_node.hash_range.0  && inactive_node.hash_range.1 == locked_local_node.hash_range.1 {
                    promote_replica = true;
                    for replica in locked_local_node.replica_nodes.clone() {
                        let replica_address = format!("127.0.0.1:{}", replica);
                        if let Some(replica_node) = locked_peer_nodes.get_mut(&replica_address) {
                            // por las dudas verifico que no sea master y que este activo
                            if replica_node.state == NodeState::Active && replica_node.role != NodeRole::Master {
                               // si hay otra replica mas actualizada o si tenemos el mismo epoch pero la otra tiene una prioridad menor, no me vuelvo master
                                if (replica_node.epoch > locked_local_node.epoch )|| (replica_node.epoch == locked_local_node.epoch && replica_node.priority < locked_local_node.priority) {
                                    promote_replica = false;
                                }
                            }                        
                        }
                    }
                }
            }
            _ => {}
         }
    }

    for (_, peer) in locked_peer_nodes.iter_mut() {
        if peer.port != inactive_port {
            let message = format!("node_status {} {:?}\n", inactive_port, peer_state);
            let _ = peer.stream.write_all(message.as_bytes());
        }
    }

    promote_replica
}


fn set_failed_node(local_node: &Arc<Mutex<LocalNode>>, peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>, inactive_port: usize, inactive_state: NodeState) -> bool {
    let mut locked_peer_nodes = peer_nodes.lock().unwrap();
    let locked_local_node = local_node.lock().unwrap();
    let mut promote_replica = false;
    
    let inactive_address = format!("127.0.0.1:{}", inactive_port);
    if let Some(inactive_node) = locked_peer_nodes.get_mut(&inactive_address) {
        if inactive_state == NodeState::Fail {
            // lo marco como failed
            inactive_node.state = NodeState::Fail;

            // me fijo si tengo que promocionar
            if inactive_node.role == NodeRole::Master && inactive_node.hash_range.0 == locked_local_node.hash_range.0  && inactive_node.hash_range.1 == locked_local_node.hash_range.1 {
                promote_replica = true;
                for replica in locked_local_node.replica_nodes.clone() {
                    let replica_address = format!("127.0.0.1:{}", replica);
                    if let Some(replica_node) = locked_peer_nodes.get_mut(&replica_address) {
                        // por las dudas verifico que no sea master y que este activo
                        if replica_node.state == NodeState::Active && replica_node.role != NodeRole::Master {
                            // si hay otra replica mas actualizada o si tenemos el mismo epoch pero la otra tiene una prioridad menor, no me vuelvo master
                             if (replica_node.epoch > locked_local_node.epoch )|| (replica_node.epoch == locked_local_node.epoch && replica_node.priority < locked_local_node.priority) {
                                 promote_replica = false;
                             }
                         }
                    }
                }
            }
        } else {
            // me llego un pfail -> depende de si yo lo tengo como active o pfail
            match inactive_node.state {
                NodeState::Active => {
                    // lo marco como pfail
                    inactive_node.state = NodeState::PFail;
                }
                NodeState::PFail => {
                    // lo marco como failed
                    inactive_node.state = NodeState::Fail;

                    // me fijo si tengo que promocionar
                    if inactive_node.role == NodeRole::Master && inactive_node.hash_range.0 == locked_local_node.hash_range.0  && inactive_node.hash_range.1 == locked_local_node.hash_range.1 {
                        promote_replica = true;
                        for replica in locked_local_node.replica_nodes.clone() {
                            let replica_address = format!("127.0.0.1:{}", replica);
                            if let Some(replica_node) = locked_peer_nodes.get_mut(&replica_address) {
                                if replica_node.priority < locked_local_node.priority {
                                    promote_replica = false;
                                }
                            }
                        }
                    }

                    // yo detecte que hizo fail -> le aviso al resto
                    for (_, peer) in locked_peer_nodes.iter_mut() {
                        if peer.port != inactive_port {
                            let message = format!("node_status {} {:?}\n", inactive_port, NodeState::Fail);
                            let _ = peer.stream.write_all(message.as_bytes());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    promote_replica
}




fn initialize_replica_promotion(
    local_node: &Arc<Mutex<LocalNode>>,
    peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>
) {
    let (mut locked_local_node, locked_peer_nodes) = match (local_node.lock(), peer_nodes.lock()) {
        (Ok(local), Ok(peers)) => (local, peers),
        _ => {
            eprintln!("Error locking local_node or peer_nodes");
            return;
        }
    };

    locked_local_node.role = NodeRole::Master;
    locked_local_node.master_node = None;
    locked_local_node.priority = 0;

    let node_info_message = format!(
        "{:?} {} {:?} {} {} {}\n",
        RedisMessage::Node,
        locked_local_node.port,
        locked_local_node.role,
        locked_local_node.hash_range.0,
        locked_local_node.hash_range.1,
        locked_local_node.priority,
    );

    for (_, peer) in locked_peer_nodes.iter() {
        if peer.state == NodeState::Active {
            println!("sending promotion info to: {}", peer.port);
            match peer.stream.try_clone() {
                Ok(mut peer_stream) => {
                    if let Err(e) = peer_stream.write_all(node_info_message.as_bytes()) {
                        eprintln!("Error writing node_info_message: {}", e);
                    }
                }
                Err(_) => {
                    eprintln!("Error cloning stream");
                }
            }
        }
    }
}
