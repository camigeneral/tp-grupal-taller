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
#[derive(Debug, Clone, PartialEq)]
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
#[allow(dead_code)]
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
#[derive(Debug, PartialEq, Clone)]
#[allow(dead_code)]
pub enum CommandResponse {
    Ok,
    String(String),
    Integer(i64),
    Null,
    Error(String),
    Array(Vec<CommandResponse>),
}

impl CommandResponse {
    pub fn get_resp(&self) -> String {
        match self {
            CommandResponse::Ok => "+OK\r\n".to_string(), // Simple String
            CommandResponse::String(s) => format!("${}\r\n{}\r\n", s.len(), s), // Bulk String
            CommandResponse::Integer(i) => format!(":{}\r\n", i), // Integer
            CommandResponse::Null => "$-1\r\n".to_string(), // Null Bulk String
            CommandResponse::Error(msg) => format!("-ERR {}\r\n", msg),
            CommandResponse::Array(arr) => {
                let mut resp = format!("*{}\r\n", arr.len());
                for item in arr {
                    resp.push_str(&item.get_resp());
                }
                resp
            }
        }
    }
}

/// Parsea un comando en formato RESP recibido desde un `BufReader<TcpStream>`.
///
/// Devuelve una estructura `CommandRequest` con el comando base, una clave opcional,
/// y argumentos adicionales como `ValueType::String`.
///
/// # Errores
/// Retorna `std::io::Error` si el formato RESP es inválido o si el comando está vacío
#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn write_response(mut stream: &TcpStream, response: &CommandResponse) -> std::io::Result<()> {
    stream.write_all(response.get_resp().as_bytes())
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

#[allow(dead_code)]
pub fn format_resp_publish(channel: &str, message: &str) -> String {
    let command_parts = ["publish", channel, message];

    format_resp_command(&command_parts)
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
    reader.read_line(&mut line)?;
    let mut unparsed_command = line.clone();

    if line.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "Empty input",
        ));
    }

    match line.chars().next() {
        Some('*') => parse_array(reader, &mut unparsed_command, &line),
        Some('+') => parse_simple_string(&line, &unparsed_command),
        Some('-') => parse_error_string(&line, &unparsed_command),
        Some(':') => parse_integer_string(&line, &unparsed_command),
        Some('$') => parse_bulk_string(reader, &mut unparsed_command, &line),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Unsupported RESP type",
        )),
    }
}

/// Parsea un array RESP completo a partir de la línea inicial '*'.
///
/// Lee cada elemento como un Bulk String según el protocolo RESP.
/// Actualiza la representación no parseada del comando para logging.
///
/// # Errores
/// Retorna un error si el formato RESP es inválido, si las longitudes no coinciden,
/// o si hay fallas en la lectura del socket.
fn parse_array(
    reader: &mut BufReader<TcpStream>,
    unparsed_command: &mut String,
    first_line: &str,
) -> std::io::Result<(Vec<String>, String)> {
    let num_elements: usize = first_line[1..].trim().parse().map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid array length")
    })?;

    let mut result = Vec::with_capacity(num_elements);

    for _ in 0..num_elements {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        unparsed_command.push_str(&line);

        if !line.starts_with('$') {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Expected bulk string",
            ));
        }

        let length: usize = line[1..].trim().parse().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid bulk string length",
            )
        })?;

        let mut buffer = vec![0u8; length];
        reader.read_exact(&mut buffer)?;

        let string_value = String::from_utf8(buffer).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8 in buffer")
        })?;
        unparsed_command.push_str(&string_value);

        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf)?;
        unparsed_command.push_str("\r\n");

        result.push(string_value);
    }

    Ok((result, unparsed_command.clone()))
}

/// Parsea una respuesta RESP de tipo Simple String ('+').
///
/// Extrae el contenido y devuelve un vector con un solo elemento.
/// No requiere lecturas adicionales del socket.
///
/// # Errores
/// No retorna errores salvo problemas de entrada vacía o inesperada.
fn parse_simple_string(
    line: &str,
    unparsed_command: &str,
) -> std::io::Result<(Vec<String>, String)> {
    let simple_string = line[1..].trim_end().to_string();
    Ok((vec![simple_string], unparsed_command.to_string()))
}

