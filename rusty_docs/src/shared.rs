use resp_parser::format_resp_command;

/// Mensajes que procesa el microservicio
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
    },
    
    LLMResponse {
        document: String,
        content: String,
        selection_mode: String,
        line: String,
        offset:String
    },
    Error(String),
    Unknown(String),
}

impl MicroserviceMessage {
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

            "LLM-RESPONSE" => MicroserviceMessage::LLMResponse { document: parts[1].clone(), content: parts[2].clone(), selection_mode: parts[3].clone(), line: parts[4].clone(), offset: parts[5].clone() },

            "MICROSERVICE-REQUEST-FILE" => {
                MicroserviceMessage::RequestFile { document: parts[1].clone(), prompt: parts[2].clone() }
            }

            cmd if cmd.starts_with("-ERR") => MicroserviceMessage::Error(cmd.to_string()),
            other => MicroserviceMessage::Unknown(other.to_string()),
        }
    }
}

impl ToString for MicroserviceMessage {
    fn to_string(&self) -> String {
        match self {
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
        }
    }
}
