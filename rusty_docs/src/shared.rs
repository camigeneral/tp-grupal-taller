use resp_parser::format_resp_command;
use std::fmt;

/// Enum que representa los distintos tipos de mensajes que puede procesar el microservicio.
///
/// Cada variante corresponde a una acción o evento relevante en la comunicación entre
/// clientes, microservicios y nodos Redis.
///
/// - `ClientSubscribed`: Un cliente se suscribió a un documento.
/// - `Doc`: Contenido de un documento recibido.
/// - `Write`: Solicitud de escritura en un documento.
/// - `RequestFile`: Solicitud de un archivo con un prompt asociado.
/// - `ClientLlmResponse`: Respuesta generada por el microservicio LLM.
/// - `Error`: Mensaje de error recibido.
/// - `Unknown`: Mensaje desconocido o no reconocido.
#[derive(Debug)]
pub enum MicroserviceMessage {
    ClientSubscribed {
        document: String,
        client_id: String,
    },
    Doc {
        document: String,
        content: String,
        stream_id: String,
    },
    Write {
        index: String,
        content: String,
        file: String,
    },
    RequestFile {
        document: String,
        prompt: String,
        id_client: String
    },
    ClientLlmResponse {
        document: String,
        content: String,
        selection_mode: String,
        line: String,
        offset: String,
    },
    Error(String),
    Unknown(String),
}

impl MicroserviceMessage {
    /// Construye un mensaje `MicroserviceMessage` a partir de una lista de partes (strings).
    ///
    /// # Argumentos
    /// * `parts` - Slice de strings que representan los campos del mensaje.
    ///
    /// # Retorna
    /// Un valor de tipo `MicroserviceMessage` correspondiente al mensaje recibido.
    pub fn from_parts(parts: &[String]) -> Self {
        if parts.is_empty() {
            return MicroserviceMessage::Unknown("Empty message".to_string());
        }

        match parts[0].to_uppercase().as_str() {
            "CLIENT-SUBSCRIBED" if parts.len() >= 3 => MicroserviceMessage::ClientSubscribed {
                document: parts[1].clone(),
                client_id: parts[2].clone(),
            },
            "DOC" if parts.len() >= 4 => MicroserviceMessage::Doc {
                document: parts[1].clone(),
                content: parts[2].clone(),
                stream_id: parts[3].clone(),
            },
            "WRITE" if parts.len() >= 2 => {
                let index = parts[1].to_string();
                let content = parts[2].to_string();
                let file = parts[4].to_string();
                MicroserviceMessage::Write {
                    index,
                    content,
                    file,
                }
            }
            "CLIENT-LLM-RESPONSE" => MicroserviceMessage::ClientLlmResponse {
                document: parts[1].clone(),
                content: parts[2].clone(),
                selection_mode: parts[3].clone(),
                line: parts[4].clone(),
                offset: parts[5].clone(),
            },
            "MICROSERVICE-REQUEST-FILE" => MicroserviceMessage::RequestFile {
                document: parts[1].clone(),
                prompt: parts[2].clone(),
                id_client: parts[3].clone(),
            },
            cmd if cmd.starts_with("-ERR") => MicroserviceMessage::Error(cmd.to_string()),
            other => MicroserviceMessage::Unknown(other.to_string()),
        }
    }
}

impl fmt::Display for MicroserviceMessage {
    /// Convierte el mensaje en un string en formato RESP para enviar por la red.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let resp = match self {
            MicroserviceMessage::ClientSubscribed {
                document,
                client_id,
            } => format_resp_command(&["client-subscribed", document, client_id]),
            MicroserviceMessage::Doc {
                document,
                content,
                stream_id,
            } => format_resp_command(&["DOC", document, content, stream_id]),
            _ => "".to_string(),
        };
        write!(f, "{}", resp)
    }
}
