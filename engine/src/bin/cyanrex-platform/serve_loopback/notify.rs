//! Explicit, fixed native service-manager datagrams; never environment-derived host policy.
use std::{
    ffi::OsStr,
    os::{
        fd::FromRawFd,
        linux::net::SocketAddrExt,
        unix::{
            ffi::OsStrExt,
            net::{SocketAddr, UnixDatagram},
        },
    },
    path::Path,
};

pub(super) struct Notifier {
    socket: UnixDatagram,
}

impl Notifier {
    pub(super) fn from_environment(enabled: bool) -> Result<Option<Self>, ()> {
        if !enabled {
            return Ok(None);
        }
        let name = std::env::var_os("NOTIFY_SOCKET").ok_or(())?;
        Self::connect(&name).map(Some)
    }

    pub(super) fn ready(&self) -> Result<(), ()> {
        self.send(b"READY=1")
    }

    pub(super) fn stopping(&self) -> Result<(), ()> {
        self.send(b"STOPPING=1")
    }

    fn connect(name: &OsStr) -> Result<Self, ()> {
        let address = address(name)?;
        // SAFETY: fixed socket family/type flags and protocol; no pointers or borrowed storage.
        let raw = unsafe {
            libc::socket(
                libc::AF_UNIX,
                libc::SOCK_DGRAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                0,
            )
        };
        if raw < 0 {
            return Err(());
        }
        // SAFETY: socket returned this one new live descriptor, now owned only by UnixDatagram.
        let socket = unsafe { UnixDatagram::from_raw_fd(raw) };
        // Capture one existing peer. Later sends never resolve a replaced pathname or read env.
        socket.connect_addr(&address).map_err(|_| ())?;
        Ok(Self { socket })
    }

    fn send(&self, bytes: &[u8]) -> Result<(), ()> {
        // One nonblocking datagram attempt, no retry, extra fields or ancillary descriptors.
        if self.socket.send(bytes).map_err(|_| ())? == bytes.len() {
            Ok(())
        } else {
            Err(())
        }
    }
}

fn address(name: &OsStr) -> Result<SocketAddr, ()> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(());
    }
    if let Some(abstract_name) = bytes.strip_prefix(b"@") {
        if abstract_name.is_empty() {
            return Err(());
        }
        SocketAddr::from_abstract_name(abstract_name).map_err(|_| ())
    } else if bytes.starts_with(b"/") {
        SocketAddr::from_pathname(Path::new(name)).map_err(|_| ())
    } else {
        Err(())
    }
}

#[cfg(test)]
#[path = "notify/tests.rs"]
mod tests;
