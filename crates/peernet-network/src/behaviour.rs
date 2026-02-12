use libp2p::gossipsub;
use libp2p::kad;
use libp2p::mdns;
use libp2p::swarm::NetworkBehaviour;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "PeernetBehaviourEvent")]
pub struct PeernetBehaviour {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}

#[derive(Debug, derive_more::From)]
pub enum PeernetBehaviourEvent {
    Kademlia(kad::Event),
    Gossipsub(gossipsub::Event),
    Mdns(mdns::Event),
}
