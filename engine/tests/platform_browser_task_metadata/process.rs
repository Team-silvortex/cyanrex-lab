use super::fixture::within;
use serde_json::{json, Value};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

pub(super) struct Browser {
    child: Child,
    input: ChildStdin,
    output: tokio::io::Lines<BufReader<ChildStdout>>,
}
impl Browser {
    pub(super) async fn start(config: Value) -> Self {
        for key in ["CYANREX_PLAYWRIGHT_MODULE", "CYANREX_CHROMIUM_PATH"] {
            let path = PathBuf::from(
                std::env::var_os(key).unwrap_or_else(|| panic!("set explicit {key}")),
            );
            assert!(
                path.is_absolute() && path.is_file(),
                "existing absolute browser dependency required"
            );
        }
        let mut child = Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/platform_browser_task_metadata/browser.mjs"
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("owned metadata browser startup failed");
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
    pub(super) async fn send(&mut self, value: Value) {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        within(self.input.write_all(&bytes)).await.unwrap();
        within(self.input.flush()).await.unwrap();
    }
    pub(super) async fn phase(&mut self, expected: &str) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(40), self.output.next_line())
            .await
            .expect("bounded metadata browser phase timed out")
            .expect("private metadata browser IPC failed")
            .expect("metadata browser exited before required phase");
        assert!(line.len() <= 2048);
        let value: Value = serde_json::from_str(&line).unwrap();
        assert!(
            value["phase"] == expected,
            "metadata browser phase {expected} failed at {} ({})",
            value["stage"].as_str().unwrap_or("protocol boundary"),
            value["check"].as_str().unwrap_or("protocol boundary")
        );
        value
    }
    pub(super) async fn advance(&mut self) {
        self.send(json!({"continue":true})).await;
    }
    pub(super) async fn finish(mut self) {
        assert!(
            within(self.child.wait()).await.unwrap().success(),
            "owned metadata browser failed"
        );
    }
}
