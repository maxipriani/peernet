use libp2p::{gossipsub, kad};
use peernet_core::{DhtKey, NetworkEvent, PeerId, TopicName};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

fn topic_hash(topic: &TopicName) -> gossipsub::TopicHash {
    gossipsub::IdentTopic::new(topic.as_ref()).hash()
}

#[derive(Debug)]
pub enum PendingQuery {
    GetRecord(DhtKey),
    PutRecord(DhtKey),
    GetProviders(DhtKey),
    StartProviding(DhtKey),
}

pub struct NetworkState {
    pub local_peer_id: PeerId,
    pub connected_peers: HashSet<PeerId>,
    pub subscribed_topics: HashSet<gossipsub::TopicHash>,
    pub pending_queries: HashMap<kad::QueryId, PendingQuery>,
    pub event_tx: mpsc::Sender<NetworkEvent>,
}

impl NetworkState {
    pub fn new(local_peer_id: PeerId, event_tx: mpsc::Sender<NetworkEvent>) -> Self {
        Self {
            local_peer_id,
            connected_peers: HashSet::new(),
            subscribed_topics: HashSet::new(),
            pending_queries: HashMap::new(),
            event_tx,
        }
    }

    pub async fn emit(&self, event: NetworkEvent) {
        let _ = self.event_tx.send(event).await;
    }

    pub fn is_subscribed(&self, topic: &TopicName) -> bool {
        let hash = topic_hash(topic);
        self.subscribed_topics.contains(&hash)
    }

    pub fn add_subscription(&mut self, topic: &TopicName) {
        let hash = topic_hash(topic);
        self.subscribed_topics.insert(hash);
    }

    pub fn remove_subscription(&mut self, topic: &TopicName) {
        let hash = topic_hash(topic);
        self.subscribed_topics.remove(&hash);
    }

    pub fn track_query(&mut self, id: kad::QueryId, query: PendingQuery) {
        self.pending_queries.insert(id, query);
    }

    pub fn complete_query(&mut self, id: &kad::QueryId) -> Option<PendingQuery> {
        self.pending_queries.remove(id)
    }
}
