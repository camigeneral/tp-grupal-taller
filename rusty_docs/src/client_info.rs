use std::io::{Result, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

/// Enum que representa el tipo de cliente conectado al sistema.
///
/// - `Client`: Cliente estándar.
/// - `Microservice`: Microservicio intermediario.
/// - `LlmMicroservice`: Microservicio de procesamiento de lenguaje natural.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientType {
    Client,
    Microservice,
    LlmMicroservice,
}

/// Estructura que representa la información de un cliente conectado.
///
/// Incluye el stream TCP (protegido por Mutex y Arc para concurrencia),
/// el tipo de cliente y el nombre de usuario asociado.
#[derive(Debug, Clone)]
pub struct Client {
    /// Stream TCP del cliente, protegido para acceso concurrente.
    pub stream: Arc<Mutex<Option<TcpStream>>>,
    /// Tipo de cliente (estándar, microservicio, LLM).
    pub client_type: ClientType,
    /// Nombre de usuario del cliente.
    pub username: String,
}

impl Write for Client {
    /// Escribe datos en el stream TCP del cliente.
    ///
    /// # Retorna
    /// - `Ok(usize)` con la cantidad de bytes escritos si el stream está disponible.
    /// - `Err` si el stream no está disponible o el Mutex fue envenenado.
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut stream_guard = self
            .stream
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Mutex poisoned"))?;

        match stream_guard.as_mut() {
            Some(stream) => stream.write(buf),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "No stream available",
            )),
        }
    }

    /// Fuerza el vaciado del buffer de escritura del stream TCP.
    fn flush(&mut self) -> Result<()> {
        let mut stream_guard = self
            .stream
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Mutex poisoned"))?;

        match stream_guard.as_mut() {
            Some(stream) => stream.flush(),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "No stream available",
            )),
        }
    }
}
