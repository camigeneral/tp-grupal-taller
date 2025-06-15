use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, PartialEq)]
pub enum NodeRole {
    Master,
    Replica,
    Unknown,
}

/// Estructura que representa la instancia de nodo levantada en la consola. Contiene el puerto en el que
/// escucha nodos entrantes, el tipo (master o replica), y su hash range.
pub struct LocalNode {
    pub port: usize,
    pub role: NodeRole,
    pub hash_range: (usize, usize),
    pub master_node: Option<usize>,
    pub replica_nodes: Vec<usize>,
}

impl LocalNode {
    pub fn new_from_config<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut buf = String::new();
        reader.read_line(&mut buf)?;
        let split_line: Vec<&str> = buf.split(",").collect();

        if split_line.len() != 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid arguments",
            ));
        }

        let port = split_line[0].trim().parse::<usize>().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid port number")
        })?;

        let hash_range_start = split_line[1].trim().parse::<usize>().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid start range")
        })?;

        let hash_range_end = split_line[2].trim().parse::<usize>().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid end range")
        })?;

        let role = match split_line[3].trim().to_lowercase().as_str() {
            "master" => NodeRole::Master,
            "replica" => NodeRole::Replica,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid role",
                ));
            }
        };

        Ok(LocalNode {
            port,
            role,
            hash_range: (hash_range_start, hash_range_end),
            master_node: None,
            replica_nodes: Vec::new(),
        })
    }
}
