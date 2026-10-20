use std::{
    fs, io,
    os::fd::AsRawFd,
    os::unix::{
        fs::{DirBuilderExt, PermissionsExt},
        net::UnixDatagram,
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(super) struct Receiver {
    directory: PathBuf,
    path: PathBuf,
    socket: Option<UnixDatagram>,
}
impl Receiver {
    pub(super) fn new() -> Self {
        let directory = PathBuf::from("/tmp").join(format!(
            "cyanrex-notify-fixture-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("notify.sock");
        let socket = UnixDatagram::bind(&path).unwrap();
        let enabled: libc::c_int = 1;
        // SAFETY: valid owned AF_UNIX socket and a correctly sized immutable option value.
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    socket.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_PASSCRED,
                    (&enabled as *const libc::c_int).cast(),
                    std::mem::size_of_val(&enabled) as _,
                )
            },
            0
        );
        socket.set_nonblocking(true).unwrap();
        Self {
            directory,
            path,
            socket: Some(socket),
        }
    }
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
    pub(super) fn take(&self) -> Option<Vec<u8>> {
        self.message().map(|(bytes, _)| bytes)
    }
    fn message(&self) -> Option<(Vec<u8>, libc::ucred)> {
        let mut bytes = [0; 512];
        let mut control = [0usize; 8];
        let mut iov = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        // SAFETY: zero is a valid initial msghdr; all buffer pointers below remain live across recvmsg.
        let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = control.as_mut_ptr().cast();
        msg.msg_controllen = std::mem::size_of_val(&control);
        let n = unsafe {
            libc::recvmsg(
                self.socket
                    .as_ref()
                    .expect("owned receiver already closed")
                    .as_raw_fd(),
                &mut msg,
                libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
            )
        };
        if n < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock {
                return None;
            }
            panic!("owned notification receive failed");
        }
        assert_eq!(
            msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC),
            0,
            "owned notification truncated"
        );
        assert!(
            (n as usize) < bytes.len(),
            "notification exceeded fixture budget"
        );
        // SAFETY: libc validates ancillary framing; credentials are copied unaligned from one complete cmsg.
        let credentials = unsafe {
            let cmsg = libc::CMSG_FIRSTHDR(&msg);
            assert!(
                !cmsg.is_null()
                    && (*cmsg).cmsg_level == libc::SOL_SOCKET
                    && (*cmsg).cmsg_type == libc::SCM_CREDENTIALS
            );
            assert_eq!(
                (*cmsg).cmsg_len,
                libc::CMSG_LEN(std::mem::size_of::<libc::ucred>() as _) as usize
            );
            assert!(
                libc::CMSG_NXTHDR(&msg, cmsg).is_null(),
                "unexpected notification ancillary data"
            );
            std::ptr::read_unaligned(libc::CMSG_DATA(cmsg).cast::<libc::ucred>())
        };
        Some((bytes[..n as usize].to_vec(), credentials))
    }
    pub(super) async fn wait(&self, expected: &[u8]) -> bool {
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(bytes) = self.take() {
                return bytes == expected;
            }
            if Instant::now() >= until {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    pub(super) async fn wait_from(&self, expected: &[u8], pid: u32, uid: u32, gid: u32) -> bool {
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some((bytes, credentials)) = self.message() {
                return bytes == expected
                    && credentials.pid == pid as i32
                    && credentials.uid == uid
                    && credentials.gid == gid;
            }
            if Instant::now() >= until {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    pub(super) fn control(&self) -> bool {
        UnixDatagram::unbound()
            .unwrap()
            .send_to(b"fixture-control", &self.path)
            .unwrap();
        self.take().as_deref() == Some(b"fixture-control".as_slice())
    }
    pub(super) fn close(&mut self) {
        drop(self.socket.take());
        fs::remove_file(&self.path).unwrap();
    }
    pub(super) fn finish(mut self) -> bool {
        drop(self.socket.take());
        let removed = match fs::remove_file(&self.path) {
            Ok(()) => true,
            Err(e) if e.kind() == io::ErrorKind::NotFound => true,
            Err(_) => false,
        };
        removed && fs::remove_dir(&self.directory).is_ok()
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        drop(self.socket.take());
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}
