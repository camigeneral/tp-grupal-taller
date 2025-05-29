//! Implementación del microservicio que actúa como intermediario.
//!
//! Este módulo implementa la lógica principal del microservicio, incluyendo:
//! - Manejo de conexiones TCP con clientes
//! - Comunicación con el servidor Redis
//! - Procesamiento de comandos
//! - Distribución de actualizaciones

extern crate relm4;
use self::relm4::Sender;
//use crate::app::AppMsg;
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
use std::sync::Arc;
use std::sync::Mutex;
use crate::commands::client::ClientCommand;

/// Estructura principal del microservicio que mantiene el estado de las conexiones.
#[derive(Debug)]
pub struct Microservice {    
    /// Socket TCP que escucha conexiones entrantes de clientes
    pub stream: TcpListener,    
    /// Conexión al servidor Redis, protegida por mutex para acceso concurrente
    pub redis_server: Mutex<Option<TcpStream>>,
    /// Lista de clientes conectados, protegida por mutex
    pub clients: Mutex<Vec<TcpStream>>
}

impl Microservice {
    /// Crea una nueva instancia del microservicio escuchando en el puerto especificado.
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

    /// Establece conexión con el servidor Redis.
    pub fn connect_to_redis(&self) -> std::io::Result<()> {
        let mut redis_server = self.redis_server.lock().unwrap();
        *redis_server = Some(TcpStream::connect(format!("127.0.0.1:{}", 4000))?);
        Ok(())
    }

    /// Maneja la comunicación con un cliente conectado.
    ///
    /// Lee comandos del cliente, los procesa y envía respuestas apropiadas.
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