/// Parsea una respuesta RESP de tipo Error ('-').
///
/// Extrae el contenido del error y lo devuelve como un string prefijado con '-'.
/// No requiere lecturas adicionales del socket.
///
/// # Errores
/// No retorna errores salvo problemas de entrada vacía o inesperada.
fn parse_error_string(
    line: &str,
    unparsed_command: &str,
) -> std::io::Result<(Vec<String>, String)> {
    let error_string = line[1..].trim_end().to_string();
    Ok((
        vec![format!("-{}", error_string)],
        unparsed_command.to_string(),
    ))
}

/// Parsea una respuesta RESP de tipo Integer (':').
///
/// Extrae el número como string y lo devuelve como un único elemento del vector.
/// No realiza conversiones a tipo numérico, deja el valor como string para flexibilidad.
///
/// # Errores
/// No retorna errores salvo problemas de entrada vacía o inesperada.
fn parse_integer_string(
    line: &str,
    unparsed_command: &str,
) -> std::io::Result<(Vec<String>, String)> {
    let integer_string = line[1..].trim_end().to_string();
    Ok((vec![integer_string], unparsed_command.to_string()))
}

/// Parsea una respuesta RESP de tipo Bulk String ('$') fuera de un array.
///
/// Lee la cantidad exacta de bytes especificada y valida que el contenido sea UTF-8.
/// También lee y valida los bytes de cierre '\r\n'.
///
/// # Errores
/// Retorna un error si la longitud es inválida, si la lectura falla,
/// o si el contenido no es una cadena UTF-8 válida.
fn parse_bulk_string(
    reader: &mut BufReader<TcpStream>,
    unparsed_command: &mut String,
    line: &str,
) -> std::io::Result<(Vec<String>, String)> {
    let length: usize = line[1..].trim().parse().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid bulk string length",
        )
    })?;

    let mut buffer = vec![0u8; length];
    reader.read_exact(&mut buffer)?;

    let string_value = String::from_utf8(buffer).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8 in buffer")
    })?;
    unparsed_command.push_str(&string_value);

    let mut crlf = [0u8; 2];
    reader.read_exact(&mut crlf)?;
    unparsed_command.push_str("\r\n");

    Ok((vec![string_value], unparsed_command.clone()))
}

/* /// Escribe una cadena como bulk string en formato RESP (`$<len>\r\n<value>\r\n`).
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
    let response: String = format!(":{}\r\n", value);
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
 */
#[allow(dead_code)]
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

    if let Err(e) = reader.read_line(&mut line) {
        return Err(std::io::Error::other(format!("Read line error: {}", e)));
    }
    unparsed_command.push_str(&line);

    if !line.starts_with('*') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a RESP array",
        ));
    }

    let num_elements: usize = match line[1..].trim().parse() {
        Ok(n) => n,
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid number of elements",
            ));
        }
    };

    let mut result = Vec::with_capacity(num_elements);

    for _ in 0..num_elements {
        line.clear();
        if let Err(e) = reader.read_line(&mut line) {
            return Err(std::io::Error::other(format!("Read line error: {}", e)));
        }
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
                    "Invalid bulk string length",
                ));
            }
        };

        let mut buffer = vec![0u8; length];
        if let Err(e) = reader.read_exact(&mut buffer) {
            return Err(std::io::Error::other(format!("Read buffer error: {}", e)));
        }

        match String::from_utf8(buffer.clone()) {
            Ok(s) => unparsed_command.push_str(&s),
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid UTF-8 content",
                ));
            }
        }

        let mut crlf = [0u8; 2];
        if let Err(e) = reader.read_exact(&mut crlf) {
            return Err(std::io::Error::other(format!("Read CRLF error: {}", e)));
        }
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

    //#[test]
    /* fn test_write_resp_functions() {
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
    } */
}
