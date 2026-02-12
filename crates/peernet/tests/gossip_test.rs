mod common;

use common::{
    DEFAULT_TIMEOUT, TestNode, drain_events, expect_gossip, wait_for_connection,
    wait_for_peer_count,
};
use peernet_core::NetworkEvent;
use std::time::Duration;

#[tokio::test]
async fn message_propagates() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    drain_events(&mut node2).await;

    let msg = "hello from node1";
    node1.publish(msg).await;

    let event = expect_gossip(&mut node2, msg).await;

    if let NetworkEvent::GossipMessage { source, .. } = event {
        assert_eq!(source, Some(node1.peer_id));
    }

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn bidirectional() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    drain_events(&mut node1).await;
    drain_events(&mut node2).await;

    node1.publish("msg1").await;
    expect_gossip(&mut node2, "msg1").await;

    node2.publish("msg2").await;
    expect_gossip(&mut node1, "msg2").await;

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn floods_to_all() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;
    let mut node3 = TestNode::spawn("node3").await;

    wait_for_peer_count(&mut node1, 2).await;
    wait_for_peer_count(&mut node2, 2).await;
    wait_for_peer_count(&mut node3, 2).await;

    tokio::time::sleep(Duration::from_secs(1)).await;
    drain_events(&mut node1).await;
    drain_events(&mut node2).await;
    drain_events(&mut node3).await;

    let msg = "broadcast";
    node1.publish(msg).await;

    expect_gossip(&mut node2, msg).await;
    expect_gossip(&mut node3, msg).await;

    node1.shutdown().await;
    node2.shutdown().await;
    node3.shutdown().await;
}

#[tokio::test]
async fn deduplication() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    drain_events(&mut node2).await;

    let msg = "duplicate";
    node1.publish(msg).await;
    node1.publish(msg).await;

    expect_gossip(&mut node2, msg).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut dup = false;
    while let Some(event) = node2.recv_timeout(Duration::from_millis(100)).await {
        if let NetworkEvent::GossipMessage { payload, .. } = event
            && payload.as_bytes() == msg.as_bytes()
        {
            dup = true;
        }
    }

    assert!(!dup);

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn multiple_messages() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    drain_events(&mut node2).await;

    let msgs = vec!["msg1", "msg2", "msg3"];
    for m in &msgs {
        node1.publish(m).await;
    }

    let mut received = std::collections::HashSet::new();
    let deadline = tokio::time::Instant::now() + DEFAULT_TIMEOUT;

    while received.len() < msgs.len() {
        if tokio::time::Instant::now() > deadline {
            panic!("timeout");
        }
        match node2.recv_timeout(Duration::from_millis(200)).await {
            Some(NetworkEvent::GossipMessage { payload, .. }) => {
                if let Some(s) = payload.as_str()
                    && msgs.contains(&s)
                {
                    received.insert(s.to_string());
                }
            }
            _ => continue,
        }
    }

    assert_eq!(received.len(), msgs.len());

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn late_joiner_receives_new() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    node1.publish("before").await;

    let mut node3 = TestNode::spawn("node3").await;
    wait_for_peer_count(&mut node3, 2).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    drain_events(&mut node3).await;

    let msg = "after";
    node1.publish(msg).await;

    expect_gossip(&mut node3, msg).await;

    node1.shutdown().await;
    node2.shutdown().await;
    node3.shutdown().await;
}
