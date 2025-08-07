use rusty_docs::vars::DOCKER;
use std::env;
use std::path::Path;

/// Obtiene la ruta absoluta a un recurso dado su ruta relativa.
///
/// Esta función toma una ruta relativa y la convierte en una ruta absoluta
/// basada en el directorio de trabajo actual del proceso.
///
/// # Argumentos
/// * `relative_path` - Ruta relativa al recurso.
///
/// # Retorna
/// Un `String` con la ruta absoluta al recurso.
///
/// # Panics
/// Si no se puede obtener el directorio actual o convertir la ruta a string.
///
/// # Ejemplo
/// ```rust
/// let path = get_resource_path("data/file.txt");
/// ```
pub fn get_resource_path<P: AsRef<Path>>(relative_path: P) -> String {
    let cwd = env::current_dir().expect("Failed to get directory");
    let full_path = cwd.join(relative_path);

    full_path
        .to_str()
        .expect("Failed to convert path to string")
        .to_string()
}

/// Obtiene la dirección de un nodo Redis según el puerto y el entorno.
///
/// Si la variable global `DOCKER` es verdadera, retorna la dirección en formato
/// `nodeX:puerto` (usado en Docker Compose). Si es falso, retorna `127.0.0.1:puerto`
/// para uso local.
///
/// # Argumentos
/// * `port` - Puerto del nodo.
///
/// # Retorna
/// Un `String` con la dirección del nodo.
///
/// # Ejemplo
/// ```rust
/// let addr = get_node_address(4001); // "node1:4001" en Docker, "127.0.0.1:4001" local
/// ```
pub fn get_node_address(port: usize) -> String {
    let last_digit = port % 10;
    if DOCKER {
        format!("node{}:{}", last_digit, port)
    } else {
        format!("127.0.0.1:{}", port)
    }
}

pub fn convert_key() -> [u8; 16] {
    let encryption_key = env::var("ENCRYPTION_KEY").unwrap_or_else(|_| {
        eprintln!("ENCRYPTION_KEY is not configured");
        "".to_string()
    });
    
    let mut key_result = [0u8; 16];
    let bytes = encryption_key.as_bytes();
    let len = bytes.len().min(16);
    key_result[..len].copy_from_slice(&bytes[..len]);
    key_result
}