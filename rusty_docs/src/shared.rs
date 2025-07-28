use resp_parser;

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
    Prompt {
        line: String,
        offset: String,
        prompt: String,
        file: String,
        selection_mode: String,
    },

    PromptResponse {
        line: String,
        file: String,
        response: String,
        selection_mode: String,
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
            "LLM-RESPONSE" if parts.len() >= 2 => {
                if parts.len() == 3 {
                    let response = parts[2].to_string();
                    let file = parts[1].to_string();
                    return MicroserviceMessage::PromptResponse {
                        line: "0".to_string(),
                        file,
                        response,
                        selection_mode: "whole-file".to_string(),
                    };
                } else {
                    let response = parts[3].to_string();
                    let line_parts: Vec<&str> = parts[2].split(':').collect();
                    let file = parts[1].to_string();
                    return MicroserviceMessage::PromptResponse {
                        line: line_parts[1].to_string(),
                        file,
                        response,
                        selection_mode: "cursor".to_string(),
                    };
                }
            }
            "PROMPT" if parts.len() >= 3 => {
                let line = parts[1].to_string();
                let file = parts[2].to_string();
                let prompt = parts[3].to_string();
                let offset = parts[4].to_string();
                let selection_mode = parts[5].to_string();
                MicroserviceMessage::Prompt {
                    line,
                    offset,
                    prompt,
                    file,
                    selection_mode,
                }
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
            } => resp_parser::format_resp_command(&["client-subscribed", document, client_id]),
            MicroserviceMessage::Doc {
                document,
                content,
                stream_id,
            } => resp_parser::format_resp_command(&["DOC", document, content, stream_id]),
            _ => "".to_string(),
        }
    }
}
