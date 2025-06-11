extern crate relm4;
use self::relm4::Sender;
use crate::app::AppMsg;
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
use utils::redis_parser::format_resp_command;

pub fn client_run(
    port: u16,
    rx: Receiver<String>,
    ui_sender: Option<Sender<AppMsg>>,
) -> std::io::Result<()> {
    let address = format!("127.0.0.1:{}", port);

    println!("Conectándome al server de redis en {:?}", address);
    let mut socket: TcpStream = TcpStream::connect(address)?;

    let redis_socket = socket.try_clone()?;

    thread::spawn(move || {
        if let Err(e) = listen_to_redis_response(redis_socket, ui_sender) {
            eprintln!("Error en la conexión con nodo: {}", e);
        }
    });

    for command in rx {
        let trimmed_command = command.to_string().trim().to_lowercase();
        if trimmed_command == "close" {
            println!("Desconectando del servidor");
            break;
        } else {
            println!("Enviando: {:?}", command);

            let parts: Vec<&str> = command.split_whitespace().collect();
            let resp_command = format_resp_command(&parts);

            println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

            socket.write_all(resp_command.as_bytes())?;
        }
    }

    Ok(())
}

fn listen_to_redis_response(
    microservice_socket: TcpStream,
    ui_sender: Option<Sender<AppMsg>>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(microservice_socket);
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            break;
        }
        println!("Respuesta de redis: {}", line);
        if let Some(sender) = &ui_sender {
            let _ = sender.send(AppMsg::RefreshData);
        }
    }
    Ok(())
}
