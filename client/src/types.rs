/// Enum que representa los diferentes tipos de respuesta del cliente Redis.
pub enum RedisClientResponseType {
    Ask,
    Status,
    Write,    
    Error,
    Llm,
    Other,
}

/// Implementación para convertir un &str en un RedisClientResponseType.
impl From<&str> for RedisClientResponseType {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "ASK" => Self::Ask,
            "STATUS" => Self::Status,
            "WRITE" => Self::Write,  
            "LLM-RESPONSE" => Self::Llm,          
            s if s.starts_with("-ERR") => Self::Error,
            _ => Self::Other,
        }
    }
}
