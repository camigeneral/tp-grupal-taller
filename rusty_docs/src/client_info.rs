use std::io::{Result, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientType {
    Client,
    Microservice,
    LlmMicroservice,
}
#[derive(Debug, Clone)]
pub struct Client {
    pub stream: Arc<Mutex<Option<TcpStream>>>,
    pub client_type: ClientType,
    pub username: String,
}

impl Write for Client {
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
