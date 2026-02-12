use thiserror::Error;

pub type PeernetResult<T> = Result<T, PeernetError>;

#[derive(Debug, Error)]
pub enum PeernetError {
    #[error("validation failed: {field} {reason}")]
    ValidationFailed {
        field: &'static str,
        reason: &'static str,
    },

    #[error("send failed to {actor} actor: channel closed")]
    SendFailed { actor: &'static str },

    #[error("channel closed: {actor} actor, {reason}")]
    ChannelClosed {
        actor: &'static str,
        reason: &'static str,
    },

    #[error("transport error: {reason}")]
    Transport { reason: String },
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("publish failed on topic {topic}: {reason}")]
    PublishFailed { topic: String, reason: String },

    #[error("subscribe failed on topic {topic}: {reason}")]
    SubscribeFailed { topic: String, reason: String },

    #[error("unsubscribe failed on topic {topic}: {reason}")]
    UnsubscribeFailed { topic: String, reason: String },

    #[error("dial failed to {addr}: {reason}")]
    DialFailed { addr: String, reason: String },

    #[error("dht operation failed for key {key}: {reason}")]
    DhtFailed { key: String, reason: String },
}
