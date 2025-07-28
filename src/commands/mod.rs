/// Módulo encargado de la gestión y manipulación de listas de Redis.
pub mod list;

/// Módulo que implementa el patrón de publicación/suscripción (pub/sub) para comunicación asincrónica.
pub mod pub_sub;

/// Módulo para la interacción y conexión con la base de datos Redis.
pub mod redis;

/// Módulo para el manejo y deserialización de respuestas provenientes de Redis.
pub mod redis_response;

/// Módulo dedicado a la gestión de conjuntos (sets) en Redis.
pub mod set;

/// Módulo para el manejo de operaciones con cadenas de texto (strings) en Redis.
pub mod string;


//Modulo para el manejo del auth
pub mod auth;

#[path = "../utils/redis_parser.rs"]
pub mod redis_parser;
