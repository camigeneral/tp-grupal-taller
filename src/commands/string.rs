use super::redis;
use super::redis_response::RedisResponse;
use parse::{CommandRequest, CommandResponse};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Maneja el comando GET para obtener el contenido de un documento.
///
/// - Si no se especifica clave (documento), devuelve un error.
/// - Si el documento existe, concatena sus líneas con `\n` y lo devuelve.
/// - Si no existe, devuelve un valor nulo.
///
/// # Parámetros
/// - `request`: comando recibido con clave y argumentos.
/// - `docs`: referencia compartida a la estructura de documentos.
///
/// # Retorna
/// - `RedisResponse` con el contenido (`String`), nulo (`Null`) o error.
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
/// - Registra el documento en el mapa de `clients_on_docs` para futuras suscripciones.
/// - Publica una notificación para los clientes suscritos.
///
/// # Parámetros
/// - `request`: contiene el documento y los argumentos (contenido).
/// - `docs`: referencia a la base de documentos compartida.
/// - `clients_on_docs`: referencia a la tabla de suscriptores.
///
/// # Retorna
/// - `RedisResponse::Ok` con notificación activa y nombre del documento.
pub fn handle_set(
    request: &CommandRequest,
    docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
    clients_on_docs: Arc<Mutex<HashMap<String, Vec<String>>>>,
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

        let mut clients_on_docs_lock = clients_on_docs.lock().unwrap();
        if !clients_on_docs_lock.contains_key(&doc_name) {
            clients_on_docs_lock.insert(doc_name.clone(), Vec::new());
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

    let notification = format!("New content in {}: {}", doc, content);
    println!("Publishing to subscribers of {}: {}", doc, notification);

    RedisResponse::new(
        CommandResponse::Integer(line_number as i64),
        true,
        notification,
        doc,
    )
}
