use rusty_docs::resp_parser::CommandRequest;
use types::*;

pub struct ExecuteCommandParams<'a> {
    pub request: CommandRequest,
    pub docs: &'a RedisDocumentsMap,
    pub document_subscribers: &'a SubscribersMap,
    pub shared_sets: &'a SetsMap,
    pub client_addr: String,
    pub active_clients: &'a ClientsMap,
    pub logged_clients: &'a LoggedClientsMap,
    pub suscription_channel: &'a ClientsMap,
    pub llm_channel: &'a LlmNodesMap,
}
