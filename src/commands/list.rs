use super::redis_response::RedisResponse;
use crate::documento::Documento;
use crate::utils::redis_parser::{CommandRequest, CommandResponse, ValueType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Maneja el comando LINSERT que inserta un elemento antes o después de un elemento pivote en una lista
///
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento, flag (BEFORE|AFTER), elemento pivote y elemento a insertar
/// * `docs` - Un mapa compartido y protegido que asocia documentos con listas de elementos
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando, que incluye la longitud actualizada de la lista
pub fn handle_linsert(
    request: &CommandRequest,
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error(
                    "Usage: LINSERT <doc> BEFORE|AFTER <pivot> <element>".to_string(),
                ),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    if request.arguments.len() != 3 {
        return RedisResponse::new(
            CommandResponse::Error("Incorrect number of arguments for LINSERT".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        );
    }

    let (flag, pivot, element) = (
        &request.arguments[0],
        &request.arguments[1],
        &request.arguments[2],
    );

    let (flag_str, pivot_str, element_str) = match (flag, pivot, element) {
        (ValueType::String(f), ValueType::String(p), ValueType::String(e)) => {
            (f.to_lowercase(), p.clone(), e.clone())
        }
        _ => {
            return RedisResponse::new(
                CommandResponse::Error("Arguments must be strings".to_string()),
                false,
                "".to_string(),
                doc,
            )
        }
    };

    let mut docs_lock = docs.lock().unwrap();
    let entry_doc = docs_lock.entry(doc.clone()).or_default();

    // if let Some(index) = entry_doc.iter().position(|x| x == &pivot_str) {
    if let Some(index) = entry_doc
        .as_texto()
        .and_then(|v| v.iter().position(|x| x == &pivot_str))
    {
        match flag_str.as_str() {
            "before" => entry_doc.insert(index, element_str.clone()),
            "after" => {
                if entry_doc.len() > index + 1 {
                    entry_doc.insert(index + 1, element_str.clone());
                } else {
                    entry_doc.push(element_str.clone());
                }
            }
            _ => {
                return RedisResponse::new(
                    CommandResponse::Error("Invalid flag argument".to_string()),
                    false,
                    "".to_string(),
                    doc,
                );
            }
        }

        let message = format!("Inserted '{}' {} '{}'", element_str, flag_str, pivot_str);
        RedisResponse::new(
            CommandResponse::Integer(entry_doc.len() as i64),
            true,
            message,
            doc,
        )
    } else {
        println!("pivot: {}", pivot_str);
        RedisResponse::new(
            CommandResponse::Error("Invalid pivot argument".to_string()),
            false,
            "".to_string(),
            doc,
        )
    }
}

/// Maneja el comando LSET que actualiza un elemento en una posición específica de una lista
///
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento, índice y elemento a establecer
/// * `docs` - Un mapa compartido y protegido que asocia documentos con listas de elementos
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando confirmando la actualización o un error
pub fn handle_lset(
    request: &CommandRequest,
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: LSET <doc> <index> <element>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    if request.arguments.len() != 2 {
        return RedisResponse::new(
            CommandResponse::Error("Invalid arguments for LSET".to_string()),
            false,
            "".to_string(),
            doc,
        );
    }

    let (index, element) = (&request.arguments[0], &request.arguments[1]);
    let (index_i64, element_str) = match (index, element) {
        (ValueType::Integer(i), ValueType::String(s)) => (*i, s.clone()),
        _ => {
            return RedisResponse::new(
                CommandResponse::Error("Invalid arguments for LSET".to_string()),
                false,
                "".to_string(),
                doc,
            )
        }
    };

    let mut docs_lock = docs.lock().unwrap();
    let doc_entry = docs_lock.entry(doc.clone()).or_default();

    match doc_entry {
        Documento::Calculo(vec) | Documento::Texto(vec) => {
            let index_usize = if index_i64 < 0 {
                let abs_index = index_i64.unsigned_abs() as usize;
                if abs_index > vec.len() {
                    return RedisResponse::new(
                        CommandResponse::Error("Index out of bounds".to_string()),
                        false,
                        "".to_string(),
                        doc.clone(),
                    );
                }
                vec.len() - abs_index
            } else {
                index_i64 as usize
            };

            if index_usize >= vec.len() {
                return RedisResponse::new(
                    CommandResponse::Error("Index out of bounds".to_string()),
                    false,
                    "".to_string(),
                    doc.clone(),
                );
            }

            vec[index_usize] = element_str.clone();

            let message = format!("Updated index {} with '{}'", index_i64, element_str);
            RedisResponse::new(
                CommandResponse::String("Ok".to_string()),
                false,
                message,
                doc.clone(),
            )
        }        
    }
}



/// Maneja el comando LLEN que devuelve la longitud de una lista
///
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento a consultar
/// * `docs` - Un mapa compartido y protegido que asocia documentos con listas de elementos
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando con la longitud de la lista
pub fn handle_llen(
    request: &CommandRequest,
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
) -> RedisResponse {    
    let doc = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: LLEN <doc>".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    let docs_lock = docs.lock().unwrap();    
    let list = docs_lock.get(&doc);
    let length = match list {
        Some(l) => l.len(),
        None => 0,
    };

    RedisResponse::new(
        CommandResponse::Integer(length as i64),
        false,
        "".to_string(),
        doc,
    )
}

/// Maneja el comando RPUSH que añade uno o más elementos al final de una lista
///
/// # Argumentos
/// * `request` - La solicitud de comando que contiene el documento y los elementos a añadir
/// * `docs` - Un mapa compartido y protegido que asocia documentos con listas de elementos
///
/// # Retorno
/// * `RedisResponse` - La respuesta al comando con la longitud actualizada de la lista
pub fn handle_rpush(
    request: &CommandRequest,
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
) -> RedisResponse {
    let doc = match &request.key {
        Some(k) => k.clone(),
        None => {
            return RedisResponse::new(
                CommandResponse::Error("Usage: RPUSH <doc> <value1> [value2 ...]".to_string()),
                false,
                "".to_string(),
                "".to_string(),
            )
        }
    };

    if request.arguments.is_empty() {
        return RedisResponse::new(
            CommandResponse::Error("Invalid arguments for RPUSH".to_string()),
            false,
            "".to_string(),
            doc.clone(),
        );
    }

    let mut docs_lock = docs.lock().unwrap();
    let list = docs_lock.entry(doc.clone()).or_default();

    let mut pushed_count = 0;
    for val in &request.arguments {
        if let ValueType::String(s) = val {
            list.push(s.clone());
            pushed_count += 1;
        } else {
            return RedisResponse::new(
                CommandResponse::Error("Invalid arguments for RPUSH".to_string()),
                false,
                "".to_string(),
                doc,
            );
        }
    }

    RedisResponse::new(
        CommandResponse::Integer(list.len() as i64),
        true,
        format!("{} elements pushed", pushed_count),
        doc,
    )
}


#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Arc<Mutex<HashMap<String, Documento>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    #[test]
    fn test_handle_linsert() {
        let docs = setup();

        // Test inserting before with empty list
        let request = CommandRequest {
            command: "LINSERT".to_string(),
            key: Some("test".to_string()),
            arguments: vec![
                ValueType::String("before".to_string()),
                ValueType::String("pivot".to_string()),
                ValueType::String("new".to_string()),
            ],
            unparsed_command: String::new(),
        };

        let response = handle_linsert(&request, &docs);
        assert!(matches!(response.response, CommandResponse::Error(_)));

        // Test inserting after existing element
        {
            let mut docs_lock = docs.lock().unwrap();
            docs_lock.insert("test".to_string(), Documento::Texto(vec!["pivot".to_string()]));
        }

        let request = CommandRequest {
            command: "LINSERT".to_string(),
            key: Some("test".to_string()),
            arguments: vec![
                ValueType::String("after".to_string()),
                ValueType::String("pivot".to_string()),
                ValueType::String("new".to_string()),
            ],
            unparsed_command: String::new(),
        };

        let response = handle_linsert(&request, &docs);
        assert!(matches!(response.response, CommandResponse::Integer(2)));
    }

    #[test]
    fn test_handle_lset() {
        let docs = setup();

        // Test setting invalid index
        let request = CommandRequest {
            command: "LSET".to_string(),
            key: Some("test".to_string()),
            arguments: vec![
                ValueType::Integer(0),
                ValueType::String("value".to_string()),
            ],
            unparsed_command: String::new(),
        };

        let response = handle_lset(&request, &docs);
        assert!(matches!(response.response, CommandResponse::Error(_)));

        // Test valid set
        {
            let mut docs_lock = docs.lock().unwrap();
            docs_lock.insert("test".to_string(), Documento::Texto(vec!["old".to_string()]));
        }

        let response = handle_lset(&request, &docs);
        assert!(matches!(response.response, CommandResponse::String(_)));
    }

    #[test]
    fn test_handle_llen() {
        let docs = setup();

        // Test empty list
        let request = CommandRequest {
            command: "LLEN".to_string(),
            key: Some("test".to_string()),
            arguments: vec![],
            unparsed_command: String::new(),
        };

        let response = handle_llen(&request, &docs);
        assert!(matches!(response.response, CommandResponse::Integer(0)));

        // Test non-empty list
        {
            let mut docs_lock = docs.lock().unwrap();
            docs_lock.insert("test".to_string(), Documento::Texto(vec!["value".to_string()]));
        }

        let response = handle_llen(&request, &docs);
        assert!(matches!(response.response, CommandResponse::Integer(1)));
    }
      
    #[test]
    fn test_handle_rpush() {
        let docs = setup();

         // Test pushing single value
         let request = CommandRequest {
             command: "RPUSH".to_string(),
             key: Some("test".to_string()),
             arguments: vec![ValueType::String("value".to_string())],
             unparsed_command: String::new(),
         };

         let response = handle_rpush(&request, &docs);
         assert!(matches!(response.response, CommandResponse::Integer(1)));

          //Test pushing multiple values
         let request = CommandRequest {
             command: "RPUSH".to_string(),
             key: Some("test".to_string()),
             arguments: vec![
                 ValueType::String("value1".to_string()),
                 ValueType::String("value2".to_string()),
             ],
             unparsed_command: String::new(),
         };

         let response = handle_rpush(&request, &docs);
         assert!(matches!(response.response, CommandResponse::Integer(3)));
     }
 }