use std::env::args;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;

static SERVER_ARGS: usize = 2;

fn main() -> Result<(), ()> {
    let argv = args().collect::<Vec<String>>();
    if argv.len() != SERVER_ARGS {
        println!("Invalid number of arguments");
        let app_name = &argv[0];
        println!("Usage:\n{:?} <port>", app_name);
        return Err(());
    }

    let address = "127.0.0.1:".to_owned() + &argv[1];
    server_run(&address).unwrap();
    Ok(())
}

fn server_run(address: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(address)?;
    let store = Arc::new(Mutex::new(HashMap::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(mut client_stream) => {
                let store = Arc::clone(&store);  
                let client_addr = client_stream.peer_addr()?;
                println!("Client socket address: {}", client_addr);

                thread::spawn(move || {
                    match handle_client(&mut client_stream, store) {
                        Ok(_) => {
                            println!("Client {} has disconnected.", client_addr);
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

fn handle_client(stream: &mut TcpStream, store: Arc<Mutex<HashMap<String, String>>>) -> std::io::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);

    for line in reader.lines() {
        if let Ok(command) = line {
            let command = command.trim().to_lowercase();

            if command.starts_with("set") {
                let parts: Vec<&str> = command.split_whitespace().collect();
                if parts.len() >= 3 {
                    let key = parts[1].to_string();
                    let value = parts[2].to_string();
                    let mut store = store.lock().unwrap();
                    store.insert(key.clone(), value.clone());  
                    write!(stream, "+OK\r\n")?;
                } else {
                    write!(stream, "-Invalid SET command. Usage: SET <key> <value>\r\n")?;
                }
            } else if command.starts_with("incr") {
                let parts: Vec<&str> = command.split_whitespace().collect();
                if parts.len() >= 2 {
                    let key = parts[1].to_string();
                    let mut store = store.lock().unwrap();
                    
                    if !store.contains_key(&key) {
                        store.insert(key.clone(), "0".to_string());
                    }
                    
                    let current_value = store.get(&key).unwrap();
                    match current_value.parse::<i64>() {
                        Ok(num) => {
                            let new_value = num + 1;
                            store.insert(key.clone(), new_value.to_string());
                            
                            write!(stream, ":{}\r\n", new_value)?;
                        }
                        Err(_) => {
                            write!(stream, "-ERR value is not an integer or out of range\r\n")?;
                        }
                    }
                } else {
                    write!(stream, "-Invalid INCR command. Usage: INCR <key>\r\n")?;
                }
            } else if command.starts_with("decr"){
                let parts: Vec<&str> = command.split_whitespace().collect();
                if parts.len() >= 2 {
                    let key = parts[1].to_string();
                    let mut store = store.lock().unwrap();
                    
                    if !store.contains_key(&key) {
                        store.insert(key.clone(), "0".to_string());
                    }
                    
                    let current_value = store.get(&key).unwrap();
                    match current_value.parse::<i64>() {
                        Ok(num) => {
                            let new_value = num - 1;
                            store.insert(key.clone(), new_value.to_string());
                            
                            write!(stream, ":{}\r\n", new_value)?;
                        }
                        Err(_) => {
                            write!(stream, "-ERR value is not an integer or out of range\r\n")?;
                        }
                    }
                } else {
                    write!(stream, "-Invalid INCR command. Usage: INCR <key>\r\n")?;
                }
            } else if command.starts_with("get") {
                let parts: Vec<&str> = command.split_whitespace().collect();
                if parts.len() >= 2 {
                    let key = parts[1].to_string();
                    let store = store.lock().unwrap();
                    match store.get(&key) {
                        Some(value) => {
                            write!(stream, "${}\r\n{}\r\n", value.len(), value)?;
                        }
                        None => {
                            write!(stream, "$-1\r\n")?;
                        }
                    }
                } else {
                    write!(stream, "-Invalid GET command. Usage: GET <key>\r\n")?;
                }
            } else {
                write!(stream, "-Unrecognized command\r\n")?;
            }
            stream.flush()?;
        }
    }

    Ok(())
}