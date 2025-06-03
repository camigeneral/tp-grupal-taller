/// Módulo que contiene la información y estructura del cliente.
///
/// Define la estructura `Client`, que encapsula el socket TCP utilizado
/// para la comunicación con el servidor.
pub mod client_info;

/// Módulo que contiene la información y estructura del nodo.
///
/// Define la estructura `Node`, que encapsula el socket TCP utilizado
/// para la comunicación enttre los nodos.
pub mod peer_node;

/// Módulo encargado de parsear comandos y respuestas en formato RESP.
///
/// Implementa funciones y tipos para interpretar comandos entrantes,
/// construir respuestas y manipular el protocolo RESP usado por Redis.
pub mod parse;

// to do: completar
pub mod hashing;

// to do: completar
pub mod local_node;

/// Módulo principal de la aplicación.
///
/// Define la estructura y lógica principal de la app, incluyendo los controladores
/// de componentes UI y la comunicación mediante mensajes.
///
/// # Componentes principales
/// - `AppModel`: modelo principal que contiene controladores de componentes.
/// - `AppMsg`: enum de mensajes para controlar interacciones y eventos.
pub mod app;

/// Módulo de componentes UI.
///
/// Contiene definiciones y lógicas de los componentes visuales de la aplicación.
/// Entre ellos, la gestión de archivos, la barra de navegación y el editor.
/// Cada submódulo gestiona un componente específico y su comportamiento.
pub mod components;

/// Módulo cliente que maneja la conexión TCP con el servidor.
///
/// Implementa la lógica para enviar comandos, recibir respuestas y escuchar
/// notificaciones del servidor, actualizando la interfaz mediante mensajes.
///
/// Usa hilos para manejar comunicación asíncrona sin bloquear la UI.
pub mod client;
