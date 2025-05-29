//! Implementación del microservicio que actúa como intermediario.
//!
//! Este módulo implementa la lógica principal del microservicio, incluyendo:
//! - Manejo de conexiones TCP con clientes
//! - Comunicación con el servidor Redis
//! - Procesamiento de comandos
//! - Distribución de actualizaciones

extern crate relm4;
//use crate::app::AppMsg;
use std::io::Read;
use std::io::Write;
use std::io::{BufRead, BufReader};
#[allow(unused_imports)]
use std::net::TcpListener;
use std::net::TcpStream;
#[allow(unused_imports)]
use std::sync::mpsc;
#[allow(unused_imports)]
use std::time::Duration;
use std::sync::Mutex;
use crate::commands::client::ClientCommand;
use std::io::BufWriter;

/// Estructura principal del microservicio que actúa como intermediario entre clientes y Redis.
/// 
/// Esta estructura mantiene:
/// - La conexión con el servidor Redis
/// - Las conexiones con los clientes
/// - El socket de escucha para nuevas conexiones
/// 
/// # Ejemplo
/// ```
/// let microservice = Microservice::new(5000);
/// microservice.connect_to_redis()?;
/// ```
#[derive(Debug)]
pub struct Microservice {    
    /// Socket TCP que escucha nuevas conexiones entrantes de clientes
    pub tcp_listener: TcpListener,    
    
    /// Conexión al servidor Redis, protegida por mutex para acceso concurrente.
    /// Se mantiene como Option para manejar la conexión/desconexión de forma segura.
    pub redis_connection: Mutex<Option<TcpStream>>,
    
    /// Lista de conexiones activas con clientes, protegida por mutex para acceso concurrente
    pub active_clients: Mutex<Vec<TcpStream>>
}

impl Microservice {
    /// Crea una nueva instancia del microservicio escuchando en el puerto especificado.
    /// 
    /// # Argumentos
    /// * `port` - Puerto en el que escuchará nuevas conexiones
    /// 
    /// # Retorna
    /// Una nueva instancia de Microservice configurada y lista para usar
    pub fn new(port: u16) -> Self {
        let bind_address = format!("127.0.0.1:{}", port);
        let tcp_listener = TcpListener::bind(bind_address.clone()).unwrap();
        println!("Microservice levantado en: {:?}", bind_address);
        
        Self {            
            tcp_listener,
            redis_connection: Mutex::new(None),
            active_clients: Mutex::new(Vec::new())
        }
    }

    /// Establece una conexión con el servidor Redis.
    /// 
    /// Intenta establecer una conexión TCP con el servidor Redis en el puerto 4000.
    /// La conexión se almacena en el Mutex redis_connection para acceso concurrente.
    /// 
    /// # Errores
    /// Retorna un error si:
    /// - No se puede establecer la conexión TCP
    /// - No se puede obtener el lock del Mutex
    pub fn connect_to_redis(&self) -> std::io::Result<()> {
        println!("Intentando conectar a Redis...");
        let mut redis_connection_guard = self.redis_connection.lock().unwrap();
        println!("Lock obtenido para redis_connection");
        
        let redis_address = format!("127.0.0.1:{}", 4000);
        println!("Intentando conectar a Redis en {}", redis_address);
        
        match TcpStream::connect(&redis_address) {
            Ok(stream) => {
                println!("Conexión establecida exitosamente con Redis");
                *redis_connection_guard = Some(stream);
                Ok(())
            },
            Err(e) => {
                println!("Error al conectar con Redis: {}", e);
                Err(e)
            }
        }
    }

    /// Cierra la conexión con el servidor Redis.
    /// 
    /// Libera la conexión TCP existente y la marca como None.
    pub fn disconnect_from_redis(&self) -> std::io::Result<()> {
        let mut redis_connection_guard = self.redis_connection.lock().unwrap();
        *redis_connection_guard = None;
        Ok(())
    }

    fn handle_client_command(&self, client_command: ClientCommand, client_writer: &mut TcpStream) -> std::io::Result<()> {
        let redis_command = self.parse_client_command_to_redis_command(client_command);                                            
        if let Ok(()) = self.send_resp_command(redis_command) {
            client_writer.write_all(b"OK\r\n")?;            
        } else {
            println!("Error al enviar el comando a Redis");
        }
        Ok(())
    }

