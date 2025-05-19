extern crate relm4;
use self::relm4::Sender;
use crate::app::AppMsg;
use std::io::Read;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

pub fn client_run(
    port: u16,
    rx: Receiver<String>,
    ui_sender: Option<Sender<AppMsg>>,
) -> std::io::Result<()> {
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

fn format_resp_command(parts: &[&str]) -> String {
    let mut resp = format!("*{}\r\n", parts.len());

    for part in parts {
        resp.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
    }

    resp
}

fn listen_to_subscriptions(
    socket: TcpStream,
    ui_sender: Option<Sender<AppMsg>>,
) -> std::io::Result<()> {
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

        if let Some(sender) = &ui_sender {
            let _ = sender.send(AppMsg::RefreshData);
        }
    }

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_resp_command() {
        let parts = vec!["SET", "key", "value"];
        let result = format_resp_command(&parts);
        assert_eq!(result, "*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n");

        let empty: Vec<&str> = vec![];
        let result = format_resp_command(&empty);
        assert_eq!(result, "*0\r\n");

        let single = vec!["PING"];
        let result = format_resp_command(&single);
        assert_eq!(result, "*1\r\n$4\r\nPING\r\n");
    }

    #[test]
    fn test_response_parsing() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let (tx, rx) = mpsc::channel();

        let server_thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.write_all(b"+OK\r\n").unwrap();
            stream.write_all(b"$5\r\nhello\r\n").unwrap();
            stream.write_all(b"-Error message\r\n").unwrap();
            stream.write_all(b":1000\r\n").unwrap();
        });

        let client_thread = thread::spawn(move || {
            client_run(port, rx, None).unwrap();
        });

        thread::sleep(Duration::from_millis(100));
        tx.send("salir".to_string()).unwrap();

        assert!(server_thread.join().is_ok());
        assert!(client_thread.join().is_ok());
    }
}
