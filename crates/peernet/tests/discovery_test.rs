mod common;

use common::{DEFAULT_TIMEOUT, TestNode, wait_for_connection, wait_for_peer_count};
use peernet_core::NetworkEvent;
use std::time::Duration;

#[tokio::test]
async fn two_nodes_connect() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn three_nodes_mesh() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;
    let mut node3 = TestNode::spawn("node3").await;

    wait_for_peer_count(&mut node1, 2).await;
    wait_for_peer_count(&mut node2, 2).await;
    wait_for_peer_count(&mut node3, 2).await;

    node1.shutdown().await;
    node2.shutdown().await;
    node3.shutdown().await;
}

#[tokio::test]
async fn discovery_before_connection() {
    let mut node1 = TestNode::spawn("node1").await;
    let node2 = TestNode::spawn("node2").await;

    let target = node2.peer_id;
    let mut discovered = false;
    let mut connected = false;
    let deadline = tokio::time::Instant::now() + DEFAULT_TIMEOUT;

    while !connected {
        if tokio::time::Instant::now() > deadline {
            panic!("timeout");
        }
        match node1.recv_timeout(Duration::from_millis(200)).await {
            Some(NetworkEvent::PeerDiscovered { peer_id }) if peer_id == target => {
                discovered = true;
            }
            Some(NetworkEvent::PeerConnected { peer_id }) if peer_id == target => {
                connected = true;
            }
            _ => continue,
        }
    }

    assert!(discovered);

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn handles_disconnection() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    let node2_id = node2.peer_id;

    wait_for_connection(&mut node1, &mut node2).await;
    node2.shutdown().await;

    let deadline = tokio::time::Instant::now() + DEFAULT_TIMEOUT;
    loop {
        if tokio::time::Instant::now() > deadline {
            panic!("timeout waiting for disconnect");
        }
        match node1.recv_timeout(Duration::from_millis(200)).await {
            Some(NetworkEvent::PeerDisconnected { peer_id }) if peer_id == node2_id => break,
            _ => continue,
        }
    }

    node1.shutdown().await;
}

#[tokio::test]
async fn late_joiner_discovers() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;

    let mut node3 = TestNode::spawn("node3").await;
    wait_for_peer_count(&mut node3, 2).await;

    node1.shutdown().await;
    node2.shutdown().await;
    node3.shutdown().await;
}
