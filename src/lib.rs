/// Módulo que contiene la información y estructura del cliente.
///
/// Define la estructura `Client`, que encapsula el socket TCP utilizado
/// para la comunicación con el servidor.
pub mod client_info;

/// Módulo que contiene la información y la estructura del nodo local.
///
/// Define la estructura `LocalNode`, que representa al nodo actual dentro del clúster.
/// Incluye información como el puerto, el rol y el rango de hash asignado.
pub mod local_node;

/// Módulo que contiene la información y la estructura de los nodos a los que se conecta.
///
/// Define la estructura `PeerNode`, que representa a los otros nodos del clúster
/// Incluye información como el puerto, el rol, el rango de hash asignado y el socket
/// TCP utilizado.
pub mod peer_node;

/// Módulo encargado de parsear comandos y respuestas en formato RESP.
///
/// Implementa funciones y tipos para interpretar comandos entrantes,
/// construir respuestas y manipular el protocolo RESP usado por Redis.
pub mod parse;

/// Módulo de hashing de claves.
///
/// Implementa la lógica para calcular el hash de una clave utilizando el algoritmo CRC16
/// usado por Redis.
pub mod hashing;

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
