mod common;

use common::{
    DEFAULT_TIMEOUT, TestNode, drain_events, expect_record_found, expect_record_stored,
    wait_for_connection, wait_for_peer_count,
};
use peernet_core::NetworkEvent;
use std::time::Duration;

#[tokio::test]
async fn put_get_same_node() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    drain_events(&mut node1).await;

    let key = "test-key";
    let value = "test-value";
    node1.put(key, value).await;

    expect_record_stored(&mut node1, key).await;

    node1.get(key).await;
    let retrieved = expect_record_found(&mut node1, key).await;

    assert_eq!(retrieved, value.as_bytes());

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn put_one_get_another() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    drain_events(&mut node1).await;
    drain_events(&mut node2).await;

    let key = "shared";
    let value = r#"{"data": true}"#;
    node1.put(key, value).await;

    expect_record_stored(&mut node1, key).await;

    tokio::time::sleep(Duration::from_secs(1)).await;

    node2.get(key).await;
    let retrieved = expect_record_found(&mut node2, key).await;

    assert_eq!(retrieved, value.as_bytes());

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn third_node_retrieves() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    drain_events(&mut node1).await;

    let key = "persistent";
    let value = "survives";
    node1.put(key, value).await;
    expect_record_stored(&mut node1, key).await;

    let mut node3 = TestNode::spawn("node3").await;
    wait_for_peer_count(&mut node3, 2).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    drain_events(&mut node3).await;

    node3.get(key).await;
    let retrieved = expect_record_found(&mut node3, key).await;

    assert_eq!(retrieved, value.as_bytes());

    node1.shutdown().await;
    node2.shutdown().await;
    node3.shutdown().await;
}

#[tokio::test]
async fn overwrite_key() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    drain_events(&mut node1).await;

    let key = "version";

    node1.put(key, "v1").await;
    expect_record_stored(&mut node1, key).await;

    node1.put(key, "v2").await;
    expect_record_stored(&mut node1, key).await;

    node1.get(key).await;
    let value = expect_record_found(&mut node1, key).await;

    assert_eq!(value, b"v2");

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn nonexistent_key() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    drain_events(&mut node1).await;

    let key = "missing";
    node1.get(key).await;

    let deadline = tokio::time::Instant::now() + DEFAULT_TIMEOUT;
    loop {
        if tokio::time::Instant::now() > deadline {
            break;
        }
        match node1.recv_timeout(Duration::from_millis(200)).await {
            Some(NetworkEvent::RecordNotFound { key: k }) if k.as_str() == key => break,
            Some(NetworkEvent::RecordFound { key: k, .. }) if k.as_str() == key => {
                panic!("found nonexistent key");
            }
            _ => continue,
        }
    }

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn multiple_keys() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    drain_events(&mut node1).await;

    let pairs = vec![("k1", "v1"), ("k2", "v2"), ("k3", "v3")];

    for (k, v) in &pairs {
        node1.put(k, v).await;
        expect_record_stored(&mut node1, k).await;
    }

    for (k, v) in &pairs {
        node1.get(k).await;
        let value = expect_record_found(&mut node1, k).await;
        assert_eq!(value, v.as_bytes());
    }

    node1.shutdown().await;
    node2.shutdown().await;
}

#[tokio::test]
async fn binary_data() {
    let mut node1 = TestNode::spawn("node1").await;
    let mut node2 = TestNode::spawn("node2").await;

    wait_for_connection(&mut node1, &mut node2).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    drain_events(&mut node1).await;

    let key = "binary";
    let value: &[u8] = b"\x00\x01\x02\xff\xfe";
    node1.put_bytes(key, value).await;
    expect_record_stored(&mut node1, key).await;

    node1.get(key).await;
    let retrieved = expect_record_found(&mut node1, key).await;

    assert_eq!(retrieved, value);

    node1.shutdown().await;
    node2.shutdown().await;
}
