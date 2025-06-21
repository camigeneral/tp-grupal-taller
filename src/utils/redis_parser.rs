#[allow(unused_imports)]
use std::io::Cursor;
use std::io::{BufRead, BufReader, Read, Write};
#[allow(unused_imports)]
use std::net::{TcpListener, TcpStream};
use std::str;
#[allow(unused_imports)]
use std::thread;

/// Representa un valor de entrada en un comando RESP.
///
/// Este enum permite modelar diferentes tipos de datos que pueden
/// enviarse como argumentos, por ejemplo enteros, cadenas o arrays.
///
/// Algunas variantes podrían no estar en uso actualmente, pero están pensadas
/// para soportar extensiones del protocolo RESP.
///
/// Variantes:
/// - `Integer`: valor entero (RESP Integer)
/// - `String`: valor de tipo bulk string
/// - `Null`: representa un valor nulo (`$-1`)
/// - `Error`: mensaje de error
/// - `Array`: lista de valores anidados
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ValueType {
    Integer(i64),
    String(String),
    Null,
    Error(String),
    Array(Vec<ValueType>),
}

/// Representa una solicitud de comando recibida desde un cliente.
///
/// Esta estructura contiene el comando base (como `"get"` o `"set"`),
/// una clave opcional, y una lista de argumentos.
///
/// Ejemplo: el comando `SET mykey myvalue` se modelaría como:
/// - command: "set"
/// - key: Some("mykey")
/// - arguments: ["myvalue"]
#[derive(Debug)]
pub struct CommandRequest {
    pub command: String,
    pub key: Option<String>,
    pub arguments: Vec<ValueType>,
    pub unparsed_command: String,
}

/// Representa una respuesta que puede enviarse a un cliente en formato RESP.
///
/// El enum abarca las respuestas típicas del protocolo, como:
/// - Ok: "+OK"
/// - String: "$n\r\n..."\r\n
/// - Integer: ":n\r\n"
/// - Null: "$-1\r\n"
/// - Error: "-ERR ...\r\n"
/// - Array: arreglo de respuestas (no completamente soportado)
#[derive(Debug, PartialEq)]
#[allow(dead_code)]
pub enum CommandResponse {
    Ok,
    String(String),
    Integer(i64),
    Null,
    Error(String),
    Array(Vec<CommandResponse>),
}

/// Parsea un comando en formato RESP recibido desde un `BufReader<TcpStream>`.
///
/// Devuelve una estructura `CommandRequest` con el comando base, una clave opcional,
/// y argumentos adicionales como `ValueType::String`.
///
/// # Errores
/// Retorna `std::io::Error` si el formato RESP es inválido o si el comando está vacío
pub fn parse_command(reader: &mut BufReader<TcpStream>) -> std::io::Result<CommandRequest> {
    let (command_parts, unparsed_command) = parse_resp_command(reader)?;

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
        unparsed_command,
    };

    if command_parts.len() > 1 {
        request.key = Some(command_parts[1].clone());
    }

    for arg in command_parts.iter().skip(2) {
        request.arguments.push(ValueType::String(arg.clone()));
    }

    Ok(request)
}

/// Escribe una respuesta en formato RESP hacia un `TcpStream`.
///
/// Soporta múltiples tipos de respuesta definidos en `CommandResponse`, como cadenas,
/// enteros, errores, nulos y arrays. En caso de `Array`, responde solo el primer elemento.
///
/// # Errores
/// Retorna `std::io::Error` si hay fallas al escribir en el stream.
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

/// Formatea un comando en el protocolo RESP (Redis Serialization Protocol).
///
/// # Retorna
/// String formateada según el protocolo RESP
#[allow(dead_code)]
pub fn format_resp_command(command_parts: &[&str]) -> String {
    let mut resp_message = format!("*{}\r\n", command_parts.len());

    for part in command_parts {
        resp_message.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
    }

    resp_message
}

/// Parsea una línea en formato RESP que representa un array de cadenas (`Vec<String>`).
///
/// Lee el número de elementos del array (`*<n>`), seguido por `n` cadenas tipo bulk (`$<len>\r\n<value>\r\n`).
///
/// # Errores
/// Retorna `std::io::Error` si el formato RESP no es válido o si ocurre un error de lectura.
pub fn parse_resp_command(
    reader: &mut BufReader<TcpStream>,
) -> std::io::Result<(Vec<String>, String)> {
    let mut line = String::new();
    let mut unparsed_command = String::new();

    reader.read_line(&mut line)?;
    unparsed_command.push_str(&line);

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
        unparsed_command.push_str(&line);

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
        unparsed_command.push_str(&String::from_utf8_lossy(&buffer));

        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf)?;
        unparsed_command.push_str("\r\n");

        result.push(String::from_utf8_lossy(&buffer).to_string());
    }

    Ok((result, unparsed_command))
}

/// Escribe una cadena como bulk string en formato RESP (`$<len>\r\n<value>\r\n`).
///
/// # Errores
/// Retorna `std::io::Error` si no puede escribir en el stream.
pub fn write_resp_string(mut stream: &TcpStream, value: &str) -> std::io::Result<()> {
    let response = format!("${}\r\n{}\r\n", value.len(), value);
    stream.write_all(response.as_bytes())
}

/// Escribe un entero como RESP Integer (`:<value>\r\n`).
///
/// # Errores
/// Retorna `std::io::Error` si no puede escribir en el stream.
pub fn write_resp_integer(mut stream: &TcpStream, value: i64) -> std::io::Result<()> {
    let response = format!(":{}\r\n", value);
    stream.write_all(response.as_bytes())
}

/// Escribe un valor nulo como RESP Null (`$-1\r\n`).
///
/// # Errores
/// Retorna `std::io::Error` si no puede escribir en el stream.
pub fn write_resp_null(mut stream: &TcpStream) -> std::io::Result<()> {
    stream.write_all(b"$-1\r\n")
}

/// Escribe un mensaje de error como RESP Error (`-ERR <msg>\r\n`).
///
/// # Errores
/// Retorna `std::io::Error` si no puede escribir en el stream.
pub fn write_resp_error(mut stream: &TcpStream, msg: &str) -> std::io::Result<()> {
    stream.write_all(format!("-ERR {}\r\n", msg).as_bytes())
}

pub fn parse_replica_command(
    reader: &mut BufReader<std::io::Cursor<String>>,
) -> std::io::Result<CommandRequest> {
    let (command_parts, unparsed_command) = parse_replica_resp(reader)?;

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
        unparsed_command,
    };

    if command_parts.len() > 1 {
        request.key = Some(command_parts[1].clone());
    }

    for arg in command_parts.iter().skip(2) {
        request.arguments.push(ValueType::String(arg.clone()));
    }

    Ok(request)
}

pub fn parse_replica_resp(
    reader: &mut BufReader<std::io::Cursor<String>>,
) -> std::io::Result<(Vec<String>, String)> {
    let mut line = String::new();
    let mut unparsed_command = String::new();

    reader.read_line(&mut line)?;
    unparsed_command.push_str(&line);

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
        unparsed_command.push_str(&line);

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
        unparsed_command.push_str(&String::from_utf8_lossy(&buffer));

        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf)?;
        unparsed_command.push_str("\r\n");

        result.push(String::from_utf8_lossy(&buffer).to_string());
    }

    Ok((result, unparsed_command))
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
