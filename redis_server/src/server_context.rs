use types::*;

/// Estructura principal que representa el contexto compartido del servidor Redis.
///
/// `ServerContext` agrupa todas las estructuras de datos y canales compartidos necesarios
/// para la operación del servidor, permitiendo el acceso concurrente y seguro desde
/// múltiples hilos. Se utiliza para gestionar clientes, documentos, suscripciones,
/// nodos, canales internos y autenticación.
///
/// # Campos
/// - `active_clients`: Mapa compartido de todos los clientes conectados y su información.
/// - `document_subscribers`: Mapa compartido de listas de suscriptores por documento.
/// - `shared_documents`: Mapa compartido de los documentos almacenados en el servidor.
/// - `shared_sets`: Mapa compartido de sets asociados a los documentos.
/// - `local_node`: Información del nodo local (dirección, puerto, etc.).
/// - `peer_nodes`: Mapa de nodos pares en la red distribuida.
/// - `logged_clients`: Mapa compartido de clientes autenticados.
/// - `internal_subscription_channel`: Canal interno para notificaciones del microservicio.
/// - `llm_channel`: Canal interno para solicitudes/respuestas del microservicio LLM.
/// - `main_addrs`: Dirección principal (host:puerto) del servidor.
///
/// # Uso
/// Esta estructura se pasa por referencia (`Arc<ServerContext>`) a todas las funciones
/// y hilos que requieren acceso al estado global del servidor.
pub struct ServerContext {
    /// Mapa compartido de todos los clientes conectados y su información.
    pub active_clients: ClientsMap,
    /// Mapa compartido de listas de suscriptores por documento.
    pub document_subscribers: SubscribersMap,
    /// Mapa compartido de los documentos almacenados en el servidor.
    pub shared_documents: RedisDocumentsMap,
    /// Mapa compartido de sets asociados a los documentos.
    pub shared_sets: SetsMap,
    /// Información del nodo local (dirección, puerto, etc.).
    pub local_node: LocalNodeMap,
    /// Mapa de nodos pares en la red distribuida.
    pub peer_nodes: PeerNodeMap,
    /// Mapa compartido de clientes autenticados.
    pub logged_clients: LoggedClientsMap,
    /// Canal interno para notificaciones del microservicio.
    pub internal_subscription_channel: ClientsMap,
    /// Canal interno para solicitudes/respuestas del microservicio LLM.
    pub llm_channel: LlmNodesMap,
    /// Dirección principal (host:puerto) del servidor.
    pub main_addrs: String,
}
