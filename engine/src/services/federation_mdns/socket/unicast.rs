//! Explicit exact-local receive channel, separate from the multicast browser.
use super::{in_addr, option, MdnsError, OwnedFd, ReceivedMdns, Result, VerifiedInterface, PORT};
use std::{
    mem::{size_of_val, zeroed},
    os::fd::{AsRawFd, FromRawFd},
};

/// Receives local-port mDNS datagrams, not proof of a response to a probe or of name ownership.
/// No port sharing, multicast subscription, outbound path or background work is provided.
#[derive(Debug)]
pub struct MdnsUnicastReceiver {
    fd: OwnedFd,
    interface: VerifiedInterface,
}

impl MdnsUnicastReceiver {
    /// Exclusively binds the selected local IPv4 address at port 5353.
    /// An overlapping listener is an error; there is no reuse or multicast-only fallback.
    pub fn open(interface: &VerifiedInterface) -> Result<Self> {
        interface.verify_unchanged()?;
        // SAFETY: fixed Linux AF_INET datagram creation, no raw socket or privilege change.
        let raw = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
        if raw < 0 {
            return Err(MdnsError::SocketUnavailable);
        }
        // SAFETY: successful socket returned one fresh descriptor, transferred once to its owner.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        option(&fd, libc::IPPROTO_IP, libc::IP_MULTICAST_ALL, &0_i32)?;
        option(&fd, libc::IPPROTO_IP, libc::IP_PKTINFO, &1_i32)?;
        option(&fd, libc::IPPROTO_IP, libc::IP_RECVTTL, &1_i32)?;
        let timeout = libc::timeval {
            tv_sec: 0,
            tv_usec: interface.timeout().as_micros() as _,
        };
        option(&fd, libc::SOL_SOCKET, libc::SO_RCVTIMEO, &timeout)?;
        // SAFETY: plain integer sockaddr fields permit zero-initialized reserved padding.
        let mut address: libc::sockaddr_in = unsafe { zeroed() };
        address.sin_family = libc::AF_INET as libc::sa_family_t;
        address.sin_port = PORT.to_be();
        address.sin_addr = in_addr(interface.local_address());
        // SAFETY: a live exact-local AF_INET sockaddr, never replaced with a wildcard on failure.
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
        interface.verify_unchanged()?;
        Ok(Self {
            fd,
            interface: interface.clone(),
        })
    }

    /// One receive is bounded by the interface's at-most-50 ms socket timeout, not a flow quota.
    pub fn receive(&mut self) -> Result<Option<ReceivedMdns>> {
        super::receive(&self.fd, &self.interface, self.interface.local_address())
    }

    // Only the sibling private probe poller may borrow this owner; no public descriptor escape.
    pub(super) fn descriptor(&self) -> &OwnedFd {
        &self.fd
    }
}
