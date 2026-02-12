#![allow(dead_code)]

use peernet_core::{DhtKey, DhtValue, GossipPayload, NetworkEvent, PeerId};
use peernet_network::{NetworkConfig, NetworkHandle};
use std::time::Duration;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct TestNode {
    pub handle: NetworkHandle,
    pub peer_id: PeerId,
    pub cancel_token: CancellationToken,
    pub name: String,
}

impl TestNode {
    pub async fn spawn(name: impl Into<String>) -> Self {
        let name = name.into();
        let cancel_token = CancellationToken::new();
        let mut handle = peernet_network::spawn(NetworkConfig::default(), cancel_token.clone());

        let started = timeout(DEFAULT_TIMEOUT, handle.recv())
            .await
            .unwrap_or_else(|_| panic!("[{name}] timeout"))
            .unwrap_or_else(|| panic!("[{name}] closed"));

        let peer_id = match started {
            NetworkEvent::Started { local_peer_id, .. } => local_peer_id,
            other => panic!("[{name}] expected Started, got {other:?}"),
        };

        Self {
            handle,
            peer_id,
            cancel_token,
            name,
        }
    }

    pub fn short_id(&self) -> String {
        self.peer_id.to_string()[..12].to_string()
    }

    pub async fn recv_timeout(&mut self, duration: Duration) -> Option<NetworkEvent> {
        timeout(duration, self.handle.recv()).await.ok().flatten()
    }

    pub async fn publish(&self, message: &str) {
        let payload = GossipPayload::from_text(message).unwrap();
        self.handle.publish(payload).await.unwrap();
    }

    pub async fn put(&self, key: &str, value: &str) {
        let k = DhtKey::new(key).unwrap();
        let v = DhtValue::new(value.as_bytes().to_vec()).unwrap();
        self.handle.put(k, v).await.unwrap();
    }

    pub async fn get(&self, key: &str) {
        let k = DhtKey::new(key).unwrap();
        self.handle.get(k).await.unwrap();
    }

    pub async fn shutdown(self) {
        self.cancel_token.cancel();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    pub async fn put_bytes(&self, key: &str, value: &[u8]) {
        let k = DhtKey::new(key).unwrap();
        let v = DhtValue::new(value.to_vec()).unwrap();
        self.handle.put(k, v).await.unwrap();
    }
}

impl Drop for TestNode {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}

pub async fn wait_for_connection(node1: &mut TestNode, node2: &mut TestNode) {
    let target = node2.peer_id;
    let deadline = tokio::time::Instant::now() + DEFAULT_TIMEOUT;

    loop {
        if tokio::time::Instant::now() > deadline {
            panic!(
                "[{}] timeout connecting to {}",
                node1.name,
                node2.short_id()
            );
        }
        match node1.recv_timeout(Duration::from_millis(200)).await {
            Some(NetworkEvent::PeerConnected { peer_id }) if peer_id == target => break,
            _ => continue,
        }
    }
}

pub async fn wait_for_peer_count(node: &mut TestNode, count: usize) {
    let mut connected = std::collections::HashSet::new();
    let deadline = tokio::time::Instant::now() + DEFAULT_TIMEOUT;

    loop {
        if connected.len() >= count {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("[{}] timeout waiting for {} peers", node.name, count);
        }
        match node.recv_timeout(Duration::from_millis(200)).await {
            Some(NetworkEvent::PeerConnected { peer_id }) => {
                connected.insert(peer_id);
            }
            Some(NetworkEvent::PeerDisconnected { peer_id }) => {
                connected.remove(&peer_id);
            }
            _ => continue,
        }
    }
}

pub async fn drain_events(node: &mut TestNode) {
    while node.recv_timeout(Duration::from_millis(50)).await.is_some() {}
}

pub async fn expect_gossip(node: &mut TestNode, expected: &str) -> NetworkEvent {
    let deadline = tokio::time::Instant::now() + DEFAULT_TIMEOUT;

    loop {
        if tokio::time::Instant::now() > deadline {
            panic!("[{}] timeout waiting for '{expected}'", node.name);
        }
        match node.recv_timeout(Duration::from_millis(200)).await {
            Some(event @ NetworkEvent::GossipMessage { .. }) => {
                if let NetworkEvent::GossipMessage { ref payload, .. } = event
                    && payload.as_bytes() == expected.as_bytes()
                {
                    return event;
                }
            }
            _ => continue,
        }
    }
}

pub async fn expect_record_found(node: &mut TestNode, expected_key: &str) -> Vec<u8> {
    let deadline = tokio::time::Instant::now() + DEFAULT_TIMEOUT;

    loop {
        if tokio::time::Instant::now() > deadline {
            panic!("[{}] timeout waiting for '{expected_key}'", node.name);
        }
        match node.recv_timeout(Duration::from_millis(200)).await {
            Some(NetworkEvent::RecordFound { key, value }) if key.as_str() == expected_key => {
                return value.as_bytes().to_vec();
            }
            Some(NetworkEvent::RecordNotFound { key }) if key.as_str() == expected_key => {
                panic!("[{}] record not found: {expected_key}", node.name);
            }
            _ => continue,
        }
    }
}

pub async fn expect_record_stored(node: &mut TestNode, expected_key: &str) {
    let deadline = tokio::time::Instant::now() + DEFAULT_TIMEOUT;

    loop {
        if tokio::time::Instant::now() > deadline {
            panic!("[{}] timeout storing '{expected_key}'", node.name);
        }
        match node.recv_timeout(Duration::from_millis(200)).await {
            Some(NetworkEvent::RecordStored { key }) if key.as_str() == expected_key => return,
            Some(NetworkEvent::RecordStoreFailed { key, reason })
                if key.as_str() == expected_key =>
            {
                panic!("[{}] store failed: {reason}", node.name);
            }
            _ => continue,
        }
    }
}
