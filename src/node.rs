use std::env::args;
use std::io::{self, BufRead, BufReader, Write};
use std::fs::{File, OpenOptions};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;

mod list_commands;

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
    let docs = match get_file_content(&file_path) {
        Ok(docs) => docs,
        Err(_) => {
            let mut new_docs: HashMap<String, Vec<String>> = HashMap::new();
            new_docs.insert("doc1".to_string(), vec![]);
            new_docs.insert("doc2".to_string(), vec![]);
            new_docs
        }
        
    };

    let shared_docs = Arc::new(Mutex::new(docs.clone()));

    // guardo la informacion de los clientes
    let clients: Arc<Mutex<HashMap<String, Client>>> = Arc::new(Mutex::new(HashMap::new()));
    let clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>> = Arc::new(Mutex::new(docs));

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
                }); // saque el .join.unwrap
            }
            Err(e) => {
                eprintln!("Error al aceptar conexión: {}", e);
            }
        }
    }

    Ok(())
}


fn handle_client(stream: &mut TcpStream, clients: Arc<Mutex<HashMap<String, Client>>>, clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>, docs: Arc<Mutex<HashMap<String, Vec<String>>>>) -> std::io::Result<()> {
    let client_addr = stream.peer_addr()?;
    let reader = BufReader::new(stream.try_clone()?);

    for line in reader.lines() {
        if let Ok(command) = line {
            let input: Vec<String> = command.split_whitespace().map(|s| s.to_string().to_lowercase()).collect();
            let command = &input[0];
            println!("Recibido: {}", command);

            match command.as_str() {
                "ver" => {
                    let doc_selected = &input[1];
                    let doc_locked = docs.lock().unwrap();
                    if let Some(selected_doc) = doc_locked.get(doc_selected) {
                        writeln!(stream, "Mensajes en el documento {}", doc_selected)?;
                        for doc_message in selected_doc {
                            writeln!(stream, "{}", doc_message)?;
                        }
                        writeln!(stream, "Fin de los mensajes")?;
                    } else {
                        writeln!(stream, "No se encontro el documento")?;
                    }
                }
                "sub" => {
                    let doc_select = &input[1];
                    {
                        let mut lock_clients_on_docs = clients_on_docs.lock().unwrap();
                        if let Some(clients_on_doc) = lock_clients_on_docs.get_mut(doc_select) {
                            if clients_on_doc.contains(&client_addr.to_string()) {
                                writeln!(stream, "Ya estás subscripto al documento")?;
                            } else {
                                clients_on_doc.push(client_addr.to_string());
                            }
                        } else {
                            writeln!(stream, "Documento no encontrado")?;
                        }
                    }  
                }
                "unsub" => {
                    let doc_select = &input[1];
                    {
                        let mut lock_clients_on_docs = clients_on_docs.lock().unwrap();
                        if let Some(clients_on_doc) = lock_clients_on_docs.get_mut(doc_select) {
                            clients_on_doc.retain(|x| x.as_str() != client_addr.to_string().as_str());
                        } else {
                            println!("Documento no encontrado");
                        }
                    }  
                }
                "insertar" => {
                    let doc_selected = &input[1];
                    let mut doc_locked = docs.lock().unwrap();
                    if let Some(selected_doc) = doc_locked.get_mut(doc_selected) {
                        let message = input[2..].join(" ");
                        let message_to_publish = format!("Nuevo mensaje en {}: {}", doc_selected, input[2..].join(" "));
                        selected_doc.push(message);
                        let _ = publish(Arc::clone(&clients), Arc::clone(&clients_on_docs), message_to_publish, doc_selected.to_string());
                    } else {
                        writeln!(stream, "No se encontro el documento")?;
                    }
                }
                "agregar" => {
                    let doc_name = &input[1];
                    let mut docs_locked = docs.lock().unwrap();
                    let mut locked_clients_on_docs = clients_on_docs.lock().unwrap();

                    docs_locked.insert(doc_name.to_string(), vec![]);
                    locked_clients_on_docs.insert(doc_name.to_string(), vec![]);
                    
                    writeln!(stream, "Documento creado")?;
                }
                _ => {
                    writeln!(stream, "Comando no reconocido")?;
                }
            }
            let _ = write_to_file(docs.clone());
        }
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