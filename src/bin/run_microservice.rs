//! Punto de entrada para el microservicio.
//!
//! Este binario inicia el microservicio que actúa como intermediario entre:
//! - Los clientes GUI que editan documentos
//! - El servidor Redis que almacena los documentos
//!
//! # Arquitectura
//! ```text
//! [Cliente GUI] <-> [Microservicio] <-> [Redis Server]
//!     |                   ^                   |
//!     |                   |                   |
//!     +-------------------+-------------------+
//!           Notificaciones de cambios
//! ```
//!
//! El microservicio:
//! 1. Acepta conexiones de múltiples clientes
//! 2. Traduce comandos del cliente al protocolo RESP
//! 3. Mantiene conexión con Redis
//! 4. Distribuye actualizaciones a los clientes suscritos

extern crate rusty_docs;
use rusty_docs::microservice::Microservice;
use std::sync::Arc;
use std::thread;

fn main() -> std::io::Result<()> {
    let microservice = Arc::new(Microservice::new(5000));

    microservice.connect_to_redis()?;
    println!("Microservicio conectado al server de redis...");
    let microservice_clone = Arc::clone(&microservice);
    thread::spawn(move || {
        println!("Escuchando a Redis");
        if let Err(e) = microservice_clone.listen_to_redis() {
            eprintln!("Error escuchando a Redis: {}", e);
        }
    });

    thread::sleep(std::time::Duration::from_millis(100));
   
    let microservice_stream_clone: std::net::TcpListener = microservice.tcp_listener.try_clone()?;
    
    for stream in microservice_stream_clone.incoming() {
        match stream {
            Ok(client_stream) => {
                let client_stream_clone = client_stream.try_clone()?;
                let microservice_clone = Arc::clone(&microservice);
                std::thread::spawn(move || {
                    if let Err(e) = microservice_clone.listen_to_client(client_stream_clone) {
                        eprintln!("Error en la conexión con nodo: {}", e);
                    }
                });
                println!("Nuevo cliente conectado: {:?}", client_stream.peer_addr());
                let mut clients = microservice.active_clients.lock().unwrap();
                clients.push(client_stream);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
    Ok(())
} 