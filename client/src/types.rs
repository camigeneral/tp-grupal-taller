/// Enum que representa los diferentes tipos de respuesta del cliente Redis.
#[derive(Debug)]
pub enum RedisClientResponseType {
    Ask,
    Status,
    Write,
    Error,
    Llm,
    ClientLlm,
    Other,
    Ignore,
}

/// Implementación para convertir un &str en un RedisClientResponseType.
impl  RedisClientResponseType {
    pub fn from_parts(parts: Vec<String>) -> Self {
        if parts.is_empty() {
            return RedisClientResponseType::Error;
        }

        match parts[0].to_uppercase().as_str() {
            "ASK" => Self::Ask,
            "STATUS" => Self::Status,
            "WRITE" => Self::Write,
            "LLM-RESPONSE" => Self::Llm,
            "CLIENT-LLM-RESPONSE" => Self::ClientLlm,
            "LLM-RESPONSE-ERROR" => Self::Error,
            "OK" => Self::Ignore,
            s if s.starts_with("-ERR") => Self::Error,
            _ => Self::Other,
        }
    }
}
