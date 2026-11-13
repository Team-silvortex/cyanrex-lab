//! Private typed probe I/O; it neither widens send_query nor permits announcement packets.
use super::{
    destination, in_addr, option, MdnsError, MdnsSocket, MdnsUnicastReceiver, Result, GROUP,
};
use crate::services::federation_mdns::{MdnsAnnouncement, ReceivedMdns, VerifiedInterface};
use std::{
    mem::{size_of, size_of_val, zeroed},
    os::fd::{AsRawFd, OwnedFd},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::services::federation_mdns) enum Channel {
    Multicast,
    Unicast,
}

pub(in crate::services::federation_mdns) struct Ingress {
    pub channel: Channel,
    pub received: Result<Option<ReceivedMdns>>,
}

pub(in crate::services::federation_mdns) struct ProbeIo {
    multicast: MdnsSocket,
    unicast: MdnsUnicastReceiver,
    unicast_first: bool,
}

impl ProbeIo {
    pub(in crate::services::federation_mdns) fn open(
        interface: &VerifiedInterface,
    ) -> Result<Self> {
        let multicast = MdnsSocket::open(interface)?;
        let unicast = MdnsUnicastReceiver::open(interface)?;
        // Prevent fragmentation of this three-record proposal even if MTU changes at send time.
        option(
            &multicast.fd,
            libc::IPPROTO_IP,
            libc::IP_MTU_DISCOVER,
            &libc::IP_PMTUDISC_DO,
        )?;
        interface.verify_unchanged()?;
        Ok(Self {
            multicast,
            unicast,
            unicast_first: false,
        })
    }

