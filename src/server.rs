use std::env::args;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

static SERVER_ARGS: usize = 2;

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
    let listener = TcpListener::bind(address)?;
    let counter = Arc::new(Mutex::new(0));

    for stream in listener.incoming() {
        match stream {
            Ok(mut client_stream) => {
                let counter = Arc::clone(&counter);
                let client_addr = client_stream.peer_addr()?;
                println!("La socket addr del client: {}", client_addr);

                thread::spawn(move || {
                    match handle_client(&mut client_stream, counter) {
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


fn handle_client(stream: &mut TcpStream, counter: Arc<Mutex<i32>>) -> std::io::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);

    for line in reader.lines() {
        if let Ok(command) = line {
            let command = command.trim().to_lowercase();
            println!("Recibido: {}", command);

            match command.as_str() {
                "incrementar" => {
                    let mut count = counter.lock().unwrap();
                    *count += 1;
                    writeln!(stream, "Contador incrementado a: {}", *count)?;
                }
                "ver" => {
                    let count = counter.lock().unwrap();
                    writeln!(stream, "Contador actual: {}", *count)?;
                }
                _ => {
                    writeln!(stream, "Comando no reconocido")?;
                }
            }
        }
    }

    Ok(())
}