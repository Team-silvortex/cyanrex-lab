//! Explicit Linux IPv4 interface observation, never interface selection or configuration.
use super::{MdnsConfig, MdnsError};
use std::{
    ffi::{CStr, CString},
    net::Ipv4Addr,
    os::unix::fs::MetadataExt,
    time::Duration,
};

type Result<T> = std::result::Result<T, MdnsError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedInterface {
    name: String,
    index: u32,
    local: Ipv4Addr,
    mask: Ipv4Addr,
    flags: u32,
    namespace: (u64, u64),
    timeout: Duration,
}

struct AddressList(*mut libc::ifaddrs);
impl Drop for AddressList {
    fn drop(&mut self) {
        // SAFETY: only the successful getifaddrs allocation is owned by this one guard.
        unsafe { libc::freeifaddrs(self.0) };
    }
}

pub(super) fn private(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 10 || a == 172 && (16..=31).contains(&b) || a == 192 && b == 168
}

pub(super) fn host_in_subnet(ip: Ipv4Addr, local: Ipv4Addr, mask: Ipv4Addr) -> bool {
    let (ip, local, mask) = (u32::from(ip), u32::from(local), u32::from(mask));
    let network = local & mask;
    ip & mask == network && ip != network && ip != (network | !mask)
}

fn valid_mask(mask: Ipv4Addr) -> bool {
    let value = u32::from(mask);
    let prefix = value.leading_ones();
    (1..=30).contains(&prefix) && value == u32::MAX << (32 - prefix)
}

fn network_namespace() -> Result<(u64, u64)> {
    let metadata =
        std::fs::metadata("/proc/thread-self/ns/net").map_err(|_| MdnsError::InvalidInterface)?;
    Ok((metadata.dev(), metadata.ino()))
}

impl VerifiedInterface {
    pub fn inspect(config: &MdnsConfig) -> Result<Self> {
        let name =
            CString::new(config.interface_name()).map_err(|_| MdnsError::InvalidInterface)?;
        // SAFETY: name is a live terminated string; this syscall changes no interface state.
        let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
        if index == 0 || index > libc::c_int::MAX as u32 || !private(config.local_address()) {
            return Err(MdnsError::InvalidInterface);
        }
        let namespace = network_namespace()?;
        let mut list = std::ptr::null_mut();
        // SAFETY: list is a writable output slot; success transfers one allocation to the guard.
        if unsafe { libc::getifaddrs(&mut list) } != 0 {
            return Err(MdnsError::InvalidInterface);
        }
        let addresses = AddressList(list);
        let mut current = addresses.0;
        let mut selected = None;
        let mut count = 0;
        while !current.is_null() {
            count += 1;
            if count > 4_096 {
                return Err(MdnsError::InvalidInterface);
            }
            // SAFETY: getifaddrs owns this finite list and its non-null records until guard drop.
            let record = unsafe { &*current };
            current = record.ifa_next;
            if record.ifa_name.is_null() || record.ifa_addr.is_null() {
                continue;
            }
            // SAFETY: getifaddrs provides terminated names and a sockaddr containing sa_family.
            let matches = unsafe {
                CStr::from_ptr(record.ifa_name).to_bytes() == name.as_bytes()
                    && (*record.ifa_addr).sa_family == libc::AF_INET as libc::sa_family_t
            };
            if !matches {
                continue;
            }
            // SAFETY: AF_INET establishes sockaddr_in layout for this getifaddrs address.
            let address = unsafe { &*record.ifa_addr.cast::<libc::sockaddr_in>() };
            let local = Ipv4Addr::from(address.sin_addr.s_addr.to_ne_bytes());
            if local != config.local_address() {
                continue;
            }
            let flags = record.ifa_flags;
            if selected.is_some()
                || flags & libc::IFF_UP as u32 == 0
                || flags & libc::IFF_MULTICAST as u32 == 0
                || flags & libc::IFF_LOOPBACK as u32 != 0
                || record.ifa_netmask.is_null()
            {
                return Err(MdnsError::InvalidInterface);
            }
            // SAFETY: check the family before reading the full AF_INET mask layout.
            if unsafe { (*record.ifa_netmask).sa_family } != libc::AF_INET as libc::sa_family_t {
                return Err(MdnsError::InvalidInterface);
            }
            // SAFETY: the family above establishes a live sockaddr_in netmask.
            let mask = unsafe { &*record.ifa_netmask.cast::<libc::sockaddr_in>() };
            let mask = Ipv4Addr::from(mask.sin_addr.s_addr.to_ne_bytes());
            if !valid_mask(mask) || !host_in_subnet(local, local, mask) {
                return Err(MdnsError::InvalidInterface);
            }
            selected = Some((mask, flags));
        }
        let (mask, flags) = selected.ok_or(MdnsError::InvalidInterface)?;
        // SAFETY: the original terminated name remains live. Detect name/index drift during scan.
        if unsafe { libc::if_nametoindex(name.as_ptr()) } != index
            || network_namespace()? != namespace
        {
            return Err(MdnsError::InvalidInterface);
        }
        Ok(Self {
            name: config.interface_name().to_owned(),
            index,
            local: config.local_address(),
            mask,
            flags,
            namespace,
            timeout: config.timeout(),
        })
    }

    pub fn index(&self) -> u32 {
        self.index
    }
    pub fn local_address(&self) -> Ipv4Addr {
        self.local
    }
    pub fn netmask(&self) -> Ipv4Addr {
        self.mask
    }
    pub(super) fn timeout(&self) -> Duration {
        self.timeout.min(Duration::from_millis(50))
    }

    pub fn verify_unchanged(&self) -> Result<()> {
        let config = MdnsConfig::new(&self.name, self.local, self.timeout)
            .map_err(|_| MdnsError::InterfaceChanged)?;
        if Self::inspect(&config).map_err(|_| MdnsError::InterfaceChanged)? != *self {
            return Err(MdnsError::InterfaceChanged);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_are_contiguous_bounded_host_subnets_not_point_to_point_or_host_routes() {
        for prefix in 1..=30 {
            assert!(valid_mask(Ipv4Addr::from(u32::MAX << (32 - prefix))));
        }
        for mask in [
            "0.0.0.0",
            "255.255.255.254",
            "255.255.255.255",
            "255.0.255.0",
        ] {
            assert!(!valid_mask(mask.parse().unwrap()));
        }
    }

    #[test]
    fn host_filter_rejects_subnet_network_broadcast_and_foreign_subnet() {
        let local = "10.2.3.4".parse().unwrap();
        let mask = "255.255.255.0".parse().unwrap();
        assert!(host_in_subnet("10.2.3.5".parse().unwrap(), local, mask));
        for ip in ["10.2.3.0", "10.2.3.255", "10.2.4.5", "8.8.8.8"] {
            assert!(!host_in_subnet(ip.parse().unwrap(), local, mask));
        }
    }

    #[test]
    fn private_address_class_is_rfc1918_only() {
        for ip in ["10.0.0.1", "172.16.0.1", "172.31.0.1", "192.168.0.1"] {
            assert!(private(ip.parse().unwrap()));
        }
        for ip in [
            "127.0.0.1",
            "169.254.1.1",
            "100.64.1.1",
            "172.15.0.1",
            "172.32.0.1",
            "8.8.8.8",
        ] {
            assert!(!private(ip.parse().unwrap()));
        }
    }
}
