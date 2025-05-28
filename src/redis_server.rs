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
mod parse;

static SERVER_ARGS: usize = 2;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argv = args().collect::<Vec<String>>();
    if argv.len() != SERVER_ARGS {
        eprintln!("Cantidad de argumentos inválida");
        let app_name = &argv[0];
        eprintln!("Usage:\n{} <puerto>", app_name);
        return Err("Cantidad de argumentos inválida".into());
    }

    let address = format!("127.0.0.1:{}", argv[1]);
    connect_clients(&address)?; // Propaga error si ocurre
    Ok(())
}


fn connect_clients(address: &str) -> std::io::Result<()> {
    let file_path = "docs.txt".to_string();
    let docs = match get_file_content(&file_path) {
        Ok(docs) => docs,
        Err(_) => {
            let new_docs: HashMap<String, Vec<String>> = HashMap::new();            
            new_docs
        }
    };

    let shared_docs = Arc::new(Mutex::new(docs.clone()));

    let mut initial_clients_on_doc = HashMap::new();

    for document in docs.keys() {
        initial_clients_on_doc.insert(document.to_string(), Vec::new());
    }

    // guardo la informacion de los clientes
    let clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>> =
        Arc::new(Mutex::new(initial_clients_on_doc));    
    let clients: Arc<Mutex<HashMap<String, client_info::Client>>> =
        Arc::new(Mutex::new(HashMap::new()));

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
                    let client = client_info::Client {
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
    clients: Arc<Mutex<HashMap<String, client_info::Client>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    loop {
        let command_request = match parse::parse_command(&mut reader) {
            Ok(req) => req,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                }
                println!("Error parsing command: {}", e);
                parse::write_response(
                    stream,
                    &parse::CommandResponse::Error("Invalid command".to_string()),
                )?;
                continue;
            }
        };

        println!("Received command: {:?}", command_request);

        let redis_response = redis::execute_command(
            command_request,
            docs.clone(),
            clients_on_docs.clone(),
            client_addr.clone(),
        );

        if redis_response.publish {
            if let Err(e) = publish(
                clients.clone(),
                clients_on_docs.clone(),
                redis_response.message,
                redis_response.doc,
            ) {
                eprintln!("Error publishing update: {}", e);
            }
        }

        let response = redis_response.response;
        if let Err(e) = parse::write_response(stream, &response) {
            println!("Error writing response: {}", e);
            break;
        }

        if let Err(e) = write_to_file(docs.clone()) {
            eprintln!("Error writing to file: {}", e);
        }
        let _ = write_to_file(docs.clone());
    }

    cleanup_client(&client_addr, &clients, &clients_on_docs);
    // to do: agregar comando para salir, esto nunca se ejecuta porque nunca termina el loop

    Ok(())
}

fn cleanup_client(
    client_addr: &str,
    clients: &Arc<Mutex<HashMap<String, client_info::Client>>>,
    clients_on_docs: &Arc<Mutex<HashMap<String, Vec<String>>>>,
) {
    clients.lock().unwrap().remove(client_addr);

    let mut docs_lock = clients_on_docs.lock().unwrap();
    for subscribers in docs_lock.values_mut() {
        subscribers.retain(|addr| addr != client_addr);
    }
}

pub fn publish(
    clients: Arc<Mutex<HashMap<String, client_info::Client>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    message: String,
    doc: String,
) -> std::io::Result<()> {
    let mut lock_clients = clients.lock().unwrap();
    let mut lock_clients_on_docs = clients_on_docs.lock().unwrap();

    if let Some(clients_on_current_doc) = lock_clients_on_docs.get_mut(&doc) {
        for subscriber_addr in clients_on_current_doc {
            if let Some(client) = lock_clients.get_mut(subscriber_addr) {
                writeln!(client.stream, "{}", message.trim())?;
            } else {
                println!("Cliente no encontrado: {}", subscriber_addr);
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

pub fn get_file_content(file_path: &String) -> Result<HashMap<String, Vec<String>>, String> {
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

    Ok(docs)
}
