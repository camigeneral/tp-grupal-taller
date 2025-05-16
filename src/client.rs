use std::io::Write;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::sync::mpsc::Receiver;
use std::thread;

pub fn client_run(port: u16, rx: Receiver<String>) -> std::io::Result<()> {
    let address = format!("127.0.0.1:{}", port);

    println!("Conectándome a {:?}", address);
    let mut socket = TcpStream::connect(address)?;

    let cloned_socket = socket.try_clone()?;

    thread::spawn(move || {
        if let Err(e) = listen_to_subscriptions(cloned_socket) {
            eprintln!("Error en la conexión con nodo: {}", e);
        }
    });

    for command in rx {
        if command.to_lowercase().trim() == "salir" {
            println!("Desconectando del servidor");
            break;
        }

        println!("Enviando: {:?}", command);
        socket.write_all(command.as_bytes())?;
        socket.write_all(b"\n")?;
    }

    Ok(())
}

fn listen_to_subscriptions(socket: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(socket);
    loop {
        let mut response = String::new();
        let flag = reader.read_line(&mut response)?;

        if flag == 0 {
            break;
        }

        println!("{}", response.trim());
    }

    Ok(())
}
