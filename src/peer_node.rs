use std::net::TcpStream;
use local_node::NodeRole;

#[allow(dead_code)]
pub struct PeerNode {
    pub stream: TcpStream,
    pub port: usize,         
    pub role: NodeRole,            
    pub hash_range: Option<(usize, usize)>,
}

impl PeerNode {
    pub fn new(stream: TcpStream, port: usize, role: NodeRole, hash_range: Option<(usize, usize)>) -> Self {
        PeerNode {stream, port, role, hash_range}
    }
}
