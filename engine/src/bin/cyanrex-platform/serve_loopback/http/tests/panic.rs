//! The panic hook is global. Only an owned, separately executed test process installs it.
use super::support::*;
use axum::{routing::get, Router};
use std::{
    collections::BTreeMap,
    io::Read,
    process::{Child, Command, Stdio},
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::{mpsc, Semaphore},
};

const CHILD: &str = "CYANREX_TEST_LOOPBACK_PRIVATE_PANIC_CHILD";
const NAME:&str="serve_loopback::http::tests::panic::loopback_http_isolated_panic_hook_hides_payload_and_task_failure_drains_then_returns_error";
const PANIC: &str = "synthetic-panic-secret-source-path-backtrace-c24f";

struct ChildGuard {
    child: Option<Child>,
    pipes: Vec<JoinHandle<(Vec<u8>, bool)>>,
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        for pipe in self.pipes.drain(..) {
            let _ = pipe.join();
        }
    }
}
fn pipe(mut reader: impl Read + Send + 'static) -> JoinHandle<(Vec<u8>, bool)> {
    std::thread::spawn(move || {
        let mut saved = Vec::new();
        let mut overflow = false;
        let mut bytes = [0; 512];
        loop {
            match reader.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    let keep = count.min(4096usize.saturating_sub(saved.len()));
                    saved.extend_from_slice(&bytes[..keep]);
                    overflow |= keep != count;
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    overflow = true;
                    break;
                }
            }
        }
        (saved, overflow)
    })
}

#[test]
fn loopback_http_isolated_panic_hook_hides_payload_and_task_failure_drains_then_returns_error() {
    if std::env::var_os(CHILD).as_deref() == Some(std::ffi::OsStr::new("synthetic-v1")) {
        crate::serve_loopback::install_panic_hook();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(task_failure());
        return;
    }
    let before = std::env::vars_os().collect::<BTreeMap<_, _>>();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
        .env(CHILD, "synthetic-v1")
        .env("RUST_BACKTRACE", "full")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pipes = vec![
        pipe(child.stdout.take().unwrap()),
        pipe(child.stderr.take().unwrap()),
    ];
    let mut child = ChildGuard {
        child: Some(child),
        pipes,
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    let finished = loop {
        if child.child.as_mut().unwrap().try_wait().unwrap().is_some() {
            break true;
        }
        if Instant::now() >= deadline {
            let _ = child.child.as_mut().unwrap().kill();
            break false;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let status = child.child.take().unwrap().wait().unwrap();
    let output = child
        .pipes
        .drain(..)
        .map(|pipe| pipe.join().unwrap())
        .collect::<Vec<_>>();
    drop(child);
    assert!(
        finished && status.success(),
        "isolated panic transport child failed"
    );
    assert!(output.iter().all(|(_, overflow)| !*overflow));
    assert!(std::env::vars_os().collect::<BTreeMap<_, _>>() == before);
    assert!(
        String::from_utf8_lossy(&output[0].0).contains("1 passed"),
        "child exact selector did not execute one successful test"
    );
    assert!(
        output[1].0 == b"platform loopback host failed\n",
        "panic hook emitted more than its fixed private notice"
    );
    for (bytes, _) in output {
        let text = String::from_utf8_lossy(&bytes);
        for forbidden in [
            PANIC,
            "stack backtrace",
            "RUST_BACKTRACE",
            "src/bin/",
            "/tmp/",
        ] {
            assert!(
                !text.contains(forbidden),
                "isolated hook echoed panic diagnostics"
            );
        }
    }
}

async fn task_failure() {
    let release = std::sync::Arc::new(Semaphore::new(0));
    let wait = release.clone();
    let (entered, mut entries) = mpsc::unbounded_channel::<()>();
    let router = Router::new()
        .route(
            "/probe",
            get(move || {
                let wait = wait.clone();
                let entered = entered.clone();
                async move {
                    entered.send(()).unwrap();
                    wait.acquire().await.unwrap().forget();
                    "synthetic-ok"
                }
            }),
        )
        .route(
            "/panic",
            get(|| async {
                panic!("{PANIC}");
                #[allow(unreachable_code)]
                "not reached"
            }),
        );
    let server = Server::start(router).await;
    let mut held = TcpStream::connect(server.address).await.unwrap();
    held.write_all(&server.request()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), entries.recv())
        .await
        .unwrap()
        .unwrap();
    let request = format!("GET /panic HTTP/1.1\r\nHost: {}\r\n\r\n", server.authority);
    denied(&exchange(server.address, request.as_bytes()).await);
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if TcpStream::connect(server.address).await.is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("task panic did not stop the listener");
    assert!(
        !server.finished(),
        "task failure skipped draining another admitted handler"
    );
    release.add_permits(1);
    accepted(&read(held).await);
    server.drain_expired().await;
}
