use super::redis_parser::{CommandRequest, CommandResponse, ValueType};
use super::redis_response::RedisResponse;
use commands::set::handle_sadd;
use commands::set::handle_srem;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Maneja el comando SUBSCRIBE que permite a un cliente suscribirse a un documento
///
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento al que suscribirse
/// * `document_subscribers` - Un mapa compartido y protegido que asocia documentos con listas de clientes suscritos
/// * `client_addr` - La dirección del cliente que solicita la suscripción
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando, que incluye si la suscripción fue exitosa
pub fn handle_subscribe(
    request: &CommandRequest,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: SUBSCRIBE <document>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    let mut map = match document_subscribers.lock() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error al bloquear document_subscribers: {}", e);
            return RedisResponse::new(
                CommandResponse::Error("Error interno".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    if let Some(list) = map.get_mut(doc) {
        list.push(client_addr.clone());

        let request = CommandRequest {
            command: "sadd".to_string(),
            key: Some(doc.clone()),
            arguments: vec![ValueType::String(client_addr.clone())],
            unparsed_command: "".to_string(),
        };

        let _ = handle_sadd(&request, shared_sets);

        let notification = format!("CLIENT {}|{}", client_addr, doc);
        RedisResponse::new(
            CommandResponse::String(notification.clone()),
            true,
            notification,
            doc.to_string(),
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

/// Maneja el comando UNSUBSCRIBE que permite a un cliente cancelar su suscripción a un documento
///
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento del cual cancelar la suscripción
/// * `document_subscribers` - Un mapa compartido y protegido que asocia documentos con listas de clientes suscritos
/// * `client_addr` - La dirección del cliente que solicita cancelar la suscripción
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando, que incluye si la cancelación de suscripción fue exitosa
pub fn handle_unsubscribe(
    request: &CommandRequest,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: UNSUBSCRIBE <document>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    let mut map = match document_subscribers.lock() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error al bloquear document_subscribers: {}", e);
            return RedisResponse::new(
                CommandResponse::Error("Error interno".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    if let Some(list) = map.get_mut(doc) {
        list.retain(|x| x != &client_addr);

        let request = CommandRequest {
            command: "srem".to_string(),
            key: Some(doc.clone()),
            arguments: vec![ValueType::String(client_addr.clone())],
            unparsed_command: "".to_string(),
        };

        let _ = handle_srem(&request, shared_sets);

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

pub fn handle_publish<T: Write>(
    request: &CommandRequest,
    document_subscribers: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    active_clients: &Arc<Mutex<HashMap<String, T>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: PUBLISH <document> <message>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    if request.arguments.is_empty() {
        return RedisResponse::new(
            CommandResponse::Error("Usage: PUBLISH <document> <message>".to_string()),
            false,
            "".to_string(),
            doc.to_string(),
        );
    }

    let message = match &request.arguments[0] {
        ValueType::String(s) => s.clone(),
        _ => {
            return RedisResponse::new(
                CommandResponse::Error("Tiene que ser un string".to_string()),
                false,
                "".to_string(),
                doc.to_string(),
            )
        }
    };

    let mut sent_count = 0;
    let subscribers_guard = document_subscribers.lock().unwrap();

    let mut clients_guard = active_clients.lock().unwrap();
    if let Some(subscribers) = subscribers_guard.get(doc) {
        for subscriber_id in subscribers {
            if let Some(client) = clients_guard.get_mut(subscriber_id) {
                let _ = writeln!(client, "{}", message);
                sent_count += 1;
            }
        }
    }
    RedisResponse::new(
        CommandResponse::Integer(sent_count),
        false,
        format!("Mensaje enviado a {} suscriptores", sent_count),
        doc.to_string(),
    )
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     fn setup_map(doc: &str, clients: Vec<&str>) -> Arc<Mutex<HashMap<String, Vec<String>>>> {
//         let mut map = HashMap::new();
//         map.insert(
//             doc.to_string(),
//             clients.into_iter().map(|s| s.to_string()).collect(),
//         );
//         Arc::new(Mutex::new(map))
//     }

//     #[test]
//     fn test_handle_subscribe_success() {
//         let doc = "doc1";
//         let document_subscribers = setup_map(doc, vec![]);
//         let request = CommandRequest {
//             command: "SUBSCRIBE".to_string(),
//             key: Some(doc.to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_subscribe(
//             &request,
//             Arc::clone(&document_subscribers),
//             "client1".to_string(),
//         );
//         assert!(matches!(resp.response, CommandResponse::String(_)));
//         let map = document_subscribers.lock().unwrap();
//         assert_eq!(map.get(doc).unwrap(), &vec!["client1".to_string()]);
//     }
/*
    #[test]
    fn test_handle_subscribe_no_key() {
        let document_subscribers = setup_map("doc1", vec![]);
        let shared_sets = Arc::new(Mutex::new(HashMap::new()));
        let request = CommandRequest {
            command: "SUBSCRIBE".to_string(),
            key: None,
            arguments: vec![],
            unparsed_command: String::new(),
        };
        let resp = handle_subscribe(
            &request,
            &document_subscribers,
            "client1".to_string(),
            &shared_sets,
        );
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_subscribe_doc_not_found() {
        let document_subscribers = Arc::new(Mutex::new(HashMap::new()));
        let shared_sets = Arc::new(Mutex::new(HashMap::new()));
        let request = CommandRequest {
            command: "SUBSCRIBE".to_string(),
            key: Some("doc2".to_string()),
            arguments: vec![],
            unparsed_command: String::new(),
        };
        let resp = handle_subscribe(
            &request,
            &document_subscribers,
            "client1".to_string(),
            &shared_sets,
        );
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_unsubscribe_success() {
        let doc = "doc1";
        let document_subscribers = setup_map(doc, vec!["client1", "client2"]);
        let shared_sets = Arc::new(Mutex::new(HashMap::new()));
        let request = CommandRequest {
            command: "UNSUBSCRIBE".to_string(),
            key: Some(doc.to_string()),
            arguments: vec![],
            unparsed_command: String::new(),
        };
        let resp = handle_unsubscribe(
            &request,
            &document_subscribers,
            "client1".to_string(),
            &shared_sets,
        );
        assert!(matches!(resp.response, CommandResponse::String(_)));
        let map = document_subscribers.lock().unwrap();
        assert_eq!(map.get(doc).unwrap(), &vec!["client2".to_string()]);
    }

    #[test]
    fn test_handle_unsubscribe_no_key() {
        let document_subscribers = setup_map("doc1", vec!["client1"]);
        let shared_sets = Arc::new(Mutex::new(HashMap::new()));
        let request = CommandRequest {
            command: "UNSUBSCRIBE".to_string(),
            key: None,
            arguments: vec![],
            unparsed_command: String::new(),
        };
        let resp = handle_unsubscribe(
            &request,
            &document_subscribers,
            "client1".to_string(),
            &shared_sets,
        );
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_unsubscribe_doc_not_found() {
        let document_subscribers = Arc::new(Mutex::new(HashMap::new()));
        let shared_sets = Arc::new(Mutex::new(HashMap::new()));
        let request = CommandRequest {
            command: "UNSUBSCRIBE".to_string(),
            key: Some("doc2".to_string()),
            arguments: vec![],
            unparsed_command: String::new(),
        };
        let resp = handle_unsubscribe(
            &request,
            &document_subscribers,
            "client1".to_string(),
            &shared_sets,
        );
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_unsubscribe_client_not_in_list() {
        let doc = "doc1";
        let document_subscribers = setup_map(doc, vec!["client2"]);
        let shared_sets = Arc::new(Mutex::new(HashMap::new()));
        let request = CommandRequest {
            command: "UNSUBSCRIBE".to_string(),
            key: Some(doc.to_string()),
            arguments: vec![],
            unparsed_command: String::new(),
        };
        let resp = handle_unsubscribe(
            &request,
            &document_subscribers,
            "client1".to_string(),
            &shared_sets,
        );
        assert!(matches!(resp.response, CommandResponse::String(_)));
        let map = document_subscribers.lock().unwrap();
        assert_eq!(map.get(doc).unwrap(), &vec!["client2".to_string()]);
    }

    #[test]
    fn test_handle_publish_envia_a_suscriptores() {
        let doc = "doc1";
        let document_subscribers = setup_map(doc, vec!["client1", "client2"]);
        let active_clients = setup_active_clients(&["client1", "client2"]);
        let request = CommandRequest {
            command: "PUBLISH".to_string(),
            key: Some(doc.to_string()),
            arguments: vec![ValueType::String("hola mundo".to_string())],
            unparsed_command: String::new(),
        };

        let resp: RedisResponse = handle_publish(&request, &document_subscribers, &active_clients);

        assert_eq!(resp.response, CommandResponse::Integer(2));
    }

    #[test]
    fn test_handle_publish_sin_suscriptores() {
        let doc = "doc1";
        let document_subscribers = setup_map(doc, vec![]);
        let active_clients = setup_active_clients(&[]);
        let request = CommandRequest {
            command: "PUBLISH".to_string(),
            key: Some(doc.to_string()),
            arguments: vec![ValueType::String("mensaje".to_string())],
            unparsed_command: String::new(),
        };
        let resp: RedisResponse = handle_publish(&request, &document_subscribers, &active_clients);

        assert_eq!(resp.response, CommandResponse::Integer(0));
    }

    #[test]
    fn test_handle_publish_argumento_invalido() {
        let doc = "doc1";
        let _document_subscribers = setup_map(doc, vec!["client1"]);
        let _active_clients = setup_active_clients(&["client1"]);
        let request = CommandRequest {
            command: "PUBLISH".to_string(),
            key: Some(doc.to_string()),
            arguments: vec![ValueType::Integer(123)],
            unparsed_command: String::new(),
        };
        let is_error = match &request.arguments[0] {
            ValueType::String(_) => false,
            _ => true,
        };
        assert!(is_error);
    }

    #[test]
    fn test_handle_publish_sin_key() {
        let _document_subscribers = setup_map("doc1", vec!["client1"]);
        let _active_clients = setup_active_clients(&["client1"]);
        let request = CommandRequest {
            command: "PUBLISH".to_string(),
            key: None,
            arguments: vec![ValueType::String("mensaje".to_string())],
            unparsed_command: String::new(),
        };
        let is_error = request.key.is_none();
        assert!(is_error);
    }

} */
