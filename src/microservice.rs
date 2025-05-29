extern crate relm4;
use self::relm4::Sender;
//use crate::app::AppMsg;
use std::io::Read;
use crate::commands::client::ClientCommand;
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
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub struct Microservice {    
    stream: TcpListener,    
    redis_server: Mutex<Option<TcpStream>>,
    clients: Mutex<Vec<TcpStream>>
}

impl Microservice {
    pub fn new(port: u16) -> Self {
        let address = format!("127.0.0.1:{}", port);
        let stream = TcpListener::bind(address.clone()).unwrap();
        println!("Microservice levantado en: {:?}", address);
        Self {            
            stream,
            redis_server: Mutex::new(None),
            clients: Mutex::new(Vec::new())
        }
    }

    pub fn connect_to_redis(&self) -> std::io::Result<()> {
        let mut redis_server = self.redis_server.lock().unwrap();
        *redis_server = Some(TcpStream::connect(format!("127.0.0.1:{}", 4000))?);
        Ok(())
    }

    pub fn listen_to_client(&self, client_stream: TcpStream) -> std::io::Result<()> {
        let mut reader = BufReader::new(client_stream);
        

        loop {
            let mut buffer = String::new();
            let bytes_read = reader.read_line(&mut buffer)?;
            println!("Recibido: {:?}", buffer);

            if bytes_read == 0 {
                break;
            }

            if let Ok(command) = ClientCommand::from_string(&buffer) {
                println!("Comando: {:?}", command);
            } else {
                println!("Error al parsear el comando: {:?}", buffer);
            }

        }
        Ok(())
    }
}

pub fn main() -> std::io::Result<()> {
    let microservice = Arc::new(Microservice::new(5000));

    microservice.connect_to_redis()?;
    println!("Microservicio conectado al server de redis en: {:?}", microservice.redis_server.lock().unwrap().as_ref().unwrap().peer_addr());    
    let microservice_stream_clone = microservice.stream.try_clone()?;
    for stream in microservice_stream_clone.incoming() {
        match stream {
            Ok(client_stream) => {
                let client_stream_clone = client_stream.try_clone()?;
                let microservice_clone = Arc::clone(&microservice);
                thread::spawn(move || {
                    if let Err(e) = microservice_clone.listen_to_client(client_stream_clone) {
                        eprintln!("Error en la conexión con nodo: {}", e);
                    }
                });
                println!("Nuevo cliente conectado: {:?}", client_stream.peer_addr());
                let mut clients = microservice.clients.lock().unwrap();
                clients.push(client_stream);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
    Ok(())
}
