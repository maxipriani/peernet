pub use libp2p::PeerId;

use crate::{DhtKey, DhtValue, GossipPayload, TopicName};
use libp2p::Multiaddr;

#[derive(Debug)]
pub enum NetworkCommand {
    Shutdown,
    Dial {
        addr: Multiaddr,
    },
    Publish {
        topic: TopicName,
        payload: GossipPayload,
    },
    Subscribe {
        topic: TopicName,
    },
    Unsubscribe {
        topic: TopicName,
    },
    PutRecord {
        key: DhtKey,
        value: DhtValue,
    },
    GetRecord {
        key: DhtKey,
    },
    StartProviding {
        key: DhtKey,
    },
    GetProviders {
        key: DhtKey,
    },
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    Started {
        local_peer_id: PeerId,
        listening_on: Multiaddr,
    },
    ShutdownComplete,
    PeerDiscovered {
        peer_id: PeerId,
    },
    PeerConnected {
        peer_id: PeerId,
    },
    PeerDisconnected {
        peer_id: PeerId,
    },
    Listening {
        address: Multiaddr,
    },
    GossipMessage {
        source: Option<PeerId>,
        topic: TopicName,
        payload: GossipPayload,
    },
    Subscribed {
        topic: TopicName,
    },
    Unsubscribed {
        topic: TopicName,
    },
    PeerSubscribed {
        peer_id: PeerId,
        topic: TopicName,
    },
    PeerUnsubscribed {
        peer_id: PeerId,
        topic: TopicName,
    },
    RecordStored {
        key: DhtKey,
    },
    RecordStoreFailed {
        key: DhtKey,
        reason: String,
    },
    RecordFound {
        key: DhtKey,
        value: DhtValue,
    },
    RecordNotFound {
        key: DhtKey,
    },
    ProviderRecordStored {
        key: DhtKey,
    },
    ProvidersFound {
        key: DhtKey,
        providers: Vec<PeerId>,
    },
    RoutingUpdated {
        peer_id: PeerId,
    },
    CommandFailed {
        reason: String,
    },
}

#[derive(Debug)]
pub enum InputCommand {
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<NetworkCommand>();
        assert_send::<NetworkEvent>();
        assert_sync::<NetworkCommand>();
        assert_sync::<NetworkEvent>();
    }
}
