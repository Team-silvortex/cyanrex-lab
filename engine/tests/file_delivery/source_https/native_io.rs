//! Native test pipes are bounded and interruptible; plaintext counters precede parsing.
use std::{
    io::{self, Read, Write},
    os::fd::AsRawFd,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use tokio::sync::watch;

pub(super) struct Control {
    stop: watch::Sender<bool>,
    received: AtomicUsize,
}
impl Control {
    pub(super) fn new() -> Self {
        let (stop, _) = watch::channel(false);
        Self {
            stop,
            received: AtomicUsize::new(0),
        }
    }
    pub(super) fn stop(&self) {
        self.stop.send_replace(true);
    }
    pub(super) fn stopped(&self) -> bool {
        *self.stop.borrow()
    }
    pub(super) fn subscribe(&self) -> watch::Receiver<bool> {
        self.stop.subscribe()
    }
    pub(super) fn received(&self) -> usize {
        self.received.load(Ordering::SeqCst)
    }
}

pub(super) fn interrupted() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "Source TLS fixture stopping")
}
pub(super) async fn until_stopped(mut receiver: watch::Receiver<bool>) {
    loop {
        if *receiver.borrow_and_update() {
            return;
        }
        if receiver.changed().await.is_err() {
            return;
        }
    }
}
pub(super) fn nonblocking<F: AsRawFd>(owned: &F) -> io::Result<()> {
    let fd = owned.as_raw_fd();
    // Only a pipe owned by this disposable fixture is changed.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn ready(fd: i32, events: i16, control: &Control, deadline: Instant) -> io::Result<()> {
    if control.stopped() {
        return Err(interrupted());
    }
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "bounded Source TLS pipe IO",
        ));
    }
    let mut descriptor = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    let result = unsafe { libc::poll(&mut descriptor, 1, 50) };
    if control.stopped() {
        return Err(interrupted());
    }
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub(super) fn write_all<W: Write + AsRawFd>(
    writer: &mut W,
    mut data: &[u8],
    control: &Control,
) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !data.is_empty() {
        ready(writer.as_raw_fd(), libc::POLLOUT, control, deadline)?;
        match writer.write(data) {
            _ if control.stopped() => return Err(interrupted()),
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "Source TLS pipe closed",
                ))
            }
            Ok(length) => data = &data[length..],
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
    }
    if control.stopped() {
        return Err(interrupted());
    }
    writer.flush()
}
pub(super) fn read_more<R: Read + AsRawFd>(
    reader: &mut R,
    pending: &mut Vec<u8>,
    control: &Control,
    deadline: Instant,
) -> io::Result<()> {
    ready(reader.as_raw_fd(), libc::POLLIN, control, deadline)?;
    let mut buffer = [0; 1024];
    let outcome = reader.read(&mut buffer);
    if let Ok(length) = outcome.as_ref() {
        control.received.fetch_add(*length, Ordering::SeqCst);
    }
    match outcome {
        _ if control.stopped() => Err(interrupted()),
        Ok(0) => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Source TLS pipe EOF",
        )),
        Ok(length) => {
            pending.extend_from_slice(&buffer[..length]);
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(error),
    }
}
