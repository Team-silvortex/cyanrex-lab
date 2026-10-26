use super::{
    child::{safe_threads, stopped},
    fixture::*,
};
use serde_json::{json, Value};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

struct Browser {
    child: Child,
    input: ChildStdin,
    output: tokio::io::Lines<BufReader<ChildStdout>>,
}
impl Browser {
    async fn start(config: Value) -> Self {
        for key in ["CYANREX_PLAYWRIGHT_MODULE", "CYANREX_CHROMIUM_PATH"] {
            let path = PathBuf::from(
                std::env::var_os(key).unwrap_or_else(|| panic!("set explicit {key}")),
            );
            assert!(
                path.is_absolute() && path.is_file(),
                "explicit browser dependency must be an existing absolute file"
            );
        }
        let mut child = Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/platform_browser_session/browser.mjs"
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("owned browser harness startup failed");
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut browser = Self {
            child,
            input,
            output,
        };
        browser.send(config).await;
        browser
    }
    async fn send(&mut self, value: Value) {
        let mut data = serde_json::to_vec(&value).unwrap();
        data.push(b'\n');
        within(self.input.write_all(&data)).await.unwrap();
        within(self.input.flush()).await.unwrap();
    }
    async fn phase(&mut self, expected: &str) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(40), self.output.next_line())
            .await
            .expect("bounded browser phase timed out")
            .expect("owned browser IPC read failed")
            .expect("owned browser exited before required phase");
        assert!(line.len() <= 2048, "bounded browser IPC frame required");
        let value: Value = serde_json::from_str(&line).expect("invalid browser phase frame");
        assert!(
            value["phase"] == expected,
            "browser phase {expected} failed at {}",
            value["stage"].as_str().unwrap_or("protocol boundary")
        );
        value
    }
    async fn advance(&mut self) {
        self.send(json!({"continue":true})).await;
    }
    async fn finish(mut self) {
        let status = within(self.child.wait()).await.unwrap();
        assert!(status.success(), "owned browser harness failed");
    }
}

pub(super) async fn exercise(tls: bool) {
    if tls {
        // Chromium checks this before XDG_DATA_HOME; never alter or open it for our CA.
        let directory = std::env::var_os("HOME")
            .map(PathBuf::from)
            .expect("browser account directory unavailable");
        assert!(!directory.join(".pki/nssdb").exists(),"TLS browser blocked: legacy user NSS overrides private XDG trust; do not change HOME, import into user NSS or bypass verification");
    }
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f, tls).await;
            let unchanged = resources(f).await;
            let files = p.files();
            let original_counter = watermark(f).await;
            no_ui(&p).await;
            assert_eq!(session_count(f).await, 0);
            assert_eq!(original_counter, watermark(f).await);
            let mut host = p.spawn(true);
            assert!(
                host.ready().await,
                "explicit Session UI host never became ready"
            );
            assert!(safe_threads(host.pid(), p.identity()));
            let mut browser = Browser::start(p.browser_input()).await;
            if tls {
                browser.phase("ca_rejected").await;
                assert_eq!(session_count(f).await, 0);
                assert_eq!(watermark(f).await, original_counter);
                assert_eq!(unchanged, resources(f).await);
                browser.advance().await;
            }
            browser.phase("ready_login").await;
            assert_eq!(session_count(f).await, 0);
            assert_eq!(watermark(f).await, original_counter);
            assert_eq!(unchanged, resources(f).await);
            assert_eq!(files, p.files());
            // Sample once only after both TLS/browser startup and the initial 401 have
            // completed. A slow CA phase cannot consume the first login's freshness budget.
            browser.send(json!({"otp":otp()})).await;
            let logged = browser.phase("authenticated").await;
            let digest = logged["digest"]
                .as_str()
                .expect("browser must hand off only a Session digest")
                .to_owned();
            exact_session(f, &digest, true).await;
            let consumed = watermark(f).await;
            assert!(consumed > original_counter);
            assert_eq!(unchanged, resources(f).await);
            assert_eq!(files, p.files());
            browser.advance().await;
            browser.phase("cancelled").await;
            exact_session(f, &digest, true).await;
            assert_eq!(watermark(f).await, consumed);
            if !tls {
                f.sql(&format!(
                    "REVOKE DELETE ON {}.sessions FROM {}",
                    f.source, f.role
                ))
                .await;
            }
            browser.advance().await;
            if !tls {
                browser.phase("uncertain_logout").await;
                exact_session(f, &digest, true).await;
                assert_eq!(watermark(f).await, consumed);
                assert_eq!(unchanged, resources(f).await);
                assert_eq!(files, p.files());
                // Only fixture administration restores its original grant; the UI never repairs it.
                f.sql(&format!(
                    "GRANT DELETE ON {}.sessions TO {}",
                    f.source, f.role
                ))
                .await;
                browser.advance().await;
            }
            browser.phase("logged_out").await;
            exact_session(f, &digest, false).await;
            assert_eq!(consumed, watermark(f).await);
            assert_eq!(unchanged, resources(f).await);
            assert_eq!(files, p.files());
            browser.advance().await;
            browser.phase("complete").await;
            browser.finish().await;
            assert!(stopped(&host.stop().await));
            assert!(within(tokio::net::TcpStream::connect(p.bind()))
                .await
                .is_err());
            assert!(super::storage_helpers::no_connections(f).await);
            assert_eq!(unchanged, resources(f).await);
            assert_eq!(files, p.files());
            assert!(p.finish());
        })
    })
    .await;
}
