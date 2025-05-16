
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use parse::{CommandRequest, CommandResponse, ValueType};
use crate::redis_response::{RedisResponse};
use crate::list_commands;
use crate::set_commands;
use crate::string_commands;
use crate::pub_sub_commands;


pub fn execute_command(
    request: CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> RedisResponse {
    match request.command.as_str() {
        "get" => string_commands::handle_get(&request, docs),
        "set" => string_commands::handle_set(&request, docs, clients_on_docs),
        "subscribe" => pub_sub_commands::handle_subscribe(&request, clients_on_docs, client_addr),
        "unsubscribe" => pub_sub_commands::handle_unsubscribe(&request, clients_on_docs, client_addr),
        "append" => string_commands::handle_append(&request, docs),
        "scard" => set_commands::handle_scard(&request, clients_on_docs),
        "smembers" => set_commands::handle_smembers(&request, clients_on_docs),
        "sscan" => set_commands::handle_sscan(&request, clients_on_docs),
        "llen" => list_commands::handle_llen(&request, docs),
        "rpush" => list_commands::handle_rpush(&request, docs),
        "lset" => list_commands::handle_lset(&request, docs),
        "linsert" => list_commands::handle_linsert(&request, docs),
        _ => RedisResponse::new(
            CommandResponse::Error("Unkown".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
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
