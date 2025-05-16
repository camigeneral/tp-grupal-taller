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
    thread::spawn(move || match listen_to_subscriptions(cloned_socket) {
        Ok(_) => {
            println!("Desconectado del nodo");
        }
        Err(e) => {
            eprintln!("Error en la conexión con nodo: {}", e);
        }
    });

    for line in reader.lines().map_while(Result::ok) {
        let command = line.trim();

        if command.to_lowercase() != "salir" {
            println!("Enviando: {:?}", command);

            let parts: Vec<&str> = command.split_whitespace().collect();
            let resp_command = format_resp_command(&parts);

            println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

            socket.write_all(resp_command.as_bytes())?;
        } else {
            println!("Desconectando del servidor");
            break;
        }
    }
    Ok(())
}


fn format_resp_command(parts: &[&str]) -> String {
    let mut resp = format!("*{}\r\n", parts.len());

    for part in parts {
        resp.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
    }

    resp
}


fn listen_to_subscriptions(socket: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(socket);

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            break;
        }

        println!("RESP recibido: {}", line.replace("\r\n", "\\r\\n"));

        match line.chars().next() {
            Some('$') => {
                let size_str = line.trim_end();

                if size_str == "$-1" || size_str == "$-1\r" {
                    println!("(nil)");
                    continue;
                }

                let size: usize = match size_str[1..].trim().parse() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("Error al parsear longitud: {}", size_str);
                        continue;
                    }
                };

                let mut buffer = vec![0u8; size + 2];
                reader.read_exact(&mut buffer)?;

                let content = String::from_utf8_lossy(&buffer[..size]).to_string();

                println!("{}", content);
            }
            Some('-') => {
                println!("Error: {}", line[1..].trim());
            }
            Some(':') => {
                println!("{}", line[1..].trim());
            }
            Some('+') => {
                println!("{}", line[1..].trim());
            }
            Some('*') => {
                let array_size_str = line.trim_end();
                let array_size: usize = match array_size_str[1..].trim().parse() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("Error al parsear tamaño de array: {}", array_size_str);
                        continue;
                    }
                };

                println!("Array de {} elementos:", array_size);
            }
            _ => {
                println!("{}", line.trim());
            }
        }
    }

    Ok(())
}
