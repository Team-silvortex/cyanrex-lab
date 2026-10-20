use super::{CHILD_CASE, CHILD_NAME};
use std::{
    ffi::OsStr,
    fs,
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, PermissionsExt},
            net::UnixDatagram,
        },
    },
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) struct Directory(pub(super) PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-notification-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

pub(super) fn child(case: &str, endpoint: Option<&OsStr>) -> (u32, bool) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            CHILD_NAME,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env(CHILD_CASE, case)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(endpoint) = endpoint {
        command.env("NOTIFY_SOCKET", endpoint);
    }
    let mut child = command.spawn().unwrap();
    let pid = child.id();
    let deadline = Instant::now() + Duration::from_secs(5);
    let finished = loop {
        if child.try_wait().unwrap().is_some() {
            break true;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break false;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let output = child.wait_with_output().unwrap();
    (
        pid,
        finished
            && output.status.success()
            && output.stdout.windows(8).any(|bytes| bytes == b"1 passed"),
    )
}

pub(super) fn pass_credentials(socket: &UnixDatagram) {
    let enabled: libc::c_int = 1;
    // SAFETY: the borrowed live socket and correctly sized int remain valid for this syscall.
    assert_eq!(
        unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PASSCRED,
                (&enabled as *const libc::c_int).cast(),
                std::mem::size_of_val(&enabled) as libc::socklen_t,
            )
        },
        0
    );
}

pub(super) struct Packet {
    pub(super) bytes: Vec<u8>,
    pub(super) credentials: Option<(libc::pid_t, libc::uid_t, libc::gid_t)>,
    pub(super) other_control: bool,
}
pub(super) fn packet(socket: &UnixDatagram) -> Option<Packet> {
    let mut bytes = [0u8; 64];
    let mut control = [0usize; 16];
    let mut vector = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    // SAFETY: zero is a valid empty msghdr; its live stack-backed buffers are assigned below.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut vector;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = std::mem::size_of_val(&control);
    // SAFETY: every output buffer is valid/writable for its stated size for this one call.
    let length = unsafe {
        libc::recvmsg(
            socket.as_raw_fd(),
            &mut message,
            libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
        )
    };
    if length < 0 {
        return None;
    }
    assert_eq!(message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC), 0);
    let mut result = Packet {
        bytes: bytes[..length as usize].to_vec(),
        credentials: None,
        other_control: false,
    };
    // SAFETY: libc's CMSG traversal stays within the kernel-populated control buffer above.
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&message);
        while !header.is_null() {
            if (*header).cmsg_level == libc::SOL_SOCKET
                && (*header).cmsg_type == libc::SCM_CREDENTIALS
                && (*header).cmsg_len
                    == libc::CMSG_LEN(std::mem::size_of::<libc::ucred>() as u32) as usize
            {
                assert!(result.credentials.is_none());
                let credentials =
                    std::ptr::read_unaligned(libc::CMSG_DATA(header).cast::<libc::ucred>());
                result.credentials = Some((credentials.pid, credentials.uid, credentials.gid));
            } else {
                result.other_control = true;
            }
            header = libc::CMSG_NXTHDR(&message, header);
        }
    }
    Some(result)
}
