use super::{
    super::{address, Notifier},
    support::{child, Directory},
};
use std::{
    ffi::{OsStr, OsString},
    os::{
        fd::AsRawFd,
        linux::net::SocketAddrExt,
        unix::{ffi::OsStringExt, net::UnixDatagram},
    },
};

#[test]
fn notifier_address_grammar_is_bounded_native_and_preserves_non_utf8_bytes() {
    for bytes in [
        b"".as_slice(),
        b"relative",
        b"@",
        b"vsock:3:1024",
        b"/tmp/a\0b",
        b"@a\0b",
    ] {
        assert!(address(&OsString::from_vec(bytes.to_vec())).is_err());
    }
    let pathname = OsString::from_vec([b"/".as_slice(), &vec![b'a'; 106]].concat());
    assert!(address(&pathname).is_ok());
    assert!(address(&OsString::from_vec(
        [b"/".as_slice(), &vec![b'a'; 107]].concat()
    ))
    .is_err());
    let abstract_name = OsString::from_vec([b"@".as_slice(), &vec![b'a'; 107]].concat());
    assert_eq!(
        address(&abstract_name)
            .unwrap()
            .as_abstract_name()
            .unwrap()
            .len(),
        107
    );
    assert!(address(&OsString::from_vec(
        [b"@".as_slice(), &vec![b'a'; 108]].concat()
    ))
    .is_err());
    assert!(address(&OsString::from_vec(b"/tmp/notify-\xff".to_vec())).is_ok());
    assert!(address(&OsString::from_vec(b"@notify-\xff".to_vec())).is_ok());
}

#[test]
fn notifier_disabled_does_not_activate_any_missing_invalid_or_reachable_ambient_endpoint() {
    let directory = Directory::new();
    let path = directory.0.join("receiver");
    let receiver = UnixDatagram::bind(&path).unwrap();
    receiver.set_nonblocking(true).unwrap();
    for endpoint in [
        None,
        Some(OsStr::new("")),
        Some(OsStr::new("relative")),
        Some(path.as_os_str()),
    ] {
        assert!(child("disabled", endpoint).1);
    }
    assert!(receiver.recv(&mut [0; 64]).is_err());
}

#[test]
fn notifier_enabled_refuses_missing_malformed_and_unreachable_destinations() {
    let directory = Directory::new();
    let missing = directory.0.join("missing");
    let abstract_missing = format!("@cyanrex-no-peer-{}", uuid::Uuid::new_v4());
    for endpoint in [
        None,
        Some(OsStr::new("")),
        Some(OsStr::new("relative")),
        Some(OsStr::new("@")),
        Some(OsStr::new("vsock:3:1024")),
        Some(missing.as_os_str()),
        Some(OsStr::new(&abstract_missing)),
    ] {
        assert!(child("invalid", endpoint).1);
    }
}

#[test]
fn notifier_connected_socket_is_atomically_nonblocking_and_close_on_exec() {
    let directory = Directory::new();
    let path = directory.0.join("receiver");
    let receiver = UnixDatagram::bind(&path).unwrap();
    let notifier = Notifier::connect(path.as_os_str()).unwrap();
    let fd = notifier.socket.as_raw_fd();
    // SAFETY: these read-only fcntl operations inspect this test's live owned socket.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    let descriptor = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert!(flags >= 0 && flags & libc::O_NONBLOCK != 0 && descriptor & libc::FD_CLOEXEC != 0);
    drop(receiver);
}
