use std::io::Read;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::str;

pub fn parse_resp_command(reader: &mut BufReader<TcpStream>) -> std::io::Result<Vec<String>> {
    let mut line = String::new();
    reader.read_line(&mut line)?;

    if !line.starts_with('*') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a RESP array",
        ));
    }

    let num_elements: usize = line[1..].trim().parse().unwrap_or(0);
    let mut result = Vec::with_capacity(num_elements);

    for _ in 0..num_elements {
        line.clear();
        reader.read_line(&mut line)?;

        if !line.starts_with('$') {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Expected bulk string",
            ));
        }

        let length: usize = match line[1..].trim().parse() {
            Ok(len) => len,
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid length",
                ))
            }
        };

        let mut buffer = vec![0u8; length];
        reader.read_exact(&mut buffer)?;

        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf)?;

        result.push(String::from_utf8_lossy(&buffer).to_string());
    }

    Ok(result)
}

pub fn write_resp_string(mut stream: &TcpStream, value: &str) -> std::io::Result<()> {
    let response = format!("${}\r\n{}\r\n", value.len(), value);
    stream.write_all(response.as_bytes())
}

pub fn write_resp_null(mut stream: &TcpStream) -> std::io::Result<()> {
    stream.write_all(b"$-1\r\n")
}

pub fn write_resp_error(mut stream: &TcpStream, msg: &str) -> std::io::Result<()> {
    stream.write_all(format!("-ERR {}\r\n", msg).as_bytes())
}
