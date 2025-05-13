use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::str;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ValueType {
    Integer(i64),
    String(String),
    Null,
    Error(String),
    Array(Vec<ValueType>),
}

#[derive(Debug)]
pub struct CommandRequest {
    pub command: String,
    pub key: Option<String>,
    pub arguments: Vec<ValueType>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum CommandResponse {
    Ok,
    String(String),
    Integer(i64),
    Null,
    Error(String),
    Array(Vec<CommandResponse>),
}

pub fn parse_command(reader: &mut BufReader<TcpStream>) -> std::io::Result<CommandRequest> {
    let command_parts = parse_resp_command(reader)?;

    if command_parts.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Empty command",
        ));
    }

    let command = command_parts[0].to_lowercase();

    let mut request = CommandRequest {
        command,
        key: None,
        arguments: Vec::new(),
    };

    if command_parts.len() > 1 {
        request.key = Some(command_parts[1].clone());
    }

    for arg in command_parts.iter().skip(2) {
        request.arguments.push(ValueType::String(arg.clone()));
    }

    Ok(request)
}

pub fn write_response(stream: &TcpStream, response: &CommandResponse) -> std::io::Result<()> {
    match response {
        CommandResponse::Ok => write_resp_string(stream, "OK"),
        CommandResponse::String(s) => write_resp_string(stream, s),
        CommandResponse::Integer(i) => write_resp_integer(stream, *i),
        CommandResponse::Null => write_resp_null(stream),
        CommandResponse::Error(msg) => write_resp_error(stream, msg),
        CommandResponse::Array(arr) => {
            if let Some(first) = arr.first() {
                write_response(stream, first)
            } else {
                write_resp_null(stream)
            }
        }
    }
}

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

pub fn write_resp_integer(mut stream: &TcpStream, value: i64) -> std::io::Result<()> {
    let response = format!(":{}\r\n", value);
    stream.write_all(response.as_bytes())
}

pub fn write_resp_null(mut stream: &TcpStream) -> std::io::Result<()> {
    stream.write_all(b"$-1\r\n")
}

pub fn write_resp_error(mut stream: &TcpStream, msg: &str) -> std::io::Result<()> {
    stream.write_all(format!("-ERR {}\r\n", msg).as_bytes())
}
