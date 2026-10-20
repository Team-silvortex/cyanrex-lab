//! Test-only ownership oracle. An address can remain connectable after its original owner closes:
//! another listener may reuse it, and concurrent fork/exec can briefly inherit CLOEXEC descriptors.
//! Never equate endpoint reachability with this process still owning the original socket.
use std::{
    fs, io,
    os::fd::{AsRawFd, RawFd},
};
use tokio::net::TcpListener;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SocketOwnership {
    device: libc::dev_t,
    inode: libc::ino_t,
}

fn identity(fd: RawFd) -> io::Result<Option<SocketOwnership>> {
    // SAFETY: fstat accepts any integer descriptor and reports EBADF on a concurrent close;
    // the stack-owned output points to a correctly sized stat record for this call only.
    let mut status: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut status) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(
        (status.st_mode & libc::S_IFMT == libc::S_IFSOCK).then_some(SocketOwnership {
            device: status.st_dev,
            inode: status.st_ino,
        }),
    )
}
impl SocketOwnership {
    pub(super) fn capture(listener: &TcpListener) -> io::Result<Self> {
        identity(listener.as_raw_fd())?
            .ok_or_else(|| io::Error::other("owned descriptor is not a socket"))
    }
    pub(super) fn is_open_in_this_process(&self) -> io::Result<bool> {
        for entry in fs::read_dir("/proc/self/fd")? {
            let entry = entry?;
            let Some(fd) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<RawFd>().ok())
            else {
                continue;
            };
            match identity(fd) {
                Ok(Some(current)) if current == *self => return Ok(true),
                Ok(_) => {}
                // Other test threads can close descriptors between readdir and fstat. A stable
                // original socket, if still held by this process, must retain at least one fd.
                Err(error) if error.raw_os_error() == Some(libc::EBADF) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(false)
    }
}

#[test]
fn socket_incarnation_detects_duplicated_owners_but_not_a_new_listener_at_the_same_address() {
    const CHILD: &str = "CYANREX_SOCKET_OWNERSHIP_TEST_CHILD";
    const NAME: &str = "serve_loopback::socket_ownership::socket_incarnation_detects_duplicated_owners_but_not_a_new_listener_at_the_same_address";
    if std::env::var_os(CHILD).as_deref() == Some(std::ffi::OsStr::new("1")) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    socket_incarnation_fixture(),
                )
                .await
                .expect("isolated socket ownership controls exceeded their bounded deadline");
            });
        return;
    }
    // All fixture sockets are created after exec. Concurrent parent fork/exec must not inherit
    // the original listener and keep its address bound after this test drops every owned fd.
    let mut child = ChildGuard(Some(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
            .env_clear()
            .env(CHILD, "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let finished = loop {
        if child.0.as_mut().unwrap().try_wait().unwrap().is_some() {
            break true;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.0.as_mut().unwrap().kill();
            break false;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let output = child.0.take().unwrap().wait_with_output().unwrap();
    assert!(
        finished && output.status.success(),
        "isolated socket ownership child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.windows(8).any(|bytes| bytes == b"1 passed"),
        "child exact selector did not execute one successful ownership test"
    );
}

struct ChildGuard(Option<std::process::Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

async fn socket_incarnation_fixture() {
    use std::os::fd::{FromRawFd, OwnedFd};
    use tokio::net::TcpStream;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let original = SocketOwnership::capture(&listener).unwrap();
    assert!(
        original.is_open_in_this_process().unwrap(),
        "live listener positive control was missed"
    );
    // SAFETY: F_DUPFD_CLOEXEC creates a separately owned copy of this borrowed live descriptor.
    let duplicated = unsafe { libc::fcntl(listener.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    assert!(duplicated >= 0);
    let duplicated = unsafe { OwnedFd::from_raw_fd(duplicated) };
    drop(listener);
    assert!(
        original.is_open_in_this_process().unwrap(),
        "oracle only checked the original fd number"
    );
    drop(duplicated);
    assert!(!original.is_open_in_this_process().unwrap());
    let replacement = TcpListener::bind(address).await.unwrap();
    let current = SocketOwnership::capture(&replacement).unwrap();
    assert!(
        current != original,
        "replacement socket unexpectedly retained the old incarnation"
    );
    assert!(current.is_open_in_this_process().unwrap());
    assert!(!original.is_open_in_this_process().unwrap());
    // Deterministic negative control for the old oracle: TCP really succeeds, but it reaches
    // the replacement while the original incarnation is absent from every owned descriptor.
    let client = TcpStream::connect(address).await.unwrap();
    let (accepted, _) = replacement.accept().await.unwrap();
    assert!(!original.is_open_in_this_process().unwrap());
    drop(client);
    drop(accepted);
    drop(replacement);
    assert!(!current.is_open_in_this_process().unwrap());
}
