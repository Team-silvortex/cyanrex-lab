use super::{certificates::Certificates, support::*};
use std::{
    io::{self, Read, Write},
    net::{IpAddr, Ipv4Addr},
    time::{Duration, Instant},
};
#[test]
fn platform_tls_cli_valid_material_reaches_one_explicit_database_handshake_without_ready_or_fallback(
) {
    let (run, reached, explicit, sent, no_retry, no_ready, safe) = Fixture::checked(|f| {
        let db = listener();
        let bind = address();
        let root = root(f, "artifact-root");
        let config = input(
            f,
            "tls.json",
            &document(&root, db.local_addr().unwrap().port(), bind),
        );
        let tls = Certificates::generate(&f.path("."), IpAddr::V4(Ipv4Addr::LOCALHOST));
        let mut child = ChildGuard::spawn(
            f,
            &arguments(f, &config, bind, &tls.certificate, &tls.key),
            None,
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut socket = None;
        loop {
            match db.accept() {
                Ok((peer, _)) => {
                    socket = Some(peer);
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if child.finished() || Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                _ => break,
            }
        }
        let reached = socket.is_some();
        let mut explicit = false;
        let mut sent = false;
        if let Some(socket) = socket.as_mut() {
            socket
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut size = [0; 4];
            if socket.read_exact(&mut size).is_ok() {
                let size = u32::from_be_bytes(size) as usize;
                if (8..=4096).contains(&size) {
                    let mut bytes = vec![0; size - 4];
                    if socket.read_exact(&mut bytes).is_ok() {
                        explicit = bytes.starts_with(&196608u32.to_be_bytes())
                            && bytes.windows(22).any(|v| v == b"user\0platform_runtime\0")
                            && !bytes.windows(CANARY.len()).any(|v| v == CANARY.as_bytes());
                    }
                }
            }
            let body = format!("SFATAL\0VFATAL\0C28000\0M{CANARY}; /tmp/synthetic-peer\0\0");
            let mut response = vec![b'E'];
            response.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
            response.extend_from_slice(body.as_bytes());
            sent = socket.write_all(&response).is_ok();
        }
        let run = child.wait(Instant::now() + Duration::from_secs(5));
        drop(socket);
        (
            run,
            reached,
            explicit,
            sent,
            no_dial(&db),
            no_host(bind),
            f.identity.safe_ids(),
        )
    });
    rejected(&run, 1);
    assert!(no_retry && no_ready);
    if safe {
        assert!(
            reached,
            "valid TLS input did not reach the synthetic database peer"
        );
        assert!(
            explicit,
            "synthetic database startup was not the explicit frozen target"
        );
        assert!(sent, "synthetic database refusal was not written");
    } else {
        assert!(!reached);
    }
}
