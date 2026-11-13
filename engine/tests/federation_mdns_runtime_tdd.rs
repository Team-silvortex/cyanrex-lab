#![cfg(target_os = "linux")]
use cyanrex_engine::services::{
    federation_discovery::{DiscoveryPolicy, ProtocolRange},
    federation_mdns::{discover_once, MdnsConfig, MdnsError},
};
use std::{net::Ipv4Addr, sync::atomic::AtomicBool, time::Duration};

#[test]
fn mdns_configuration_is_explicit_private_and_time_bounded_before_network_work() {
    for name in [
        "",
        ".",
        "..",
        "interface-name-too-long",
        "eth0:1",
        "eth 0",
        "eth0\n",
        "网卡",
    ] {
        assert!(matches!(
            MdnsConfig::new(name, Ipv4Addr::new(10, 2, 3, 4), Duration::from_secs(1)),
            Err(MdnsError::InvalidConfig)
        ));
    }
    for address in [
        Ipv4Addr::UNSPECIFIED,
        Ipv4Addr::LOCALHOST,
        Ipv4Addr::BROADCAST,
        Ipv4Addr::new(8, 8, 8, 8),
        Ipv4Addr::new(169, 254, 169, 254),
        Ipv4Addr::new(224, 0, 0, 251),
    ] {
        assert!(MdnsConfig::new("eth0", address, Duration::from_secs(1)).is_err());
    }
    for timeout in [
        Duration::ZERO,
        Duration::from_nanos(1),
        Duration::from_millis(5001),
    ] {
        assert!(MdnsConfig::new("eth0", Ipv4Addr::new(10, 2, 3, 4), timeout).is_err());
    }
    for name in ["eth0", "enp1s0", "veth-test", "eth0.10"] {
        let config =
            MdnsConfig::new(name, Ipv4Addr::new(10, 2, 3, 4), Duration::from_secs(1)).unwrap();
        assert_eq!(config.interface_name(), name);
        assert_eq!(config.local_address(), Ipv4Addr::new(10, 2, 3, 4));
        assert_eq!(config.timeout(), Duration::from_secs(1));
    }
}

#[test]
fn already_cancelled_discovery_does_not_inspect_or_open_an_interface() {
    let config = MdnsConfig::new(
        "absent-fixture",
        Ipv4Addr::new(10, 2, 3, 4),
        Duration::from_secs(1),
    )
    .unwrap();
    let policy = DiscoveryPolicy::new(
        "a".repeat(64).parse().unwrap(),
        ProtocolRange::new(1, 3).unwrap(),
        vec![],
        vec![],
    )
    .unwrap();
    assert!(matches!(
        discover_once(&config, policy, &AtomicBool::new(true)),
        Err(MdnsError::Cancelled)
    ));
}
