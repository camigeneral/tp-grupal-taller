use super::redis;
use super::redis_response::RedisResponse;
use crate::client_info;
use crate::commands::list::{handle_llen, handle_lset, handle_rpush};
use crate::commands::set::handle_scard;
use crate::commands::string::handle_get;
use commands::redis_parser::{CommandRequest, CommandResponse, ValueType};
use documento::Documento;
#[allow(unused_imports)]
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub fn handle_welcome(
    request: &CommandRequest,
    _active_clients: &Arc<Mutex<HashMap<String, client_info::Client>>>,
    shared_sets: &Arc<Mutex<HashMap<String, HashSet<String>>>>,
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
) -> RedisResponse {
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

    // Obtener cantidad de suscriptores
    let scard_request = CommandRequest {
        command: "scard".to_string(),
        key: Some(doc.clone()),
        arguments: vec![],
        unparsed_command: "".to_string(),
    };
    let response = handle_scard(&scard_request, shared_sets);

    // Obtener contenido del documento
    let get_request = CommandRequest {
        command: "get".to_string(),
        key: Some(doc.clone()),
        arguments: vec![],
        unparsed_command: "".to_string(),
    };
    let get_response = handle_get(&get_request, docs);

    let mut notification = String::new();

    if let CommandResponse::String(ref s) = response.response {
        if let Some(qty_subs) = s.split_whitespace().last() {
            match &get_response.response {
                CommandResponse::String(content) => {
                    let lines: Vec<&str> = content.split('\n').collect();
                    notification = format!("STATUS {}|{}|{}|", client_addr_str, qty_subs, doc);

                    // Agregar las líneas hasta un máximo de 100 elementos
                    for (i, line) in lines.iter().enumerate().take(100) {
                        if i > 0 {
                            notification.push(',');
                        }
                        notification.push_str(line);
                    }

                    // Completar con elementos vacíos si hay menos de 100 líneas
                    for i in lines.len()..100 {
                        if i > 0 {
                            notification.push(',');
                        }
                        // Agrega elemento vacío (no necesitas push_str aquí)
                    }
                }
                CommandResponse::Null => {
                    notification = format!("STATUS {}|{}|<vacio>|{}", client_addr_str, qty_subs, doc);
                    // Agregar 99 comas para elementos vacíos
                    for _ in 0..99 {
                        notification.push(',');
                    }
                }
                _ => {
                    notification = format!("STATUS {}|{}|<error>|{}", client_addr_str, qty_subs, doc);
                    // Agregar 99 comas para elementos vacíos
                    for _ in 0..99 {
                        notification.push(',');
                    }
                }
            }
        }
    }


    println!("Llegue aca {}", notification.clone());
    RedisResponse::new(
        CommandResponse::String(notification.clone()),
        true,
        notification,
        doc,
    )
}

struct ConfigurationFile {
    doc: String,
    final_text: String,
}

pub fn set_content_file(
    request: &CommandRequest,
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
) -> RedisResponse {
    let doc = match get_document_name(request) {
        Ok(doc) => doc,
        Err(resp) => return resp,
    };

    if request.arguments.len() < 2 {
        return error_response("Faltan argumentos: índice, carácter", &doc);
    }

    let is_calc = !doc.ends_with(".txt");

    let text = match get_text_argument(&request.arguments[1], &doc) {
        Ok(text) => text,
        Err(resp) => return resp,
    };

    let final_text = if text.as_str() == "<delete>" {
        String::new()
    } else {
        text.clone()
    };

    let config = ConfigurationFile {
        doc: doc.clone(),
        final_text,
    };

    if is_calc {
        proccess_as_calc(docs, &config, request)
    } else {
        proccess_as_text(docs, &config, request)
    }
}

fn proccess_as_text(
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
    config: &ConfigurationFile,
    request: &CommandRequest,
) -> RedisResponse {
    let line_number = match parse_line_number(&request.arguments[0], &config.doc) {
        Ok(num) => num,
        Err(resp) => return resp,
    };
    let len_request: CommandRequest = CommandRequest {
        command: "llen".to_string(),
        key: Some(config.doc.clone()),
        arguments: vec![],
        unparsed_command: "".to_string(),
    };

    let response_len: RedisResponse = handle_llen(&len_request, docs);
    match response_len.response {
        CommandResponse::Integer(len) => {
            if line_number >= len {
                let rpush_request = CommandRequest {
                    command: "rpush".to_string(),
                    key: Some(config.doc.clone()),
                    arguments: vec![ValueType::String(config.final_text.clone())],
                    unparsed_command: "".to_string(),
                };

                let response = handle_rpush(&rpush_request, docs);
                if matches!(response.response, CommandResponse::Error(_)) {
                    return error_response("Error al hacer rpush", &config.doc);
                }
            } else {
                let lset_request = CommandRequest {
                    command: "lset".to_string(),
                    key: Some(config.doc.clone()),
                    arguments: vec![
                        ValueType::Integer(line_number),
                        ValueType::String(config.final_text.clone()),
                    ],
                    unparsed_command: "".to_string(),
                };

                let response = handle_lset(&lset_request, docs);
                if matches!(response.response, CommandResponse::Error(_)) {
                    return error_response("Error al hacer lset", &config.doc);
                }
            }
        }
        _ => return error_response("Error al obtener la longitud de la lista", &config.doc),
    }

    RedisResponse::new(
        CommandResponse::String(format!(
            "UPDATE-FILES|{}|{}|{}",
            config.doc, line_number, config.final_text
        )),
        true,
        "Texto actualizado".to_string(),
        config.doc.clone(),
    )
}

