//! F1a: pure LAN data admission, finite caching and TXT framing, not an mDNS adapter.
//!
//! Sources are trusted caller-supplied interface incarnations, not discovered identities or OS
//! interface indices. The caller owns packet provenance and clock sampling. No socket, interface,
//! clock, environment or persistence is accessed; candidates remain unverified F0 observations.

use super::{DiscoveryError, DiscoveryPolicy, PeerAdvertisement, UnverifiedPeerCandidate};

mod cache;
mod source;
mod txt;

pub use cache::{LanCandidateCache, LanDiscoveredCandidate, LanObservation};
pub use source::{CacheTime, LanSource};
pub use txt::{decode_txt, encode_txt, MAX_TXT_CHUNKS, MAX_TXT_CHUNK_BYTES, MAX_TXT_RECORD_BYTES};

pub const MAX_LAN_CANDIDATES: usize = 64;
pub const MAX_LAN_INTERFACES: usize = 8;
pub const MAX_LAN_TTL_SECONDS: u32 = 120;
pub const MAX_LAN_EVENTS_PER_SECOND: u32 = 64;

/// Fixed classifications: retain no packet, source-label, parser or private transport error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LanDiscoveryError {
    #[error("invalid LAN discovery configuration")]
    InvalidConfig,
    #[error("invalid LAN discovery source")]
    InvalidSource,
    #[error("invalid LAN discovery TXT data")]
    InvalidTxt,
    #[error("LAN discovery data exceeds its bound")]
    TooLarge,
    #[error("invalid LAN discovery TTL")]
    InvalidTtl,
    #[error("invalid LAN discovery time")]
    InvalidTime,
    #[error("LAN discovery clock rollback")]
    ClockRollback,
    #[error("LAN discovery is closed")]
    Closed,
    #[error("LAN discovery event rate exhausted")]
    RateLimited,
    #[error("LAN discovery candidate capacity exhausted")]
    Capacity,
    #[error("LAN discovery source conflicts with its live observation")]
    SourceConflict,
    #[error("LAN discovery advertisement rejected")]
    RejectedAdvertisement(#[from] DiscoveryError),
}

type Result<T> = std::result::Result<T, LanDiscoveryError>;
