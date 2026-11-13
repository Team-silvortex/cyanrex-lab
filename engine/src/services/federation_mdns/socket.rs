//! Owned Linux IPv4 mDNS socket. No wildcard bind, interface fallback, threads or advertisement.
use super::{interface, MdnsError, VerifiedInterface, MAX_MDNS_PACKET_BYTES};
use std::{
    io,
    mem::{size_of, size_of_val, zeroed},
    net::Ipv4Addr,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};

type Result<T> = std::result::Result<T, MdnsError>;
const GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const PORT: u16 = 5_353;

#[derive(Debug)]
pub struct ReceivedMdns {
    pub bytes: Vec<u8>,
    pub source: Ipv4Addr,
}

#[derive(Debug)]
pub struct MdnsSocket {
    fd: OwnedFd,
    interface: VerifiedInterface,
}

fn in_addr(ip: Ipv4Addr) -> libc::in_addr {
    libc::in_addr {
        s_addr: u32::from_ne_bytes(ip.octets()),
    }
}

fn destination() -> libc::sockaddr_in {
    // SAFETY: the sockaddr has only plain integer fields; zero initializes reserved padding.
    let mut address: libc::sockaddr_in = unsafe { zeroed() };
    address.sin_family = libc::AF_INET as libc::sa_family_t;
    address.sin_port = PORT.to_be();
    address.sin_addr = in_addr(GROUP);
    address
}

fn option<T>(fd: &OwnedFd, level: libc::c_int, name: libc::c_int, value: &T) -> Result<()> {
    // SAFETY: callers supply the documented option's live, correctly typed fixed-size value.
    if unsafe {
        libc::setsockopt(
            fd.as_raw_fd(),
            level,
            name,
            (value as *const T).cast(),
            size_of::<T>() as _,
        )
    } != 0
    {
        return Err(MdnsError::SocketUnavailable);
    }
    Ok(())
}

impl MdnsSocket {
    pub fn open(interface: &VerifiedInterface) -> Result<Self> {
        interface.verify_unchanged()?;
        // SAFETY: fixed Linux AF_INET datagram creation; no raw socket or privilege change.
        let raw = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
        if raw < 0 {
            return Err(MdnsError::SocketUnavailable);
        }
        // SAFETY: successful socket returned one fresh descriptor, transferred to this owner once.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        option(&fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, &1_i32)?;
        option(&fd, libc::IPPROTO_IP, libc::IP_MULTICAST_ALL, &0_i32)?;
        option(&fd, libc::IPPROTO_IP, libc::IP_PKTINFO, &1_i32)?;
        option(&fd, libc::IPPROTO_IP, libc::IP_RECVTTL, &1_i32)?;
        option(&fd, libc::IPPROTO_IP, libc::IP_MULTICAST_TTL, &255_i32)?;
        option(&fd, libc::IPPROTO_IP, libc::IP_MULTICAST_LOOP, &0_i32)?;
        let timeout = libc::timeval {
            tv_sec: 0,
            tv_usec: interface.timeout().as_micros() as _,
        };
        option(&fd, libc::SOL_SOCKET, libc::SO_RCVTIMEO, &timeout)?;
        option(&fd, libc::SOL_SOCKET, libc::SO_SNDTIMEO, &timeout)?;
        let membership = libc::ip_mreqn {
            imr_multiaddr: in_addr(GROUP),
            imr_address: in_addr(interface.local_address()),
            imr_ifindex: interface.index() as libc::c_int,
        };
        option(&fd, libc::IPPROTO_IP, libc::IP_MULTICAST_IF, &membership)?;
        let address = destination();
        // SAFETY: address is a live exact AF_INET sockaddr; never bind INADDR_ANY on failure.
        if unsafe {
            libc::bind(
                fd.as_raw_fd(),
                (&address as *const libc::sockaddr_in).cast(),
                size_of_val(&address) as _,
            )
        } != 0
        {
            return Err(MdnsError::SocketUnavailable);
        }
        option(&fd, libc::IPPROTO_IP, libc::IP_ADD_MEMBERSHIP, &membership)?;
        interface.verify_unchanged()?;
        Ok(Self {
            fd,
            interface: interface.clone(),
        })
    }

