
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use parse::{CommandRequest, CommandResponse, ValueType};
use crate::redis_response::{RedisResponse};
use crate::client_info::Client;


pub fn execute_command(
    request: CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    clients: Arc<Mutex<HashMap<String, Client>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> RedisResponse {
    match request.command.as_str() {
        "get" => handle_get(&request, docs),
        "set" => handle_set(&request, docs, clients, clients_on_docs),
        "subscribe" => handle_subscribe(&request, clients_on_docs, client_addr),
        "unsubscribe" => handle_unsubscribe(&request, clients_on_docs, client_addr),
        "append" => handle_append(&request, docs, clients, clients_on_docs),
        "scard" => handle_scard(&request, clients_on_docs),
        "smembers" => handle_smembers(&request, clients_on_docs),
        "sscan" => handle_sscan(&request, clients_on_docs),
        _ => RedisResponse::new(
            CommandResponse::Error("Unkown".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
    }
}


fn handle_get(
    request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> RedisResponse {
    let key = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Wrong number of arguments for GET".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        },
    };

    let docs = docs.lock().unwrap();
    match docs.get(key) {
        Some(value) => RedisResponse::new(
            CommandResponse::String(value.join("\n")),
            false,
            "".to_string(),
            "".to_string(),
        ),
        None => RedisResponse::new(
            CommandResponse::Null,
            false,
            "".to_string(),
            "".to_string(),
        ),
    }

}


fn handle_set(
    request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    clients: Arc<Mutex<HashMap<String, Client>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> RedisResponse {
    let doc_name = match &request.key {
        Some(k) => k.clone(),
        None => return RedisResponse::new(
            CommandResponse::Error("Wrong number of arguments for SET".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    };

    if request.arguments.is_empty() {
        return RedisResponse::new(
            CommandResponse::Error("Wrong number of arguments for SET".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        );
    }

    let content = extract_string_arguments(&request.arguments);

    {
        let mut docs_lock = docs.lock().unwrap();
        docs_lock.insert(doc_name.clone(), vec![content.clone()]);

        let mut clients_on_docs_lock = clients_on_docs.lock().unwrap();
        if !clients_on_docs_lock.contains_key(&doc_name) {
            clients_on_docs_lock.insert(doc_name.clone(), Vec::new());
        }
    }

    let notification = format!("Document {} was replaced with: {}", doc_name, content);
    println!(
        "Publishing to subscribers of {}: {}",
        doc_name, notification
    );

    RedisResponse::new(
        CommandResponse::Ok,
        true,
        notification,
        doc_name,
    )
}


fn handle_subscribe(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => return RedisResponse::new(
            CommandResponse::Error("Usage: SUBSCRIBE <document>".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    };

    let mut map = clients_on_docs.lock().unwrap();
    if let Some(list) = map.get_mut(doc) {
        list.push(client_addr);
        RedisResponse::new(
            CommandResponse::String(format!("Subscribed to {}", doc)),
            false,
            "".to_string(),
            "".to_string(),
        )
    } else {
        RedisResponse::new(
            CommandResponse::Error("Document not found".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
    }
}


fn handle_unsubscribe(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => return RedisResponse::new(
            CommandResponse::Error("Usage: UNSUBSCRIBE <document>".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    };

    let mut map = clients_on_docs.lock().unwrap();
    if let Some(list) = map.get_mut(doc) {
        list.retain(|x| x != &client_addr);
        RedisResponse::new(
            CommandResponse::String(format!("Unsubscribed from {}", doc)),
            false,
            "".to_string(),
            "".to_string(),
        )
    } else {
        RedisResponse::new(
            CommandResponse::Error("Document not found".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
    }
}


fn handle_append(
    request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    clients: Arc<Mutex<HashMap<String, Client>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k.clone(),
        None => return RedisResponse::new(
            CommandResponse::Error("Usage: APPEND <document> <text...>".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    };

    if request.arguments.is_empty() {
        return RedisResponse::new(
            CommandResponse::Error("Usage: APPEND <document> <text...>".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
        ;
    }

    let content = extract_string_arguments(&request.arguments);
    let line_number;

    {
        let mut docs_lock = docs.lock().unwrap();
        let entry = docs_lock.entry(doc.clone()).or_default();
        entry.push(content.clone());
        line_number = entry.len();
    }

    let notification = format!("New content in {}: {}", doc, content);
    println!("Publishing to subscribers of {}: {}", doc, notification);

    RedisResponse::new(
        CommandResponse::Integer(line_number as i64),
        true,
        notification,
        doc,
    )
}


fn handle_scard(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => return RedisResponse::new(
            CommandResponse::Error("Usage: SCARD <document>".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    };

    let lock_clients_on_docs = clients_on_docs.lock().unwrap();
    if let Some(subscribers) = lock_clients_on_docs.get(doc) {
        RedisResponse::new(
            CommandResponse::String(format!("Number of subscribers in channel {}: {}", doc, subscribers.len())),
            false,
            "".to_string(),
            "".to_string(),
        )        
    } else {
        RedisResponse::new(
            CommandResponse::Error("Document not found".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
    }
}


fn handle_smembers(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => return RedisResponse::new(
            CommandResponse::Error("Usage: SMEMBERS <document>".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    };

    let lock_clients_on_docs = clients_on_docs.lock().unwrap();
    if let Some(subscribers) = lock_clients_on_docs.get(doc) {
        if subscribers.is_empty() {
            return RedisResponse::new(
                CommandResponse::String(format!("No subscribers in document {}", doc)),
                false,
                "".to_string(),
                "".to_string(),
            );
        }

        let mut response = format!("Subscribers in document {}:\n", doc);
        for subscriber in subscribers {
            response.push_str(&format!("{}\n", subscriber));
        }
        RedisResponse::new(
            CommandResponse::String(response),
            false,
            "".to_string(),
            "".to_string(),
        )
    } else {
        RedisResponse::new(
            CommandResponse::Error("Document not found".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
    }
}


fn handle_sscan(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => return RedisResponse::new(
            CommandResponse::Error("Usage: SSCAN <document> [pattern]".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    };

    let pattern = if !request.arguments.is_empty() {
        match &request.arguments[0] {
            ValueType::String(s) => s,
            ValueType::Integer(i) => {
                return RedisResponse::new(
                    CommandResponse::Error(format!("Expected string pattern, got integer: {}", i)),
                    false,
                    "".to_string(),
                    "".to_string(),
                )
            }
            _ => return RedisResponse::new(
                CommandResponse::Error("Pattern must be a string".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            ),
        }
    } else {
        ""
    };

    let lock_clients_on_docs = clients_on_docs.lock().unwrap();
    if let Some(subscribers) = lock_clients_on_docs.get(doc) {
        let matching_subscribers: Vec<&String> =
            subscribers.iter().filter(|s| s.contains(pattern)).collect();

        if matching_subscribers.is_empty() {
            return RedisResponse::new(
                CommandResponse::String(format!("No subscribers matching '{}' in document {}", pattern, doc)),
                false,
                "".to_string(),
                "".to_string(),
            )
        }

        let mut response = format!("Subscribers in {} matching '{}':\n", doc, pattern);
        for subscriber in matching_subscribers {
            response.push_str(&format!("{}\n", subscriber));
        }
        RedisResponse::new(
            CommandResponse::String(response),
            false,
            "".to_string(),
            "".to_string(),
        )

    } else {
        RedisResponse::new(
            CommandResponse::Error("Document not found".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
    }
}


fn extract_string_arguments(arguments: &[ValueType]) -> String {
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


