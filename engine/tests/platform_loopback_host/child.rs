use std::{
    io::{self, Read},
    os::fd::AsRawFd,
    process::{Child, Command, ExitStatus},
    time::{Duration, Instant},
};

pub(super) const READY: &[u8] = b"platform loopback host ready\n";
const STOPPED: &[u8] = b"platform loopback host stopped\n";
const FAILED: &[u8] = b"platform loopback host failed\n";
const MAX_OUTPUT: usize = 4096;

// Own only one direct child. No global PID/group signals or lifetime-15s background killer.
pub(super) struct Running {
    child: Child,
    status: Option<ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    output_ok: bool,
}
pub(super) struct Run {
    status: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    forced: bool,
    output_ok: bool,
}

fn nonblocking(fd: i32) -> io::Result<()> {
    // SAFETY: read/change only flags on this owned child-pipe descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn drain(reader: &mut impl Read, output: &mut Vec<u8>) -> bool {
    let mut bytes = [0; 512];
    loop {
        match reader.read(&mut bytes) {
            Ok(0) => return true,
            Ok(count) => {
                if output.len().saturating_add(count) > MAX_OUTPUT {
                    return false;
                }
                output.extend_from_slice(&bytes[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return true,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return false,
        }
    }
}

impl Running {
    pub(super) fn spawn(command: &mut Command) -> Self {
        let child = command
            .spawn()
            .unwrap_or_else(|_| panic!("owned loopback child spawn failed"));
        let mut running = Self {
            child,
            status: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            output_ok: true,
        };
        nonblocking(running.child.stdout.as_ref().unwrap().as_raw_fd()).unwrap();
        nonblocking(running.child.stderr.as_ref().unwrap().as_raw_fd()).unwrap();
        running.poll();
        running
    }
    pub(super) fn pid(&self) -> u32 {
        self.child.id()
    }
    pub(super) fn poll(&mut self) -> bool {
        self.output_ok &= drain(self.child.stdout.as_mut().unwrap(), &mut self.stdout);
        self.output_ok &= drain(self.child.stderr.as_mut().unwrap(), &mut self.stderr);
        if self.status.is_none() {
            self.status = self
                .child
                .try_wait()
                .unwrap_or_else(|_| panic!("owned child wait failed"));
        }
        self.status.is_some()
    }
    // Used by the sibling TLS notification cohort, not this helper's plain-only target.
    #[allow(dead_code)]
    pub(super) fn quiet_before_ready(&mut self) -> bool {
        !self.poll() && self.stdout.is_empty() && self.stderr.is_empty() && self.output_ok
    }
    pub(super) async fn ready(&mut self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let finished = self.poll();
            if self.stdout == READY && self.stderr.is_empty() && !finished && self.output_ok {
                return true;
            }
            if finished || !self.stderr.is_empty() || !self.output_ok || Instant::now() >= deadline
            {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    pub(super) fn signal(&mut self, signal: i32) -> bool {
        if self.poll() {
            return false;
        }
        // SAFETY: the retained Child is still unreaped/running; signal its exact positive PID.
        unsafe { libc::kill(self.child.id() as libc::pid_t, signal) == 0 }
    }
    pub(super) async fn finish(mut self, signal: Option<i32>, budget: Duration) -> Run {
        if let Some(signal) = signal {
            self.signal(signal);
        }
        let deadline = Instant::now() + budget;
        let mut forced = false;
        while !self.poll() {
            if Instant::now() >= deadline {
                forced = true;
                let _ = self.child.kill();
                self.status = Some(self.child.wait().unwrap());
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        self.poll();
        Run {
            status: self.status.and_then(|s| s.code()),
            stdout: std::mem::take(&mut self.stdout),
            stderr: std::mem::take(&mut self.stderr),
            forced,
            output_ok: self.output_ok,
        }
    }
    pub(super) async fn stop(self) -> Run {
        self.finish(Some(libc::SIGTERM), Duration::from_secs(35))
            .await
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if self.status.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
pub(super) fn stopped(run: &Run) -> bool {
    run.status == Some(0)
        && run.output_ok
        && !run.forced
        && run.stderr.is_empty()
        && run.stdout == [READY, STOPPED].concat()
}
pub(super) fn failed(run: &Run) -> bool {
    run.status == Some(1)
        && run.output_ok
        && !run.forced
        && run.stdout.is_empty()
        && run.stderr == FAILED
}
// Used by the sibling TLS notification cohort, not the plain-only target.
#[allow(dead_code)]
pub(super) fn failed_after_ready(run: &Run) -> bool {
    run.status == Some(1)
        && run.output_ok
        && !run.forced
        && run.stdout == READY
        && run.stderr == FAILED
}
pub(super) fn killed_before_ready(run: &Run) -> bool {
    run.status.is_none()
        && run.output_ok
        && !run.forced
        && run.stdout.is_empty()
        && run.stderr.is_empty()
}

/// Inspect all currently live child threads, not historical execution or a sandbox proof.
pub(super) fn safe_threads(pid: u32, identity: &super::kernel::Identity) -> bool {
    let entries = match std::fs::read_dir(format!("/proc/{pid}/task")) {
        Ok(e) => e,
        Err(_) => return false,
    };
    let mut observed = 0;
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        let text = match std::fs::read_to_string(entry.path().join("status")) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return false,
        };
        let field = |name: &str| {
            text.lines()
                .find_map(|line| line.strip_prefix(name))
                .map(str::trim)
        };
        let ids = |name| {
            field(name).and_then(|v| {
                v.split_whitespace()
                    .map(str::parse::<u32>)
                    .collect::<Result<Vec<_>, _>>()
                    .ok()
            })
        };
        let groups = ids("Groups:");
        if ids("Uid:") != Some(identity.uid.to_vec())
            || ids("Gid:") != Some(identity.gid.to_vec())
            || groups != Some(identity.groups.clone())
            || field("NoNewPrivs:") != Some("1")
            || ["CapPrm:", "CapEff:", "CapInh:", "CapAmb:"]
                .iter()
                .any(|name| !field(name).is_some_and(|s| u64::from_str_radix(s, 16) == Ok(0)))
        {
            return false;
        }
        // CapBnd is deliberately not required to be zero by the existing product policy.
        observed += 1;
    }
    observed > 0
}
