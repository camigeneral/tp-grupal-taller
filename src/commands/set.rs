use super::redis_response::RedisResponse;
use super::redis_parser::{CommandRequest, CommandResponse, ValueType};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Maneja el comando SCARD que devuelve el número de suscriptores en un set
///
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el set a consultar
/// * `shared_sets` - Un mapa compartido y protegido que asocia sets con listas de clientes suscritos
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando con el número de suscriptores en el set
pub fn handle_scard(
    request: &CommandRequest,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> RedisResponse {
    // Validar que se haya pasado una clave
    let set_key = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Uso: SCARD <clave_del_set>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    // Intentar obtener acceso exclusivo a los sets compartidos
    let lock_shared_sets = match shared_sets.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return RedisResponse::new(
                CommandResponse::Error("No se pudo acceder al conjunto compartido".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    // Verificar si existe el set y obtener su tamaño
    match lock_shared_sets.get(set_key) {
        Some(subs) => RedisResponse::new(
            CommandResponse::String(format!(
                "Cantidad de elementos en '{}': {}",
                set_key,
                subs.len()
            )),
            false,
            "".to_string(),
            "".to_string(),
        ),
        None => RedisResponse::new(
            CommandResponse::Error(format!("No existe el set '{}'", set_key)),
            false,
            "".to_string(),
            "".to_string(),
        ),
    }
}

/// Maneja el comando SMEMBERS que lista todos los suscriptores de un set
///
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el set a consultar
/// * `shared_sets` - Un mapa compartido y protegido que asocia sets con listas de clientes suscritos
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando con la lista de suscriptores del set
pub fn handle_smembers(
    request: &CommandRequest,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> RedisResponse {
    let key = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Uso: SMEMBERS <key>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    let sets = match shared_sets.lock() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error al bloquear shared_sets: {}", e);
            return RedisResponse::new(
                CommandResponse::Error("Error interno al acceder a conjuntos".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    match sets.get(key) {
        Some(set) => {
            let members = set.iter().cloned().collect::<Vec<String>>().join(", ");
            RedisResponse::new(
                CommandResponse::String(format!("Miembros: {}", members)),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
        None => RedisResponse::new(
            CommandResponse::Error("Set no encontrado".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    }
}


/// Maneja el comando SREM que elimina uno o más elementos de un conjunto.
///
/// # Argumentos
/// * `request` - La solicitud del comando que contiene la clave del conjunto y los elementos a eliminar.
/// * `shared_sets` - Un mapa compartido y protegido que asocia claves con conjuntos de elementos (`HashSet<String>`).
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando indicando cuántos elementos fueron eliminados.
pub fn handle_srem(
    request: &CommandRequest,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> RedisResponse {
    let key = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Uso: SREM <key> <member1> [member2 ...]".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    let mut sets = match shared_sets.lock() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error al bloquear shared_sets: {}", e);
            return RedisResponse::new(
                CommandResponse::Error("Error interno al acceder a conjuntos".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    match sets.get_mut(key) {
        Some(set) => {
            let mut removed = 0;
            for arg in &request.arguments {
                if let Some(arg_str) = extract_string(arg) {
                    if set.remove(&arg_str) {
                        removed += 1;
                    }
                }
            }

            RedisResponse::new(
                CommandResponse::String(format!("{} miembro(s) eliminado(s)", removed)),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
        None => RedisResponse::new(
            CommandResponse::Error("Set no encontrado".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        ),
    }
}


/// Maneja el comando SADD que agrega uno o más elementos a un conjunto.
///
/// # Argumentos
/// * `request` - La solicitud del comando que contiene la clave del conjunto y los elementos a agregar.
/// * `shared_sets` - Un mapa compartido y protegido que asocia claves con conjuntos de elementos (`HashSet<String>`).
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando indicando cuántos elementos fueron agregados.
pub fn handle_sadd(
    request: &CommandRequest,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
) -> RedisResponse {
    let key = match &request.key {
        Some(k) => k,
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Uso: SADD <key> <member1> [member2 ...]".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    let mut sets = match shared_sets.lock() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error al bloquear shared_sets: {}", e);
            return RedisResponse::new(
                CommandResponse::Error("Error interno al acceder a conjuntos".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            );
        }
    };

    let set = sets.entry(key.clone()).or_insert_with(HashSet::new);

    let mut added = 0;
    for arg in &request.arguments {
        if let Some(arg_str) = extract_string(arg) {
            if set.insert(arg_str) {
                added += 1;
            }
        }
    }

    RedisResponse::new(
        CommandResponse::String(format!("{} miembro(s) agregado(s)", added)),
        false,
        "".to_string(),
        "".to_string(),
    )
}


fn extract_string(value: &ValueType) -> Option<String> {
    match value {
        ValueType::String(s) => Some(s.clone()),
        ValueType::Integer(i) => Some(i.to_string()),
        _ => None,
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     fn setup_clients_on_sets() -> Arc<Mutex<HashMap<String, HashSet<String>>>> {
//         let mut map = HashMap::new();
//         map.insert(
//             "doc1".to_string(),
//             vec!["alice".to_string(), "bob".to_string(), "carol".to_string()],
//         );
//         map.insert("doc2".to_string(), vec![]);
//         Arc::new(Mutex::new(map))
//     }

//     #[test]
//     fn test_handle_scard_ok() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SCARD".to_string(),
//             key: Some("doc1".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_scard(&req, clients);
//         match resp.response {
//             CommandResponse::String(s) => {
//                 assert!(s.contains("Number of subscribers in channel doc1: 3"))
//             }
//             _ => panic!("Expected String response"),
//         }
//     }

//     #[test]
//     fn test_handle_scard_no_key() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SCARD".to_string(),
//             key: None,
//             arguments: vec![],
//         };
//         let resp = handle_scard(&req, clients);
//         match resp.response {
//             CommandResponse::Error(s) => assert!(s.contains("Usage: SCARD")),
//             _ => panic!("Expected Error response"),
//         }
//     }

//     #[test]
//     fn test_handle_scard_doc_not_found() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SCARD".to_string(),
//             key: Some("docX".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_scard(&req, clients);
//         match resp.response {
//             CommandResponse::Error(s) => assert!(s.contains("Document not found")),
//             _ => panic!("Expected Error response"),
//         }
//     }

//     #[test]
//     fn test_handle_smembers_ok() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SMEMBERS".to_string(),
//             key: Some("doc1".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_smembers(&req, clients);
//         match resp.response {
//             CommandResponse::String(s) => {
//                 assert!(s.contains("alice"));
//                 assert!(s.contains("bob"));
//                 assert!(s.contains("carol"));
//             }
//             _ => panic!("Expected String response"),
//         }
//     }

//     #[test]
//     fn test_handle_smembers_empty() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SMEMBERS".to_string(),
//             key: Some("doc2".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_smembers(&req, clients);
//         match resp.response {
//             CommandResponse::String(s) => assert!(s.contains("No subscribers in document doc2")),
//             _ => panic!("Expected String response"),
//         }
//     }

//     #[test]
//     fn test_handle_smembers_no_key() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SMEMBERS".to_string(),
//             key: None,
//             arguments: vec![],
//         };
//         let resp = handle_smembers(&req, clients);
//         match resp.response {
//             CommandResponse::Error(s) => assert!(s.contains("Usage: SMEMBERS")),
//             _ => panic!("Expected Error response"),
//         }
//     }

//     #[test]
//     fn test_handle_smembers_doc_not_found() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SMEMBERS".to_string(),
//             key: Some("docX".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_smembers(&req, clients);
//         match resp.response {
//             CommandResponse::Error(s) => assert!(s.contains("Document not found")),
//             _ => panic!("Expected Error response"),
//         }
//     }

//     #[test]
//     fn test_handle_sscan_pattern_found() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SSCAN".to_string(),
//             key: Some("doc1".to_string()),
//             arguments: vec![ValueType::String("ali".to_string())],
//         };
//         let resp = handle_sscan(&req, clients);
//         match resp.response {
//             CommandResponse::String(s) => assert!(s.contains("alice")),
//             _ => panic!("Expected String response"),
//         }
//     }

//     #[test]
//     fn test_handle_sscan_pattern_not_found() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SSCAN".to_string(),
//             key: Some("doc1".to_string()),
//             arguments: vec![ValueType::String("zzz".to_string())],
//         };
//         let resp = handle_sscan(&req, clients);
//         match resp.response {
//             CommandResponse::String(s) => assert!(s.contains("No subscribers matching")),
//             _ => panic!("Expected String response"),
//         }
//     }

//     #[test]
//     fn test_handle_sscan_no_pattern() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SSCAN".to_string(),
//             key: Some("doc1".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_sscan(&req, clients);
//         match resp.response {
//             CommandResponse::String(s) => {
//                 assert!(s.contains("alice"));
//                 assert!(s.contains("bob"));
//                 assert!(s.contains("carol"));
//             }
//             _ => panic!("Expected String response"),
//         }
//     }

//     #[test]
//     fn test_handle_sscan_pattern_wrong_type() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SSCAN".to_string(),
//             key: Some("doc1".to_string()),
//             arguments: vec![ValueType::Integer(123)],
//         };
//         let resp = handle_sscan(&req, clients);
//         match resp.response {
//             CommandResponse::Error(s) => assert!(s.contains("Expected string pattern")),
//             _ => panic!("Expected Error response"),
//         }
//     }

//     #[test]
//     fn test_handle_sscan_no_key() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SSCAN".to_string(),
//             key: None,
//             arguments: vec![],
//         };
//         let resp = handle_sscan(&req, clients);
//         match resp.response {
//             CommandResponse::Error(s) => assert!(s.contains("Usage: SSCAN")),
//             _ => panic!("Expected Error response"),
//         }
//     }

//     #[test]
//     fn test_handle_sscan_doc_not_found() {
//         let clients = setup_clients_on_sets();
//         let req = CommandRequest {
//             command: "SSCAN".to_string(),
//             key: Some("docX".to_string()),
//             arguments: vec![],
//         };
//         let resp = handle_sscan(&req, clients);
//         match resp.response {
//             CommandResponse::Error(s) => assert!(s.contains("Document not found")),
//             _ => panic!("Expected Error response"),
//         }
//     }
// }
