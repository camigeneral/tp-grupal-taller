use super::redis;
use documento::Documento;
use super::redis_response::RedisResponse;
use crate::client_info;
use crate::commands::set::handle_scard;
#[allow(unused_imports)]
use crate::utils::redis_parser::{CommandRequest, CommandResponse, ValueType};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub fn handle_welcome(request: &CommandRequest, _active_clients: &Arc<Mutex<HashMap<String, client_info::Client>>>, shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>) -> RedisResponse {
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
        unparsed_command: format!("scard {}", doc.clone())
    };

    let response = handle_scard(&request, shared_sets);

    let mut notification= " ".to_string();

    if let CommandResponse::String(ref s) = response.response {
        if let Some(qty_subs) = s.split_whitespace().last() {
            notification = format!("status {}|{:?}", client_addr_str, qty_subs);
        };
    }
    RedisResponse::new(CommandResponse::String(notification.clone()), true, notification, doc)
}


pub fn set_content_file(
    request: &CommandRequest,
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Falta el nombre del documento".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    if request.arguments.len() < 2 {
        return RedisResponse::new(
            CommandResponse::Error("Faltan argumentos: índice y carácter".to_string()),
            false,
            "".to_string(),
            doc,
        );
    }
    println!("Arguments: {:#?}",request.arguments);
    let index = match &request.arguments[0] {
        ValueType::Integer(i) => *i as usize,
        ValueType::String(s) => match s.parse::<usize>() {
            Ok(i) => i,
            _ => {
                return RedisResponse::new(
                    CommandResponse::Error("El índice debe ser un entero".to_string()),
                    false,
                    "".to_string(),
                    doc,
                )
            }
        },
        _ => {
            return RedisResponse::new(
                CommandResponse::Error("El índice debe ser un entero".to_string()),
                false,
                "".to_string(),
                doc,
            )
        }
    };

    let caracter = match &request.arguments[1] {
        ValueType::String(s) => {
            match s.as_str() {
                "<space>" =>  " ".to_string(),
                "<enter>" =>  "\n".to_string(),
                "<tab>" =>  "\t".to_string(),
                _ =>  s.clone()
            }
        },
        _ => {
            return RedisResponse::new(
                CommandResponse::Error("El carácter debe ser un string".to_string()),
                false,
                "".to_string(),
                doc,
            )
        }
    };

    let mut docs_lock = docs.lock().unwrap();
    let documento = docs_lock.entry(doc.clone()).or_default();

    match documento {
        Documento::Texto(ref mut vec) => {
            println!("vec: {:#?}", vec);
            if index > vec.len() {
                vec.push(caracter);
            } else {

                
                vec.insert(index, caracter);
            }
            
            RedisResponse::new(CommandResponse::String(format!("update-files {}", doc)), true, "Carácter insertado".to_string(), doc)
        }
        _ => RedisResponse::new(
            CommandResponse::Error("El documento no es de texto".to_string()),
            false,
            "".to_string(),
            doc,
        ),
    }
}


pub fn delete_content_file( request: &CommandRequest,
         _docs: &Arc<Mutex<HashMap<String, Documento>>>,
) 
    -> RedisResponse {
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

