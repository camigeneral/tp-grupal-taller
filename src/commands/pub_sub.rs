use super::redis_response::RedisResponse;
use parse::{CommandRequest, CommandResponse};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Maneja el comando SUBSCRIBE que permite a un cliente suscribirse a un documento
///
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento al que suscribirse
/// * `clients_on_docs` - Un mapa compartido y protegido que asocia documentos con listas de clientes suscritos
/// * `client_addr` - La dirección del cliente que solicita la suscripción
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando, que incluye si la suscripción fue exitosa
pub fn handle_subscribe(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: SUBSCRIBE <document>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: SUBSCRIBE <document>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
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

/// Maneja el comando UNSUBSCRIBE que permite a un cliente cancelar su suscripción a un documento
///
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento del cual cancelar la suscripción
/// * `clients_on_docs` - Un mapa compartido y protegido que asocia documentos con listas de clientes suscritos
/// * `client_addr` - La dirección del cliente que solicita cancelar la suscripción
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando, que incluye si la cancelación de suscripción fue exitosa
pub fn handle_unsubscribe(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    client_addr: String,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: UNSUBSCRIBE <document>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_map(doc: &str, clients: Vec<&str>) -> Arc<Mutex<HashMap<String, Vec<String>>>> {
        let mut map = HashMap::new();
        map.insert(
            doc.to_string(),
            clients.into_iter().map(|s| s.to_string()).collect(),
        );
        Arc::new(Mutex::new(map))
    }

    #[test]
    fn test_handle_subscribe_success() {
        let doc = "doc1";
        let clients_on_docs = setup_map(doc, vec![]);
        let request = CommandRequest {
            command: "SUBSCRIBE".to_string(),
            key: Some(doc.to_string()),
            arguments: vec![],
        };
        let resp = handle_subscribe(
            &request,
            Arc::clone(&clients_on_docs),
            "client1".to_string(),
        );
        assert!(matches!(resp.response, CommandResponse::String(_)));
        let map = clients_on_docs.lock().unwrap();
        assert_eq!(map.get(doc).unwrap(), &vec!["client1".to_string()]);
    }

    #[test]
    fn test_handle_subscribe_no_key() {
        let clients_on_docs = setup_map("doc1", vec![]);
        let request = CommandRequest {
            command: "SUBSCRIBE".to_string(),
            key: None,
            arguments: vec![],
        };
        let resp = handle_subscribe(
            &request,
            Arc::clone(&clients_on_docs),
            "client1".to_string(),
        );
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_subscribe_doc_not_found() {
        let clients_on_docs = Arc::new(Mutex::new(HashMap::new()));
        let request = CommandRequest {
            command: "SUBSCRIBE".to_string(),
            key: Some("doc2".to_string()),
            arguments: vec![],
        };
        let resp = handle_subscribe(
            &request,
            Arc::clone(&clients_on_docs),
            "client1".to_string(),
        );
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_unsubscribe_success() {
        let doc = "doc1";
        let clients_on_docs = setup_map(doc, vec!["client1", "client2"]);
        let request = CommandRequest {
            command: "UNSUBSCRIBE".to_string(),
            key: Some(doc.to_string()),
            arguments: vec![],
        };
        let resp = handle_unsubscribe(
            &request,
            Arc::clone(&clients_on_docs),
            "client1".to_string(),
        );
        assert!(matches!(resp.response, CommandResponse::String(_)));
        let map = clients_on_docs.lock().unwrap();
        assert_eq!(map.get(doc).unwrap(), &vec!["client2".to_string()]);
    }

    #[test]
    fn test_handle_unsubscribe_no_key() {
        let clients_on_docs = setup_map("doc1", vec!["client1"]);
        let request = CommandRequest {
            command: "UNSUBSCRIBE".to_string(),
            key: None,
            arguments: vec![],
        };
        let resp = handle_unsubscribe(
            &request,
            Arc::clone(&clients_on_docs),
            "client1".to_string(),
        );
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_unsubscribe_doc_not_found() {
        let clients_on_docs = Arc::new(Mutex::new(HashMap::new()));
        let request = CommandRequest {
            command: "UNSUBSCRIBE".to_string(),
            key: Some("doc2".to_string()),
            arguments: vec![],
        };
        let resp = handle_unsubscribe(
            &request,
            Arc::clone(&clients_on_docs),
            "client1".to_string(),
        );
        assert!(matches!(resp.response, CommandResponse::Error(_)));
    }

    #[test]
    fn test_handle_unsubscribe_client_not_in_list() {
        let doc = "doc1";
        let clients_on_docs = setup_map(doc, vec!["client2"]);
        let request = CommandRequest {
            command: "UNSUBSCRIBE".to_string(),
            key: Some(doc.to_string()),
            arguments: vec![],
        };
        let resp = handle_unsubscribe(
            &request,
            Arc::clone(&clients_on_docs),
            "client1".to_string(),
        );
        assert!(matches!(resp.response, CommandResponse::String(_)));
        let map = clients_on_docs.lock().unwrap();
        assert_eq!(map.get(doc).unwrap(), &vec!["client2".to_string()]);
    }
}