    pub(in crate::services::federation_mdns) fn receive_ready(
        &mut self,
        wait: Duration,
    ) -> Result<Vec<Ingress>> {
        self.multicast.interface.verify_unchanged()?;
        let mut descriptors = [
            libc::pollfd {
                fd: self.multicast.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.unicast.descriptor().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let milliseconds = wait.as_nanos().div_ceil(1_000_000).min(50) as libc::c_int;
        // SAFETY: two live owned descriptors and a fully initialized, bounded writable array.
        let status = unsafe { libc::poll(descriptors.as_mut_ptr(), 2, milliseconds) };
        self.multicast.interface.verify_unchanged()?;
        validate_poll(status, &descriptors)?;
        let order = if self.unicast_first { [1, 0] } else { [0, 1] };
        self.unicast_first = !self.unicast_first;
        let mut received = Vec::with_capacity(2);
        for index in order {
            if descriptors[index].revents & libc::POLLIN == 0 {
                continue;
            }
            let (channel, fd, destination) = if index == 0 {
                (Channel::Multicast, &self.multicast.fd, GROUP)
            } else {
                (
                    Channel::Unicast,
                    self.unicast.descriptor(),
                    self.multicast.interface.local_address(),
                )
            };
            received.push(Ingress {
                channel,
                received: super::receive_with_flags(
                    fd,
                    &self.multicast.interface,
                    destination,
                    libc::MSG_DONTWAIT,
                ),
            });
        }
        Ok(received)
    }

    pub(in crate::services::federation_mdns) fn send_probe(
        &self,
        announcement: &MdnsAnnouncement,
        now: u64,
    ) -> Result<()> {
        let interface = &self.multicast.interface;
        interface.verify_unchanged()?;
        announcement.probe_remaining(interface.local_address(), now)?;
        let bytes = announcement.encode_probe_at(now)?;
        check_mtu(bytes.len(), interface_mtu(&self.multicast.fd, interface)?)?;
        interface.verify_unchanged()?;
        let address = destination();
        let mut vector = libc::iovec {
            iov_base: bytes.as_ptr().cast_mut().cast(),
            iov_len: bytes.len(),
        };
        let mut control = [0_usize; 8];
        // SAFETY: the zeroed header is populated only with live stack-backed input buffers.
        let mut message: libc::msghdr = unsafe { zeroed() };
        message.msg_name = (&address as *const libc::sockaddr_in).cast_mut().cast();
        message.msg_namelen = size_of_val(&address) as _;
        message.msg_iov = &mut vector;
        message.msg_iovlen = 1;
        message.msg_control = control.as_mut_ptr().cast();
        // SAFETY: all fixed CMSG sizing and writes fit the aligned control array above.
        unsafe {
            message.msg_controllen = libc::CMSG_SPACE(size_of::<libc::in_pktinfo>() as _) as usize;
            let header = libc::CMSG_FIRSTHDR(&message);
            (*header).cmsg_level = libc::IPPROTO_IP;
            (*header).cmsg_type = libc::IP_PKTINFO;
            (*header).cmsg_len = libc::CMSG_LEN(size_of::<libc::in_pktinfo>() as _) as usize;
            std::ptr::write_unaligned(
                libc::CMSG_DATA(header).cast::<libc::in_pktinfo>(),
                libc::in_pktinfo {
                    ipi_ifindex: interface.index() as libc::c_int,
                    ipi_spec_dst: in_addr(interface.local_address()),
                    ipi_addr: in_addr(GROUP),
                },
            );
        }
        // SAFETY: the bounded complete proposal and all message buffers remain live for this send.
        let length =
            unsafe { libc::sendmsg(self.multicast.fd.as_raw_fd(), &message, libc::MSG_DONTWAIT) };
        interface.verify_unchanged()?;
        if length < 0 || length as usize != bytes.len() {
            return Err(MdnsError::SocketUnavailable);
        }
        Ok(())
    }
}

fn validate_poll(status: libc::c_int, descriptors: &[libc::pollfd; 2]) -> Result<()> {
    // Even EINTR is terminal here: an interrupted poll never establishes a drained queue.
    if status < 0
        || descriptors
            .iter()
            .any(|fd| fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0)
    {
        return Err(MdnsError::SocketUnavailable);
    }
    Ok(())
}

fn interface_mtu(fd: &OwnedFd, interface: &VerifiedInterface) -> Result<usize> {
    // SAFETY: integer/array ifreq fields allow zero initialization; names are bounded by IF_NAMESIZE.
    let mut request: libc::ifreq = unsafe { zeroed() };
    // SAFETY: ifr_name has exactly IF_NAMESIZE writable bytes; no interface is modified.
    if unsafe { libc::if_indextoname(interface.index(), request.ifr_name.as_mut_ptr()) }.is_null() {
        return Err(MdnsError::InterfaceChanged);
    }
    // SAFETY: SIOCGIFMTU only fills this initialized ifreq's integer union member.
    if unsafe { libc::ioctl(fd.as_raw_fd(), libc::SIOCGIFMTU, &mut request) } != 0 {
        return Err(MdnsError::SocketUnavailable);
    }
    // SAFETY: successful SIOCGIFMTU initialized this union member.
    usize::try_from(unsafe { request.ifr_ifru.ifru_mtu }).map_err(|_| MdnsError::SocketUnavailable)
}

fn check_mtu(payload: usize, mtu: usize) -> Result<()> {
    if payload.checked_add(28).is_none_or(|size| size > mtu) {
        return Err(MdnsError::TooLarge);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_or_faulted_poll_cannot_establish_an_empty_receive_queue() {
        let mut descriptors = [libc::pollfd {
            fd: 1,
            events: libc::POLLIN,
            revents: 0,
        }; 2];
        assert_eq!(
            validate_poll(-1, &descriptors),
            Err(MdnsError::SocketUnavailable)
        );
        assert_eq!(validate_poll(0, &descriptors), Ok(()));
        for flag in [libc::POLLERR, libc::POLLHUP, libc::POLLNVAL] {
            for index in 0..2 {
                descriptors[index].revents = flag | libc::POLLIN;
                assert_eq!(
                    validate_poll(1, &descriptors),
                    Err(MdnsError::SocketUnavailable)
                );
                descriptors[index].revents = 0;
            }
        }
    }

    #[test]
    fn three_record_probe_must_fit_ipv4_and_udp_headers_without_fragmentation() {
        assert_eq!(check_mtu(1472, 1500), Ok(()));
        assert_eq!(check_mtu(1473, 1500), Err(MdnsError::TooLarge));
        assert_eq!(check_mtu(0, 27), Err(MdnsError::TooLarge));
        assert_eq!(check_mtu(usize::MAX, usize::MAX), Err(MdnsError::TooLarge));
    }
}
