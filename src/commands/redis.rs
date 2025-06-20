use super::list;
use super::pub_sub;
use super::redis_response::RedisResponse;
use super::set;
use super::string;
use super::auth;
use crate::client_info;
use crate::utils::redis_parser::{CommandRequest, CommandResponse, ValueType};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub fn execute_command(
    request: CommandRequest,
    docs: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
    client_addr: String,
    active_clients: &Arc<Mutex<HashMap<String, client_info::Client>>>,
    logged_clients: &Arc<Mutex<HashMap<String, bool>>>
) -> RedisResponse {
    match request.command.as_str() {
        "get" => string::handle_get(&request, docs),
        "set" => string::handle_set(&request, docs, document_subscribers, active_clients),
        "subscribe" => pub_sub::handle_subscribe(&request, document_subscribers, client_addr, shared_sets),
        "unsubscribe" => pub_sub::handle_unsubscribe(&request, document_subscribers, client_addr,shared_sets),
        "append" => string::handle_append(&request, docs),
        "scard" => set::handle_scard(&request, shared_sets),
        "smembers" => set::handle_smembers(&request, shared_sets),
        // "sscan" => set::handle_sscan(&request, shared_sets),
        "sadd" => set::handle_sadd(&request, shared_sets), // subscribe
        "srem" => set::handle_srem(&request, shared_sets), // unsubscribe
        "llen" => list::handle_llen(&request, docs),
        "rpush" => list::handle_rpush(&request, docs),
        "lset" => list::handle_lset(&request, docs),
        "linsert" => list::handle_linsert(&request, docs),
        "auth" => auth::handle_auth(&request, logged_clients, active_clients, client_addr),
        "welcome" => string::handle_welcome(&request, active_clients, shared_sets),
        _ => RedisResponse::new(
            CommandResponse::Error("Unknown".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    }
}


#[allow(unused_variables)]
pub fn execute_replica_command(
    request: CommandRequest,
    docs: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> RedisResponse {
    match request.command.as_str() {
        "get" => string::handle_get(&request, docs),
        // "set" => string::handle_set(&request, docs, document_subscribers), // to do: arreglar
        "append" => string::handle_append(&request, docs),
        "sadd" => set::handle_sadd(&request, shared_sets),
        "srem" => set::handle_srem(&request, shared_sets),
        "rpush" => list::handle_rpush(&request, docs),
        "lset" => list::handle_lset(&request, docs),
        "linsert" => list::handle_linsert(&request, docs),
        _ => RedisResponse::new(
            CommandResponse::Error("Unknown".to_string()),
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
