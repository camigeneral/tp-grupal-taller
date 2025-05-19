use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use parse::{CommandRequest, CommandResponse, ValueType};
use super::redis_response::{RedisResponse};

/// Maneja el comando SCARD que devuelve el número de suscriptores en un documento
/// 
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento a consultar
/// * `clients_on_docs` - Un mapa compartido y protegido que asocia documentos con listas de clientes suscritos
/// 
/// # Retorno
/// * `RedisResponse` - La respuesta al comando con el número de suscriptores en el documento
pub fn handle_scard(
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

/// Maneja el comando SMEMBERS que lista todos los suscriptores de un documento
/// 
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento a consultar
/// * `clients_on_docs` - Un mapa compartido y protegido que asocia documentos con listas de clientes suscritos
/// 
/// # Retorno
/// * `RedisResponse` - La respuesta al comando con la lista de suscriptores del documento
pub fn handle_smembers(
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


/// Maneja el comando SSCAN que busca suscriptores en un documento que coincidan con un patrón
/// 
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento a consultar y opcionalmente un patrón de búsqueda
/// * `clients_on_docs` - Un mapa compartido y protegido que asocia documentos con listas de clientes suscritos
/// 
/// # Retorno
/// * `RedisResponse` - La respuesta al comando con los suscriptores que coinciden con el patrón
pub fn handle_sscan(
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
