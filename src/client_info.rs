use std::net::TcpStream;

pub struct Client {
    pub stream: TcpStream,
    pub client_type: String,
}
