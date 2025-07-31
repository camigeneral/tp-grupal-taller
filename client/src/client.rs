extern crate relm4;
use self::relm4::Sender as UiSender;
use crate::app::AppMsg;
use crate::components::structs::document_value_info::DocumentValueInfo;
use rusty_docs::resp_parser;
use rusty_docs::resp_parser::{format_resp_command, format_resp_publish};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, Sender as MpscSender};
use std::sync::{Arc, Mutex};
use std::thread;
use types::RedisClientResponseType;

/// Registro de canales de escritura asociados a nodos.
///
/// Esta estructura permite registrar y acceder a los canales (`Sender<String>`)
/// que se utilizan para enviar mensajes a cada nodo conectado. Es útil para manejar
/// múltiples conexiones a nodos Redis y enviar comandos de forma concurrente.
///
/// Internamente, utiliza un `HashMap` protegido por un `Mutex` y envuelto en un `Arc`
/// para permitir acceso seguro desde múltiples hilos.
#[derive(Clone)]
struct WriterRegistry {
    /// Mapa de identificadores de nodo a canales de envío de mensajes.
    writers: Arc<Mutex<HashMap<String, MpscSender<String>>>>,
}

impl WriterRegistry {
    pub fn new() -> Self {
        Self {
            writers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn insert(&self, node_id: String, sender: MpscSender<String>) {
        if let Ok(mut map) = self.writers.lock() {
            map.insert(node_id, sender);
        } else {
            eprintln!("No se pudo bloquear writer_registry para insertar");
        }
    }

    pub fn get(&self, node_id: &str) -> Option<MpscSender<String>> {
        self.writers.lock().ok()?.get(node_id).cloned()
    }
}

/// Cliente local que maneja la conexión y comunicación con el servidor Redis.
///
/// Esta estructura representa el cliente principal de la aplicación, encargado de:
/// - Mantener la conexión TCP con el servidor Redis.
/// - Enviar y recibir comandos y respuestas.
/// - Gestionar la comunicación con la interfaz de usuario (UI).
/// - Administrar los canales de escritura a nodos y el registro de conexiones.
/// - Sincronizar el estado del último comando enviado.
///
/// Todos los campos que pueden ser accedidos desde múltiples hilos están protegidos
/// mediante `Arc` y `Mutex` para garantizar la seguridad en concurrencia.
pub struct LocalClient {
    /// Dirección IP y puerto del servidor Redis al que se conecta este cliente.
    address: String,
    /// Canal para enviar mensajes a la interfaz de usuario (UI).
    ui_sender: Option<UiSender<AppMsg>>,
    /// Último comando enviado al servidor Redis, protegido por un Mutex para acceso concurrente.
    last_command_sent: Arc<Mutex<String>>,
    /// Socket TCP activo para la comunicación con el servidor Redis.
    redis_socket: TcpStream,
    /// Canal de envío de comandos al servidor Redis.
    redis_sender: Option<MpscSender<String>>,
    /// Canal de recepción de mensajes provenientes de la UI.
    rx_ui: Option<Receiver<String>>,
    /// Registro de canales de escritura para los nodos conectados.
    writer_registry: WriterRegistry,
}

/// Contexto de conexión para un nodo Redis.
///
/// Esta estructura agrupa los recursos necesarios para manejar la comunicación
/// y el estado asociado a una conexión con un nodo específico. Permite compartir
/// el estado entre hilos y facilita el manejo de mensajes y respuestas.
///
/// Se utiliza principalmente al crear nuevas conexiones o al redirigir comandos
/// a otros nodos del clúster.
#[derive(Clone)]
struct NodeConnectionContext {
    /// Último comando enviado a este nodo.
    last_command_sent: Arc<Mutex<String>>,
    /// Canal para enviar mensajes a la interfaz de usuario.
    ui_sender: Option<UiSender<AppMsg>>,
    /// Registro de canales de escritura para los nodos.
    writer_registry: WriterRegistry,
}

impl LocalClient {
    /// Crea una nueva instancia de `LocalClient` y establece la conexión TCP con el servidor Redis.
    ///
    /// # Argumentos
    /// * `port` - Puerto al que conectarse en localhost.
    /// * `ui_sender` - Canal opcional para enviar mensajes a la UI.
    /// * `rx_ui` - Canal opcional para recibir mensajes desde la UI.
    ///
    /// # Errores
    /// Retorna un error si no se puede conectar al servidor Redis.
    pub fn new(
        port: u16,
        ui_sender: Option<UiSender<AppMsg>>,
        rx_ui: Option<Receiver<String>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let address = format!("127.0.0.1:{}", port);
        let socket: TcpStream = match TcpStream::connect(address.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error al conectar al servidor: {}", e);
                return Err(Box::new(e));
            }
        };

        Ok(Self {
            address,
            ui_sender,
            last_command_sent: Arc::new(Mutex::new("".to_string())),
            redis_socket: socket,
            redis_sender: None,
            rx_ui,
            writer_registry: WriterRegistry::new(),
        })
    }

    /// Crea y lanza un canal de escritura dedicado para un nodo Redis.
    ///
    /// Este canal permite enviar mensajes de forma concurrente a un nodo específico.
    ///
    /// # Argumentos
    /// * `node_id` - Identificador del nodo.
    /// * `stream` - Stream TCP hacia el nodo.
    /// * `writer_registry` - Registro global de writers.
    ///
    /// # Retorna
    /// El sender asociado al canal de escritura.
    fn spawn_writer_channel(
        node_id: String,
        stream: TcpStream,
        writer_registry: &WriterRegistry,
    ) -> MpscSender<String> {
        let (tx, rx) = channel::<String>();

        let node_id_for_thread = node_id.clone();

        thread::spawn(move || {
            let mut writer = BufWriter::new(stream);
            for msg in rx {
                if let Err(e) = writer.write_all(msg.as_bytes()) {
                    eprintln!("Error escribiendo al nodo {}: {}", node_id_for_thread, e);
                    break;
                }
                let _ = writer.flush();
            }
        });
        writer_registry.insert(node_id, tx.clone());

        tx
    }

    /// Registra y conecta un nuevo nodo Redis, lanzando un hilo para manejar la conexión.
    ///
    /// # Errores
    /// Retorna un error si falla la clonación del socket o la conexión.
    fn register_and_connect_node(&self) -> std::io::Result<()> {
        let redis_socket = match self.redis_socket.try_clone() {
            Ok(clone) => clone,
            Err(e) => {
                eprintln!("Error al clonar el socket: {}", e);
                return Err(e);
            }
        };

        let (connect_node_sender, connect_nodes_receiver) = channel::<TcpStream>();
        let connect_node_sender_cloned = connect_node_sender.clone();
        let params = NodeConnectionContext::from(self);
        thread::spawn(move || {
            if let Err(e) =
                Self::connect_to_nodes(connect_node_sender_cloned, connect_nodes_receiver, params)
            {
                eprintln!("Error en la conexión con el nodo: {}", e);
            }
        });

        let _ = connect_node_sender.send(redis_socket);
        Ok(())
    }

    /// Inicializa el canal de envío de comandos al servidor Redis.
    ///
    /// Clona el socket y crea el canal de escritura principal.
    fn set_redis_sender(&mut self) {
        let redis_socket = match self.redis_socket.try_clone() {
            Ok(clone) => clone,
            Err(e) => {
                eprintln!("Error al clonar el socket de Redis: {}", e);
                return;
            }
        };

        let tx =
            Self::spawn_writer_channel(self.address.clone(), redis_socket, &self.writer_registry);

        self.redis_sender = Some(tx);
    }

    /// Ejecuta el ciclo principal del cliente, inicializando canales y manejando mensajes.
    ///
    /// Este método debe llamarse para iniciar la lógica de comunicación y procesamiento.
    pub fn run(&mut self) {
        self.set_redis_sender();

        let initial_command = resp_parser::format_resp_command(&["Cliente"]);
        if let Some(redis_sender) = &self.redis_sender {
            let _ = redis_sender.send(initial_command);
        }

        let _ = self.register_and_connect_node();

        let _ = self.read_comming_messages();
    }

    /// Genera el comando RESP adecuado según el tipo de comando recibido.
    ///
    /// # Argumentos
    /// * `parts` - Partes del comando separadas por espacio.
    /// * `command` - Comando original como string.
    ///
    /// # Retorna
    /// El comando RESP serializado.
    fn get_resp_command(&self, parts: Vec<&str>, command: &str) -> String {
        if parts.is_empty() {
            return String::new();
        }

        let cmd = parts[0];

        const SIMPLE_COMMANDS: [&str; 4] = ["AUTH", "SUBSCRIBE", "UNSUBSCRIBE", "SET"];

        if SIMPLE_COMMANDS.iter().any(|c| cmd.eq_ignore_ascii_case(c)) {
            return format_resp_command(&parts);
        }

        let cmd_upper = cmd.to_ascii_uppercase();

        if cmd_upper.contains("WRITE") {
            let splited_command: Vec<&str> = command.split('|').collect();
            let client_command = format_resp_command(&splited_command);
            let key = splited_command.get(4).unwrap_or(&"");
            return format_resp_publish(key, &client_command);
        }

        if cmd_upper.contains("PROMPT") {
            println!("command: {:#?}", command);
            let splited_command: Vec<&str> = command.split('|').collect();
            let client_command = format_resp_command(&splited_command);
            let key = splited_command.get(2).unwrap_or(&"");
            return format_resp_publish(key, &client_command);
        }

        let key = parts.get(1).unwrap_or(&"");
        format_resp_publish(key, command)
    }

    /// Actualiza el último comando enviado, almacenándolo de forma segura.
    ///
    /// # Argumentos
    /// * `resp_command` - Comando RESP a guardar.
    ///
    /// # Errores
    /// Retorna un error si falla el acceso al mutex.
    fn set_last_command(&self, resp_command: String) -> std::io::Result<()> {
        let mut last_command = match self.last_command_sent.lock() {
            Ok(locked) => locked,
            Err(e) => {
                eprintln!("Error al bloquear el mutex de last_command_sent: {}", e);
                return Err(std::io::Error::other("Mutex lock failed"));
            }
        };
        *last_command = resp_command.clone();
        Ok(())
    }

    /// Maneja la conexión y escucha respuestas de múltiples nodos Redis.
    ///
    /// Lanza un hilo por cada nueva conexión recibida.
    ///
    /// # Argumentos
    /// * `node_sender` - Canal para enviar nuevos streams.
    /// * `reciever` - Canal para recibir streams TCP.
    /// * `params` - Contexto de conexión.
    ///
    /// # Errores
    /// Retorna un error si falla la conexión o el manejo de streams.
    fn connect_to_nodes(
        node_sender: MpscSender<TcpStream>,
        reciever: Receiver<TcpStream>,
        params: NodeConnectionContext,
    ) -> std::io::Result<()> {
        for stream in reciever {
            let cloned_own_sender = node_sender.clone();
            let params_clone = params.clone();
            thread::spawn(move || {
                if let Err(e) =
                    Self::listen_to_redis_response(stream, cloned_own_sender, params_clone)
                {
                    eprintln!("Error en la conexión con el nodo: {}", e);
                }
            });
        }

        Ok(())
    }

    /// Lee y procesa los mensajes entrantes desde la UI.
    ///
    /// Envía los comandos al servidor Redis y actualiza el estado local.
    ///
    /// # Errores
    /// Retorna un error si falla el envío o la recepción de mensajes.
    fn read_comming_messages(&mut self) -> std::io::Result<()> {
        let rx_ui = match &self.rx_ui {
            Some(rx) => rx,
            None => return Ok(()),
        };

        let redis_sender = match &self.redis_sender {
            Some(tx) => tx,
            None => return Ok(()),
        };

        for command in rx_ui {
            let trimmed_command = command.to_string().trim().to_lowercase();
            let parts: Vec<&str> = command.split_whitespace().collect();
            if trimmed_command == "close" {
                println!("Desconectando del servidor");
                let resp_command =
                    resp_parser::format_resp_publish(parts[0], parts.get(1).unwrap_or(&""));

                println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

                if let Err(e) = redis_sender.send(resp_command) {
                    eprintln!("Error al escribir en el socket: {}", e);
                    return Ok(());
                }
                break;
            } else {
                println!("Enviando: {:?}", command);
                let resp_command = self.get_resp_command(parts, &command);

                let _ = self.set_last_command(resp_command.clone());

                println!("RESP enviado: {}", resp_command.replace("\r\n", "\\r\\n"));

                if let Err(e) = redis_sender.send(resp_command) {
                    eprintln!("Error al escribir en el socket: {}", e);
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// Maneja respuestas de tipo ASK, redirigiendo comandos a otros nodos si es necesario.
    ///
    /// # Argumentos
    /// * `response` - Respuesta recibida.
    /// * `connect_node_sender` - Canal para enviar streams a nuevos nodos.
    /// * `contenxt` - Contexto de conexión.
    fn handle_ask(
        response: Vec<String>,
        connect_node_sender: MpscSender<TcpStream>,
        contenxt: NodeConnectionContext,
    ) {
        if response.len() < 3 {
            println!("Nodo de redireccion no disponible");
        } else {
            let _ = Self::send_command_to_nodes(
                response,
                connect_node_sender.clone(),
                contenxt.clone(),
            );
        }
    }
    /// Maneja respuestas de tipo STATUS, actualizando la UI con el estado del documento.
    ///
    /// # Argumentos
    /// * `response` - Respuesta recibida.
    /// * `local_addr` - Dirección local del cliente.
    /// * `ui_sender` - Canal para enviar mensajes a la UI.
    fn handle_status(
        response: Vec<String>,
        _local_addr: String,
        ui_sender: Option<UiSender<AppMsg>>,
    ) {
        let doc = response[1].clone();
        let content: String = response[3].clone();
        if let Some(sender) = &ui_sender {
            let mut document = DocumentValueInfo::new(content, 0);
            document.decode_text();
            let _ = sender.send(AppMsg::ManageSubscribeResponse(
                doc.to_string(),
                "1".to_string(),
                document.value.to_string(),
            ));
        }
    }
    /// Maneja respuestas de tipo WRITE, actualizando el contenido del documento en la UI.
    ///
    /// # Argumentos
    /// * `response` - Respuesta recibida.
    /// * `ui_sender` - Canal para enviar mensajes a la UI.
    fn handle_write(response: Vec<String>, ui_sender: Option<UiSender<AppMsg>>) {
        if let Some(sender) = &ui_sender {
            let index = match response[1].parse::<i32>() {
                Ok(i) => i,
                Err(_) => {
                    eprintln!("Error parsing index from response: {:?}", response[1]);
                    return;
                }
            };

            let text = response[2].to_string();
            let file = response[4].to_string();

            let split_text = text.split("<enter>").collect::<Vec<_>>();

            if split_text.len() == 2 {
                let (before_newline, after_newline) = (split_text[0], split_text[1]);

                for (offset, content) in [(0, before_newline), (1, after_newline)] {
                    let mut doc_info = DocumentValueInfo::new(content.to_string(), index + offset);
                    doc_info.file = file.clone();
                    doc_info.decode_text();
                    let _ = sender.send(AppMsg::RefreshData(doc_info));
                }
            } else {
                let mut doc_info = DocumentValueInfo::new(text, index);
                doc_info.file = file.clone();
                doc_info.decode_text();
                let _ = sender.send(AppMsg::RefreshData(doc_info));
            }
        }
    }

    /// Maneja respuestas de tipo LLM-RESPONSE, actualizando el contenido del documento en la UI.
    ///
    /// # Argumentos
    /// * `response` - Respuesta recibida.
    /// * `ui_sender` - Canal para enviar mensajes a la UI.
    fn handle_llm_response(response: Vec<String>, ui_sender: Option<UiSender<AppMsg>>) {
        if let Some(sender) = &ui_sender {
            if response.len() == 3 {
                let content = response[2].to_string();
                let file = response[1].to_string();
                let mut new_lines = Vec::new();
                new_lines.extend(content.split("<enter>").map(String::from));
                let _ = sender.send(AppMsg::UpdateAllFileData(
                    file.to_string(),
                    new_lines.to_vec(),
                ));
            } else {
                let content = response[3].to_string();
                let line_parts: Vec<&str> = response[2].split(':').collect();
                let line = line_parts[1];
                let file = response[1].to_string();
                let _ = sender.send(AppMsg::UpdateLineFile(
                    file.to_string(),
                    line.to_string(),
                    content,
                ));
            }
        }
    }

    /// Maneja respuestas de tipo ERROR, mostrando mensajes de error en la UI.
    ///
    /// # Argumentos
    /// * `response` - Respuesta recibida.
    /// * `ui_sender` - Canal para enviar mensajes a la UI.
    fn handle_error(response: Vec<String>, ui_sender: Option<UiSender<AppMsg>>) {
        let error_message = if response.len() > 1 {
            response[1..].join(" ")
        } else {
            "Error desconocido".to_string()
        };
        if let Some(sender) = &ui_sender {
            let _ = sender.send(AppMsg::Error(format!(
                "Hubo un problema: {}",
                error_message
            )));
        }
    }
    /// Maneja respuestas desconocidas, intentando deducir la acción adecuada.
    ///
    /// # Argumentos
    /// * `response` - Respuesta recibida.
    /// * `ui_sender` - Canal para enviar mensajes a la UI.
    /// * `last_command_sent` - Último comando enviado, para contexto adicional.
    fn handle_unknown(
        response: Vec<String>,
        ui_sender: Option<UiSender<AppMsg>>,
        last_command_sent: Arc<Mutex<String>>,
    ) {
        if let Some(sender) = &ui_sender {
            let _ = sender.send(AppMsg::ManageResponse(response[0].clone()));
        }
        if let Ok(last_command) = last_command_sent.lock() {
            if last_command.to_uppercase().contains("SET") {
                let lines: Vec<&str> = last_command.split("\r\n").collect();
                if lines.len() >= 5 {
                    let file_name = lines[4];
                    if let Some(sender) = &ui_sender {
                        let _ = sender.send(AppMsg::AddFile(file_name.to_string()));
                    }
                }
            }
        }
    }

    /// Escucha y procesa respuestas del servidor Redis en un hilo dedicado.
    ///
    /// # Argumentos
    /// * `client_socket` - Socket TCP hacia el servidor Redis.
    /// * `connect_node_sender` - Canal para enviar streams a nuevos nodos.
    /// * `params` - Contexto de conexión.
    ///
    /// # Errores
    /// Retorna un error si falla la lectura o el procesamiento de respuestas.
    fn listen_to_redis_response(
        client_socket: TcpStream,
        connect_node_sender: MpscSender<TcpStream>,
        params: NodeConnectionContext,
    ) -> std::io::Result<()> {
        let client_socket_cloned = match client_socket.try_clone() {
            Ok(clone) => clone,
            Err(e) => {
                eprintln!("Error al clonar el socket del cliente: {}", e);
                return Err(std::io::Error::other("Socket clone failed"));
            }
        };

        let mut reader: BufReader<TcpStream> = BufReader::new(client_socket);
        let cloned_last_command: Arc<Mutex<String>> = Arc::clone(&params.last_command_sent.clone());

        loop {
            let (response, _) = match resp_parser::parse_resp_command(&mut reader) {
                Ok((parts, s)) => (parts, s),
                Err(e) => {
                    eprintln!("Error al leer línea desde el socket: {}", e);
                    break;
                }
            };

            let params_clone: NodeConnectionContext = params.clone();

            if response.is_empty() {
                break;
            }

            let local_addr = match client_socket_cloned.local_addr() {
                Ok(addr) => addr,
                Err(e) => {
                    eprintln!("Error al obtener la dirección local: {}", e);
                    return Err(e);
                }
            };
            let cloned_ui_sender = params.ui_sender.clone();

            println!("Respuesta de redis: {}", response.join(" "));

            let response_type = RedisClientResponseType::from(response[0].as_str());
            let cloned_connect_node_sender = connect_node_sender.clone();

            match response_type {
                RedisClientResponseType::Ask => {
                    Self::handle_ask(response, cloned_connect_node_sender, params_clone)
                }
                RedisClientResponseType::Status => {
                    Self::handle_status(response, local_addr.to_string(), cloned_ui_sender)
                }
                RedisClientResponseType::Write => Self::handle_write(response, cloned_ui_sender),
                RedisClientResponseType::Llm => {
                    Self::handle_llm_response(response, cloned_ui_sender)
                }
                RedisClientResponseType::Error => Self::handle_error(response, cloned_ui_sender),
                RedisClientResponseType::Other => {
                    Self::handle_unknown(response, cloned_ui_sender, cloned_last_command.clone())
                }
            }
        }
        Ok(())
    }

    /// Envía un comando a uno o varios nodos Redis, gestionando la redirección si es necesario.
    ///
    /// # Argumentos
    /// * `response` - Respuesta recibida que puede indicar redirección.
    /// * `connect_node_sender` - Canal para enviar streams a nuevos nodos.
    /// * `context` - Contexto de conexión.
    ///
    /// # Errores
    /// Retorna un error si falla el envío o la conexión.
    fn send_command_to_nodes(
        response: Vec<String>,
        connect_node_sender: MpscSender<TcpStream>,
        context: NodeConnectionContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Extraigo el último comando enviado
        let last_line_cloned = {
            let locked = context.last_command_sent.lock().map_err(|e| {
                eprintln!("Error locking last_command_sent mutex: {}", e);
                std::io::Error::new(std::io::ErrorKind::Other, "Mutex lock failed")
            })?;
            locked.clone()
        };

        let new_node_address = response
            .get(2)
            .ok_or_else(|| {
                eprintln!("No new node address in response");
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid response")
            })?
            .to_string();

        println!("Last command executed: {:#?}", last_line_cloned);
        println!("Redirecting to node: {}", new_node_address);

        if let Some(sender) = context.writer_registry.get(&new_node_address) {
            println!("Using existing writer for node {}", new_node_address);
            sender.send(last_line_cloned.clone()).map_err(|e| {
                eprintln!("Failed to send command to node {}: {}", new_node_address, e);
                std::io::Error::new(std::io::ErrorKind::Other, "Send failed")
            })?;
        } else {
            println!("Creating new connection for node {}", new_node_address);

            let parts: Vec<&str> = "connect".split_whitespace().collect();
            let resp_command = resp_parser::format_resp_command(&parts);

            let stream = TcpStream::connect(new_node_address.clone()).map_err(|e| {
                eprintln!("Error connecting to new node: {}", e);
                e
            })?;

            let writer_sender = Self::spawn_writer_channel(
                new_node_address.clone(),
                stream.try_clone()?,
                &context.writer_registry.clone(),
            );

            writer_sender.send(resp_command)?;

            connect_node_sender.send(stream.try_clone()?).map_err(|e| {
                eprintln!("Failed to send connected node stream: {}", e);
                std::io::Error::new(std::io::ErrorKind::Other, "Send failed")
            })?;

            std::thread::sleep(std::time::Duration::from_millis(2));

            // Si hay documento en el último comando, vuelvo a hacer subscribe
            if let Some(doc_name) = Self::extract_document_name(&last_line_cloned) {
                let subscribe_command = format!(
                    "*2\r\n$9\r\nsubscribe\r\n${}\r\n{}\r\n",
                    doc_name.len(),
                    doc_name
                );
                writer_sender.send(subscribe_command).map_err(|e| {
                    eprintln!("Error sending subscribe command: {}", e);
                    std::io::Error::new(std::io::ErrorKind::Other, "Send failed")
                })?;
            }

            writer_sender.send(last_line_cloned).map_err(|e| {
                eprintln!("Error resending last command: {}", e);
                std::io::Error::new(std::io::ErrorKind::Other, "Send failed")
            })?;
        }

        Ok(())
    }

    pub fn extract_document_name(resp: &str) -> Option<String> {
        let parts: Vec<&str> = resp.split("\r\n").collect();

        for part in parts.iter().rev() {
            if !part.is_empty() && (part.ends_with(".txt") || part.ends_with(".xlsx")) {
                return Some(part.to_string());
            }
        }

        None
    }
}

impl From<&LocalClient> for NodeConnectionContext {
    fn from(client: &LocalClient) -> Self {
        NodeConnectionContext {
            last_command_sent: Arc::clone(&client.last_command_sent),
            ui_sender: client.ui_sender.clone(),
            writer_registry: client.writer_registry.clone(),
        }
    }
}
