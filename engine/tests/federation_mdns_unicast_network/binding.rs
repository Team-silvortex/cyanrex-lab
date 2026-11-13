use super::{fd_count, unicast, verified, MdnsError, MdnsUnicastReceiver};
use std::{
    io,
    mem::{size_of, zeroed},
    net::Ipv4Addr,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};

const MODES: [(bool, bool); 4] = [(false, false), (true, false), (false, true), (true, true)];

fn option(fd: &OwnedFd, level: i32, option: i32, value: bool) -> io::Result<()> {
    let value = i32::from(value);
    // SAFETY: live integer option and its exact size are provided for this owned fixture socket.
    if unsafe {
        libc::setsockopt(
            fd.as_raw_fd(),
            level,
            option,
            (&value as *const i32).cast(),
            size_of::<i32>() as _,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn bind(ip: Option<Ipv4Addr>, dual_stack: bool, reuse: (bool, bool)) -> io::Result<OwnedFd> {
    let family = if dual_stack {
        libc::AF_INET6
    } else {
        libc::AF_INET
    };
    // SAFETY: fixed ordinary datagram socket creation in the already-verified owned namespace.
    let raw = unsafe { libc::socket(family, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful socket creation returned one fresh descriptor, transferred exactly once.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    option(&fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, reuse.0)?;
    option(&fd, libc::SOL_SOCKET, libc::SO_REUSEPORT, reuse.1)?;
    let status = if dual_stack {
        option(&fd, libc::IPPROTO_IPV6, libc::IPV6_V6ONLY, false)?;
        // SAFETY: all sockaddr_in6 fields are integers/arrays and zero is valid for unused fields.
        let mut address: libc::sockaddr_in6 = unsafe { zeroed() };
        address.sin6_family = libc::AF_INET6 as _;
        address.sin6_port = 5353_u16.to_be();
        // SAFETY: the live initialized wildcard IPv6 address has the exact advertised size.
        unsafe {
            libc::bind(
                fd.as_raw_fd(),
                (&address as *const libc::sockaddr_in6).cast(),
                size_of::<libc::sockaddr_in6>() as _,
            )
        }
    } else {
        // SAFETY: all sockaddr_in fields are integers and zero initializes reserved padding.
        let mut address: libc::sockaddr_in = unsafe { zeroed() };
        address.sin_family = libc::AF_INET as _;
        address.sin_port = 5353_u16.to_be();
        address.sin_addr.s_addr = u32::from_ne_bytes(ip.unwrap_or(Ipv4Addr::UNSPECIFIED).octets());
        // SAFETY: the live initialized IPv4 address has the exact advertised size.
        unsafe {
            libc::bind(
                fd.as_raw_fd(),
                (&address as *const libc::sockaddr_in).cast(),
                size_of::<libc::sockaddr_in>() as _,
            )
        }
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(fd)
}

fn unavailable() {
    assert!(matches!(
        MdnsUnicastReceiver::open(&verified("crx0", "10.23.0.1")),
        Err(MdnsError::SocketUnavailable)
    ));
}

pub(super) fn existing_ipv4_binds_are_exclusive() {
    let baseline = fd_count();
    for ip in [Some(Ipv4Addr::new(10, 23, 0, 1)), None] {
        for reuse in MODES {
            let occupant = bind(ip, false, reuse).unwrap();
            unavailable();
            assert_eq!(
                fd_count(),
                baseline + 1,
                "failed receiver open leaked a descriptor"
            );
            drop(occupant);
            drop(unicast("crx0", "10.23.0.1"));
            assert_eq!(fd_count(), baseline);
        }
    }
}

pub(super) fn existing_dual_stack_binds_are_exclusive() {
    let baseline = fd_count();
    for reuse in MODES {
        let occupant = bind(None, true, reuse).unwrap();
        unavailable();
        assert_eq!(fd_count(), baseline + 1);
        drop(occupant);
        drop(unicast("crx0", "10.23.0.1"));
        assert_eq!(fd_count(), baseline);
    }
}

pub(super) fn receiver_first_is_exclusive_and_releases() {
    let baseline = fd_count();
    let receiver = unicast("crx0", "10.23.0.1");
    unavailable();
    for reuse in MODES {
        for (ip, dual_stack) in [
            (Some(Ipv4Addr::new(10, 23, 0, 1)), false),
            (None, false),
            (None, true),
        ] {
            let error = bind(ip, dual_stack, reuse).unwrap_err();
            assert_eq!(error.raw_os_error(), Some(libc::EADDRINUSE));
            assert_eq!(fd_count(), baseline + 1);
        }
    }
    drop(receiver);
    assert_eq!(fd_count(), baseline);
    for (ip, dual_stack) in [
        (Some(Ipv4Addr::new(10, 23, 0, 1)), false),
        (None, false),
        (None, true),
    ] {
        drop(bind(ip, dual_stack, (false, false)).unwrap());
        assert_eq!(fd_count(), baseline);
    }
}
