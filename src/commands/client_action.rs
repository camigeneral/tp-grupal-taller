use super::redis;
use super::redis_response::RedisResponse;
use crate::client_info;
use crate::commands::set::handle_scard;
#[allow(unused_imports)]
use crate::utils::redis_parser::{CommandRequest, CommandResponse, ValueType};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub fn handle_welcome(request: &CommandRequest, _active_clients: Arc<Mutex<HashMap<String, client_info::Client>>>, shared_sets: Arc<Mutex<HashMap<String, HashSet<String>>>>) -> RedisResponse {
    let client_addr_str = redis::extract_string_arguments(&request.arguments);

    let doc = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: WELCOME <client> <document>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    let request = CommandRequest {
        command: "scard".to_string(),
        key: Some(doc.clone()),
        arguments: vec![],
    };

    let response = handle_scard(&request, shared_sets);

    let mut notification= " ".to_string();

    if let CommandResponse::String(ref s) = response.response {
        if let Some(qty_subs) = s.split_whitespace().last() {
            notification = format!("STATUS {}|{:?}", client_addr_str, qty_subs);
        };
    }
    RedisResponse::new(CommandResponse::String(notification.clone()), true, notification, doc)
}


pub fn set_content_file( request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>) -> RedisResponse {
    // valor, posición. lset(posicion, valor)
    //Written CommandResponse valor|posicion
    let doc = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: LTRIM <key> <start> <stop>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    RedisResponse::new(CommandResponse::Ok, true, "Ok".to_string(), doc)
}


pub fn delete_content_file( request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: LTRIM <key> <start> <stop>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    RedisResponse::new(CommandResponse::Ok, true, "Ok".to_string(), doc)
}

