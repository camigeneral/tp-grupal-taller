use std::env::args;
use std::io::stdin;
use std::io::Write;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;
use std::thread;

static CLIENT_ARGS: usize = 3;

fn main() -> Result<(), ()> {
    let argv = args().collect::<Vec<String>>();
    if argv.len() != CLIENT_ARGS {
        println!("Cantidad de argumentos inválido");
        let app_name = &argv[0];
        println!("{:?} <host> <puerto>", app_name);
        return Err(());
    }

    let address = argv[1].clone() + ":" + &argv[2];
    println!("Conectándome a {:?}", address);


    client_run(&address, &mut stdin()).unwrap();
    Ok(())
}


fn client_run(address: &str, stream: &mut dyn Read) -> std::io::Result<()> {
    let reader = BufReader::new(stream);
    let mut socket = TcpStream::connect(address)?;
    
    let cloned_socket = socket.try_clone()?;
    thread::spawn(move || {
        match listen_to_subscriptions(cloned_socket) {
            Ok(_) => {
                println!("Desconectado del nodo");
            }
            Err(e) => {
                eprintln!("Error en la conexión con nodo: {}", e);
            }
        }
    });

    for line in reader.lines() {
        if let Ok(line) = line {
            let command = line.trim().to_lowercase();

            if command != "salir" {
                println!("Enviando: {:?}", command);
                
                socket.write(command.as_bytes())?;
                socket.write("\n".as_bytes())?;
            } else {
                println!("Desconectando del servidor");
                break;
            }
        }
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