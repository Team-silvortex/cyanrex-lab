//! Owned Unix datagrams only; no service manager, deployment or privileged notification tests.
use std::{
    os::unix::{fs::DirBuilderExt, net::UnixDatagram},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const CHILD_CASE: &str = "CYANREX_TEST_NATIVE_NOTIFY_CHILD";
const CHILD_NAME: &str = "serve_loopback::notify::tests::notification_child";

#[path = "delivery.rs"]
mod delivery;
#[path = "driver.rs"]
mod driver;
#[path = "endpoints.rs"]
mod endpoints;
#[path = "support.rs"]
mod support;

#[test]
fn notifier_real_pathname_delivers_both_fixed_messages_from_an_isolated_process() {
    let directory =
        std::env::temp_dir().join(format!("cyanrex-native-notify-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let path = directory.join("receiver");
    let receiver = UnixDatagram::bind(&path).unwrap();
    receiver.set_nonblocking(true).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            CHILD_NAME,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env(CHILD_CASE, "both")
        .env("NOTIFY_SOCKET", &path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut bytes = [0; 64];
    let first = receiver
        .recv(&mut bytes)
        .ok()
        .map(|length| bytes[..length].to_vec());
    let second = receiver
        .recv(&mut bytes)
        .ok()
        .map(|length| bytes[..length].to_vec());
    drop(receiver);
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&directory).unwrap();
    assert!(status.success(), "the real notification child failed");
    assert_eq!(first.as_deref(), Some(b"READY=1".as_slice()));
    assert_eq!(second.as_deref(), Some(b"STOPPING=1".as_slice()));
}

#[test]
#[ignore = "isolated subprocess helper, selected only by owned parent tests"]
fn notification_child() {
    match std::env::var(CHILD_CASE).as_deref() {
        Ok("both") => {
            let notifier = super::Notifier::from_environment(true).unwrap().unwrap();
            notifier.ready().unwrap();
            notifier.stopping().unwrap();
        }
        Ok("disabled") => assert!(super::Notifier::from_environment(false).unwrap().is_none()),
        Ok("invalid") => assert!(super::Notifier::from_environment(true).is_err()),
        Ok("peerpin") => delivery::retained_peer_fixture(),
        Ok("stopping_error") => {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(driver::stopping_failure_fixture());
        }
        _ => panic!("unexpected isolated notification fixture mode"),
    }
}
