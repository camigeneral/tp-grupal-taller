use super::auth;
use super::client_action;
use super::list;
use super::pub_sub;
use super::redis_parser::{CommandRequest, CommandResponse, ValueType};
use super::redis_response::RedisResponse;
use super::set;
use super::string;
use crate::client_info;
use crate::documento::Documento;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub fn execute_command(
    request: CommandRequest,
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
    client_addr: String,
    active_clients: &Arc<Mutex<HashMap<String, client_info::Client>>>,
    logged_clients: &Arc<Mutex<HashMap<String, bool>>>,
) -> RedisResponse {
    match request.command.as_str() {
        "get" => string::handle_get(&request, docs),
        "set" => string::handle_set(&request, docs, document_subscribers, active_clients),
        "subscribe" => {
            pub_sub::handle_subscribe(&request, document_subscribers, client_addr, shared_sets)
        }
        "unsubscribe" => {
            pub_sub::handle_unsubscribe(&request, document_subscribers, client_addr, shared_sets)
        }
        "publish" => pub_sub::handle_publish(&request, document_subscribers, active_clients),
        "append" => string::handle_append(&request, docs),
        "scard" => set::handle_scard(&request, shared_sets),
        "smembers" => set::handle_smembers(&request, shared_sets),
        "sadd" => set::handle_sadd(&request, shared_sets), // subscribe
        "srem" => set::handle_srem(&request, shared_sets), // unsubscribe
        "llen" => list::handle_llen(&request, docs),
        "rpush" => list::handle_rpush(&request, docs),
        "lset" => list::handle_lset(&request, docs),
        "linsert" => list::handle_linsert(&request, docs),
        "auth" => auth::handle_auth(&request, logged_clients, active_clients, client_addr),
        "add_content" => client_action::set_content_file(&request, docs),
        "welcome" => client_action::handle_welcome(&request, active_clients, shared_sets, docs),
        "list_files" => string::handle_list_files(),
        "get_files" => client_action::get_files(docs),
        _ => RedisResponse::new(
            CommandResponse::Error("Unknown".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    }
}

pub fn execute_replica_command(
    request: CommandRequest,
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> RedisResponse {
    let shared_map: Arc<Mutex<HashMap<String, client_info::Client>>> =
        Arc::new(Mutex::new(HashMap::new()));
    match request.command.as_str() {
        "get" => string::handle_get(&request, docs),
        "set" => string::handle_set(&request, docs, document_subscribers, &shared_map),
        "append" => string::handle_append(&request, docs),
        "sadd" => set::handle_sadd(&request, shared_sets),
        "srem" => set::handle_srem(&request, shared_sets),
        "rpush" => list::handle_rpush(&request, docs),
        "lset" => list::handle_lset(&request, docs),
        "get_files" => client_action::get_files(docs),
        "add_content" => client_action::set_content_file(&request, docs),
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
