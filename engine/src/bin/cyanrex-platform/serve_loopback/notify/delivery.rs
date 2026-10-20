use super::{
    super::Notifier,
    support::{child, packet, pass_credentials, Directory},
};
use std::{
    ffi::OsString,
    fs,
    os::{
        fd::AsRawFd,
        linux::net::SocketAddrExt,
        unix::{
            ffi::OsStringExt,
            net::{SocketAddr, UnixDatagram},
        },
    },
    time::{Duration, Instant},
};

#[test]
fn notifier_pathname_abstract_and_raw_names_deliver_only_fixed_bytes_and_actual_kernel_identity() {
    let directory = Directory::new();
    for kind in 0..3 {
        let (endpoint, receiver) = if kind == 1 {
            let name = format!("cyanrex-notify-{}", uuid::Uuid::new_v4()).into_bytes();
            let address = SocketAddr::from_abstract_name(&name).unwrap();
            (
                OsString::from_vec([b"@".as_slice(), &name].concat()),
                UnixDatagram::bind_addr(&address).unwrap(),
            )
        } else {
            let leaf = if kind == 0 {
                OsString::from("receiver")
            } else {
                OsString::from_vec(b"receiver-\xff".to_vec())
            };
            let path = directory.0.join(leaf);
            let receiver = UnixDatagram::bind(&path).unwrap();
            (path.into_os_string(), receiver)
        };
        pass_credentials(&receiver);
        let (pid, success) = child("both", Some(&endpoint));
        let first = packet(&receiver);
        let second = packet(&receiver);
        assert!(success);
        // SAFETY: getuid/getgid only return this process's real identity; child does not change it.
        let expected = unsafe { (pid as libc::pid_t, libc::getuid(), libc::getgid()) };
        for (packet, bytes) in [
            (first, b"READY=1".as_slice()),
            (second, b"STOPPING=1".as_slice()),
        ] {
            let packet = packet.expect("native packet was not queued");
            assert_eq!(packet.bytes, bytes);
            assert_eq!(packet.credentials, Some(expected));
            assert!(
                !packet.other_control,
                "unexpected ancillary payload or descriptor"
            );
        }
        assert!(packet(&receiver).is_none());
    }
}

#[test]
fn notifier_retains_the_original_connected_peer_across_pathname_replacement_and_close() {
    // Build both receivers after exec: parallel parent fork/exec must not temporarily inherit
    // the old receiver and turn a genuine local drop into a still-live kernel peer.
    assert!(child("peerpin", None).1);
}

pub(super) fn retained_peer_fixture() {
    let directory = Directory::new();
    let path = directory.0.join("receiver");
    let original = UnixDatagram::bind(&path).unwrap();
    original.set_nonblocking(true).unwrap();
    let notifier = Notifier::connect(path.as_os_str()).unwrap();
    fs::remove_file(&path).unwrap();
    let replacement = UnixDatagram::bind(&path).unwrap();
    replacement.set_nonblocking(true).unwrap();
    assert!(notifier.ready().is_ok());
    let mut bytes = [0; 64];
    let length = original.recv(&mut bytes).unwrap();
    assert_eq!(&bytes[..length], b"READY=1");
    assert!(replacement.recv(&mut bytes).is_err());
    drop(original);
    assert!(notifier.stopping().is_err());
    assert!(
        replacement.recv(&mut bytes).is_err(),
        "send silently re-resolved the new pathname peer"
    );
}

#[test]
fn notifier_full_queue_fails_one_nonblocking_attempt_without_duplicate_or_retry() {
    let directory = Directory::new();
    let path = directory.0.join("receiver");
    let receiver = UnixDatagram::bind(&path).unwrap();
    receiver.set_nonblocking(true).unwrap();
    let notifier = Notifier::connect(path.as_os_str()).unwrap();
    let small: libc::c_int = 1024;
    // SAFETY: per-socket queue configuration of only this fixture's live sender; no global sysctl.
    assert_eq!(
        unsafe {
            libc::setsockopt(
                notifier.socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                (&small as *const libc::c_int).cast(),
                std::mem::size_of_val(&small) as libc::socklen_t,
            )
        },
        0
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut sent = 0;
    while sent < 1024 && Instant::now() < deadline && notifier.ready().is_ok() {
        sent += 1;
    }
    assert!(
        sent > 0 && sent < 1024,
        "owned sender queue never reached its real bound"
    );
    let started = Instant::now();
    assert!(notifier.stopping().is_err());
    assert!(started.elapsed() < Duration::from_millis(250));
    let mut bytes = [0; 64];
    let mut received = 0;
    while let Ok(length) = receiver.recv(&mut bytes) {
        assert_eq!(&bytes[..length], b"READY=1");
        received += 1;
    }
    assert_eq!(
        received, sent,
        "failed sends queued extra notification datagrams"
    );
    assert!(notifier.stopping().is_ok());
    let length = receiver.recv(&mut bytes).unwrap();
    assert_eq!(&bytes[..length], b"STOPPING=1");
    assert!(receiver.recv(&mut bytes).is_err());
}
