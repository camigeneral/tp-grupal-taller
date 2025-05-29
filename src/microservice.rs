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

#[derive(Debug)]
pub struct Microservice {    
    stream: TcpListener,
    address: String,
    redis_server: Option<TcpStream>
}

impl Microservice {
    pub fn new(port: u16) -> Self {
        let address = format!("127.0.0.1:{}", port);
        let stream = TcpListener::bind(address.clone()).unwrap();
        println!("Microservice levantado en: {:?}", address);
        Self {            
            address,
            stream,
            redis_server: None
        }
    }

    pub fn connect_to_redis(&mut self) -> std::io::Result<()> {
        self.redis_server = Some(TcpStream::connect(format!("127.0.0.1:{}", 4000))?);
        Ok(())
    }
}

pub fn main() -> std::io::Result<()> {
    let mut microservice = Microservice::new(5000);

    microservice.connect_to_redis()?;
    println!("Microservicio conectado al server de redis en: {:?}", microservice.redis_server.as_ref().unwrap().peer_addr());

    for stream in microservice.stream.incoming() {
        match stream {
            Ok(client_stream) => {
                println!("New client connected: {:?}", client_stream.peer_addr());
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
    Ok(())
}