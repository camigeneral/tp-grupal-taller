use super::list;
use super::pub_sub;
use super::redis_response::RedisResponse;
use super::set;
use super::string;
use crate::utils::redis_parser::{CommandRequest, CommandResponse, ValueType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub fn execute_command(
    request: CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> RedisResponse {
    match request.command.as_str() {
        "get" => string::handle_get(&request, docs),
        "set" => string::handle_set(&request, docs, clients_on_docs),
        "subscribe" => pub_sub::handle_subscribe(&request, clients_on_docs, client_addr),
        "unsubscribe" => pub_sub::handle_unsubscribe(&request, clients_on_docs, client_addr),
        "append" => string::handle_append(&request, docs),
        "scard" => set::handle_scard(&request, clients_on_docs),
        "smembers" => set::handle_smembers(&request, clients_on_docs),
        "sscan" => set::handle_sscan(&request, clients_on_docs),
        "llen" => list::handle_llen(&request, docs),
        "rpush" => list::handle_rpush(&request, docs),
        "lset" => list::handle_lset(&request, docs),
        "linsert" => list::handle_linsert(&request, docs),
        _ => RedisResponse::new(
            CommandResponse::Error("Unkown".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    }
}

pub fn extract_string_arguments(arguments: &[ValueType]) -> String {
    arguments
        .iter()
        .filter_map(|arg| {
            if let ValueType::String(s) = arg {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
