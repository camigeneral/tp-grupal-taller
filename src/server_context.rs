use redis_types::*;

pub struct ServerContext {
    pub active_clients: ClientsMap,
    pub document_subscribers: SubscribersMap,
    pub shared_documents: RedisDocumentsMap,
    pub shared_sets: SetsMap,
    pub local_node: LocalNodeMap,
    pub peer_nodes: PeerNodeMap,
    pub logged_clients: LoggedClientsMap,
    pub internal_subscription_channel: ClientsMap,
    pub main_addrs: String
}
