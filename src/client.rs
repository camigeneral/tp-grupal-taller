use std::env::args;
use std::io::stdin;
use std::io::Write;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;

static CLIENT_ARGS: usize = 3;

fn main() -> Result<(), ()> {
    let argv = args().collect::<Vec<String>>();
    if argv.len() != CLIENT_ARGS {
        println!("Invalid number of arguments");
        let app_name = &argv[0];
        println!("{:?} <host> <port>", app_name);
        return Err(());
    }

    let address = argv[1].clone() + ":" + &argv[2];
    println!("Connecting to {:?}", address);

    client_run(&address, &mut stdin()).unwrap();
    Ok(())
}

fn client_run(address: &str, stream: &mut dyn Read) -> std::io::Result<()> {
    let reader = BufReader::new(stream);
    let mut socket = TcpStream::connect(address)?;
    
    let mut socket_reader = BufReader::new(socket.try_clone()?);

    for line in reader.lines() {
        if let Ok(line) = line {
            let command = line.trim().to_lowercase();

            if command.starts_with("set") {
                let parts: Vec<&str> = command.split_whitespace().collect();
                if parts.len() >= 3 {
                    let key = parts[1];
                    let value = parts[2];
                    let set_command = format!("SET {} {}\r\n", key, value);
                    socket.write(set_command.as_bytes())?;

                    let mut response = String::new();
                    socket_reader.read_line(&mut response)?;
                    println!("{}", response.trim());
                } else {
                    println!("Invalid SET command. Usage: SET <key> <value>");
                }
            } else if command.starts_with("incr") {
                let parts: Vec<&str> = command.split_whitespace().collect();
                if parts.len() >= 2 {
                    let key = parts[1];
                    let incr_command = format!("INCR {}\r\n", key);
                    socket.write(incr_command.as_bytes())?;

                    let mut response = String::new();
                    socket_reader.read_line(&mut response)?;
                    
                    if response.starts_with(":") {
                        println!("{}", response);
                    } else {
                        println!("{}", response.trim());
                    }
                } else {
                    println!("Invalid INCR command. Usage: INCR <key>");
                }
            } else if command.starts_with("decr") {
                let parts: Vec<&str> = command.split_whitespace().collect();
                if parts.len() >= 2 {
                    let key = parts[1];
                    let incr_command = format!("DECR {}\r\n", key);
                    socket.write(incr_command.as_bytes())?;
    
                    let mut response = String::new();
                    socket_reader.read_line(&mut response)?;
                        
                    if response.starts_with(":") {
                        println!("{}", response);
                    } else {
                        println!("{}", response.trim());
                    }
                } else {
                    println!("Invalid DECR command. Usage: INCR <key>");
                }
            } else if command.starts_with("get") {
                let parts: Vec<&str> = command.split_whitespace().collect();
                if parts.len() >= 2 {
                    let key = parts[1];
                    let get_command = format!("GET {}\r\n", key);
                    socket.write(get_command.as_bytes())?;
                    
                    let mut size_line = String::new();
                    socket_reader.read_line(&mut size_line)?;
                    

                    print!("{}", size_line);  
                    
                    if size_line.starts_with("$") {
                        let size_str = size_line.trim_end().trim_start_matches("$");
                        
                        if size_str == "-1" {
                        } else {
                            let mut value = String::new();
                            socket_reader.read_line(&mut value)?;
                            
                            print!("{}", value);
                            
                        }
                    } else {
                        println!("{}", size_line);
                    }
                } else {
                    println!("Invalid GET command. Usage: GET <key>");
                }
            } else if command == "exit" {
                println!("Disconnecting from the server");
                break;
            } else {
                let command_parts: Vec<&str> = command.split_whitespace().collect();
                if !command_parts.is_empty() {
                    let full_command = format!("{}\r\n", command);
                    socket.write(full_command.as_bytes())?;
                    
                    let mut response = String::new();
                    socket_reader.read_line(&mut response)?;
                    println!("{}", response.trim());
                } else {
                    println!("Empty command");
                }
            }
        }
    }
    Ok(())
}