    pub fn send_query(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_MDNS_PACKET_BYTES {
            return Err(MdnsError::TooLarge);
        }
        if bytes != super::browse_query().as_slice() {
            return Err(MdnsError::InvalidPacket);
        }
        self.interface.verify_unchanged()?;
        let address = destination();
        let mut vector = libc::iovec {
            iov_base: bytes.as_ptr().cast_mut().cast(),
            iov_len: bytes.len(),
        };
        let mut control = [0_usize; 8];
        // SAFETY: an empty msghdr is initialized with only live stack-backed input buffers below.
        let mut message: libc::msghdr = unsafe { zeroed() };
        message.msg_name = (&address as *const libc::sockaddr_in).cast_mut().cast();
        message.msg_namelen = size_of_val(&address) as _;
        message.msg_iov = &mut vector;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        // SAFETY: fixed CMSG sizing and traversal fit the aligned control array above.
        unsafe {
            message.msg_controllen = libc::CMSG_SPACE(size_of::<libc::in_pktinfo>() as _) as usize;
            let header = libc::CMSG_FIRSTHDR(&message);
            (*header).cmsg_level = libc::IPPROTO_IP;
            (*header).cmsg_type = libc::IP_PKTINFO;
            (*header).cmsg_len = libc::CMSG_LEN(size_of::<libc::in_pktinfo>() as _) as usize;
            std::ptr::write_unaligned(
                libc::CMSG_DATA(header).cast::<libc::in_pktinfo>(),
                libc::in_pktinfo {
                    ipi_ifindex: self.interface.index() as libc::c_int,
                    ipi_spec_dst: in_addr(self.interface.local_address()),
                    ipi_addr: in_addr(GROUP),
                },
            );
        }
        // SAFETY: all msg buffers remain live/readable for this bounded one-shot datagram send.
        let length = unsafe { libc::sendmsg(self.fd.as_raw_fd(), &message, libc::MSG_DONTWAIT) };
        self.interface.verify_unchanged()?;
        if length < 0 || length as usize != bytes.len() {
            return Err(MdnsError::SocketUnavailable);
        }
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Option<ReceivedMdns>> {
        self.interface.verify_unchanged()?;
        let mut bytes = [0_u8; MAX_MDNS_PACKET_BYTES];
        let mut control = [0_usize; 16];
        // SAFETY: plain output structs are zero-initialized before passing their real bounds.
        let mut source: libc::sockaddr_storage = unsafe { zeroed() };
        let mut vector = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        // SAFETY: all fields are assigned before kernel use; zero is valid for unused fields.
        let mut message: libc::msghdr = unsafe { zeroed() };
        message.msg_name = (&mut source as *mut libc::sockaddr_storage).cast();
        message.msg_namelen = size_of_val(&source) as _;
        message.msg_iov = &mut vector;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        message.msg_controllen = size_of_val(&control);
        // SAFETY: every output pointer has live writable backing for its advertised bound.
        let length =
            unsafe { libc::recvmsg(self.fd.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC) };
        // Capture errno before the interface recheck can make another syscall.
        let failure = (length < 0).then(|| io::Error::last_os_error().kind());
        self.interface.verify_unchanged()?;
        if let Some(failure) = failure {
            return match failure {
                io::ErrorKind::WouldBlock
                | io::ErrorKind::TimedOut
                | io::ErrorKind::Interrupted => Ok(None),
                _ => Err(MdnsError::SocketUnavailable),
            };
        }
        validate_envelope(
            length as usize,
            message.msg_namelen as usize,
            source.ss_family,
            message.msg_flags,
            message.msg_controllen,
            size_of_val(&control),
        )?;
        // SAFETY: checked AF_INET and exact kernel output size establish sockaddr_in layout.
        let source =
            unsafe { &*(&source as *const libc::sockaddr_storage).cast::<libc::sockaddr_in>() };
        // SAFETY: control is fully initialized; the checked length cannot exceed its allocation.
        let controls = unsafe {
            std::slice::from_raw_parts(control.as_ptr().cast::<u8>(), message.msg_controllen)
        };
        let metadata = ancillary(controls)?;
        let source_ip = Ipv4Addr::from(source.sin_addr.s_addr.to_ne_bytes());
        validate_metadata(
            self.interface.index(),
            self.interface.local_address(),
            self.interface.netmask(),
            source_ip,
            u16::from_be(source.sin_port),
            &metadata,
        )?;
        Ok(Some(ReceivedMdns {
            bytes: bytes[..length as usize].to_vec(),
            source: source_ip,
        }))
    }
}

#[derive(Debug)]
struct Metadata {
    index: libc::c_int,
    local: Ipv4Addr,
    destination: Ipv4Addr,
    ttl: libc::c_int,
}

fn validate_envelope(
    length: usize,
    address_length: usize,
    family: libc::sa_family_t,
    flags: libc::c_int,
    control_length: usize,
    control_capacity: usize,
) -> Result<()> {
    if length == 0
        || length > MAX_MDNS_PACKET_BYTES
        || flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
        || address_length != size_of::<libc::sockaddr_in>()
        || family != libc::AF_INET as libc::sa_family_t
        || control_length > control_capacity
    {
        return Err(MdnsError::InvalidPacket);
    }
    Ok(())
}

fn ancillary(bytes: &[u8]) -> Result<Metadata> {
    let mut offset = 0;
    let mut packet = None;
    let mut ttl = None;
    while offset < bytes.len() {
        if bytes.len() - offset < size_of::<libc::cmsghdr>() {
            return Err(MdnsError::InvalidPacket);
        }
        // SAFETY: the bound above permits an unaligned read of this all-integer header.
        let header =
            unsafe { std::ptr::read_unaligned(bytes.as_ptr().add(offset).cast::<libc::cmsghdr>()) };
        // SAFETY: constant CMSG sizing has no pointer access.
        let base = unsafe { libc::CMSG_LEN(0) } as usize;
        if header.cmsg_len < base || header.cmsg_len > bytes.len() - offset {
            return Err(MdnsError::InvalidPacket);
        }
        let data = &bytes[offset + base..offset + header.cmsg_len];
        match (header.cmsg_level, header.cmsg_type, data.len()) {
            (libc::IPPROTO_IP, libc::IP_PKTINFO, size)
                if size == size_of::<libc::in_pktinfo>() && packet.is_none() =>
            {
                // SAFETY: exact checked length permits unaligned access to the integer-only value.
                packet = Some(unsafe {
                    std::ptr::read_unaligned(data.as_ptr().cast::<libc::in_pktinfo>())
                });
            }
            (libc::IPPROTO_IP, libc::IP_TTL, size)
                if size == size_of::<libc::c_int>() && ttl.is_none() =>
            {
                // SAFETY: exact checked length permits an unaligned integer read.
                ttl =
                    Some(unsafe { std::ptr::read_unaligned(data.as_ptr().cast::<libc::c_int>()) });
            }
            _ => return Err(MdnsError::InvalidPacket),
        }
        let align = size_of::<usize>();
        let padded = header
            .cmsg_len
            .checked_add(align - 1)
            .ok_or(MdnsError::InvalidPacket)?
            & !(align - 1);
        let next = offset.checked_add(padded).ok_or(MdnsError::InvalidPacket)?;
        if next > bytes.len() {
            if offset + header.cmsg_len != bytes.len() {
                return Err(MdnsError::InvalidPacket);
            }
            offset = bytes.len();
        } else {
            offset = next;
        }
    }
    let packet = packet.ok_or(MdnsError::InvalidPacket)?;
    Ok(Metadata {
        index: packet.ipi_ifindex,
        local: Ipv4Addr::from(packet.ipi_spec_dst.s_addr.to_ne_bytes()),
        destination: Ipv4Addr::from(packet.ipi_addr.s_addr.to_ne_bytes()),
        ttl: ttl.ok_or(MdnsError::InvalidPacket)?,
    })
}

fn validate_metadata(
    index: u32,
    local: Ipv4Addr,
    mask: Ipv4Addr,
    source: Ipv4Addr,
    port: u16,
    metadata: &Metadata,
) -> Result<()> {
    if port != PORT
        || source == local
        || !interface::private(source)
        || !interface::host_in_subnet(source, local, mask)
        || metadata.index <= 0
        || metadata.index as u32 != index
        || metadata.local != local
        || metadata.destination != GROUP
        || metadata.ttl != 255
    {
        return Err(MdnsError::InvalidPacket);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmsg<T>(kind: libc::c_int, value: &T) -> Vec<u8> {
        // SAFETY: fixed CMSG sizes and initialized byte storage bound every write below.
        unsafe {
            let mut bytes = vec![0_u8; libc::CMSG_SPACE(size_of::<T>() as _) as usize];
            let header = libc::cmsghdr {
                cmsg_len: libc::CMSG_LEN(size_of::<T>() as _) as usize,
                cmsg_level: libc::IPPROTO_IP,
                cmsg_type: kind,
            };
            std::ptr::write_unaligned(bytes.as_mut_ptr().cast::<libc::cmsghdr>(), header);
            std::ptr::copy_nonoverlapping(
                (value as *const T).cast::<u8>(),
                bytes.as_mut_ptr().add(libc::CMSG_LEN(0) as usize),
                size_of::<T>(),
            );
            bytes
        }
    }

    fn packet() -> Vec<u8> {
        let info = libc::in_pktinfo {
            ipi_ifindex: 7,
            ipi_spec_dst: in_addr(Ipv4Addr::new(10, 2, 3, 4)),
            ipi_addr: in_addr(GROUP),
        };
        let mut bytes = cmsg(libc::IP_PKTINFO, &info);
        bytes.extend(cmsg(libc::IP_TTL, &255_i32));
        bytes
    }

    #[test]
    fn ancillary_requires_exact_unique_packet_info_and_ttl_without_unknown_controls() {
        let valid = packet();
        assert!(ancillary(&valid).is_ok());
        let mut duplicate = valid.clone();
        duplicate.extend(cmsg(libc::IP_TTL, &255_i32));
        let mut duplicate_info = valid.clone();
        // SAFETY: constant CMSG sizing has no pointer access.
        let info_size = unsafe { libc::CMSG_SPACE(size_of::<libc::in_pktinfo>() as _) } as usize;
        duplicate_info.extend_from_slice(&valid[..info_size]);
        let mut trailing = valid.clone();
        trailing.push(0);
        for invalid in [
            vec![],
            vec![0],
            cmsg(libc::IP_TTL, &255_i32),
            cmsg(libc::IP_TTL, &1_u8),
            cmsg(libc::IP_RECVTTL, &255_i32),
            duplicate,
            duplicate_info,
            trailing,
        ] {
            assert!(matches!(ancillary(&invalid), Err(MdnsError::InvalidPacket)));
        }
        // The final header's alignment padding is not payload; omitting only that padding is valid.
        let last_data_end =
            info_size + unsafe { libc::CMSG_LEN(size_of::<libc::c_int>() as _) } as usize;
        for length in 0..last_data_end {
            assert!(ancillary(&valid[..length]).is_err());
        }
    }

    #[test]
    fn ancillary_rejects_invalid_header_lengths_before_payload_access() {
        for length in [0, 1, size_of::<libc::cmsghdr>() - 1, 4_096, usize::MAX] {
            let mut invalid = packet();
            // SAFETY: a live complete header backs this one unaligned length overwrite.
            unsafe {
                std::ptr::write_unaligned(invalid.as_mut_ptr().cast::<usize>(), length);
            }
            assert!(ancillary(&invalid).is_err());
        }
    }

    #[test]
    fn metadata_binds_ingress_interface_destination_local_address_port_and_hop_limit() {
        let local = Ipv4Addr::new(10, 2, 3, 4);
        let mask = Ipv4Addr::new(255, 255, 255, 0);
        let source = Ipv4Addr::new(10, 2, 3, 5);
        assert!(
            validate_metadata(7, local, mask, source, PORT, &ancillary(&packet()).unwrap()).is_ok()
        );
        for bad in [
            Ipv4Addr::new(10, 2, 3, 0),
            Ipv4Addr::new(10, 2, 3, 255),
            local,
            Ipv4Addr::new(10, 2, 4, 5),
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(8, 8, 8, 8),
        ] {
            assert!(
                validate_metadata(7, local, mask, bad, PORT, &ancillary(&packet()).unwrap())
                    .is_err()
            );
        }
        for port in [0, 53, 5_354] {
            assert!(validate_metadata(
                7,
                local,
                mask,
                source,
                port,
                &ancillary(&packet()).unwrap()
            )
            .is_err());
        }
        for mode in 0..5 {
            let mut metadata = ancillary(&packet()).unwrap();
            match mode {
                0 => metadata.index = 8,
                1 => metadata.index = -1,
                2 => metadata.local = source,
                3 => metadata.destination = local,
                _ => metadata.ttl = 254,
            }
            assert!(validate_metadata(7, local, mask, source, PORT, &metadata).is_err());
        }
    }

    #[test]
    fn receive_envelope_rejects_data_or_control_truncation_and_wrong_source_layout() {
        let size = size_of::<libc::sockaddr_in>();
        let family = libc::AF_INET as libc::sa_family_t;
        assert!(validate_envelope(MAX_MDNS_PACKET_BYTES, size, family, 0, 56, 128).is_ok());
        for flags in [
            libc::MSG_TRUNC,
            libc::MSG_CTRUNC,
            libc::MSG_TRUNC | libc::MSG_CTRUNC,
        ] {
            assert!(validate_envelope(12, size, family, flags, 56, 128).is_err());
        }
        for length in [0, MAX_MDNS_PACKET_BYTES + 1, usize::MAX] {
            assert!(validate_envelope(length, size, family, 0, 56, 128).is_err());
        }
        for length in [0, size - 1, size + 1] {
            assert!(validate_envelope(12, length, family, 0, 56, 128).is_err());
        }
        assert!(validate_envelope(12, size, libc::AF_INET6 as _, 0, 56, 128).is_err());
        assert!(validate_envelope(12, size, family, 0, 129, 128).is_err());
    }
}
