/// Módulo que contiene la información y estructura del cliente.
///
/// Define la estructura `Client`, que encapsula el socket TCP utilizado
/// para la comunicación con el servidor.
pub mod client_info;

pub mod document;

pub mod logger;

pub mod resp_parser;

pub mod shared;

pub mod utils;

// /// Módulo que contiene la información y la estructura del nodo local.
// ///
// /// Define la estructura `LocalNode`, que representa al nodo actual dentro del clúster.
// /// Incluye información como el puerto, el rol y el rango de hash asignado.
// pub mod local_node;

// /// Módulo que contiene la información y la estructura de los nodos a los que se conecta.
// ///
// /// Define la estructura `PeerNode`, que representa a los otros nodos del clúster
// /// Incluye información como el puerto, el rol, el rango de hash asignado y el socket
// /// TCP utilizado.
// pub mod peer_node;

// /// Módulo de hashing de claves.
// ///
// /// Implementa la lógica para calcular el hash de una clave utilizando el algoritmo CRC16
// /// usado por Redis.
// pub mod hashing;

// /// Módulo de encriptación de mensajes.
// ///
// /// Implementa funciones para encriptar y desencriptar mensajes usando XOR
// /// con una clave arbitraria.
// pub mod encryption;

// /// Módulo principal de la aplicación.
// ///
// /// Define la estructura y lógica principal de la app, incluyendo los controladores
// /// de componentes UI y la comunicación mediante mensajes.
// ///
// /// # Componentes principales
// /// - `AppModel`: modelo principal que contiene controladores de componentes.
// /// - `AppMsg`: enum de mensajes para controlar interacciones y eventos.
// pub mod app;

// /// Módulo de componentes UI.
// ///
// /// Contiene definiciones y lógicas de los componentes visuales de la aplicación.
// /// Entre ellos, la gestión de archivos, la barra de navegación y el editor.
// /// Cada submódulo gestiona un componente específico y su comportamiento.
// pub mod components;

// /// Módulo cliente que maneja la conexión TCP con el servidor.
// ///
// /// Implementa la lógica para enviar comandos, recibir respuestas y escuchar
// /// notificaciones del servidor, actualizando la interfaz mediante mensajes.
// ///
// /// Usa hilos para manejar comunicación asíncrona sin bloquear la UI.
// pub mod client;

// /// Módulo microservicio que maneja la conexión TCP con el servidor.
// ///
// /// Implementa la lógica para enviar comandos, recibir respuestas y escuchar
// /// notificaciones del servidor, actualizando la interfaz mediante mensajes.
// ///
// /// Usa hilos para manejar comunicación asíncrona sin bloquear la UI.
// pub mod microservice;

// /// Módulo que contiene los diferentes tipos de comandos y sus implementaciones.
// pub mod commands;

// /// Módulo de utilidades que contiene funciones y tipos comunes.
// ///
// /// Incluye el parser de comandos RESP y otras utilidades compartidas.
// pub mod utils;

// pub mod server_context;

// pub mod redis_types;


