use std::path::Path;
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::io::Cursor;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::collections::HashMap;
use std::collections::HashSet;
use std::time::{Instant, Duration};
use commands::redis;
use local_node::{LocalNode, NodeRole, NodeState};
use peer_node;
use utils;


#[derive(Debug)]
pub enum RedisMessage {
    Node,
}


/// Intenta establecer una primera conexion con los otros nodos del servidor
///
/// # Errores
/// Retorna un error si el puerto no corresponde a uno definido en los archivos de configuracion.
pub fn start_node_connection(
    port: usize,
    node_address: String,
    peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_documents: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> Result<Arc<Mutex<LocalNode>>, std::io::Error> {
    let cloned_nodes = Arc::clone(peer_nodes);

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

    let local_node = LocalNode::new_from_config(config_path)?;
    let mutex_node = Arc::new(Mutex::new(local_node));
    let cloned_mutex_node = Arc::clone(&mutex_node);
    let cloned_document_subscribers = Arc::clone(&document_subscribers);
    let cloned_shared_documents = Arc::clone(&shared_documents);
    let cloned_shared_sets = Arc::clone(&shared_sets);
    let node_ports = read_node_ports(config_path)?;

    thread::spawn(
        move || match connect_nodes(&node_address, cloned_nodes, cloned_mutex_node, cloned_document_subscribers, cloned_shared_documents, cloned_shared_sets) {
            Ok(_) => {}
            Err(_e) => {}
        },

    );

    let cloned_local_node_for_ping_pong = Arc::clone(&mutex_node);
    let cloned_peer_nodes = Arc::clone(peer_nodes);

    thread::spawn(
        move || match ping_to_master(cloned_local_node_for_ping_pong, cloned_peer_nodes) {
            Ok(_) => {}
            Err(_e) => {}
        },
    );

    {
        let mut lock_peer_nodes: std::sync::MutexGuard<'_, HashMap<String, peer_node::PeerNode>> =
            peer_nodes.lock().unwrap();
        let locked_mutex_node = mutex_node.lock().unwrap();

        for connection_port in node_ports {
            if connection_port != locked_mutex_node.port {
                let node_address_to_connect = format!("127.0.0.1:{}", connection_port);
                let peer_addr = format!("127.0.0.1:{}", connection_port);
                match TcpStream::connect(node_address_to_connect) {
                    Ok(stream) => {
                        let mut cloned_stream = stream.try_clone()?;

                        let message = format!(
                            "{:?} {} {:?} {} {}\n",
                            RedisMessage::Node,
                            locked_mutex_node.port,
                            locked_mutex_node.role,
                            locked_mutex_node.hash_range.0,
                            locked_mutex_node.hash_range.1
                        );

                        cloned_stream.write_all(message.as_bytes())?;

                        lock_peer_nodes.insert(
                            peer_addr,
                            peer_node::PeerNode::new(
                                stream,
                                connection_port,
                                NodeRole::Unknown,
                                (0, 16383),
                                NodeState::Active,
                            ),
                        );

                        ()
                    }
                    Err(_) => {}
                };
            }
        }
    }

    Ok(mutex_node)
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
    shared_documents: Arc<Mutex<HashMap<String, Vec<String>>>>,
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
                    match handle_node(&mut node_stream, cloned_nodes, &cloned_local_node, cloned_document_subscribers, cloned_shared_documents, cloned_shared_sets) {
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
    shared_documents: Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_sets: Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> std::io::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);
    let mut saving_command = false;
    let mut command_string = String::new();

    let mut serialized_hashmap: Vec<String> = Vec::new();
    let mut serialized_vec: Vec<String> = Vec::new();

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
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid port")
                })?;
                let node_address = format!("127.0.0.1:{}", node_listening_port);

                let node_role = match input[2].trim().to_lowercase().as_str() {
                    "master" => NodeRole::Master,
                    "replica" => NodeRole::Replica,
                    _ => NodeRole::Unknown,
                };

                let cloned_role = match input[2].trim().to_lowercase().as_str() {
                    "master" => NodeRole::Master,
                    "replica" => NodeRole::Replica,
                    _ => NodeRole::Unknown,
                };

                let hash_range_start = &input[3].trim().parse::<usize>().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid start range")
                })?;
                let hash_range_end = &input[4].trim().parse::<usize>().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid end range")
                })?;

                {
                    let mut lock_nodes = nodes.lock().unwrap();
                    let mut local_node_locked = local_node.lock().unwrap();
                    // no lo conozco -> creo el stream y me guardo todo, y le mando mi info
                    if !lock_nodes.contains_key(&node_address) {
                        let node_address_to_connect = format!("127.0.0.1:{}", node_listening_port);
                        let new_stream = TcpStream::connect(node_address_to_connect)?;
                        let mut stream_to_respond = new_stream.try_clone()?;

                        let node_client = peer_node::PeerNode::new(
                            new_stream,
                            *parsed_port,
                            node_role,
                            (*hash_range_start, *hash_range_end),
                            NodeState::Active,
                        );
                        // me fijo si es mi master/replica
                        if *hash_range_start == local_node_locked.hash_range.0 {
                            if cloned_role != local_node_locked.role {
                                // estoy hablando con un nodo de otro tipo -> si soy replica, mi master, si soy master, mis replicas
                                if local_node_locked.role == NodeRole::Master {
                                    // estoy hablando con mi replica
                                    local_node_locked.replica_nodes.push(*parsed_port);
                                } else {
                                    // estoy hablando con mi master
                                    local_node_locked.master_node = Some(*parsed_port);
                                    let message = format!("sync_request {}\n", local_node_locked.port);
                                    stream_to_respond.write_all(message.as_bytes())?;
                                }
                            } else {
                               // soy una replica, hablando con la ottra replica de mi master
                               local_node_locked.replica_nodes.push(*parsed_port);
                            }
                        }


                        let node_address_to_connect = format!("127.0.0.1:{}", node_listening_port);

                        lock_nodes.insert(node_address_to_connect.to_string(), node_client);

                        let message = format!(
                            "{:?} {} {:?} {} {}\n",
                            RedisMessage::Node,
                            local_node_locked.port,
                            local_node_locked.role,
                            local_node_locked.hash_range.0,
                            local_node_locked.hash_range.1
                        );

                        stream_to_respond.write_all(message.as_bytes())?;
                        println!("03");
                    }
                    // si lo conozco, actualizo todo menos el stream
                    else {
                        let peer_node_to_update = lock_nodes.get_mut(&node_address).unwrap();
                        peer_node_to_update.role = node_role;
                        peer_node_to_update.hash_range = (*hash_range_start, *hash_range_end);

                        if peer_node_to_update.hash_range.0 == local_node_locked.hash_range.0 {
                            if cloned_role != local_node_locked.role {
                                // estoy hablando con un nodo de otro tipo -> si soy replica, mi master, si soy master, mis replicas
                                if local_node_locked.role == NodeRole::Master {
                                    // estoy hablando con mi replica
                                    local_node_locked.replica_nodes.push(*parsed_port);
                                } else {
                                    // estoy hablando con mi master
                                    local_node_locked.master_node = Some(*parsed_port);
                                    let message = format!("sync_request {}\n", local_node_locked.port);
                                    peer_node_to_update.stream.write_all(message.as_bytes())?;
                                }
                            } else {
                               // soy una replica, hablando con la ottra replica de mi master
                               local_node_locked.replica_nodes.push(*parsed_port);
                            }

                        }

                    }
                }
            }
            "sync_request" => {
                handle_replica_sync(&input[1], &nodes, &shared_sets, &shared_documents)?;
            }
            "serialize_hashmap" => {
                serialized_hashmap.push(input[1].clone());
            }
            "serialize_vec" => {
                serialized_vec.push(input[1].clone());
            }
            "end_serialize_hashmap" => {
                deserialize_hashset_hashmap(&serialized_hashmap, &shared_sets);
                serialized_hashmap = Vec::new();
            }
            "end_serialize_vec" => {
                deserialize_vec_hashmap(&serialized_vec, &shared_documents);
                serialized_vec = Vec::new();
            } 
            "start_replica_command" => {
                saving_command = true;
            }
            "end_replica_command" => {
                saving_command = false;
                let cursor = Cursor::new(command_string.clone());
                let mut reader_for_command = BufReader::new(cursor);
                let command_request: utils::redis_parser::CommandRequest =
                match utils::redis_parser::parse_replica_command(&mut reader_for_command) {
                    Ok(req) => req,
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::UnexpectedEof {
                            break;
                        }
                        println!("Error al parsear comando: {}", e);
                        utils::redis_parser::write_response(
                            stream,
                            &utils::redis_parser::CommandResponse::Error(
                                "Comando inválido".to_string(),
                            ),
                        )?;
                        continue;
                    }
                };
                println!("Replica command request {:?}", command_request);

                let redis_response = redis::execute_replica_command(
                    command_request,
                    shared_documents.clone(),
                    document_subscribers.clone(),
                    shared_sets.clone(),
                );

                println!("Replica redis response {:?}", redis_response.response);

                command_string = "".to_string();
            }
            "ping" => {
                writeln!(stream, "pong")?;
            }
            "confirm_master_down" => {
                match confirm_master_state(local_node, &nodes) {
                    Ok(master_state) => {
                        println!("master state: {:?}", master_state);
                        if master_state == NodeState::Inactive {
                            let locked_nodes = nodes.lock().unwrap();
                            let hash_range;
                            {
                                let locked_local_node = local_node.lock().unwrap();
                                if locked_local_node.role != NodeRole::Replica {
                                    println!("Master node cannot initiate replica promotion");
                                }

                                hash_range = locked_local_node.hash_range;
                            }

                            for (_, peer) in locked_nodes.iter() {
                                if peer.role == NodeRole::Replica && peer.hash_range == hash_range {
                                    match peer.stream.try_clone() {
                                        Ok(mut peer_stream) => {
                                            let message = "initialize_replica_promotion\n";
                                            let _ = peer_stream.write_all(message.as_bytes());
                                        }
                                        Err(_) => {
                                            eprintln!("Error cloning stream");
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => eprintln!("err"),
                };
            }
            "initialize_replica_promotion" => { 
                {
                    // actualizo mi estado
                    let mut locked_local_node = local_node.lock().unwrap();
                    let inactive_port = locked_local_node.master_node.unwrap().clone();
                    locked_local_node.role = NodeRole::Master;
                    locked_local_node.master_node = None;
                

                     // le aviso a mis peers
                    let locked_peer_nodes = nodes.lock().unwrap();

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
                        match peer.stream.try_clone() {
                            Ok(mut peer_stream) => {
                                let _ = peer_stream.write_all(node_info_message.as_bytes());
                                let _ = peer_stream.write_all(inactive_node_message.as_bytes());
                            }
                            Err(_) => {
                                eprintln!("Error cloning stream");
                            }
                        }
                        
                    }
                }

            }
            "inactive_node" => {
                let inactive_node = &input[1];
                let inactive_node_addr = format!("127.0.0.1:{}", inactive_node);

                {
                    let mut locked_peer_nodes = nodes.lock().unwrap();
                    if let Some(peer_node) = locked_peer_nodes.get_mut(&inactive_node_addr) {
                        peer_node.state = NodeState::Inactive;
                    }
                }
            }
            _ => {
                if saving_command {
                    let formated = format!("{}\r\n", command);
                    command_string.push_str(&formated);
                } else {
                    writeln!(stream, "Comando no reconocido")?;
                }
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


pub fn broadcast_to_replicas(
    local_node: &Arc<Mutex<LocalNode>>,
    peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
    unparsed_command: String
) -> std::io::Result<()> {
    
    let locked_local_node = local_node.lock().unwrap();
    let mut locked_peer_nodes = peer_nodes.lock().unwrap();
    let replicas = &locked_local_node.replica_nodes;
    println!("initial command {}", unparsed_command);

    // to do: guardarme el stream en localnode
    for replica in replicas {
        let key = format!("127.0.0.1:{}", replica);
        if let Some(peer_node) = locked_peer_nodes.get_mut(&key) {
            let mut stream = &peer_node.stream;
            let message = format!(
                "{}",
                unparsed_command
            );
            stream.write_all("start_replica_command\n".to_string().as_bytes())?;
            stream.write_all(message.as_bytes())?;
            stream.write_all("end_replica_command\n".to_string().as_bytes())?;
        }
    }

    Ok(())
}


fn handle_replica_sync(replica_port: &String, peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>, shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>, shared_documents: &Arc<Mutex<HashMap<String, Vec<String>>>>) -> std::io::Result<()> {
    let replica_addr = format!("127.0.0.1:{}", replica_port);
    let cloned_sets;
    let cloned_shared_documents;

    {
        let locked_shared_sets = shared_sets.lock().unwrap();
        cloned_sets = locked_shared_sets.clone();
    }
    {
        let locked_shared_documents = shared_documents.lock().unwrap();
        cloned_shared_documents = locked_shared_documents.clone();
    }
    {
        let mut locked_peer_nodes = peer_nodes.lock().unwrap();
        if let Some(peer_node) = locked_peer_nodes.get_mut(&replica_addr) {
            match peer_node.stream.try_clone() {
                Ok(peer_stream) => {
                    let _ = serialize_hashset_hashmap(&cloned_sets, peer_stream.try_clone()?);
                    let _ = serialize_vec_hashmap(&cloned_shared_documents, peer_stream.try_clone()?);
                }
                Err(_) => {
                    eprintln!("Error cloning stream");
                }
            }
        }
    }

    


    Ok(())
}


fn serialize_vec_hashmap(map: &HashMap<String, Vec<String>>, mut stream: TcpStream) -> std::io::Result<()> {
    for (key, values) in map {
        let line = format!("serialize_vec {}:{}\n", key, values.join(","));
        stream.write_all(line.as_bytes())?;
    }
    stream.write_all(b"end_serialize_vec\n")?;
    Ok(())
}


fn serialize_hashset_hashmap(map: &HashMap<String, HashSet<String>>, mut stream: TcpStream) -> std::io::Result<()> {
    for (key, set) in map {
        let values: Vec<String> = set.iter().cloned().collect();
        let line = format!("serialize_hashmap {}:{}\n", key, values.join(","));
        stream.write_all(line.as_bytes())?;
    }
    stream.write_all(b"end_serialize_hashmap\n")?;
    Ok(())
}


fn deserialize_hashset_hashmap(lines: &Vec<String>, shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>) {
    {
        let mut locked_sets = shared_sets.lock().unwrap();

        for line in lines {
            if let Some((key, values_str)) = line.split_once(':') {
                let values: HashSet<String> = values_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                locked_sets.insert(key.to_string(), values);
            }
        }
    }
}


fn deserialize_vec_hashmap(lines: &Vec<String>, shared_documents: &Arc<Mutex<HashMap<String, Vec<String>>>>){
    {
        let mut locked_documents = shared_documents.lock().unwrap();

        for line in lines {
            if let Some((key, values_str)) = line.split_once(':') {
                let values: Vec<String> = values_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                locked_documents.insert(key.to_string(), values);
            }
        }
    }
}


fn ping_to_master(local_node: Arc<Mutex<LocalNode>>, peer_nodes: Arc<Mutex<HashMap<String, peer_node::PeerNode>>>)-> std::io::Result<()> {
    let ping_interval = Duration::from_secs(3);
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
                            // todo ok
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
                },
                None => {
                    std::hint::spin_loop();
                }
            }
        }
        std::hint::spin_loop();
    }
}


fn request_master_state_confirmation(local_node: &Arc<Mutex<LocalNode>>, peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>)  {
    let master_port_option;
    let hash_range;
    {
        let locked_local_node = local_node.lock().unwrap();
        if locked_local_node.role != NodeRole::Replica {
            println!("Master node cannot initiate replica promotion");
            return;
        }

        master_port_option = locked_local_node.master_node;
        hash_range = locked_local_node.hash_range;
    }

    let master_port = match master_port_option {
        Some(p) => p,
        None => {
            println!("No master node");
            return;
        }
    };

    let mut locked_peer_nodes = peer_nodes.lock().unwrap();

    // marco el master como inactivo
    for peer in locked_peer_nodes.values_mut() {
        if peer.port == master_port {
            peer.state = NodeState::Inactive;
        }
    }

    // hablo con la otra replica
    for (_, peer) in locked_peer_nodes.iter() {
        if peer.role == NodeRole::Replica && peer.hash_range == hash_range && peer.port != master_port {
            let message = format!("confirm_master_down {}\n", master_port);
            match peer.stream.try_clone() {
                Ok(mut peer_stream) => {
                    let _ = peer_stream.write_all(message.as_bytes());
                }
                Err(_) => {
                    eprintln!("Error cloning stream");
                }
            }
        }
    }

}


fn confirm_master_state(local_node: &Arc<Mutex<LocalNode>>, peer_nodes: &Arc<Mutex<HashMap<String, peer_node::PeerNode>>>) -> std::io::Result<NodeState> {
    let master_port_option;
    let mut master_state = NodeState::Active;
    {
        let locked_local_node = local_node.lock().unwrap();
        master_port_option = locked_local_node.master_node;
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

    let peer_nodes_locked = peer_nodes.lock().unwrap();
    for (_addr, peer_node) in peer_nodes_locked.iter() {
        if peer_node.port == master_port && peer_node.state == NodeState::Inactive {
            master_state = NodeState::Inactive;
        }
    }
    Ok(master_state)
}