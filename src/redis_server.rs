use std::collections::HashMap;
mod commands;
use commands::redis;
use std::env::args;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::str;
use std::sync::{Arc, Mutex};
use std::thread;
mod client_info;
mod utils;


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

    let bind_address = format!("127.0.0.1:{}", cli_args[1]);
    start_server(&bind_address)?;
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

    for incoming_connection in tcp_listener.incoming() {
        match incoming_connection {
            Ok(client_stream) => {
                handle_new_microservice_connection(
                    client_stream,
                    &active_clients,
                    &document_subscribers,
                    &shared_documents
                )?;
            }
            Err(e) => {
                eprintln!("Error al aceptar conexión: {}", e);
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
    documents: &HashMap<String, Vec<String>>
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
) -> std::io::Result<()> {
    let client_addr = client_stream.peer_addr()?;
    println!("Microservice conectado: {}", client_addr);

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

    thread::spawn(move || {
        match handle_client(
            &mut client_stream,
            cloned_clients,
            cloned_clients_on_docs,
            cloned_docs,
            client_addr_str,
        ) {
            Ok(_) => {
                println!("Client {} disconnected.", client_addr);
            }
            Err(e) => {
                eprintln!("Error in connection with {}: {}", client_addr, e);
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
                continue;
            }
        };

        println!("Comando recibido: {:?}", command_request);

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
            break;
        }

        if let Err(e) = persist_documents(shared_documents.clone()) {
            eprintln!("Error al persistir documentos: {}", e);
        }
    }

    cleanup_client_resources(&client_id, &active_clients, &document_subscribers);
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
