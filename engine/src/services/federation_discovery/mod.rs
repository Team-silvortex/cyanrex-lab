//! F0: bounded untrusted discovery claims and pure local candidate selection.
//!
//! No socket, clock, environment, registry, Session or content service is accessed. A selected
//! candidate is neither a verified peer identity nor registration, pairing or workspace authority.
//! `NodeFingerprint` is an independent claimed label; it is not a verified key or `AuthorityId`.
//! Network discovery, key proofs/rotation, rendezvous, NAT traversal and relays remain separate.

use crate::models::collaboration::{AuthorityId, QualifiedName};
use std::{fmt, net::SocketAddr, str::FromStr};

mod endpoint;
pub mod lan;
mod policy;
mod wire;

pub const MAX_ADVERTISEMENT_BYTES: usize = 4_096;
pub const MAX_HINTS: usize = 8;
pub const MAX_CAPABILITIES: usize = 16;
pub const MAX_LEASE_SECONDS: u64 = 300;
pub const MAX_FUTURE_SKEW_SECONDS: u64 = 5;
const MAX_SAFE_TIME: u64 = 9_007_199_254_740_991;

/// Static classifications only: untrusted names, addresses and parser errors are never retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DiscoveryError {
    #[error("invalid federation advertisement")]
    InvalidAdvertisement,
    #[error("invalid federation discovery policy")]
    InvalidPolicy,
    #[error("federation advertisement expired")]
    Expired,
    #[error("unsupported federation discovery protocol")]
    UnsupportedProtocol,
    #[error("required federation capability is unavailable")]
    MissingCapability,
    #[error("self federation advertisement")]
    SelfAdvertisement,
    #[error("no locally allowed federation connection hint")]
    NoAllowedHint,
    #[error("invalid federation observation time")]
    InvalidTime,
}

type Result<T> = std::result::Result<T, DiscoveryError>;

/// A canonical 64-lowercase-hex claim, not a digest verification or possession proof.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeFingerprint(String);

impl NodeFingerprint {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for NodeFingerprint {
    type Err = DiscoveryError;

    fn from_str(value: &str) -> Result<Self> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DiscoveryError::InvalidAdvertisement);
        }
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for NodeFingerprint {
    type Error = DiscoveryError;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

impl fmt::Display for NodeFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolRange {
    min: u16,
    max: u16,
}

impl ProtocolRange {
    pub fn new(min: u16, max: u16) -> Result<Self> {
        if min == 0 || min > max {
            return Err(DiscoveryError::InvalidPolicy);
        }
        Ok(Self { min, max })
    }
}

/// Inert hints only. Relay names resolve exclusively through future trusted local configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConnectionHint {
    Direct(SocketAddr),
    Relay(QualifiedName),
}

#[derive(Debug, Clone)]
pub struct DiscoveryPolicy {
    local_node: NodeFingerprint,
    protocol: ProtocolRange,
    supported: Vec<QualifiedName>,
    required: Vec<QualifiedName>,
    public_direct: bool,
    relays: Vec<QualifiedName>,
}

/// Parsed claims have no public-field or deserialization route around the bounded raw parser.
#[derive(Debug, Clone)]
pub struct PeerAdvertisement {
    node: NodeFingerprint,
    authority: AuthorityId,
    protocol: ProtocolRange,
    offered: Vec<QualifiedName>,
    required: Vec<QualifiedName>,
    issued_at: u64,
    expires_at: u64,
    hints: Vec<ConnectionHint>,
}

/// Compatibility and local hint-policy result only; never authenticated or authorized authority.
#[derive(Debug, Clone)]
pub struct UnverifiedPeerCandidate {
    node: NodeFingerprint,
    authority: AuthorityId,
    protocol: u16,
    capabilities: Vec<QualifiedName>,
    hints: Vec<ConnectionHint>,
    expires_at: u64,
}

impl UnverifiedPeerCandidate {
    pub fn claimed_node(&self) -> &NodeFingerprint {
        &self.node
    }
    pub fn claimed_authority(&self) -> AuthorityId {
        self.authority
    }
    pub fn protocol(&self) -> u16 {
        self.protocol
    }
    pub fn capabilities(&self) -> &[QualifiedName] {
        &self.capabilities
    }
    pub fn hints(&self) -> &[ConnectionHint] {
        &self.hints
    }
    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }
}
