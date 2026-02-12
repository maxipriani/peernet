use crate::behaviour::PeernetBehaviour;
use crate::state::NetworkState;
use libp2p::{Swarm, mdns};
use peernet_core::NetworkEvent;

pub struct MdnsHandler;

impl MdnsHandler {
    pub async fn handle(
        state: &mut NetworkState,
        swarm: &mut Swarm<PeernetBehaviour>,
        event: mdns::Event,
    ) {
        match event {
            mdns::Event::Discovered(peers) => {
                for (peer_id, addr) in peers.into_iter().filter(|(p, _)| {
                    *p != state.local_peer_id && !state.connected_peers.contains(p)
                }) {
                    state.emit(NetworkEvent::PeerDiscovered { peer_id }).await;
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr.clone());
                    swarm.add_peer_address(peer_id, addr);
                    let _ = swarm.dial(peer_id);
                }
            }
            mdns::Event::Expired(_) => {}
        }
    }
}