    /// Maneja la comunicación con un cliente conectado.
    /// 
    /// Procesa los comandos recibidos del cliente y los envía a Redis.
    /// Mantiene un bucle de lectura hasta que el cliente se desconecte o
    /// envíe un comando de cierre.
    /// 
    /// # Argumentos
    /// * `client_stream` - Stream TCP del cliente conectado
    /// 
    /// # Errores
    /// Retorna un error si hay problemas de lectura/escritura en el stream
    pub fn listen_to_client(&self, client_stream: TcpStream) -> std::io::Result<()> {
        let mut client_writer = client_stream.try_clone()?;
        let mut command_reader = BufReader::new(client_stream);
        
        loop {
            let mut command_buffer = String::new();
            let bytes_read = command_reader.read_line(&mut command_buffer)?;
            println!("Comando recibido: {:?}", command_buffer);

            if bytes_read == 0 {
                break;
            }

            if let Ok(client_command) = ClientCommand::from_string(&command_buffer.clone()) {
                match client_command.clone() {
                    ClientCommand::CreateFile { file_id: _, content: _ } => self.handle_client_command(client_command, &mut client_writer)?,
                    ClientCommand::Close => {
                        println!("Comando de cierre recibido");
                        break;
                    }
                    _ => self.handle_client_command(client_command, &mut client_writer)?,                    
                }
            } else {
                println!("Error al parsear el comando: {:?}", command_buffer);
            }
        }
        Ok(())
    }

    /// Formatea un comando en el protocolo RESP (Redis Serialization Protocol).
    /// 
    /// # Argumentos
    /// * `command_parts` - Array de strings que representan las partes del comando
    /// 
    /// # Retorna
    /// String formateada según el protocolo RESP
    /// 
    /// # Ejemplo
    /// ```
    /// let command_parts = ["SET", "key", "value"];
    /// let resp_command = format_resp_command(&command_parts);
    /// assert_eq!(resp_command, "*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n");
    /// ```
    pub fn format_resp_command(&self, command_parts: &[&str]) -> String {
        let mut resp_message = format!("*{}\r\n", command_parts.len());
    
        for part in command_parts {
            resp_message.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
        }
    
        resp_message
    }
    
    /// Convierte un comando del cliente al formato RESP de Redis.
    /// 
    /// Traduce los comandos del protocolo del cliente al protocolo RESP
    /// que entiende Redis.
    /// 
    /// # Argumentos
    /// * `client_command` - Comando del cliente a convertir
    /// 
    /// # Retorna
    /// String con el comando formateado en protocolo RESP
    pub fn parse_client_command_to_redis_command(&self, client_command: ClientCommand) -> String {
        println!("Traduciendo comando del cliente: {:?}", client_command);
        match client_command {
            ClientCommand::CreateFile { file_id, content } => { 
                let resp_command = self.format_resp_command(&["set", &file_id, &content]);
                println!("Comando traducido a RESP: {}", resp_command);
                resp_command
            },
            _ => client_command.to_string(),
        }
    }

