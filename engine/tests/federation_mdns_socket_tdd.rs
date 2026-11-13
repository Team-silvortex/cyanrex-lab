#![cfg(target_os = "linux")]
//! Ordinary cases inspect only configuration/metadata; never open a discovery socket on host LAN.

use cyanrex_engine::services::federation_mdns::{MdnsConfig, MdnsError, VerifiedInterface};
use std::{net::Ipv4Addr, time::Duration};

#[test]
fn mdns_config_keeps_an_explicit_interface_private_address_and_bounded_timeout() {
    let config = MdnsConfig::new(
        "lan-0.2",
        Ipv4Addr::new(10, 1, 2, 3),
        Duration::from_millis(50),
    )
    .unwrap();
    assert_eq!(config.interface_name(), "lan-0.2");
    assert_eq!(config.local_address(), Ipv4Addr::new(10, 1, 2, 3));
    assert_eq!(config.timeout(), Duration::from_millis(50));
}

#[test]
fn mdns_config_never_selects_an_interface_or_address_by_default() {
    for name in [
        "",
        "eth0\0",
        "eth 0",
        "eth0/1",
        "eth0:1",
        "éth0",
        "abcdefghijklmnop",
    ] {
        assert!(matches!(
            MdnsConfig::new(name, Ipv4Addr::new(10, 1, 2, 3), Duration::from_millis(50)),
            Err(MdnsError::InvalidConfig)
        ));
    }
    for address in [
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::LOCALHOST,
        Ipv4Addr::new(169, 254, 1, 2),
        Ipv4Addr::new(8, 8, 8, 8),
        Ipv4Addr::new(224, 0, 0, 251),
    ] {
        assert!(matches!(
            MdnsConfig::new("eth0", address, Duration::from_millis(50)),
            Err(MdnsError::InvalidConfig)
        ));
    }
    for timeout in [
        Duration::ZERO,
        Duration::from_nanos(999_999),
        Duration::from_millis(5_001),
    ] {
        assert!(matches!(
            MdnsConfig::new("eth0", Ipv4Addr::new(10, 1, 2, 3), timeout),
            Err(MdnsError::InvalidConfig)
        ));
    }
}

#[test]
fn mdns_inspection_rejects_loopback_and_missing_interfaces_without_opening_sockets() {
    for name in ["lo", "zz-no-cyanrex-0"] {
        let config =
            MdnsConfig::new(name, Ipv4Addr::new(10, 1, 2, 3), Duration::from_millis(50)).unwrap();
        assert!(matches!(
            VerifiedInterface::inspect(&config),
            Err(MdnsError::InvalidInterface)
        ));
    }
}
