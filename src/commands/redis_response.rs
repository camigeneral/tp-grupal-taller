use super::redis_parser::CommandResponse;

#[derive(Debug)]
pub struct RedisResponse {
    pub response: CommandResponse,
    pub publish: bool,
    pub message: String,
    pub doc: String,
}

impl RedisResponse {
    pub fn new(response: CommandResponse, publish: bool, message: String, doc: String) -> Self {
        RedisResponse {
            response,
            publish,
            message,
            doc,
        }
    }
}
