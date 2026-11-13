//! Pure fixed-order probe arguments; raw declarations are validated after the process gate.
use cyanrex_engine::{
    platform_host::PlatformProcessIdentity,
    services::{
        federation_discovery::{lan::LanSource, PeerAdvertisement, MAX_ADVERTISEMENT_BYTES},
        federation_mdns::{MdnsAnnouncement, MdnsConfig},
    },
};
use std::{ffi::OsString, net::Ipv4Addr, net::SocketAddrV4, time::Duration};

pub const USAGE: &str = concat!(
    "usage: cyanrex-discovery probe-lan --uid N --gid N --groups CSV|none ",
    "--interface IFACE --address RFC1918 --timeout-ms 1000..2000 ",
    "--instance LABEL --host LABEL --port N --ttl 1..120 --advertisement F0_JSON"
);

pub struct Options {
    pub identity: PlatformProcessIdentity,
    pub config: MdnsConfig,
    instance: String,
    host: String,
    port: u16,
    ttl: u32,
    advertisement: String,
}

impl Options {
    /// Strictly decode the original bytes and prepare frozen claims at a supplied time, with no I/O.
    /// The CLI never supplies or refreshes the declaration's issued/expiry times or identity.
    pub fn prepare_at(&self, now: u64) -> Result<MdnsAnnouncement, ()> {
        let advertisement =
            PeerAdvertisement::parse(self.advertisement.as_bytes()).map_err(|_| ())?;
        MdnsAnnouncement::new(
            &self.instance,
            &self.host,
            SocketAddrV4::new(self.config.local_address(), self.port),
            &advertisement,
            now,
            self.ttl,
        )
        .map_err(|_| ())
    }
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

/// No defaults, aliases, inferred identity, input files or declaration normalization are supplied.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Options, ()> {
    let args = args.into_iter().take(24).collect::<Vec<_>>();
    if args.len() != 23 {
        return Err(());
    }
    let mut total = 0_usize;
    let text = args
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            let value = argument.to_str().ok_or(())?;
            total = total.checked_add(value.len()).ok_or(())?;
            let maximum = if index == 22 {
                MAX_ADVERTISEMENT_BYTES
            } else {
                2_048
            };
            if value.len() > maximum || total > 16_384 {
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
        "--timeout-ms",
        "--instance",
        "--host",
        "--port",
        "--ttl",
        "--advertisement",
    ];
    if text[0] != "probe-lan"
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
    let timeout = positive(text[12])?;
    if !(1000..=2000).contains(&timeout) {
        return Err(());
    }
    let config = MdnsConfig::new(text[8], address, Duration::from_millis(u64::from(timeout)))
        .map_err(|_| ())?;
    // Reuse the same pure canonical-label rule as announcement preparation, never an OS epoch.
    LanSource::new(1, text[14]).map_err(|_| ())?;
    LanSource::new(1, text[16]).map_err(|_| ())?;
    let port = u16::try_from(positive(text[18])?).map_err(|_| ())?;
    let ttl = positive(text[20])?;
    if !(1..=120).contains(&ttl) {
        return Err(());
    }
    Ok(Options {
        identity,
        config,
        instance: text[14].to_owned(),
        host: text[16].to_owned(),
        port,
        ttl,
        advertisement: text[22].to_owned(),
    })
}
