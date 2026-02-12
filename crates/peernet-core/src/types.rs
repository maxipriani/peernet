use crate::PeernetError;
use derive_more::{AsRef, Deref, Display};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, AsRef, Deref)]
pub struct TopicName(String);

impl TopicName {
    pub fn new(s: impl Into<String>) -> Result<Self, PeernetError> {
        let s = s.into();
        if s.is_empty() {
            return Err(PeernetError::ValidationFailed {
                field: "topic",
                reason: "cannot be empty",
            });
        }
        if s.len() > 128 {
            return Err(PeernetError::ValidationFailed {
                field: "topic",
                reason: "exceeds 128 characters",
            });
        }
        Ok(Self(s))
    }

    pub fn new_unchecked(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

#[derive(Clone, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct DhtKey(String);

impl DhtKey {
    pub fn new(s: impl Into<String>) -> Result<Self, PeernetError> {
        let s = s.into();
        if s.is_empty() {
            return Err(PeernetError::ValidationFailed {
                field: "dht_key",
                reason: "cannot be empty",
            });
        }
        if s.len() > 256 {
            return Err(PeernetError::ValidationFailed {
                field: "dht_key",
                reason: "exceeds 256 characters",
            });
        }
        if s.contains('\0') {
            return Err(PeernetError::ValidationFailed {
                field: "dht_key",
                reason: "contains null byte",
            });
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DhtKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DhtKey({})", self.0)
    }
}

impl AsRef<[u8]> for DhtKey {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DhtValue(Vec<u8>);

impl DhtValue {
    const MAX_SIZE: usize = 65536;

    pub fn new(data: impl Into<Vec<u8>>) -> Result<Self, PeernetError> {
        let data = data.into();
        if data.len() > Self::MAX_SIZE {
            return Err(PeernetError::ValidationFailed {
                field: "dht_value",
                reason: "exceeds 64KB limit",
            });
        }
        Ok(Self(data))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }
}

impl fmt::Debug for DhtValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DhtValue({} bytes)", self.0.len())
    }
}

impl AsRef<[u8]> for DhtValue {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GossipPayload(Vec<u8>);

impl GossipPayload {
    const MAX_SIZE: usize = 1048576;

    pub fn new(data: impl Into<Vec<u8>>) -> Result<Self, PeernetError> {
        let data = data.into();
        if data.len() > Self::MAX_SIZE {
            return Err(PeernetError::ValidationFailed {
                field: "gossip_payload",
                reason: "exceeds 1MB limit",
            });
        }
        Ok(Self(data))
    }

    pub fn from_text(s: &str) -> Result<Self, PeernetError> {
        Self::new(s.as_bytes().to_vec())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }
}

impl fmt::Debug for GossipPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Some(s) if s.len() <= 50 => write!(f, "GossipPayload({s:?})"),
            Some(s) => write!(f, "GossipPayload({:?}...)", &s[..50]),
            None => write!(f, "GossipPayload({} bytes)", self.0.len()),
        }
    }
}

impl AsRef<[u8]> for GossipPayload {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dht_key_validates_empty() {
        assert!(DhtKey::new("").is_err());
    }

    #[test]
    fn dht_key_validates_length() {
        let long_key = "x".repeat(257);
        assert!(DhtKey::new(long_key).is_err());
    }

    #[test]
    fn dht_key_validates_null_byte() {
        assert!(DhtKey::new("hello\0world").is_err());
    }

    #[test]
    fn dht_key_accepts_valid() {
        assert!(DhtKey::new("valid-key").is_ok());
    }

    #[test]
    fn dht_value_validates_size() {
        let big_value = vec![0u8; 65537];
        assert!(DhtValue::new(big_value).is_err());
    }

    #[test]
    fn topic_name_validates_empty() {
        assert!(TopicName::new("").is_err());
    }
}
