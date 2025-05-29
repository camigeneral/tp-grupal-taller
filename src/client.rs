extern crate relm4;
use self::relm4::Sender;
use crate::app::AppMsg;
use std::io::Read;
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
use commands::client::ClientCommand;

pub fn client_run(
    port: u16,
    rx: Receiver<ClientCommand>,
    ui_sender: Option<Sender<AppMsg>>,
) -> std::io::Result<()> {
    let address = format!("127.0.0.1:{}", port);

    println!("Conectándome al microservicio en {:?}", address);
    let mut socket: TcpStream = TcpStream::connect(address)?;

    let microservice_socket = socket.try_clone()?;

    thread::spawn(move || {
        if let Err(e) = listen_to_microservice_response(microservice_socket, ui_sender) {
            eprintln!("Error en la conexión con nodo: {}", e);
        }
    });

    for command in rx {
        let trimmed_command = command.to_string().trim().to_lowercase();
        println!("Enviando comando: {}", trimmed_command);
        socket.write_all(command.to_string().as_bytes())?;  

        if trimmed_command == "salir" {
            println!("Desconectando del servidor");
            break;
        }
    }

    Ok(())
}

fn listen_to_microservice_response(
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
        println!("Respuesta del microservicio: {}", line);
    }
    Ok(())
}