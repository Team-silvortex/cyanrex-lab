//! DNS wire preparation and explicit Linux IPv4 discovery/probing, never peer trust.
//! The pure F0/F1a modules remain independent of this networking adapter.
//! Browse, probe, finite retries and retained receive-only review keep distinct runtime owners.
use crate::services::federation_discovery::{lan::LanDiscoveryError, DiscoveryError};
use std::{net::Ipv4Addr, time::Duration};

mod announcement;
mod controller;
mod interface;
mod packet;
mod probing;
mod runtime;
mod socket;
mod wire;

pub use announcement::{
    AnnouncementStep, MdnsAnnouncement, MdnsAnnouncementSchedule, MdnsQueryContext, MdnsQueryPlan,
    MdnsQueryRecordSet, MdnsQueryResponse, ProbeObservation, ResponseConflictObservation,
};
pub use controller::{MdnsProbeController, ProbeControllerState};
pub use interface::VerifiedInterface;
pub use packet::{
    browse_query, parse_response, MdnsEvent, MAX_MDNS_PACKET_BYTES, MAX_MDNS_RECORDS,
};
pub use probing::{probe_once, ProbeRunObservation};
pub use probing::{probe_retained_once, MdnsRetainedProbe, RetainedProbeOutcome};
pub use runtime::discover_once;
pub use socket::{MdnsSocket, MdnsUnicastReceiver, ReceivedMdns};

pub const MDNS_GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
pub const MDNS_PORT: u16 = 5353;
pub const MAX_PACKETS_PER_RUN: usize = 256;
pub const MAX_PACKETS_PER_SECOND: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MdnsError {
    #[error("invalid mDNS configuration")]
    InvalidConfig,
    #[error("invalid mDNS interface")]
    InvalidInterface,
    #[error("mDNS interface changed")]
    InterfaceChanged,
    #[error("mDNS socket unavailable")]
    SocketUnavailable,
    #[error("invalid mDNS packet")]
    InvalidPacket,
    #[error("mDNS packet exceeds its bound")]
    TooLarge,
    #[error("mDNS TXT rejected")]
    RejectedTxt(#[from] LanDiscoveryError),
    #[error("mDNS advertisement rejected")]
    RejectedAdvertisement(#[from] DiscoveryError),
    #[error("mDNS cancelled")]
    Cancelled,
    #[error("mDNS is closed")]
    Closed,
    #[error("mDNS receive budget exhausted")]
    BudgetExceeded,
    #[error("invalid mDNS clock")]
    ClockInvalid,
    #[error("mDNS probe deadline exceeded")]
    DeadlineExceeded,
    #[error("mDNS probe entropy unavailable")]
    EntropyUnavailable,
    #[error("mDNS probe retry is cooling down")]
    CoolingDown,
}
type Result<T> = std::result::Result<T, MdnsError>;

/// Constructing configuration never looks up interfaces or opens sockets.
#[derive(Debug, Clone)]
pub struct MdnsConfig {
    interface: String,
    local: Ipv4Addr,
    timeout: Duration,
}

impl MdnsConfig {
    pub fn new(interface: &str, local: Ipv4Addr, timeout: Duration) -> Result<Self> {
        if interface.is_empty()
            || interface.len() > 15
            || !interface
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            || !interface
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
            || !local.is_private()
            || timeout < Duration::from_millis(1)
            || timeout > Duration::from_secs(5)
        {
            return Err(MdnsError::InvalidConfig);
        }
        Ok(Self {
            interface: interface.to_owned(),
            local,
            timeout,
        })
    }
    pub fn interface_name(&self) -> &str {
        &self.interface
    }
    pub fn local_address(&self) -> Ipv4Addr {
        self.local
    }
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}
