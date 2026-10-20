use super::{
    fixture::{Fixture, Run},
    kernel::clear_child_capabilities,
};
use std::{
    ffi::{OsStr, OsString},
    io::{self, Read},
    os::fd::AsRawFd,
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};

struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn nonblocking(fd: i32) {
    // SAFETY: only changes flags on this child's retained output pipe.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    assert!(flags >= 0 && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } >= 0);
}
fn drain(reader: &mut impl Read, bytes: &mut Vec<u8>) {
    let mut chunk = [0; 512];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => return,
            Ok(n) => {
                assert!(bytes.len() + n <= 4096, "owned output budget exceeded");
                bytes.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => panic!("owned output read failed"),
        }
    }
}
pub(super) fn run(f: &Fixture, args: &[OsString], socket: Option<&OsStr>) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cyanrex-platform"));
    cmd.args(args)
        .current_dir(f.path("."))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, _) in std::env::vars_os() {
        if name.as_encoded_bytes().starts_with(b"PG") {
            cmd.env_remove(name);
        }
    }
    cmd.env_remove("DATABASE_URL").env_remove("NOTIFY_SOCKET");
    if let Some(socket) = socket {
        cmd.env("NOTIFY_SOCKET", socket);
    }
    clear_child_capabilities(&mut cmd);
    let mut child = Owned(cmd.spawn().unwrap());
    nonblocking(child.0.stdout.as_ref().unwrap().as_raw_fd());
    nonblocking(child.0.stderr.as_ref().unwrap().as_raw_fd());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let until = Instant::now() + Duration::from_secs(5);
    let mut timed_out = false;
    let status = loop {
        drain(child.0.stdout.as_mut().unwrap(), &mut out);
        drain(child.0.stderr.as_mut().unwrap(), &mut err);
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= until {
            timed_out = true;
            child.0.kill().unwrap();
            break child.0.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    drain(child.0.stdout.as_mut().unwrap(), &mut out);
    drain(child.0.stderr.as_mut().unwrap(), &mut err);
    Run {
        output: Output {
            status,
            stdout: out,
            stderr: err,
        },
        timed_out,
    }
}
