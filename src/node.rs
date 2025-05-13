use std::collections::HashMap;
use std::env::args;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::str;
use std::sync::{Arc, Mutex};
use std::thread;

mod parse;
use parse::{parse_command, write_response, CommandRequest, CommandResponse, ValueType};

static SERVER_ARGS: usize = 2;

struct Client {
    stream: TcpStream,
}

fn main() -> Result<(), ()> {
    let argv = args().collect::<Vec<String>>();
    if argv.len() != SERVER_ARGS {
        println!("Cantidad de argumentos inválido");
        let app_name = &argv[0];
        println!("Usage:\n{:?} <puerto>", app_name);
        return Err(());
    }

    let address = "127.0.0.1:".to_owned() + &argv[1];
    server_run(&address).unwrap();
    Ok(())
}

fn server_run(address: &str) -> std::io::Result<()> {
    let file_path = "docs.txt".to_string();
    let shared_docs = match get_file_content(&file_path) {
        Ok(docs) => docs,
        Err(_) => {
            let mut new_docs: HashMap<String, Vec<String>> = HashMap::new();
            new_docs.insert("doc1".to_string(), vec![]);
            new_docs.insert("doc2".to_string(), vec![]);
            Arc::new(Mutex::new(new_docs))
        }
    };

    let mut initial_docs: HashMap<String, Vec<String>> = HashMap::new();
    initial_docs.insert("doc1".to_string(), vec![]);
    initial_docs.insert("doc2".to_string(), vec![]);

    let clients: Arc<Mutex<HashMap<String, Client>>> = Arc::new(Mutex::new(HashMap::new()));
    let clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>> =
        Arc::new(Mutex::new(initial_docs));

    let listener = TcpListener::bind(address)?;
    println!("Server listening on {}", address);

    for stream in listener.incoming() {
        match stream {
            Ok(mut client_stream) => {
                let client_addr = client_stream.peer_addr()?;
                println!("New client connected: {}", client_addr);

                let cloned_stream = client_stream.try_clone()?;

                {
                    let client_addr = cloned_stream.peer_addr()?;
                    let client_key = client_addr.to_string();
                    let client = Client {
                        stream: cloned_stream,
                    };
                    let mut lock_clients = clients.lock().unwrap();
                    lock_clients.insert(client_key, client);
                }

                let cloned_clients = Arc::clone(&clients);
                let cloned_clients_on_docs = Arc::clone(&clients_on_docs);
                let cloned_docs = Arc::clone(&shared_docs);
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
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(
    stream: &mut TcpStream,
    clients: Arc<Mutex<HashMap<String, Client>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    loop {
        let command_request = match parse_command(&mut reader) {
            Ok(req) => req,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                }
                println!("Error parsing command: {}", e);
                write_response(
                    stream,
                    &CommandResponse::Error("Invalid command".to_string()),
                )?;
                continue;
            }
        };

        println!("Received command: {:?}", command_request);

        let response = execute_command(
            command_request,
            docs.clone(),
            clients.clone(),
            clients_on_docs.clone(),
            client_addr.clone(),
        );

        if let Err(e) = write_response(stream, &response) {
            println!("Error writing response: {}", e);
            break;
        }
    }

    cleanup_client(&client_addr, &clients, &clients_on_docs);
    Ok(())
}

fn cleanup_client(
    client_addr: &str,
    clients: &Arc<Mutex<HashMap<String, Client>>>,
    clients_on_docs: &Arc<Mutex<HashMap<String, Vec<String>>>>,
) {
    clients.lock().unwrap().remove(client_addr);

    let mut docs_lock = clients_on_docs.lock().unwrap();
    for subscribers in docs_lock.values_mut() {
        subscribers.retain(|addr| addr != client_addr);
    }
}

fn execute_command(
    request: CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    clients: Arc<Mutex<HashMap<String, Client>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> CommandResponse {
    match request.command.as_str() {
        "get" => handle_get(&request, docs),
        "set" => handle_set(&request, docs, clients, clients_on_docs),
        "subscribe" => handle_subscribe(&request, clients_on_docs, client_addr),
        "unsubscribe" => handle_unsubscribe(&request, clients_on_docs, client_addr),
        "append" => handle_append(&request, docs, clients, clients_on_docs),
        "scard" => handle_scard(&request, clients_on_docs),
        "smembers" => handle_smembers(&request, clients_on_docs),
        "sscan" => handle_sscan(&request, clients_on_docs),
        _ => CommandResponse::Error("Unknown command".to_string()),
    }
}

fn handle_get(
    request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> CommandResponse {
    let key = match &request.key {
        Some(k) => k,
        None => return CommandResponse::Error("Wrong number of arguments for GET".to_string()),
    };

    let docs = docs.lock().unwrap();
    match docs.get(key) {
        Some(value) => CommandResponse::String(value.join("\n")),
        None => CommandResponse::Null,
    }
}

fn handle_set(
    request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    clients: Arc<Mutex<HashMap<String, Client>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> CommandResponse {
    let doc_name = match &request.key {
        Some(k) => k.clone(),
        None => return CommandResponse::Error("Wrong number of arguments for SET".to_string()),
    };

    if request.arguments.is_empty() {
        return CommandResponse::Error("Wrong number of arguments for SET".to_string());
    }

    let content = extract_string_arguments(&request.arguments);

    {
        let mut docs_lock = docs.lock().unwrap();
        docs_lock.insert(doc_name.clone(), vec![content.clone()]);

        let mut clients_on_docs_lock = clients_on_docs.lock().unwrap();
        if !clients_on_docs_lock.contains_key(&doc_name) {
            clients_on_docs_lock.insert(doc_name.clone(), Vec::new());
        }
    }

    let notification = format!("Document {} was replaced with: {}", doc_name, content);
    println!(
        "Publishing to subscribers of {}: {}",
        doc_name, notification
    );

    if let Err(e) = publish(clients, clients_on_docs, notification, doc_name.clone()) {
        eprintln!("Error publishing update: {}", e);
    }

    if let Err(e) = write_to_file(docs.clone()) {
        eprintln!("Error writing to file: {}", e);
    }

    CommandResponse::Ok
}

fn handle_subscribe(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> CommandResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => return CommandResponse::Error("Usage: SUBSCRIBE <document>".to_string()),
    };

    let mut map = clients_on_docs.lock().unwrap();
    if let Some(list) = map.get_mut(doc) {
        list.push(client_addr);
        CommandResponse::String(format!("Subscribed to {}", doc))
    } else {
        CommandResponse::Error("Document not found".to_string())
    }
}

fn handle_unsubscribe(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> CommandResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => return CommandResponse::Error("Usage: UNSUBSCRIBE <document>".to_string()),
    };

    let mut map = clients_on_docs.lock().unwrap();
    if let Some(list) = map.get_mut(doc) {
        list.retain(|x| x != &client_addr);
        CommandResponse::String(format!("Unsubscribed from {}", doc))
    } else {
        CommandResponse::Error("Document not found".to_string())
    }
}

fn handle_append(
    request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    clients: Arc<Mutex<HashMap<String, Client>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> CommandResponse {
    let doc = match &request.key {
        Some(k) => k.clone(),
        None => return CommandResponse::Error("Usage: APPEND <document> <text...>".to_string()),
    };

    if request.arguments.is_empty() {
        return CommandResponse::Error("Usage: APPEND <document> <text...>".to_string());
    }

    let content = extract_string_arguments(&request.arguments);
    let line_number;

    {
        let mut docs_lock = docs.lock().unwrap();
        let entry = docs_lock.entry(doc.clone()).or_default();
        entry.push(content.clone());
        line_number = entry.len();
    }

    let notification = format!("New content in {}: {}", doc, content);
    println!("Publishing to subscribers of {}: {}", doc, notification);

    if let Err(e) = publish(clients, clients_on_docs, notification, doc) {
        eprintln!("Error publishing update: {}", e);
    }

    if let Err(e) = write_to_file(docs.clone()) {
        eprintln!("Error writing to file: {}", e);
    }

    CommandResponse::Integer(line_number as i64)
}

fn handle_scard(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> CommandResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => return CommandResponse::Error("Usage: SCARD <document>".to_string()),
    };

    let lock_clients_on_docs = clients_on_docs.lock().unwrap();
    if let Some(subscribers) = lock_clients_on_docs.get(doc) {
        CommandResponse::String(format!(
            "Number of subscribers in channel {}: {}",
            doc,
            subscribers.len()
        ))
    } else {
        CommandResponse::Error("Document not found".to_string())
    }
}

fn handle_smembers(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> CommandResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => return CommandResponse::Error("Usage: SMEMBERS <document>".to_string()),
    };

    let lock_clients_on_docs = clients_on_docs.lock().unwrap();
    if let Some(subscribers) = lock_clients_on_docs.get(doc) {
        if subscribers.is_empty() {
            return CommandResponse::String(format!("No subscribers in document {}", doc));
        }

        // Opción 1: Devolver como una cadena con formato
        let mut response = format!("Subscribers in document {}:\n", doc);
        for subscriber in subscribers {
            response.push_str(&format!("{}\n", subscriber));
        }
        CommandResponse::String(response)
    } else {
        CommandResponse::Error("Document not found".to_string())
    }
}

fn handle_sscan(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> CommandResponse {
    // Obtener el documento (key) del request
    let doc = match &request.key {
        Some(k) => k,
        None => return CommandResponse::Error("Usage: SSCAN <document> [pattern]".to_string()),
    };

    // Extraer el patrón del primer argumento (si existe)
    let pattern = if !request.arguments.is_empty() {
        match &request.arguments[0] {
            ValueType::String(s) => s,
            ValueType::Integer(i) => {
                return CommandResponse::Error(format!(
                    "Expected string pattern, got integer: {}",
                    i
                ))
            }
            // Agrega otros casos según los tipos que pueda tener ValueType
            _ => return CommandResponse::Error("Pattern must be a string".to_string()),
        }
    } else {
        "" // Si no hay patrón, usamos cadena vacía (coincide con todo)
    };

    let lock_clients_on_docs = clients_on_docs.lock().unwrap();
    if let Some(subscribers) = lock_clients_on_docs.get(doc) {
        // Filtrar los suscriptores que coinciden con el patrón
        let matching_subscribers: Vec<&String> =
            subscribers.iter().filter(|s| s.contains(pattern)).collect();

        if matching_subscribers.is_empty() {
            return CommandResponse::String(format!(
                "No subscribers matching '{}' in document {}",
                pattern, doc
            ));
        }

        // Construir la respuesta con todos los suscriptores que coinciden
        let mut response = format!("Subscribers in {} matching '{}':\n", doc, pattern);
        for subscriber in matching_subscribers {
            response.push_str(&format!("{}\n", subscriber));
        }

        CommandResponse::String(response)
    } else {
        CommandResponse::Error("Document not found".to_string())
    }
}

fn extract_string_arguments(arguments: &[ValueType]) -> String {
    arguments
        .iter()
        .filter_map(|arg| {
            if let ValueType::String(s) = arg {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn publish(
    clients: Arc<Mutex<HashMap<String, Client>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    message: String,
    doc: String,
) -> std::io::Result<()> {
    let mut lock_clients = clients.lock().unwrap();
    let mut lock_clients_on_docs = clients_on_docs.lock().unwrap();

    if let Some(clients_on_doc) = lock_clients_on_docs.get_mut(&doc) {
        for subscriber_addr in clients_on_doc {
            if let Some(client) = lock_clients.get_mut(subscriber_addr) {
                writeln!(client.stream, "{}", message.trim())?;
            } else {
                println!("Cliente no encontrado");
            }
        }
    } else {
        println!("Documento no encontrado");
    }

    Ok(())
}

pub fn write_to_file(docs: Arc<Mutex<HashMap<String, Vec<String>>>>) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open("docs.txt")?;

    let locked_docs: std::sync::MutexGuard<'_, HashMap<String, Vec<String>>> = docs.lock().unwrap();
    let documents: Vec<&String> = locked_docs.keys().collect();
    for document in documents {
        let mut base_string = document.to_string();
        base_string.push_str("/++/");
        let messages = locked_docs.get(document).unwrap();
        for message in messages {
            base_string.push_str(message);
            base_string.push_str("/--/");
        }
        writeln!(file, "{}", base_string)?;
    }

    Ok(())
}

pub fn get_file_content(
    file_path: &String,
) -> Result<Arc<Mutex<HashMap<String, Vec<String>>>>, String> {
    let file = File::open(file_path).map_err(|_| "file-not-found".to_string())?;
    let reader = BufReader::new(file);
    let lines = reader.lines();

    let mut docs: HashMap<String, Vec<String>> = HashMap::new();

    for line in lines {
        match line {
            Ok(read_line) => {
                let parts: Vec<&str> = read_line.split("/++/").collect();
                if parts.len() != 2 {
                    continue;
                }

                let doc_name = parts[0].to_string();
                let messages_str = parts[1];

                let messages: Vec<String> = messages_str
                    .split("/--/")
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();

                docs.insert(doc_name, messages);
            }
            Err(_) => return Err("unable-to-read-file".to_string()),
        }
    }

    Ok(Arc::new(Mutex::new(docs)))
}
