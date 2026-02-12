mod behaviour;
mod handlers;
mod state;
mod swarm;

use behaviour::{PeernetBehaviour, PeernetBehaviourEvent};
use futures::StreamExt;
use handlers::{GossipsubHandler, KademliaHandler, MdnsHandler};
use libp2p::{gossipsub, kad::RecordKey, swarm::SwarmEvent};
use peernet_core::{
    CommandError, DhtKey, DhtValue, GossipPayload, Multiaddr, NetworkCommand, NetworkEvent,
    PeernetError, PeernetResult, TopicName,
};
use state::{NetworkState, PendingQuery};
use swarm::{DEFAULT_TOPIC, build_swarm};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::warn;

#[derive(Debug, Clone, Default)]
pub struct NetworkConfig {
    pub port: u16,
    pub swarm: SwarmConfig,
}

#[derive(Debug)]
pub struct NetworkHandle {
    command_tx: mpsc::Sender<NetworkCommand>,
    event_rx: mpsc::Receiver<NetworkEvent>,
}

impl NetworkHandle {
    pub async fn send(&self, cmd: NetworkCommand) -> PeernetResult<()> {
        self.command_tx
            .send(cmd)
            .await
            .map_err(|_| PeernetError::SendFailed { actor: "network" })
    }

    pub async fn recv(&mut self) -> Option<NetworkEvent> {
        self.event_rx.recv().await
    }

    pub async fn shutdown(&self) -> PeernetResult<()> {
        self.send(NetworkCommand::Shutdown).await
    }

    pub async fn publish(&self, payload: GossipPayload) -> PeernetResult<()> {
        self.send(NetworkCommand::Publish {
            topic: TopicName::new_unchecked(DEFAULT_TOPIC),
            payload,
        })
        .await
    }

    pub async fn put(&self, key: DhtKey, value: DhtValue) -> PeernetResult<()> {
        self.send(NetworkCommand::PutRecord { key, value }).await
    }

    pub async fn get(&self, key: DhtKey) -> PeernetResult<()> {
        self.send(NetworkCommand::GetRecord { key }).await
    }
}

pub fn spawn(config: NetworkConfig, cancel_token: CancellationToken) -> NetworkHandle {
    let (command_tx, command_rx) = mpsc::channel::<NetworkCommand>(32);
    let (event_tx, event_rx) = mpsc::channel::<NetworkEvent>(32);
    tokio::spawn(run_network_loop(config, command_rx, event_tx, cancel_token));
    NetworkHandle {
        command_tx,
        event_rx,
    }
}

pub use swarm::SwarmConfig;

struct NetworkActor {
    swarm: libp2p::Swarm<PeernetBehaviour>,
    state: NetworkState,
}

enum CommandOutcome {
    Continue,
    Shutdown,
}

impl NetworkActor {
    fn handle_command(&mut self, cmd: NetworkCommand) -> Result<CommandOutcome, CommandError> {
        match cmd {
            NetworkCommand::Shutdown => return Ok(CommandOutcome::Shutdown),

            NetworkCommand::Dial { addr } => {
                self.swarm
                    .dial(addr.clone())
                    .map_err(|e| CommandError::DialFailed {
                        addr: addr.to_string(),
                        reason: e.to_string(),
                    })?;
            }

            NetworkCommand::Subscribe { topic } => {
                let ident = gossipsub::IdentTopic::new(topic.as_ref());
                self.swarm
                    .behaviour_mut()
                    .gossipsub
                    .subscribe(&ident)
                    .map_err(|e| CommandError::SubscribeFailed {
                        topic: topic.to_string(),
                        reason: e.to_string(),
                    })?;
                self.state.add_subscription(&topic);
            }

            NetworkCommand::Unsubscribe { topic } => {
                let ident = gossipsub::IdentTopic::new(topic.as_ref());
                if !self.swarm.behaviour_mut().gossipsub.unsubscribe(&ident) {
                    return Err(CommandError::UnsubscribeFailed {
                        topic: topic.to_string(),
                        reason: "not subscribed".into(),
                    });
                }
                self.state.remove_subscription(&topic);
            }

            NetworkCommand::Publish { topic, payload } => {
                if !self.state.is_subscribed(&topic) {
                    return Err(CommandError::PublishFailed {
                        topic: topic.to_string(),
                        reason: "not subscribed".into(),
                    });
                }
                let ident = gossipsub::IdentTopic::new(topic.as_ref());
                self.swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(ident, payload.as_bytes().to_vec())
                    .map_err(|e| CommandError::PublishFailed {
                        topic: topic.to_string(),
                        reason: e.to_string(),
                    })?;
            }

            NetworkCommand::PutRecord { key, value } => {
                let record = libp2p::kad::Record {
                    key: RecordKey::new(&key.as_str()),
                    value: value.into_bytes(),
                    publisher: Some(self.state.local_peer_id),
                    expires: None,
                };
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .put_record(record, libp2p::kad::Quorum::One)
                    .map_err(|e| CommandError::DhtFailed {
                        key: key.to_string(),
                        reason: e.to_string(),
                    })?;
                self.state
                    .track_query(query_id, PendingQuery::PutRecord(key));
            }

            NetworkCommand::GetRecord { key } => {
                let record_key = RecordKey::new(&key.as_str());
                let query_id = self.swarm.behaviour_mut().kademlia.get_record(record_key);
                self.state
                    .track_query(query_id, PendingQuery::GetRecord(key));
            }

            NetworkCommand::StartProviding { key } => {
                let record_key = RecordKey::new(&key.as_str());
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(record_key)
                    .map_err(|e| CommandError::DhtFailed {
                        key: key.to_string(),
                        reason: e.to_string(),
                    })?;
                self.state
                    .track_query(query_id, PendingQuery::StartProviding(key));
            }

            NetworkCommand::GetProviders { key } => {
                let record_key = RecordKey::new(&key.as_str());
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_providers(record_key);
                self.state
                    .track_query(query_id, PendingQuery::GetProviders(key));
            }
        }
        Ok(CommandOutcome::Continue)
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<PeernetBehaviourEvent>) {
        match event {
            SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
                ..
            } => {
                if num_established.get() == 1 {
                    self.state.connected_peers.insert(peer_id);
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .add_explicit_peer(&peer_id);
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, endpoint.get_remote_address().clone());
                    self.state
                        .emit(NetworkEvent::PeerConnected { peer_id })
                        .await;
                }
            }

