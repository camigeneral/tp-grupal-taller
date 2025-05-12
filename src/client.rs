use std::env::args;
use std::io::stdin;
use std::io::Write;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;

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
    
    let mut socket_reader = BufReader::new(socket.try_clone()?);

    for line in reader.lines() {
        if let Ok(line) = line {
            let command = line.trim().to_lowercase();

            if command == "incrementar" || command == "ver" {
                println!("Enviando: {:?}", command);
                
                socket.write(command.as_bytes())?;
                socket.write("\n".as_bytes())?;

                let mut response = String::new();
                socket_reader.read_line(&mut response)?;
                println!("Respuesta del servidor: {}", response.trim());
            } else if command == "salir" {
                println!("Desconectando del servidor");
                break;
            } 
            
            else {
                println!("Comando no reconocido");
            }
        }
    }
    Ok(())
}