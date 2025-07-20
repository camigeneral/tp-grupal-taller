use super::redis;
#[allow(unused_imports)]
use super::resp_parser::{CommandRequest, CommandResponse, ValueType};
use super::redis_response::RedisResponse;
use crate::client_info;
use client_info::ClientType;
use types::RedisDocumentsMap;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub fn handle_get(request: &CommandRequest, docs: &RedisDocumentsMap) -> RedisResponse {
    let key = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Wrong number of arguments for GET".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    let docs_lock = match docs.lock() {
        Ok(d) => d,
        Err(_) => {
            return RedisResponse::new(
                CommandResponse::Error("Internal server error".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    match docs_lock.get(key) {
        Some(data) => RedisResponse::new(
            CommandResponse::String(data.clone()),
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
    docs: &RedisDocumentsMap,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    active_clients: &Arc<Mutex<HashMap<String, client_info::Client>>>,
) -> RedisResponse {
    let doc_name = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Wrong number of arguments for SET".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
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

    // Bloqueo y escritura de documento
    let docs_result = docs.lock();
    if let Ok(mut docs_lock) = docs_result {
        docs_lock.insert(doc_name.clone(), content.clone());
    } else {
        return RedisResponse::new(
            CommandResponse::Error("Internal server error: could not access docs".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        );
    }

    // Intentar bloquear ambos mapas
    let subs_result = document_subscribers.lock();
    let clients_result = active_clients.lock();

    if let (Ok(mut subs_lock), Ok(clients_lock)) = (subs_result, clients_result) {
        let subscribers = subs_lock.entry(doc_name.clone()).or_default();

        for (addr, client) in clients_lock.iter() {
            if client.client_type == ClientType::Microservice && !subscribers.contains(addr) {
                subscribers.push(addr.clone());
                println!(
                    "Microservicio {} suscripto automáticamente a {}",
                    addr, doc_name
                );
                break;
            }
        }
    } else {
        return RedisResponse::new(
            CommandResponse::Error(
                "Internal error accessing client or subscription data".to_string(),
            ),
            false,
            "".to_string(),
            "".to_string(),
        );
    }

    let notification = format!("Document {} was replaced with: {}", doc_name, content);
    println!(
        "Publishing to subscribers of {}: {}",
        doc_name, notification
    );

    RedisResponse::new(CommandResponse::Ok, false, notification, doc_name)
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     fn setup_docs() -> Arc<Mutex<HashMap<String, Vec<String>>>> {
//         Arc::new(Mutex::new(HashMap::new()))
//     }
//     fn setup_clients() -> Arc<Mutex<HashMap<String, Vec<String>>>> {
//         Arc::new(Mutex::new(HashMap::new()))
//     }

//     #[test]
//     fn test_handle_get_existing_key() {
//         let docs = setup_docs();
//         docs.lock().unwrap().insert(
//             "doc1".to_string(),
//             vec!["line1".to_string(), "line2".to_string()],
//         );
//         let req = CommandRequest {
//             command: "GET".to_string(),
//             key: Some("doc1".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_get(&req, docs);
//         let com_resp = CommandResponse::String("line1\nline2".to_string());
//         assert_eq!(resp.response, com_resp);
//     }

//     #[test]
//     fn test_handle_get_missing_key() {
//         let docs = setup_docs();
//         let req = CommandRequest {
//             command: "GET".to_string(),
//             key: Some("missing".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_get(&req, docs);
//         assert_eq!(resp.response, CommandResponse::Null);
//     }

//     #[test]
//     fn test_handle_get_no_key() {
//         let docs = setup_docs();
//         let req = CommandRequest {
//             command: "GET".to_string(),
//             key: None,
//             arguments: vec![],
//         };
//         let resp = handle_get(&req, docs);
//         assert!(matches!(resp.response, CommandResponse::Error(_)));
//     }

//     #[test]
//     fn test_handle_set_success() {
//         let docs = setup_docs();
//         let clients = setup_clients();
//         let req = CommandRequest {
//             command: "SET".to_string(),
//             key: Some("doc2".to_string()),
//             arguments: vec![ValueType::String("hello world".to_string())],
//         };
//         let resp = handle_set(
//             &req,
//             docs.clone(),
//             clients.clone(),
//             Arc::new(Mutex::new(HashMap::new())),
//         );
//         assert_eq!(resp.response, CommandResponse::Ok);
//         let docs_guard = docs.lock().unwrap();
//         assert_eq!(
//             docs_guard.get("doc2").unwrap(),
//             &vec!["hello world".to_string()]
//         );
//         let clients_guard = clients.lock().unwrap();
//         assert!(clients_guard.contains_key("doc2"));
//     }

//     #[test]
//     fn test_handle_set_no_key() {
//         let docs = setup_docs();
//         let clients = setup_clients();
//         let req = CommandRequest {
//             command: "SET".to_string(),
//             key: None,
//             arguments: vec![ValueType::String("something".to_string())],
//         };
//         let resp = handle_set(&req, docs, clients, Arc::new(Mutex::new(HashMap::new())));
//         assert!(matches!(resp.response, CommandResponse::Error(_)));
//     }

//     #[test]
//     fn test_handle_set_no_arguments() {
//         let docs = setup_docs();
//         let clients = setup_clients();
//         let req = CommandRequest {
//             command: "SET".to_string(),
//             key: Some("doc3".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_set(&req, docs, clients, Arc::new(Mutex::new(HashMap::new())));
//         assert!(matches!(resp.response, CommandResponse::Error(_)));
//     }

//     #[test]
//     fn test_handle_append_success() {
//         let docs = setup_docs();
//         docs.lock()
//             .unwrap()
//             .insert("doc4".to_string(), vec!["first".to_string()]);
//         let req = CommandRequest {
//             command: "APPEND".to_string(),
//             key: Some("doc4".to_string()),
//             arguments: vec![ValueType::String("second".to_string())],
//         };
//         let resp = handle_append(&req, docs.clone());
//         assert_eq!(resp.response, CommandResponse::Integer(2));
//         let docs_guard = docs.lock().unwrap();
//         assert_eq!(
//             docs_guard.get("doc4").unwrap(),
//             &vec!["first".to_string(), "second".to_string()]
//         );
//     }

//     #[test]
//     fn test_handle_append_new_doc() {
//         let docs = setup_docs();
//         let req = CommandRequest {
//             command: "APPEND".to_string(),
//             key: Some("doc5".to_string()),
//             arguments: vec![ValueType::String("line".to_string())],
//         };
//         let resp = handle_append(&req, docs.clone());
//         assert_eq!(resp.response, CommandResponse::Integer(1));
//         let docs_guard = docs.lock().unwrap();
//         assert_eq!(docs_guard.get("doc5").unwrap(), &vec!["line".to_string()]);
//     }

//     #[test]
//     fn test_handle_append_no_key() {
//         let docs = setup_docs();
//         let req = CommandRequest {
//             command: "APPEND".to_string(),
//             key: None,
//             arguments: vec![ValueType::String("text".to_string())],
//         };
//         let resp = handle_append(&req, docs);
//         assert!(matches!(resp.response, CommandResponse::Error(_)));
//     }

//     #[test]
//     fn test_handle_append_no_arguments() {
//         let docs = setup_docs();
//         let req = CommandRequest {
//             command: "APPEND".to_string(),
//             key: Some("doc6".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_append(&req, docs);
//         assert!(matches!(resp.response, CommandResponse::Error(_)));
//     }
// }
