//! Conservative literal-address policy, not DNS, routing, reachability or SSRF authorization.
//! The static public-candidate rule does not prove current IANA allocation or LAN proximity.
use super::{DiscoveryError, Result};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Class {
    Private,
    Public,
}

pub(super) fn parse(value: &str) -> Result<SocketAddr> {
    let address: SocketAddr = value
        .parse()
        .map_err(|_| DiscoveryError::InvalidAdvertisement)?;
    if address.port() == 0
        || address.to_string() != value
        || matches!(address, SocketAddr::V6(v6) if v6.scope_id() != 0 || v6.flowinfo() != 0)
        || classify(address).is_none()
    {
        return Err(DiscoveryError::InvalidAdvertisement);
    }
    Ok(address)
}

pub(super) fn classify(address: SocketAddr) -> Option<Class> {
    match address.ip() {
        IpAddr::V4(ip) => ipv4(ip),
        IpAddr::V6(ip) => ipv6(ip),
    }
}

fn ipv4(ip: Ipv4Addr) -> Option<Class> {
    let [a, b, c, _] = ip.octets();
    if a == 10 || a == 172 && (16..=31).contains(&b) || a == 192 && b == 168 {
        return Some(Class::Private);
    }
    if a == 0
        || a == 127
        || a == 100 && (64..=127).contains(&b)
        || a == 169 && b == 254
        || a == 192 && b == 0 && matches!(c, 0 | 2)
        || a == 192 && b == 88 && c == 99
        || a == 198 && matches!(b, 18 | 19)
        || a == 198 && b == 51 && c == 100
        || a == 203 && b == 0 && c == 113
        || a >= 224
    {
        return None;
    }
    Some(Class::Public)
}

fn ipv6(ip: Ipv6Addr) -> Option<Class> {
    let segments = ip.segments();
    if segments[0] & 0xfe00 == 0xfc00 {
        return Some(Class::Private);
    }
    // Only global-unicast space is eligible. Exclude conservative special-use aggregates,
    // documentation and 6to4; this also rejects mapped/compatible/NAT64/link/site-local values.
    if segments[0] & 0xe000 != 0x2000
        || segments[0] == 0x2001 && segments[1] < 0x0200
        || segments[0] == 0x2001 && segments[1] == 0x0db8
        || segments[0] == 0x2002
        || segments[0] == 0x3fff && segments[1] & 0xf000 == 0
    {
        return None;
    }
    Some(Class::Public)
}
