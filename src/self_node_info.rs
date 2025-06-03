use std::fs::{File};
use std::io::{BufRead, BufReader};
use std::path::Path;

pub enum NodeType {
    M,
    R,
}

#[allow(dead_code)]
pub struct SelfNode {
    pub port: usize,
    pub hash_range_start: usize,
    pub hash_range_end: usize,
    pub node_type: NodeType,
}

impl SelfNode {
    pub fn new_node_from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut buf = String::new();
        reader.read_line(&mut buf)?;
        let split_line: Vec<&str> = buf.split(",").collect();

        let mut port = 0;
        if let Ok(p) = split_line[0].trim().parse::<usize>() {
            port = p;
        } else {
            println!("Invalid port number: {}", split_line[0].trim());
        }

        let mut hash_range_start = 0;
        if let Ok(s) = split_line[1].trim().parse::<usize>() {
            hash_range_start = s;
        } else {
            println!("Invalid port number: {}", split_line[0].trim());
        }

        let mut hash_range_end = 0;
        if let Ok(e) = split_line[2].trim().parse::<usize>() {
            hash_range_end = e;
        } else {
            println!("Invalid port number: {}", split_line[0].trim());
        }

        let r = "R".to_string();

        let node_type_string = split_line[3].trim().to_string();

        let mut node_type: NodeType = NodeType::M;
        if node_type_string == r {
            node_type = NodeType::R;
        }

        Ok(
            SelfNode {
                port,
                hash_range_start,
                hash_range_end,
                node_type,
            }
        )

    }
}
