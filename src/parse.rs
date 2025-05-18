use std::io::Cursor;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::str;
use std::thread;

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

#[derive(Debug, PartialEq, Eq, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_tcp_server() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || listener.accept().unwrap().0);

        let client = TcpStream::connect(addr).unwrap();
        let server = server.join().unwrap();

        (client, server)
    }

    #[test]
    fn test_parse_command() {
        let (mut client, server) = setup_tcp_server();

        // Write RESP command to server
        write!(client, "*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n").unwrap();

        let mut reader = BufReader::new(server);
        let request = parse_command(&mut reader).unwrap();

        assert_eq!(request.command, "set");
        assert_eq!(request.key, Some("key".to_string()));
        assert_eq!(request.arguments.len(), 1);
    }

    #[test]
    // fn test_write_response() {
    //     let (client, server) = setup_tcp_server();

    //     // Test different response types
    //     let responses = vec![
    //         CommandResponse::Ok,
    //         CommandResponse::String("test".to_string()),
    //         CommandResponse::Integer(42),
    //         CommandResponse::Null,
    //         CommandResponse::Error("error message".to_string())
    //     ];

    //     for response in responses {
    //         write_response(&server, &response).unwrap();
    //     }

    //     let mut reader = BufReader::new(client);
    //     let mut buffer = String::new();

    //     // Verify OK response
    //     reader.read_line(&mut buffer).unwrap();
    //     assert!(buffer.contains("OK"));
    //     buffer.clear();

    //     // Verify String response
    //     reader.read_line(&mut buffer).unwrap();
    //     assert!(buffer.contains("test"));
    //     buffer.clear();

    //     // Verify Integer response
    //     reader.read_line(&mut buffer).unwrap();
    //     assert!(buffer.contains("42"));
    //     buffer.clear();

    //     // Verify Null response
    //     reader.read_line(&mut buffer).unwrap();
    //     assert!(buffer.contains("$-1"));
    //     buffer.clear();

    //     // Verify Error response
    //     reader.read_line(&mut buffer).unwrap();
    //     assert!(buffer.contains("error message"));
    // }
    #[test]
    fn test_parse_resp_command_errors() {
        let (mut client, server) = setup_tcp_server();

        // Test invalid RESP format
        write!(client, "invalid\r\n").unwrap();
        let mut reader = BufReader::new(server);
        let result = parse_resp_command(&mut reader);
        assert!(result.is_err());

        // Test invalid length
        let (mut client, server) = setup_tcp_server();
        write!(client, "*1\r\n$invalid\r\n").unwrap();
        let mut reader = BufReader::new(server);
        let result = parse_resp_command(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_resp_functions() {
        let (client, server) = setup_tcp_server();

        // Test write_resp_string
        write_resp_string(&server, "test").unwrap();

        let mut reader = BufReader::new(client);
        let mut buffer = String::new();
        reader.read_line(&mut buffer).unwrap();
        reader.read_line(&mut buffer).unwrap();
        assert_eq!(buffer, "$4\r\ntest\r\n");

        // Test write_resp_integer
        let (client, server) = setup_tcp_server();
        write_resp_integer(&server, 42).unwrap();

        let mut reader = BufReader::new(client);
        let mut buffer = String::new();
        reader.read_line(&mut buffer).unwrap();
        assert_eq!(buffer, ":42\r\n");

        // Test write_resp_null
        let (client, server) = setup_tcp_server();
        write_resp_null(&server).unwrap();

        let mut reader = BufReader::new(client);
        let mut buffer = String::new();
        reader.read_line(&mut buffer).unwrap();
        assert_eq!(buffer, "$-1\r\n");

        // Test write_resp_error
        let (client, server) = setup_tcp_server();
        write_resp_error(&server, "test error").unwrap();

        let mut reader = BufReader::new(client);
        let mut buffer = String::new();
        reader.read_line(&mut buffer).unwrap();
        assert_eq!(buffer, "-ERR test error\r\n");
    }
}
