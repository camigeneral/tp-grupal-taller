use super::auth;
use super::pub_sub;
use super::redis_response::RedisResponse;
use super::resp_parser::{CommandRequest, CommandResponse, ValueType};
use super::set;
use super::string;
use ExecuteCommandParams;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use types::*;

pub fn execute_command(
    execute_command_params: ExecuteCommandParams
) -> RedisResponse {

    let docs = execute_command_params.docs;
    let request = execute_command_params.request;
    let document_subscribers = execute_command_params.document_subscribers;
    let active_clients = execute_command_params.active_clients;
    let client_addr = execute_command_params.client_addr;
    let shared_sets = execute_command_params.shared_sets;
    let subscription_channel = execute_command_params.suscription_channel;
    let llm_channel = execute_command_params.llm_channel;
    let logged_clients = execute_command_params.logged_clients;

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
            subscription_channel,
            llm_channel,
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
