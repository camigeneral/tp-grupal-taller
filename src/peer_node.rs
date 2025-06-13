use local_node::NodeRole;
use std::net::TcpStream;

/// Estructura que representa un nodo que se conecto al la instancia del nodo levantado en consola. Contiene el TCP stream para comunicarse, el puerto en el que
/// escucha nodos entrantes, el tipo (master o replica), y su hash range.
#[allow(dead_code)]
pub struct PeerNode {
    pub stream: TcpStream,
    pub port: usize,
    pub role: NodeRole,
    pub hash_range: (usize, usize),
}

impl PeerNode {
    pub fn new(stream: TcpStream, port: usize, role: NodeRole, hash_range: (usize, usize)) -> Self {
        PeerNode {
            stream,
            port,
            role,
            hash_range,
        }
    }
}
