extern crate relm4;
use std::io::Write;
use std::io::Read;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::sync::mpsc::Receiver;
use std::thread;
use self::relm4::Sender;
use crate::app::AppMsg; 

pub fn client_run(port: u16, rx: Receiver<String>, ui_sender: Sender<AppMsg>) -> std::io::Result<()> {
    let address = format!("127.0.0.1:{}", port);

    println!("Conectándome a {:?}", address);
    let mut socket = TcpStream::connect(address)?;

    let cloned_socket = socket.try_clone()?;

    thread::spawn(move || {
        if let Err(e) = listen_to_subscriptions(cloned_socket, ui_sender) {
            eprintln!("Error en la conexión con nodo: {}", e);
        }
    });

    for command in rx {

        let trimmed_command = command.trim().to_lowercase();

        if trimmed_command == "salir" {
            println!("Desconectando del servidor");
            break;
        }else{
            println!("Enviando: {:?}", command);

            let parts: Vec<&str> = command.split_whitespace().collect();
            let resp_command = format_resp_command(&parts);

            println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

            socket.write_all(resp_command.as_bytes())?;
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


fn listen_to_subscriptions(socket: TcpStream, ui_sender: Sender<AppMsg>) -> std::io::Result<()> {
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
        ui_sender.send(AppMsg::RefreshData).unwrap();
    }

    Ok(())
}
