//! Real plaintext PostgreSQL startup failures, not authenticated catalog cancellation.
use super::{
    fixture::{Fixture, Run, CANARY},
    support::{self, ChildGuard},
};
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, Instant},
};

pub(super) fn listener() -> TcpListener {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

pub(super) fn no_second_connection(listener: &TcpListener) -> bool {
    let deadline = Instant::now() + Duration::from_millis(150);
    loop {
        match listener.accept() {
            Ok((socket, _)) => {
                drop(socket);
                return false;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return false,
        }
    }
}

fn startup(listener: &TcpListener, child: &mut ChildGuard) -> Option<(TcpStream, bool)> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut socket = loop {
        match listener.accept() {
            Ok((socket, _)) => break socket,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if child.finished() || Instant::now() >= deadline {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return None,
        }
    };
    socket.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    socket
        .set_write_timeout(Some(Duration::from_secs(1)))
        .ok()?;
    let mut header = [0; 4];
    socket.read_exact(&mut header).ok()?;
    let length = u32::from_be_bytes(header) as usize;
    if !(8..=4096).contains(&length) {
        return None;
    }
    let mut payload = vec![0; length - 4];
    socket.read_exact(&mut payload).ok()?;
    let contains = |value: &[u8]| payload.windows(value.len()).any(|window| window == value);
    let explicit = payload.starts_with(&196608u32.to_be_bytes())
        && contains(b"user\0platform_runtime\0")
        && contains(b"database\0platform_db\0")
        && contains(b"search_path=source_private")
        && !contains(CANARY.as_bytes());
    Some((socket, explicit))
}

fn closed(socket: &mut TcpStream) -> bool {
    match socket.read(&mut [0; 1]) {
        Ok(0) => true,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
            ) =>
        {
            true
        }
        _ => false,
    }
}

struct Observation {
    run: Run,
    safe_ids: bool,
    reached: bool,
    explicit: bool,
    sent: bool,
    closed: bool,
    no_retry: bool,
    elapsed: Duration,
}

fn exercise(stall: bool) -> Observation {
    Fixture::checked(|fixture| {
        let listener = listener();
        let root = support::root(fixture, "artifacts");
        let input = support::input(
            fixture,
            "input.json",
            &root,
            listener.local_addr().unwrap().port(),
        );
        let started = Instant::now();
        let mut child = ChildGuard::spawn(fixture, &support::arguments(fixture, &input), None);
        let mut peer = startup(&listener, &mut child);
        let reached = peer.is_some();
        let explicit = peer.as_ref().is_some_and(|(_, explicit)| *explicit);
        let mut sent = false;
        if !stall {
            if let Some((socket, _)) = peer.as_mut() {
                let body = format!(
                    "SFATAL\0VFATAL\0C28000\0M{CANARY}; source_private; /tmp/synthetic-peer\0\0"
                );
                let mut frame = vec![b'E'];
                frame.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
                frame.extend_from_slice(body.as_bytes());
                sent = socket.write_all(&frame).is_ok();
            }
        }
        let run = child.wait(started + Duration::from_secs(14));
        let elapsed = started.elapsed();
        let closed = peer.as_mut().is_some_and(|(socket, _)| closed(socket));
        drop(peer);
        let no_retry = no_second_connection(&listener);
        drop(listener);
        Observation {
            run,
            safe_ids: fixture.identity.safe_ids(),
            reached,
            explicit,
            sent,
            closed,
            no_retry,
            elapsed,
        }
    })
}

#[test]
fn platform_storage_cli_peer_error_is_generic_single_attempt_and_closes_without_secret_echo() {
    let observation = exercise(false);
    support::failed(&observation.run);
    assert!(observation.no_retry);
    if observation.safe_ids {
        assert!(
            observation.reached && observation.explicit && observation.sent && observation.closed
        );
    } else {
        assert!(
            !observation.reached,
            "unsafe process identity must fail before dialing"
        );
    }
}

#[test]
fn platform_storage_cli_stalled_handshake_reaches_real_deadline_and_closes_without_retry() {
    let observation = exercise(true);
    support::failed(&observation.run);
    assert!(observation.no_retry);
    if observation.safe_ids {
        assert!(observation.reached && observation.explicit && observation.closed);
        assert!(observation.elapsed >= Duration::from_secs(9));
        assert!(observation.elapsed < Duration::from_secs(14));
    } else {
        assert!(
            !observation.reached,
            "unsafe process identity must fail before dialing"
        );
    }
}
