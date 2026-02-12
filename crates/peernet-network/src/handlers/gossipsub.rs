use crate::state::NetworkState;
use libp2p::gossipsub;
use peernet_core::{GossipPayload, NetworkEvent, TopicName};

pub struct GossipsubHandler;

impl GossipsubHandler {
    pub async fn handle(state: &mut NetworkState, event: gossipsub::Event) {
        match event {
            gossipsub::Event::Message { message, .. } => {
                let topic = TopicName::new_unchecked(message.topic.to_string());
                let payload =
                    GossipPayload::new(message.data).unwrap_or_else(|_| GossipPayload::empty());
                state
                    .emit(NetworkEvent::GossipMessage {
                        source: message.source,
                        topic,
                        payload,
                    })
                    .await;
            }
            gossipsub::Event::Subscribed { peer_id, topic } => {
                let topic = TopicName::new_unchecked(topic.to_string());
                state
                    .emit(NetworkEvent::PeerSubscribed { peer_id, topic })
                    .await;
            }
            gossipsub::Event::Unsubscribed { peer_id, topic } => {
                let topic = TopicName::new_unchecked(topic.to_string());
                state
                    .emit(NetworkEvent::PeerUnsubscribed { peer_id, topic })
                    .await;
            }
            _ => {}
        }
    }
}