    /// Envía un comando RESP al servidor Redis.
    /// 
    /// # Argumentos
    /// * `resp_command` - Comando en formato RESP a enviar
    /// 
    /// # Errores
    /// Retorna un error si:
    /// - No hay conexión establecida con Redis
    /// - Hay problemas al escribir en el stream
    /// - No se puede obtener el lock del Mutex
    pub fn send_resp_command(&self, resp_command: String) -> std::io::Result<()> {
        println!("Iniciando envío de comando RESP");  
        println!("Obteniendo acceso a la conexión Redis...");
        
        let redis_connection_guard = self.redis_connection.lock().unwrap();
        println!("Acceso obtenido. Estado de la conexión: {:#?}", redis_connection_guard);
        
        match redis_connection_guard.as_ref() {
            Some(redis_stream) => {
                println!("Conexión activa encontrada, preparando escritura...");
                let mut command_writer = BufWriter::new(redis_stream);
                println!("Enviando comando: {}", resp_command);
                command_writer.write_all(resp_command.as_bytes())?;
                command_writer.flush()?;
                println!("Comando enviado exitosamente");
                Ok(())
            },
            None => {
                println!("Error: No hay conexión activa con Redis");
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "No hay conexión establecida con Redis"
                ))
            }
        }
    }

    /// Escucha y procesa las respuestas del servidor Redis.
    /// 
    /// Mantiene un bucle de lectura continua para procesar las respuestas
    /// que envía Redis. Soporta todos los tipos de respuesta del protocolo RESP:
    /// - Strings simples (+)
    /// - Errores (-)
    /// - Enteros (:)
    /// - Bulk strings ($)
    /// - Arrays (*)
    /// 
    /// # Errores
    /// Retorna un error si:
    /// - No hay conexión con Redis
    /// - Hay problemas al leer del stream
    /// - No se puede obtener el lock del Mutex
    pub fn listen_to_redis(&self) -> std::io::Result<()> {
        let redis_connection_guard = self.redis_connection.lock().unwrap();
        let redis_stream = redis_connection_guard.as_ref().unwrap().try_clone()?;
        drop(redis_connection_guard);
        
        let mut response_reader = BufReader::new(redis_stream);
        println!("Iniciando escucha de respuestas Redis");
        
        loop {
            let mut response_line = String::new();
            let bytes_read = response_reader.read_line(&mut response_line)?;
            println!("Bytes leídos: {:?}", bytes_read);

            if bytes_read == 0 {
                println!("Conexión cerrada por Redis");
                break;
            }

            println!("Respuesta RESP recibida: {}", response_line.replace("\r\n", "\\r\\n"));
            match response_line.chars().next() {
                Some('$') => {
                    let size_str = response_line.trim_end();

                    if size_str == "$-1" || size_str == "$-1\r" {
                        println!("Valor nulo recibido");
                        continue;
                    }

                    let content_size: usize = match size_str[1..].trim().parse() {
                        Ok(n) => n,
                        Err(_) => {
                            eprintln!("Error al parsear tamaño del contenido: {}", size_str);
                            continue;
                        }
                    };

                    let mut content_buffer = vec![0u8; content_size + 2]; // +2 para CRLF
                    response_reader.read_exact(&mut content_buffer)?;

                    let content = String::from_utf8_lossy(&content_buffer[..content_size]).to_string();
                    println!("Contenido recibido: {}", content);
                }
                Some('-') => {
                    println!("Error de Redis: {}", response_line[1..].trim());
                }
                Some(':') => {
                    println!("Entero recibido: {}", response_line[1..].trim());
                }
                Some('+') => {
                    println!("Respuesta simple: {}", response_line[1..].trim());
                }
                Some('*') => {
                    let array_size_str = response_line.trim_end();
                    let array_size: usize = match array_size_str[1..].trim().parse() {
                        Ok(n) => n,
                        Err(_) => {
                            eprintln!("Error al parsear tamaño del array: {}", array_size_str);
                            continue;
                        }
                    };

                    println!("Array RESP recibido con {} elementos", array_size);
                }
                _ => {
                    println!("Respuesta desconocida: {}", response_line.trim());
                }
            }
        }

        println!("Finalizando escucha de Redis");
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use std::net::TcpListener;
    use std::io::Write;
    use std::sync::mpsc;
    use crate::client::client_run;

    fn format_resp_command(parts: &[&str]) -> String {
        let mut result = format!("*{}\r\n", parts.len());
        for part in parts {
            result.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
        }
        result
    }

    #[test]
    fn test_format_resp_command() {
        let parts = vec!["SET", "key", "value"];
        let result = format_resp_command(&parts);
        assert_eq!(result, "*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n");

        let empty: Vec<&str> = vec![];
        let result = format_resp_command(&empty);
        assert_eq!(result, "*0\r\n");

        let single = vec!["PING"];
        let result = format_resp_command(&single);
        assert_eq!(result, "*1\r\n$4\r\nPING\r\n");

        // Prueba con caracteres especiales
        let special = vec!["SET", "key:1", "hello world"];
        let result = format_resp_command(&special);
        assert_eq!(result, "*3\r\n$3\r\nSET\r\n$5\r\nkey:1\r\n$11\r\nhello world\r\n");
    }

    #[test]
    fn test_response_parsing() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let (tx, rx) = mpsc::channel();

        let server_thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Prueba diferentes tipos de respuestas RESP
            stream.write_all(b"+OK\r\n").unwrap();
            stream.write_all(b"$5\r\nhello\r\n").unwrap();
            stream.write_all(b"-Error message\r\n").unwrap();
            stream.write_all(b":1000\r\n").unwrap();
            stream.write_all(b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n").unwrap();
            stream.write_all(b"$-1\r\n").unwrap(); // Valor nulo
        });

        let client_thread = thread::spawn(move || {
            client_run(port, rx, None).unwrap();
        });

        thread::sleep(Duration::from_millis(100));
        tx.send(ClientCommand::Close).unwrap();

        assert!(server_thread.join().is_ok());
        assert!(client_thread.join().is_ok());
    }

    #[test]
    fn test_connection_errors() {
        let port = 9999; // Puerto no utilizado
        let (_tx, rx) = mpsc::channel();
        
        let result = client_run(port, rx, None);
        assert!(result.is_err());
    }

   /*  #[test]
   TODO: Revisar 
    fn test_multiple_clients() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let server_thread = thread::spawn(move || {
            for _ in 0..3 {
                if let Ok((mut stream, _)) = listener.accept() {
                    stream.write_all(b"+OK\r\n").unwrap();
                }
            }
        });

        let mut client_threads = vec![];
        for _ in 0..3 {
            let (tx, rx) = mpsc::channel();
            let client_thread = thread::spawn(move || {
                let _ = client_run(port, rx, None);
            });
            thread::sleep(Duration::from_millis(50));
            tx.send(ClientCommand::Close).unwrap();
            client_threads.push(client_thread);
        }

        for thread in client_threads {
            assert!(thread.join().is_ok());
        }
        assert!(server_thread.join().is_ok());
    } */
}