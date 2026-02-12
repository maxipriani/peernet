use crate::behaviour::PeernetBehaviour;
use libp2p::{
    PeerId, StreamProtocol, SwarmBuilder,
    gossipsub::{self, MessageAuthenticity, ValidationMode},
    identity::Keypair,
    kad::{self, Mode, store::MemoryStore},
    mdns,
    swarm::Swarm,
};
use peernet_core::{PeernetError, PeernetResult, TopicName};
use std::time::Duration;

pub const DEFAULT_TOPIC: &str = "peernet-global";
const KADEMLIA_PROTOCOL: &str = "/peernet/kad/1.0.0";

#[derive(Debug, Clone)]
pub struct SwarmConfig {
    pub keypair: Option<Keypair>,
    pub mdns_query_interval: Duration,
    pub gossipsub_heartbeat: Duration,
    pub initial_topics: Vec<TopicName>,
    pub kademlia_replication: usize,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            keypair: None,
            mdns_query_interval: Duration::from_secs(5),
            gossipsub_heartbeat: Duration::from_secs(1),
            initial_topics: vec![TopicName::new_unchecked(DEFAULT_TOPIC)],
            kademlia_replication: 3,
        }
    }
}

pub fn build_swarm(config: SwarmConfig) -> PeernetResult<(Swarm<PeernetBehaviour>, PeerId)> {
    let keypair = config.keypair.unwrap_or_else(Keypair::generate_ed25519);
    let local_peer_id = PeerId::from(keypair.public());

    let swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )
        .map_err(|e| PeernetError::Transport {
            reason: e.to_string(),
        })?
        .with_behaviour(|key| {
            let mut kad_config = kad::Config::new(
                StreamProtocol::try_from_owned(KADEMLIA_PROTOCOL.to_string())
                    .expect("valid protocol"),
            );
            kad_config.set_replication_factor(
                std::num::NonZeroUsize::new(config.kademlia_replication).expect("replication > 0"),
            );
            kad_config.set_query_timeout(Duration::from_secs(60));

            let store = MemoryStore::new(key.public().to_peer_id());
            let mut kademlia =
                kad::Behaviour::with_config(key.public().to_peer_id(), store, kad_config);
            kademlia.set_mode(Some(Mode::Server));

            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(config.gossipsub_heartbeat)
                .validation_mode(ValidationMode::Strict)
                .message_id_fn(|msg| {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    msg.data.hash(&mut hasher);
                    msg.topic.hash(&mut hasher);
                    gossipsub::MessageId::from(hasher.finish().to_be_bytes().to_vec())
                })
                .build()
                .expect("valid config");

            let gossipsub = gossipsub::Behaviour::new(
                MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )
            .expect("valid behaviour");

            let mdns_config = mdns::Config {
                query_interval: config.mdns_query_interval,
                ..Default::default()
            };
            let mdns = mdns::tokio::Behaviour::new(mdns_config, key.public().to_peer_id())
                .expect("mdns init");

            PeernetBehaviour {
                kademlia,
                gossipsub,
                mdns,
            }
        })
        .map_err(|e| PeernetError::Transport {
            reason: e.to_string(),
        })?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    Ok((swarm, local_peer_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builds_swarm_with_default_config() {
        let result = build_swarm(SwarmConfig::default());
        assert!(result.is_ok());
        let (_, peer_id) = result.unwrap();
        assert!(peer_id.to_string().starts_with("12D3K"));
    }

    #[tokio::test]
    async fn uses_provided_keypair() {
        let keypair = Keypair::generate_ed25519();
        let expected = PeerId::from(keypair.public());
        let config = SwarmConfig {
            keypair: Some(keypair),
            ..Default::default()
        };
        let (_, peer_id) = build_swarm(config).unwrap();
        assert_eq!(peer_id, expected);
    }
}
