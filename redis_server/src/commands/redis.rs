use super::auth;
use super::pub_sub;
use super::resp_parser::{CommandRequest, CommandResponse, ValueType};
use super::redis_response::RedisResponse;
use super::set;
use super::string;
use types::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub fn execute_command(
    request: CommandRequest,
    docs: &RedisDocumentsMap,
    document_subscribers: &SubscribersMap,
    shared_sets: &SetsMap,
    client_addr: String,
    active_clients: &ClientsMap,
    logged_clients: &LoggedClientsMap,
    suscription_channel: &ClientsMap,
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
        "publish" => pub_sub::handle_publish(
            &request,
            document_subscribers,
            active_clients,
            suscription_channel,
        ),
        "scard" => set::handle_scard(&request, shared_sets),
        "smembers" => set::handle_smembers(&request, shared_sets),
        "sadd" => set::handle_sadd(&request, shared_sets), // subscribe
        "srem" => set::handle_srem(&request, shared_sets), // unsubscribe
        /*         "llen" => list::handle_llen(&request, docs),
        "rpush" => list::handle_rpush(&request, docs),
        "lset" => list::handle_lset(&request, docs),
        "linsert" => list::handle_linsert(&request, docs), */
        "auth" => auth::handle_auth(&request, logged_clients, active_clients, client_addr),
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
    docs: &RedisDocumentsMap,
    document_subscribers: &SubscribersMap,
    shared_sets: &SetsMap,
) -> RedisResponse {
    let shared_map: ClientsMap = Arc::new(Mutex::new(HashMap::new()));
    match request.command.as_str() {
        "get" => string::handle_get(&request, docs),
        "set" => string::handle_set(&request, docs, document_subscribers, &shared_map),
        "sadd" => set::handle_sadd(&request, shared_sets),
        "srem" => set::handle_srem(&request, shared_sets),
        /* "rpush" => list::handle_rpush(&request, docs),
        "lset" => list::handle_lset(&request, docs), */
        /* "linsert" => list::handle_linsert(&request, docs), */
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
