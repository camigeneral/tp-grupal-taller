use std::io::{Result, Write};
use std::net::TcpStream;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientType {
    Cliente,
    Microservicio,
}

pub struct Client {
    pub stream: TcpStream,
    pub client_type: ClientType,
    pub username: String,
}

impl Write for Client {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.stream.flush()
    }
}
