use super::redis;
use super::redis_response::RedisResponse;
use crate::client_info;
use crate::commands::set::handle_scard;
#[allow(unused_imports)]
use crate::utils::redis_parser::{CommandRequest, CommandResponse, ValueType};
use client_info::ClientType;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub fn handle_get(
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
        }
    };

    let docs = docs.lock().unwrap();
    match docs.get(key) {
        Some(value) => RedisResponse::new(
            CommandResponse::String(value.join("\n")),
            false,
            "".to_string(),
            "".to_string(),
        ),
        None => RedisResponse::new(CommandResponse::Null, false, "".to_string(), "".to_string()),
    }
}

/// Maneja el comando SET para sobrescribir el contenido de un documento.
///
/// - Si no se especifica documento o contenido, devuelve un error.
/// - Si el documento existe, lo sobreescribe.
/// - Si no existe, lo crea.
/// - Registra el documento en el mapa de `document_subscribers` para futuras suscripciones.
/// - Publica una notificación para los clientes suscritos.
///
/// # Parámetros
/// - `request`: contiene el documento y los argumentos (contenido).
/// - `docs`: referencia a la base de documentos compartida.
/// - `document_subscribers`: referencia a la tabla de suscriptores.
///
/// # Retorna
/// - `RedisResponse::Ok` con notificación activa y nombre del documento.
pub fn handle_set(
    request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    document_subscribers: Arc<Mutex<HashMap<String, Vec<String>>>>,
    active_clients: Arc<Mutex<HashMap<String, client_info::Client>>>,
) -> RedisResponse {
    let doc_name = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Wrong number of arguments for SET".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    if request.arguments.is_empty() {
        return RedisResponse::new(
            CommandResponse::Error("Wrong number of arguments for SET".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        );
    }

    let content = redis::extract_string_arguments(&request.arguments);

    {
        let mut docs_lock = docs.lock().unwrap();
        docs_lock.insert(doc_name.clone(), vec![content.clone()]);
    }

    {
        let mut document_subscribers_lock = document_subscribers.lock().unwrap();
        let active_clients_lock = active_clients.lock().unwrap();

        let subscribers = document_subscribers_lock
            .entry(doc_name.clone())
            .or_insert_with(Vec::new);

        for (addr, client) in active_clients_lock.iter() {
            if client.client_type == ClientType::Microservicio && !subscribers.contains(addr) {
                subscribers.push(addr.clone());
                println!(
                    "Microservicio {} suscripto automáticamente a {}",
                    addr, doc_name
                );
                break;
            }
        }
    }

    let notification = format!("Document {} was replaced with: {}", doc_name, content);
    println!(
        "Publishing to subscribers of {}: {}",
        doc_name, notification
    );

    RedisResponse::new(CommandResponse::Ok, true, notification, doc_name)
}

