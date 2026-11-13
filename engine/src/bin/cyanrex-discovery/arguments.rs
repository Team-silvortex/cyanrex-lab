//! Bounded explicit CLI values only; parsing performs no process, interface or network work.
use cyanrex_engine::{
    models::collaboration::QualifiedName,
    platform_host::PlatformProcessIdentity,
    services::{
        federation_discovery::{DiscoveryPolicy, ProtocolRange, MAX_CAPABILITIES},
        federation_mdns::MdnsConfig,
    },
};
use std::{ffi::OsString, net::Ipv4Addr, time::Duration};

pub const USAGE: &str = concat!(
    "usage: cyanrex-discovery browse-lan --uid N --gid N --groups GIDS|none ",
    "--interface IFACE --address RFC1918 --local-node SHA64 --protocol MIN:MAX ",
    "--timeout-ms 1..5000 --capabilities CSV|none --require CSV|none"
);

pub struct Options {
    pub identity: PlatformProcessIdentity,
    pub config: MdnsConfig,
    pub policy: DiscoveryPolicy,
}

fn positive(value: &str) -> Result<u32, ()> {
    if value.is_empty()
        || value.len() > 10
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(());
    }
    value.parse().map_err(|_| ())
}

fn capabilities(value: &str) -> Result<Vec<QualifiedName>, ()> {
    if value == "none" {
        return Ok(Vec::new());
    }
    let names = value
        .split(',')
        .take(MAX_CAPABILITIES + 1)
        .map(|name| name.parse().map_err(|_| ()))
        .collect::<Result<Vec<_>, _>>()?;
    if names.len() > MAX_CAPABILITIES {
        return Err(());
    }
    Ok(names)
}

/// Accept one complete fixed command, never defaults, aliases, inferred identity or hint opt-ins.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Options, ()> {
    // Read at most one excess argument, including when a supplied iterator has no end.
    let args = args.into_iter().take(22).collect::<Vec<_>>();
    if args.len() != 21 {
        return Err(());
    }
    let mut total = 0_usize;
    let text = args
        .iter()
        .map(|argument| {
            let value = argument.to_str().ok_or(())?;
            total = total.checked_add(value.len()).ok_or(())?;
            if value.len() > 2_048 || total > 16_384 {
                return Err(());
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let flags = [
        "--uid",
        "--gid",
        "--groups",
        "--interface",
        "--address",
        "--local-node",
        "--protocol",
        "--timeout-ms",
        "--capabilities",
        "--require",
    ];
    if text[0] != "browse-lan"
        || flags
            .iter()
            .enumerate()
            .any(|(index, flag)| text[index * 2 + 1] != *flag)
    {
        return Err(());
    }
    let groups = if text[6] == "none" {
        Vec::new()
    } else {
        text[6]
            .split(',')
            .take(129)
            .map(positive)
            .collect::<Result<Vec<_>, _>>()?
    };
    let identity = PlatformProcessIdentity::new(positive(text[2])?, positive(text[4])?, &groups)
        .map_err(|_| ())?;
    let address: Ipv4Addr = text[10].parse().map_err(|_| ())?;
    if address.to_string() != text[10] {
        return Err(());
    }
    let config = MdnsConfig::new(
        text[8],
        address,
        Duration::from_millis(u64::from(positive(text[16])?)),
    )
    .map_err(|_| ())?;
    let (min, max) = text[14].split_once(':').ok_or(())?;
    let protocol = ProtocolRange::new(
        positive(min)?.try_into().map_err(|_| ())?,
        positive(max)?.try_into().map_err(|_| ())?,
    )
    .map_err(|_| ())?;
    let policy = DiscoveryPolicy::new(
        text[12].parse().map_err(|_| ())?,
        protocol,
        capabilities(text[18])?,
        capabilities(text[20])?,
    )
    .map_err(|_| ())?;
    Ok(Options {
        identity,
        config,
        policy,
    })
}
