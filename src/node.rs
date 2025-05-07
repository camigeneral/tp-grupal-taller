use std::env::args;
use std::io::{self, BufRead, BufReader, Write};
use std::fs::{File, OpenOptions};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;
use std::str;
use std::io::Read;

static SERVER_ARGS: usize = 2;

 struct Client {
    // addr: String,
    stream: TcpStream
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

    // hardcodeado, por ahora
    let mut initial_docs: HashMap<String, Vec<String>> = HashMap::new();
    initial_docs.insert("doc1".to_string(), vec![]);
    initial_docs.insert("doc2".to_string(), vec![]);

    // guardo la informacion de los clientes
    let clients: Arc<Mutex<HashMap<String, Client>>> = Arc::new(Mutex::new(HashMap::new()));
    let clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>> = Arc::new(Mutex::new(initial_docs));

    let listener = TcpListener::bind(address)?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut client_stream) => {
                let client_addr = client_stream.peer_addr()?;
                println!("La socket addr del client: {}", client_addr);

                let cloned_stream = client_stream.try_clone()?;

                {
                    let client_addr = cloned_stream.peer_addr()?;
                    let client_key = client_addr.to_string();
                    let client = Client {
                        // addr: client_addr.to_string(),
                        stream: cloned_stream
                    };
                    let mut lock_clients = clients.lock().unwrap();
                    lock_clients.insert(client_key, client);
                }   
                // bloque inseguro?

                let cloned_clients = Arc::clone(&clients);
                let cloned_clients_on_docs = Arc::clone(&clients_on_docs);
                let cloned_docs = Arc::clone(&shared_docs);

                thread::spawn(move || {
                    match handle_client(&mut client_stream, cloned_clients, cloned_clients_on_docs, cloned_docs) {
                        Ok(_) => {
                            println!("El cliente {} se ha desconectado.", client_addr);
                        }
                        Err(e) => {
                            eprintln!("Error en la conexión con {}: {}", client_addr, e);
                        }
                    }
                });
            }
            Err(e) => {
                eprintln!("Error al aceptar conexión: {}", e);
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
) -> std::io::Result<()>{
    let client_addr = stream.peer_addr()?;
    let mut reader = BufReader::new(stream.try_clone()?);

    loop {
        let command = match parse_resp_command(&mut reader) {
            Ok(cmd) => cmd,
            Err(_) => {
                write_resp_error(&stream, "Invalid command")?;
                break;
            }
        };

        if command.is_empty() {
            continue;
        }

        let cmd = command[0].to_lowercase();

        match cmd.as_str() {
            "get" => {
                if command.len() != 2 {
                    write_resp_error(&stream, "Wrong number of arguments for GET")?;
                    continue;
                }

                let key = &command[1];
                let docs = docs.lock().unwrap();
                match docs.get(key) {
                    Some(value) => write_resp_string(&stream, &value.join("\n"))?,

                    None => write_resp_null(&stream)?,
                }
            }

            "subscribe" => {
                if command.len() != 2 {
                    write_resp_error(&stream, "Usage: SUBSCRIBE <document>")?;
                    continue;
                }

                let doc = &command[1];
                let mut map = clients_on_docs.lock().unwrap();
                if let Some(list) = map.get_mut(doc) {
                    list.push(client_addr.to_string());
                    write_resp_string(&stream, &format!("Subscribed to {}", doc))?;
                } else {
                    write_resp_error(&stream, "Document not found")?;
                }
            }

            "unsubscribe" => {
                if command.len() != 2 {
                    write_resp_error(&stream, "Usage: UNSUBSCRIBE <document>")?;
                    continue;
                }

                let doc = &command[1];
                let mut map = clients_on_docs.lock().unwrap();
                if let Some(list) = map.get_mut(doc) {
                    list.retain(|x| x != &client_addr.to_string());
                    write_resp_string(&stream, &format!("Unsubscribed from {}", doc))?;
                } else {
                    write_resp_error(&stream, "Document not found")?;
                }
            }

            "append" => {
                if command.len() < 3 {
                    write_resp_error(stream, "Usage: APPEND <document> <text...>")?;
                    continue;
                }

                let doc = command[1].clone();
                let content = command[2..].join(" ");
                let mut docs_lock = docs.lock().unwrap();
                let entry = docs_lock.entry(doc.clone()).or_insert_with(Vec::new);
                entry.push(content.clone());
                
                let line_number = entry.len();
                write_resp_string(stream, &line_number.to_string())?;
                
                drop(docs_lock);
                
                let notification = format!("New content in {}: {}", doc, content);
                println!("Publishing to subscribers of {}: {}", doc, notification);
                publish(clients.clone(), clients_on_docs.clone(), notification, doc)?;
                
                let _ = write_to_file(docs.clone());
            }
            _ => {
                write_resp_error(&stream, "Unknown command")?;
            }
        }

        let _ = write_to_file(docs.clone());
    }

    Ok(())
}

fn publish(clients: Arc<Mutex<HashMap<String, Client>>>, clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>, message: String, doc: String) -> std::io::Result<()> {
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

pub fn write_to_file(docs:  Arc<Mutex<HashMap<String, Vec<String>>>>) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open("docs.txt")?;

    let locked_docs: std::sync::MutexGuard<'_, HashMap<String, Vec<String>>> = docs.lock().unwrap();
    let documents: Vec<&String> = locked_docs.keys().collect();
    for document in documents {
        let mut base_string = format!("{}",document);
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

pub fn get_file_content(file_path: &String) -> Result<Arc<Mutex<HashMap<String, Vec<String>>>>, String> {
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

fn parse_resp_command(reader: &mut BufReader<TcpStream>) -> std::io::Result<Vec<String>> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if !line.starts_with('*') {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Not a RESP array"));
    }

    let num_elements: usize = line[1..].trim().parse().unwrap_or(0);
    let mut result = Vec::with_capacity(num_elements);

    for _ in 0..num_elements {
        line.clear();
        reader.read_line(&mut line)?; // leer $n
        if !line.starts_with('$') {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Expected bulk string"));
        }

        let length: usize = line[1..].trim().parse().unwrap_or(0);
        let mut buffer = vec![0u8; length + 2]; // +2 for \r\n
        reader.read_exact(&mut buffer)?;
        result.push(String::from_utf8_lossy(&buffer[..length]).to_string());
    }

    Ok(result)
}

fn write_resp_string(mut stream: &TcpStream, value: &str) -> std::io::Result<()> {
    let response = format!("${}\r\n{}\r\n", value.len(), value);
    stream.write_all(response.as_bytes())
}

fn write_resp_null(mut stream: &TcpStream) -> std::io::Result<()> {
    stream.write_all(b"$-1\r\n")
}

fn write_resp_error(mut stream: &TcpStream, msg: &str) -> std::io::Result<()> {
    stream.write_all(format!("-ERR {}\r\n", msg).as_bytes())
}
