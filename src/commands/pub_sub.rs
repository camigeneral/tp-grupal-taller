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
