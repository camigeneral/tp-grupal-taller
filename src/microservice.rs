extern crate relm4;
use std::io::Write;
use std::io::{BufRead, BufReader};
#[allow(unused_imports)]
use std::net::TcpListener;
use std::net::TcpStream;
#[allow(unused_imports)]
use std::sync::mpsc;
use std::thread;
#[allow(unused_imports)]
use std::time::Duration;
use std::env::args;


static REQUIRED_ARGS: usize = 2;


pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args: Vec<String> = args().collect();
    if cli_args.len() != REQUIRED_ARGS {
        eprintln!("Error: Cantidad de argumentos inválida");
        eprintln!("Uso: {} <puerto>", cli_args[0]);
        return Err("Error: Cantidad de argumentos inválida".into());
    }

    let redis_port = match cli_args[1].parse::<usize>() {
        Ok(n) => n,
        Err(_e) => return Err("Failed to parse arguments".into()),
    };

    let address = format!("127.0.0.1:{}", redis_port);

    println!("Conectándome al server de redis en {:?}", address);
    let mut socket: TcpStream = TcpStream::connect(address)?;

    let command = "Microservicio\r\n".to_string();

    println!("Enviando: {:?}", command);
    let parts: Vec<&str> = command.split_whitespace().collect();
    let resp_command = format_resp_command(&parts);

    println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

    socket.write_all(resp_command.as_bytes())?;

    let redis_socket = socket.try_clone()?;

    thread::spawn(move || {
        if let Err(e) = listen_to_redis_response(redis_socket) {
            eprintln!("Error en la conexión con nodo: {}", e);
        }
    });

    loop{
        
    }
}

fn listen_to_redis_response(
    mut microservice_socket: TcpStream
) -> std::io::Result<()> {
    let mut reader = BufReader::new(microservice_socket.try_clone()?);
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        println!("Respuesta de redis: {}", line);

        if line.starts_with("Client ") && line.contains(" subscribed to ") {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.len() >= 5 {
                let client_addr = parts[1];

                let doc_name = parts[4];

                let bienvenida = format!("Welcome {} {}",doc_name, client_addr);
                

                let parts: Vec<&str> = bienvenida.split_whitespace().collect();

                let mensaje_final = format_resp_command(&parts);

                if let Err(e) = microservice_socket.write_all(mensaje_final.as_bytes()) {
                    eprintln!("Error al enviar mensaje de bienvenida: {}", e);
                }
            }
        }
    }
    Ok(())
}

pub fn format_resp_command(command_parts: &[&str]) -> String {
    let mut resp_message = format!("*{}\r\n", command_parts.len());

    for part in command_parts {
        resp_message.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
    }

    resp_message
}