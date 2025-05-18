use crate::redis_response::RedisResponse;
use parse::{CommandRequest, CommandResponse, ValueType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub fn handle_scard(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: SCARD <document>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    let lock_clients_on_docs = clients_on_docs.lock().unwrap();
    if let Some(subscribers) = lock_clients_on_docs.get(doc) {
        RedisResponse::new(
            CommandResponse::String(format!(
                "Number of subscribers in channel {}: {}",
                doc,
                subscribers.len()
            )),
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

pub fn handle_smembers(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: SMEMBERS <document>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
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

pub fn handle_sscan(
    request: &CommandRequest,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: SSCAN <document> [pattern]".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
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
            _ => {
                return RedisResponse::new(
                    CommandResponse::Error("Pattern must be a string".to_string()),
                    false,
                    "".to_string(),
                    "".to_string(),
                )
            }
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
                CommandResponse::String(format!(
                    "No subscribers matching '{}' in document {}",
                    pattern, doc
                )),
                false,
                "".to_string(),
                "".to_string(),
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_clients_on_docs() -> Arc<Mutex<HashMap<String, Vec<String>>>> {
        let mut map = HashMap::new();
        map.insert(
            "doc1".to_string(),
            vec!["alice".to_string(), "bob".to_string(), "carol".to_string()],
        );
        map.insert("doc2".to_string(), vec![]);
        Arc::new(Mutex::new(map))
    }

    #[test]
    fn test_handle_scard_ok() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SCARD".to_string(),
            key: Some("doc1".to_string()),
            arguments: vec![],
        };
        let resp = handle_scard(&req, clients);
        match resp.response {
            CommandResponse::String(s) => {
                assert!(s.contains("Number of subscribers in channel doc1: 3"))
            }
            _ => panic!("Expected String response"),
        }
    }

    #[test]
    fn test_handle_scard_no_key() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SCARD".to_string(),
            key: None,
            arguments: vec![],
        };
        let resp = handle_scard(&req, clients);
        match resp.response {
            CommandResponse::Error(s) => assert!(s.contains("Usage: SCARD")),
            _ => panic!("Expected Error response"),
        }
    }

    #[test]
    fn test_handle_scard_doc_not_found() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SCARD".to_string(),
            key: Some("docX".to_string()),
            arguments: vec![],
        };
        let resp = handle_scard(&req, clients);
        match resp.response {
            CommandResponse::Error(s) => assert!(s.contains("Document not found")),
            _ => panic!("Expected Error response"),
        }
    }

    #[test]
    fn test_handle_smembers_ok() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SMEMBERS".to_string(),
            key: Some("doc1".to_string()),
            arguments: vec![],
        };
        let resp = handle_smembers(&req, clients);
        match resp.response {
            CommandResponse::String(s) => {
                assert!(s.contains("alice"));
                assert!(s.contains("bob"));
                assert!(s.contains("carol"));
            }
            _ => panic!("Expected String response"),
        }
    }

    #[test]
    fn test_handle_smembers_empty() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SMEMBERS".to_string(),
            key: Some("doc2".to_string()),
            arguments: vec![],
        };
        let resp = handle_smembers(&req, clients);
        match resp.response {
            CommandResponse::String(s) => assert!(s.contains("No subscribers in document doc2")),
            _ => panic!("Expected String response"),
        }
    }

    #[test]
    fn test_handle_smembers_no_key() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SMEMBERS".to_string(),
            key: None,
            arguments: vec![],
        };
        let resp = handle_smembers(&req, clients);
        match resp.response {
            CommandResponse::Error(s) => assert!(s.contains("Usage: SMEMBERS")),
            _ => panic!("Expected Error response"),
        }
    }

    #[test]
    fn test_handle_smembers_doc_not_found() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SMEMBERS".to_string(),
            key: Some("docX".to_string()),
            arguments: vec![],
        };
        let resp = handle_smembers(&req, clients);
        match resp.response {
            CommandResponse::Error(s) => assert!(s.contains("Document not found")),
            _ => panic!("Expected Error response"),
        }
    }

    #[test]
    fn test_handle_sscan_pattern_found() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SSCAN".to_string(),
            key: Some("doc1".to_string()),
            arguments: vec![ValueType::String("ali".to_string())],
        };
        let resp = handle_sscan(&req, clients);
        match resp.response {
            CommandResponse::String(s) => assert!(s.contains("alice")),
            _ => panic!("Expected String response"),
        }
    }

    #[test]
    fn test_handle_sscan_pattern_not_found() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SSCAN".to_string(),
            key: Some("doc1".to_string()),
            arguments: vec![ValueType::String("zzz".to_string())],
        };
        let resp = handle_sscan(&req, clients);
        match resp.response {
            CommandResponse::String(s) => assert!(s.contains("No subscribers matching")),
            _ => panic!("Expected String response"),
        }
    }

    #[test]
    fn test_handle_sscan_no_pattern() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SSCAN".to_string(),
            key: Some("doc1".to_string()),
            arguments: vec![],
        };
        let resp = handle_sscan(&req, clients);
        match resp.response {
            CommandResponse::String(s) => {
                assert!(s.contains("alice"));
                assert!(s.contains("bob"));
                assert!(s.contains("carol"));
            }
            _ => panic!("Expected String response"),
        }
    }

    #[test]
    fn test_handle_sscan_pattern_wrong_type() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SSCAN".to_string(),
            key: Some("doc1".to_string()),
            arguments: vec![ValueType::Integer(123)],
        };
        let resp = handle_sscan(&req, clients);
        match resp.response {
            CommandResponse::Error(s) => assert!(s.contains("Expected string pattern")),
            _ => panic!("Expected Error response"),
        }
    }

    #[test]
    fn test_handle_sscan_no_key() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SSCAN".to_string(),
            key: None,
            arguments: vec![],
        };
        let resp = handle_sscan(&req, clients);
        match resp.response {
            CommandResponse::Error(s) => assert!(s.contains("Usage: SSCAN")),
            _ => panic!("Expected Error response"),
        }
    }

    #[test]
    fn test_handle_sscan_doc_not_found() {
        let clients = setup_clients_on_docs();
        let req = CommandRequest {
            command: "SSCAN".to_string(),
            key: Some("docX".to_string()),
            arguments: vec![],
        };
        let resp = handle_sscan(&req, clients);
        match resp.response {
            CommandResponse::Error(s) => assert!(s.contains("Document not found")),
            _ => panic!("Expected Error response"),
        }
    }
}
