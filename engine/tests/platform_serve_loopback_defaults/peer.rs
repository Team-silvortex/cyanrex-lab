//! Actual PostgreSQL startup bytes only. The synthetic peer cannot authenticate or create storage.
use super::{
    fixture::{Fixture, Run, Watch, CANARY},
    support::{self, ChildGuard},
};
use std::{
    fs::File,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, Instant},
};

fn startup(listener: &TcpListener, child: &mut ChildGuard) -> Option<(TcpStream, bool)> {
    let until = Instant::now() + Duration::from_secs(3);
    let mut socket = loop {
        match listener.accept() {
            Ok((socket, _)) => break socket,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if child.finished() || Instant::now() >= until {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            _ => return None,
        }
    };
    socket.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    socket
        .set_write_timeout(Some(Duration::from_secs(1)))
        .ok()?;
    let mut length = [0; 4];
    socket.read_exact(&mut length).ok()?;
    let length = u32::from_be_bytes(length) as usize;
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
    safe: bool,
    reached: bool,
    explicit: bool,
    sent: bool,
    closed: bool,
    no_retry: bool,
    no_host: bool,
    ambient_untouched: bool,
    oracle_positive: bool,
    elapsed: Duration,
}
fn exercise(stalled: bool) -> Observation {
    Fixture::checked(|fixture| {
        let db = support::listener();
        let bind = support::address();
        let root = support::root(fixture, "artifacts");
        let value = support::document(&root, db.local_addr().unwrap().port(), bind);
        let input = support::input(fixture, "serve.json", &value);
        let pass = fixture.write("ambient.pgpass", b"*:1:*:*:synthetic-unused\n", 0o600);
        let service = fixture.write(
            "ambient.service",
            b"[untrusted]\nhost=untrusted.invalid\n",
            0o600,
        );
        let mut watch = Watch::new(&[&pass, &service]);
        let started = Instant::now();
        let mut child =
            ChildGuard::spawn(fixture, &support::arguments(fixture, &input, bind), None);
        let mut peer = startup(&db, &mut child);
        let reached = peer.is_some();
        let explicit = peer.as_ref().is_some_and(|(_, explicit)| *explicit);
        let mut sent = false;
        if !stalled {
            if let Some((socket, _)) = peer.as_mut() {
                let body = format!(
                    "SFATAL\0VFATAL\0C28000\0M{CANARY}; source_private; /tmp/synthetic-peer\0\0"
                );
                let mut bytes = vec![b'E'];
                bytes.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
                bytes.extend_from_slice(body.as_bytes());
                sent = socket.write_all(&bytes).is_ok();
            }
        }
        let run = child.wait(started + Duration::from_secs(14));
        let elapsed = started.elapsed();
        let closed = peer.as_mut().is_some_and(|(socket, _)| closed(socket));
        drop(peer);
        let ambient_untouched = !watch.activity();
        let control = File::open(&pass).unwrap();
        let oracle_positive = watch.activity();
        drop(control);
        Observation {
            run,
            safe: fixture.identity.safe_ids(),
            reached,
            explicit,
            sent,
            closed,
            no_retry: support::no_dial(&db),
            no_host: support::no_host(bind),
            ambient_untouched,
            oracle_positive,
            elapsed,
        }
    })
}

#[test]
fn platform_serve_cli_failed_peer_is_single_explicit_attempt_without_ready_or_ambient_fallback() {
    let result = exercise(false);
    support::rejected(&result.run, 1);
    assert!(
        result.no_retry && result.no_host && result.ambient_untouched && result.oracle_positive
    );
    if result.safe {
        assert!(result.reached && result.explicit && result.sent && result.closed);
    } else {
        assert!(!result.reached, "unsafe identity reached the database peer");
    }
}

#[test]
fn platform_serve_cli_stalled_peer_hits_open_deadline_and_closes_before_any_listener() {
    let result = exercise(true);
    support::rejected(&result.run, 1);
    assert!(
        result.no_retry && result.no_host && result.ambient_untouched && result.oracle_positive
    );
    if result.safe {
        assert!(result.reached && result.explicit && result.closed);
        assert!(
            result.elapsed >= Duration::from_secs(9) && result.elapsed < Duration::from_secs(14)
        );
    } else {
        assert!(!result.reached);
    }
}