            SwarmEvent::ConnectionClosed {
                peer_id,
                num_established,
                ..
            } => {
                if num_established == 0 {
                    self.state.connected_peers.remove(&peer_id);
                    self.state
                        .emit(NetworkEvent::PeerDisconnected { peer_id })
                        .await;
                }
            }

            SwarmEvent::NewListenAddr { address, .. } => {
                self.state.emit(NetworkEvent::Listening { address }).await;
            }

            SwarmEvent::Behaviour(PeernetBehaviourEvent::Kademlia(event)) => {
                KademliaHandler::handle(&mut self.state, event).await;
            }

            SwarmEvent::Behaviour(PeernetBehaviourEvent::Gossipsub(event)) => {
                GossipsubHandler::handle(&mut self.state, event).await;
            }

            SwarmEvent::Behaviour(PeernetBehaviourEvent::Mdns(event)) => {
                MdnsHandler::handle(&mut self.state, &mut self.swarm, event).await;
            }

            _ => {}
        }
    }
}

async fn await_first_listen_addr(swarm: &mut libp2p::Swarm<PeernetBehaviour>) -> Multiaddr {
    loop {
        if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
            return address;
        }
    }
}

async fn run_network_loop(
    config: NetworkConfig,
    mut command_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
    cancel_token: CancellationToken,
) {
    let initial_topics = config.swarm.initial_topics.clone();

    let (mut swarm, local_peer_id) = match build_swarm(config.swarm) {
        Ok(result) => result,
        Err(e) => {
            warn!(?e, "failed to build swarm");
            let _ = event_tx.send(NetworkEvent::ShutdownComplete).await;
            return;
        }
    };

    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.port)
        .parse()
        .expect("valid multiaddr");

    if let Err(e) = swarm.listen_on(listen_addr.clone()) {
        warn!(?e, "failed to listen");
        let _ = event_tx.send(NetworkEvent::ShutdownComplete).await;
        return;
    }

    let actual_addr = await_first_listen_addr(&mut swarm).await;

    let state = NetworkState::new(local_peer_id, event_tx.clone());
    let mut actor = NetworkActor { swarm, state };

    for topic in initial_topics {
        let ident = gossipsub::IdentTopic::new(topic.as_ref());
        let _ = actor.swarm.behaviour_mut().gossipsub.subscribe(&ident);
        actor.state.add_subscription(&topic);
    }

    let _ = event_tx
        .send(NetworkEvent::Started {
            local_peer_id,
            listening_on: actual_addr,
        })
        .await;

    loop {
        tokio::select! {
            () = cancel_token.cancelled() => break,

            Some(cmd) = command_rx.recv() => {
                match actor.handle_command(cmd) {
                    Ok(CommandOutcome::Shutdown) => break,
                    Ok(CommandOutcome::Continue) => {}
                    Err(e) => {
                        actor.state.emit(NetworkEvent::CommandFailed {
                            reason: e.to_string(),
                        }).await;
                    }
                }
            }

            event = actor.swarm.select_next_some() => {
                actor.handle_swarm_event(event).await;
            }
        }
    }

    let _ = event_tx.send(NetworkEvent::ShutdownComplete).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn network_actor_starts() {
        let cancel = CancellationToken::new();
        let mut handle = spawn(NetworkConfig::default(), cancel.clone());

        let event = timeout(Duration::from_secs(5), handle.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            NetworkEvent::Started { local_peer_id, .. } => {
                assert!(local_peer_id.to_string().starts_with("12D3K"));
            }
            other => panic!("expected Started, got {other:?}"),
        }

        cancel.cancel();
    }

    #[tokio::test]
    async fn network_actor_shuts_down() {
        let cancel = CancellationToken::new();
        let mut handle = spawn(NetworkConfig::default(), cancel);

        let _ = timeout(Duration::from_secs(5), handle.recv()).await;
        handle.shutdown().await.unwrap();

        loop {
            match timeout(Duration::from_secs(5), handle.recv()).await {
                Ok(Some(NetworkEvent::ShutdownComplete)) => break,
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(_) => panic!("timeout waiting for ShutdownComplete"),
            }
        }
    }
}
