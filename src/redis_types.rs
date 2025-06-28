use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use documento::Documento;
use crate::client_info::Client;
use peer_node::PeerNode;
use local_node::LocalNode;

/// Mapa de suscriptores a documentos.
/// 
/// Almacena para cada documento (String) una lista de IDs de clientes (Vec<String>)
/// que están suscritos a recibir actualizaciones de ese documento.
/// 
pub type SubscribersMap = Arc<Mutex<HashMap<String, Vec<String>>>>;

/// Mapa de sets para cada documento.
/// 
/// Almacena para cada documento (String) un conjunto de elementos (HashSet<String>).
/// Útil para operaciones de conjuntos como unión, intersección, etc.
/// 
pub type SetsMap = Arc<Mutex<HashMap<String, HashSet<String>>>>;

/// Mapa compartido de documentos.
/// 
/// Almacena todos los documentos del sistema, donde la clave es el ID del documento (String)
/// y el valor es el documento en sí (Documento). Este mapa es compartido entre múltiples hilos.
pub type SharedDocumentsMap =  Arc<Mutex<HashMap<String, Documento>>>;

/// Mapa de clientes activos.
/// 
/// Almacena todos los clientes conectados al servidor, donde la clave es el ID del cliente (String)
/// y el valor es la información del cliente (Client). Este mapa es compartido entre múltiples hilos.
/// 
pub type ClientsMap = Arc<Mutex<HashMap<String, Client>>>;

/// Mapa del nodo local.
/// 
/// Contiene la información del nodo actual del cluster Redis, incluyendo su rol (Master/Replica),
/// rango de hash asignado, estado, etc. Este mapa es compartido entre múltiples hilos.
/// 
pub type LocalNodeMap = Arc<Mutex<LocalNode>>;

/// Mapa de nodos peer.
/// 
/// Almacena información sobre otros nodos en el cluster Redis, donde la clave es la dirección del nodo (String)
/// y el valor es la información del nodo peer (PeerNode). Este mapa es compartido entre múltiples hilos.
/// 
pub type PeerNodeMap = Arc<Mutex<HashMap<String, PeerNode>>>;

/// Mapa de clientes autenticados.
/// 
/// Almacena el estado de autenticación de los clientes, donde la clave es el ID del cliente (String)
/// y el valor es un booleano que indica si el cliente está autenticado (bool).
/// 
pub type LoggedClientsMap = Arc<Mutex<HashMap<String, bool>>>;

/// Mapa genérico para clientes con capacidad de escritura.
/// 
/// Un tipo genérico que permite crear mapas de clientes con diferentes tipos de datos,
/// manteniendo la capacidad de escritura. Útil para casos donde se necesita flexibilidad
/// en el tipo de datos almacenado.
pub type WriteClient<T> = Arc<Mutex<HashMap<String, T>>>;