/// Maneja el comando APPEND para agregar contenido a un documento línea por línea.
///
/// - Si no se especifica documento o contenido, devuelve un error.
/// - Si el documento no existe, lo crea automáticamente.
/// - Agrega una nueva línea de texto al final del documento.
/// - Retorna el número de línea donde se agregó el contenido.
/// - Publica una notificación para los clientes suscritos.
///
/// # Parámetros
/// - `request`: contiene la clave del documento y el contenido a agregar.
/// - `docs`: acceso a los documentos en memoria compartida.
///
/// # Retorna
/// - `RedisResponse::Integer(line_number)` con notificación activa y nombre del documento.
pub fn handle_append(
    request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: APPEND <document> <text...>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    if request.arguments.is_empty() {
        return RedisResponse::new(
            CommandResponse::Error("Usage: APPEND <document> <text...>".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        );
    }

    let content = redis::extract_string_arguments(&request.arguments);
    let line_number;

    {
        let mut docs_lock = docs.lock().unwrap();
        let entry = docs_lock.entry(doc.clone()).or_default();
        entry.push(content.clone());
        line_number = entry.len();
    }

    // let notification = format!("New content in {}: {} L{}", doc, content, line_number);
    let notification = format!("L{}: {} ", line_number, content);
    println!("Publishing to subscribers of {}: {}", doc, notification);

    RedisResponse::new(
        CommandResponse::Integer(line_number as i64),
        true,
        notification,
        doc,
    )
}

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
#[cfg(test)]
mod tests {
    use super::*;
    fn setup_docs() -> Arc<Mutex<HashMap<String, Vec<String>>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }
    fn setup_clients() -> Arc<Mutex<HashMap<String, Vec<String>>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    #[test]
    fn test_handle_get_existing_key() {
        let docs = setup_docs();
        docs.lock().unwrap().insert(
            "doc1".to_string(),
            vec!["line1".to_string(), "line2".to_string()],
        );
        let req = CommandRequest {
            command: "GET".to_string(),
            key: Some("doc1".to_string()),
            arguments: vec![],
        };
        let resp = handle_get(&req, docs);
        let com_resp = CommandResponse::String("line1\nline2".to_string());
        assert_eq!(resp.response, com_resp);
    }

    #[test]
    fn test_handle_get_missing_key() {
        let docs = setup_docs();
        let req = CommandRequest {
            command: "GET".to_string(),
            key: Some("missing".to_string()),
            arguments: vec![],
        };
        let resp = handle_get(&req, docs);
        assert_eq!(resp.response, CommandResponse::Null);
    }

    #[test]
    fn test_handle_get_no_key() {
        let docs = setup_docs();
        let req = CommandRequest {
            command: "GET".to_string(),
            key: None,
            arguments: vec![],
        };
        let resp = handle_get(&req, docs);
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_set_success() {
        let docs = setup_docs();
        let clients = setup_clients();
        let req = CommandRequest {
            command: "SET".to_string(),
            key: Some("doc2".to_string()),
            arguments: vec![ValueType::String("hello world".to_string())],
        };
        let resp = handle_set(
            &req,
            docs.clone(),
            clients.clone(),
            Arc::new(Mutex::new(HashMap::new())),
        );
        assert_eq!(resp.response, CommandResponse::Ok);
        let docs_guard = docs.lock().unwrap();
        assert_eq!(
            docs_guard.get("doc2").unwrap(),
            &vec!["hello world".to_string()]
        );
        let clients_guard = clients.lock().unwrap();
        assert!(clients_guard.contains_key("doc2"));
    }

    #[test]
    fn test_handle_set_no_key() {
        let docs = setup_docs();
        let clients = setup_clients();
        let req = CommandRequest {
            command: "SET".to_string(),
            key: None,
            arguments: vec![ValueType::String("something".to_string())],
        };
        let resp = handle_set(&req, docs, clients, Arc::new(Mutex::new(HashMap::new())));
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_set_no_arguments() {
        let docs = setup_docs();
        let clients = setup_clients();
        let req = CommandRequest {
            command: "SET".to_string(),
            key: Some("doc3".to_string()),
            arguments: vec![],
        };
        let resp = handle_set(&req, docs, clients, Arc::new(Mutex::new(HashMap::new())));
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_append_success() {
        let docs = setup_docs();
        docs.lock()
            .unwrap()
            .insert("doc4".to_string(), vec!["first".to_string()]);
        let req = CommandRequest {
            command: "APPEND".to_string(),
            key: Some("doc4".to_string()),
            arguments: vec![ValueType::String("second".to_string())],
        };
        let resp = handle_append(&req, docs.clone());
        assert_eq!(resp.response, CommandResponse::Integer(2));
        let docs_guard = docs.lock().unwrap();
        assert_eq!(
            docs_guard.get("doc4").unwrap(),
            &vec!["first".to_string(), "second".to_string()]
        );
    }

    #[test]
    fn test_handle_append_new_doc() {
        let docs = setup_docs();
        let req = CommandRequest {
            command: "APPEND".to_string(),
            key: Some("doc5".to_string()),
            arguments: vec![ValueType::String("line".to_string())],
        };
        let resp = handle_append(&req, docs.clone());
        assert_eq!(resp.response, CommandResponse::Integer(1));
        let docs_guard = docs.lock().unwrap();
        assert_eq!(docs_guard.get("doc5").unwrap(), &vec!["line".to_string()]);
    }

    #[test]
    fn test_handle_append_no_key() {
        let docs = setup_docs();
        let req = CommandRequest {
            command: "APPEND".to_string(),
            key: None,
            arguments: vec![ValueType::String("text".to_string())],
        };
        let resp = handle_append(&req, docs);
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_append_no_arguments() {
        let docs = setup_docs();
        let req = CommandRequest {
            command: "APPEND".to_string(),
            key: Some("doc6".to_string()),
            arguments: vec![],
        };
        let resp = handle_append(&req, docs);
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }
}
