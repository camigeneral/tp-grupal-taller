extern crate relm4;
use self::relm4::Sender;
use std::io::Write;
use std::io::{BufRead, BufReader};
#[allow(unused_imports)]
use std::net::TcpListener;
use std::net::TcpStream;
#[allow(unused_imports)]
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
#[allow(unused_imports)]
use std::time::Duration;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let redis_port = 4000;
    let address = format!("127.0.0.1:{}", redis_port);

    println!("Conectándome al server de redis en {:?}", address);
    let mut socket: TcpStream = TcpStream::connect(address)?;

    let mut command = "Soy Microservicio\r\n".to_string();
    let trimmed_command = command.trim().to_lowercase();

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
        let mut command = String::new();
        println!("Ingrese un comando (o 'close' para desconectar):");
        std::io::stdin().read_line(&mut command)?;
        let trimmed_command = command.trim().to_lowercase();

        if trimmed_command == "close" {
            println!("Desconectando del servidor");
            break;
        } else {
            println!("Enviando: {:?}", command);
            let parts: Vec<&str> = trimmed_command.split_whitespace().collect();
        }
    }

    Ok(())
}

fn listen_to_redis_response(
    microservice_socket: TcpStream
) -> std::io::Result<()> {
    let mut reader = BufReader::new(microservice_socket);
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            break;
        }
        println!("Respuesta de redis: {}", line);
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