fn proccess_as_calc(
    docs: &Arc<Mutex<HashMap<String, Documento>>>,
    config: &ConfigurationFile,
    request: &CommandRequest,
) -> RedisResponse {
    let cell_id = match get_text_argument(&request.arguments[0], &config.doc) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let (col_index, row_index) = match parse_cell_id(&cell_id) {
        Ok(indices) => indices,
        Err(e) => return error_response(e, &config.doc),
    };

    let total_columns = 10;
    let index = col_index + total_columns * row_index;

    if index >= 100 {
        return error_response("Celda fuera de rango", &config.doc);
    }

    let lset_request = CommandRequest {
        command: "lset".to_string(),
        key: Some(config.doc.clone()),
        arguments: vec![
            ValueType::Integer(index as i64),
            ValueType::String(config.final_text.clone()),
        ],
        unparsed_command: "".to_string(),
    };

    let response = handle_lset(&lset_request, docs);
    if matches!(response.response, CommandResponse::Error(_)) {
        return response;
    }

    RedisResponse::new(
        CommandResponse::String(format!("UPDATE-FILES|{}|{}|{}", config.doc, index, config.final_text)),
        true,
        "Celda actualizada".to_string(),
        config.doc.clone(),
    )
}

fn parse_cell_id(cell_id: &str) -> Result<(usize, usize), &'static str> {
    let mut chars = cell_id.chars();
    let col_char = chars
        .next()
        .ok_or("Formato de celda invalido: falta la columna")?;
    if !col_char.is_ascii_alphabetic() {
        return Err("Formato de celda invalido: la columna no es alfabetica");
    }
    let col_index = (col_char.to_ascii_uppercase() as u32 - 'A' as u32) as usize;

    let row_str: String = chars.collect();
    if row_str.is_empty() {
        return Err("Formato de celda invalido: falta la fila");
    }
    let row_index = match row_str.parse::<usize>() {
        Ok(r) if r > 0 => r - 1,
        _ => return Err("Formato de celda invalido: la fila debe ser un numero positivo"),
    };

    Ok((col_index, row_index))
}

fn get_document_name(request: &CommandRequest) -> Result<String, RedisResponse> {
    request.key.clone().ok_or_else(|| {
        RedisResponse::new(
            CommandResponse::Error("Falta el nombre del documento".to_string()),
            false,
            "".to_string(),
            "".to_string(),
        )
    })
}

fn parse_line_number(arg: &ValueType, doc: &str) -> Result<i64, RedisResponse> {
    match arg {
        ValueType::Integer(i) => Ok(*i),
        ValueType::String(s) => s
            .parse::<i64>()
            .map_err(|_| error_response("El índice debe ser un entero válido", doc)),
        _ => Err(error_response("El índice debe ser un entero", doc)),
    }
}

fn get_text_argument(arg: &ValueType, doc: &str) -> Result<String, RedisResponse> {
    match arg {
        ValueType::String(s) => Ok(s.clone()),
        _ => Err(error_response("El texto debe ser un string", doc)),
    }
}

