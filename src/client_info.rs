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
