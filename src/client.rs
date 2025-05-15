use std::io::Write;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::thread;
// use std::io::stdin;
// use std::env::args;

use std::sync::mpsc::Receiver;

// static CLIENT_ARGS: usize = 3;

// fn main() -> Result<(), ()> {
//     let argv = args().collect::<Vec<String>>();
//     if argv.len() != CLIENT_ARGS {
//         println!("Cantidad de argumentos inválido");
//         let app_name = &argv[0];
//         println!("{:?} <host> <puerto>", app_name);
//         return Err(());
//     }

//     let address = argv[1].clone() + ":" + &argv[2];
//     println!("Conectándome a {:?}", address);

//     client_run(&address, &mut stdin()).unwrap();
//     Ok(())
// }

// pub fn connect_client_with_channel(port: u16, rx: Receiver<String>) -> Result<(), Box<dyn std::error::Error>>{
//     let address = format!("127.0.0.1:{}", port);    
//     println!("Conectándome a {:?}", address);


//     //client_run(&address, &mut stdin()).unwrap();
//     let mut socket = TcpStream::connect(address).unwrap();

//     for command in rx {
//         socket.write_all(command.as_bytes()).unwrap();
//         socket.write_all(b"\n").unwrap();
//     }
    
//     Ok(())
// }


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


fn listen_to_subscriptions(socket: TcpStream)-> std::io::Result<()> {
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