use crate::client_info;
use crate::documento::Documento;
use crate::local_node;
use crate::peer_node;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub struct ServerContext {
    pub active_clients: Arc<Mutex<HashMap<String, client_info::Client>>>,
    pub document_subscribers: Arc<Mutex<HashMap<String, Vec<String>>>>,
    pub shared_documents: Arc<Mutex<HashMap<String, Documento>>>,
    pub shared_sets: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    pub local_node: Arc<Mutex<local_node::LocalNode>>,
    pub peer_nodes: Arc<Mutex<HashMap<String, peer_node::PeerNode>>>,
    pub logged_clients: Arc<Mutex<HashMap<String, bool>>>,
    pub log_path: String,
    pub internal_subscription_channel: Arc<Mutex<HashMap<String, String>>>,
}
