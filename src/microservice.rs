extern crate relm4;
use std::io::Write;
use std::io::{BufRead, BufReader};
#[allow(unused_imports)]
use std::net::TcpListener;
use std::net::TcpStream;
#[allow(unused_imports)]
use std::sync::mpsc;
use std::thread;
#[allow(unused_imports)]
use std::time::Duration;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let redis_port = 4000;
    let address = format!("127.0.0.1:{}", redis_port);

    println!("Conectándome al server de redis en {:?}", address);
    let mut socket: TcpStream = TcpStream::connect(address)?;

    let command = "Microservicio\r\n".to_string();

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
        
    }
}

fn listen_to_redis_response(
    mut microservice_socket: TcpStream
) -> std::io::Result<()> {
    let mut reader = BufReader::new(microservice_socket.try_clone()?);
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        println!("Respuesta de redis: {}", line);

        // Detectar mensaje de subscripción
        if line.starts_with("Client ") && line.contains(" subscribed to ") {
            // Parsear "Client <addr> subscribed to <doc>"
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.len() >= 5 {
                let client_addr = parts[1];

                let doc_name = parts[4];

                let bienvenida = format!("Welcome {} {}",doc_name, client_addr);
                

                let parts: Vec<&str> = bienvenida.split_whitespace().collect();

                // Enviar mensaje al canal del documento
                let mensaje_final = format_resp_command(&parts);

                if let Err(e) = microservice_socket.write_all(mensaje_final.as_bytes()) {
                    eprintln!("Error al enviar mensaje de bienvenida: {}", e);
                }
            }
        }
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



// fn handle_user_subscribed_event(
//     doc_name: &str,
//     user_addr: &str,
//     clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
//     clients_streams: Arc<Mutex<HashMap<String, TcpStream>>>,
// ) {
//     // Obtener lista de suscriptores al documento
//     let subscribers = {
//         let lock = clients_on_docs.lock().unwrap();
//         lock.get(doc_name).cloned().unwrap_or_default()
//     };

//     // Formatear mensaje
//     let msg = format!(
//         "Suscriptores actuales al documento '{}': {:?}",
//         doc_name, subscribers
//     );

//     // Enviar mensaje al usuario que se suscribió
//     let mut streams_lock = clients_streams.lock().unwrap();
//     if let Some(mut stream) = streams_lock.get_mut(user_addr) {
//         use std::io::Write;
//         if let Err(e) = stream.write_all(msg.as_bytes()) {
//             eprintln!("Error al enviar lista de suscriptores a {}: {}", user_addr, e);
//         }
//     } else {
//         eprintln!("No se encontró conexión TCP para {}", user_addr);
//     }
// }