fn error_response(msg: &str, doc: &str) -> RedisResponse {
    RedisResponse::new(
        CommandResponse::Error(msg.to_string()),
        false,
        "".to_string(),
        doc.to_string(),
    )
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_texto_insertar() {
        let docs = Arc::new(Mutex::new(HashMap::new()));
        let doc_name = "archivo.txt".to_string();
        let req = CommandRequest {
            command: "add_content".to_string(),
            key: Some(doc_name.clone()),
            arguments: vec![ValueType::Integer(0), ValueType::String("Hola".to_string())],
            unparsed_command: String::new(),
        };
        let resp = set_content_file(&req, &docs);
        assert!(matches!(resp.response, CommandResponse::String(_)));
        let docs_lock = docs.lock().unwrap();
        if let Some(Documento::Texto(vec)) = docs_lock.get(&doc_name) {
            assert_eq!(vec[0], "Hola");
        }
    }

    #[test]
    fn test_texto_modificar() {
        let docs = Arc::new(Mutex::new(HashMap::new()));
        let doc_name = "archivo.txt".to_string();
        let req_insert = CommandRequest {
            command: "add_content".to_string(),
            key: Some(doc_name.clone()),
            arguments: vec![ValueType::Integer(0), ValueType::String("Hola".to_string())],
            unparsed_command: String::new(),
        };
        set_content_file(&req_insert, &docs);
        let req = CommandRequest {
            command: "add_content".to_string(),
            key: Some(doc_name.clone()),
            arguments: vec![ValueType::Integer(0), ValueType::String("Chau".to_string())],
            unparsed_command: String::new(),
        };
        let resp = set_content_file(&req, &docs);
        assert!(matches!(resp.response, CommandResponse::String(_)));
        let docs_lock = docs.lock().unwrap();
        if let Some(Documento::Texto(vec)) = docs_lock.get(&doc_name) {
            assert_eq!(vec[0], "Chau");
        } else {
            panic!("No se encontró el documento de texto");
        }
    }

    #[test]
    fn test_texto_borrar() {
        let docs = Arc::new(Mutex::new(HashMap::new()));
        let doc_name = "archivo.txt".to_string();
        let req_insert = CommandRequest {
            command: "add_content".to_string(),
            key: Some(doc_name.clone()),
            arguments: vec![ValueType::Integer(0), ValueType::String("Hola".to_string())],
            unparsed_command: String::new(),
        };
        set_content_file(&req_insert, &docs);
        let req = CommandRequest {
            command: "add_content".to_string(),
            key: Some(doc_name.clone()),
            arguments: vec![
                ValueType::Integer(0),
                ValueType::String("<delete>".to_string()),
            ],
            unparsed_command: String::new(),
        };
        let resp = set_content_file(&req, &docs);
        assert!(matches!(resp.response, CommandResponse::String(_)));
        let docs_lock = docs.lock().unwrap();
        if let Some(Documento::Texto(vec)) = docs_lock.get(&doc_name) {
            assert_eq!(vec[0], "");
        } else {
            panic!("No se encontró el documento de texto");
        }
    }
    #[test]
    fn test_calculo_escribir_a1() {
        let doc_name = "archivo.xlsx".to_string();
        let mut initial_map = HashMap::new();
        initial_map.insert(
            doc_name.clone(),
            Documento::Calculo(vec![String::new(); 100]),
        );
        let docs = Arc::new(Mutex::new(initial_map));

        let req = CommandRequest {
            command: "add_content".to_string(),
            key: Some(doc_name.clone()),
            arguments: vec![
                ValueType::String("A1".to_string()),
                ValueType::String("123".to_string()),
            ],
            unparsed_command: String::new(),
        };

        let resp = set_content_file(&req, &docs);
        assert!(matches!(resp.response, CommandResponse::String(_)));

        let docs_lock = docs.lock().unwrap();
        if let Some(Documento::Calculo(vec)) = docs_lock.get(&doc_name) {
            assert_eq!(&vec[0], "123");
        } else {
            panic!("No se encontró el documento de cálculo");
        }
    }

    #[test]
    fn test_calculo_escribir_b2() {
        let doc_name = "archivo.xlsx".to_string();
        let mut initial_map = HashMap::new();
        initial_map.insert(
            doc_name.clone(),
            Documento::Calculo(vec![String::new(); 100]),
        );
        let docs = Arc::new(Mutex::new(initial_map));
        let req = CommandRequest {
            command: "add_content".to_string(),
            key: Some(doc_name.clone()),
            arguments: vec![
                ValueType::String("B2".to_string()),
                ValueType::String("abc".to_string()),
            ],
            unparsed_command: String::new(),
        };
        let resp = set_content_file(&req, &docs);
        assert!(matches!(resp.response, CommandResponse::String(_)));
        let docs_lock = docs.lock().unwrap();
        if let Some(Documento::Calculo(vec)) = docs_lock.get(&doc_name) {
            assert_eq!(vec[11], "abc");
        } else {
            panic!("No se encontró el documento de cálculo");
        }
    }

    #[test]
    fn test_calculo_borrar_b2() {
        let doc_name = "archivo.xlsx".to_string();
        let mut initial_map = HashMap::new();
        initial_map.insert(
            doc_name.clone(),
            Documento::Calculo(vec![String::new(); 100]),
        );
        let docs = Arc::new(Mutex::new(initial_map));
        let req_insert = CommandRequest {
            command: "add_content".to_string(),
            key: Some(doc_name.clone()),
            arguments: vec![
                ValueType::String("B2".to_string()),
                ValueType::String("abc".to_string()),
            ],
            unparsed_command: String::new(),
        };
        set_content_file(&req_insert, &docs);
        let req = CommandRequest {
            command: "add_content".to_string(),
            key: Some(doc_name.clone()),
            arguments: vec![
                ValueType::String("B2".to_string()),
                ValueType::String("<delete>".to_string()),
            ],
            unparsed_command: String::new(),
        };
        let resp = set_content_file(&req, &docs);
        assert!(matches!(resp.response, CommandResponse::String(_)));
        let docs_lock = docs.lock().unwrap();
        if let Some(Documento::Calculo(vec)) = docs_lock.get(&doc_name) {
            assert_eq!(vec[11], "");
        } else {
            panic!("No se encontró el documento de cálculo");
        }
    }
}
