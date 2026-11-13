//! Explicit one-attempt observation only, never a name-ownership or announcement permit.
use super::{
    socket::{Channel, Ingress, ProbeIo},
    MdnsAnnouncement, MdnsConfig, MdnsError, ProbeObservation, Result, VerifiedInterface,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod limits;
mod retained;
mod schedule;
#[cfg(test)]
mod tests;
use limits::{jitter_value, Limits};
pub use retained::{MdnsRetainedProbe, RetainedProbeOutcome};
use schedule::{ProbeSchedule, Step};

/// Completed observations of one restricted attempt; no reusable sending authority is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeRunObservation {
    NoConflictObserved,
    ExistingResponse,
    CompetingProbe,
}

fn cancelled(flag: &AtomicBool) -> Result<()> {
    if flag.load(Ordering::Acquire) {
        Err(MdnsError::Cancelled)
    } else {
        Ok(())
    }
}

fn wall_time() -> Result<Duration> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| MdnsError::ClockInvalid)
}

fn jitter() -> Result<u16> {
    for _ in 0..8 {
        let mut sample = 0_u16;
        // SAFETY: exactly two writable bytes, nonblocking kernel entropy, no file or weak fallback.
        let length =
            unsafe { libc::getrandom((&mut sample as *mut u16).cast(), 2, libc::GRND_NONBLOCK) };
        if length != 2 {
            return Err(MdnsError::EntropyUnavailable);
        }
        if let Some(value) = jitter_value(sample) {
            return Ok(value);
        }
    }
    Err(MdnsError::EntropyUnavailable)
}

struct Sample {
    elapsed: Duration,
    wall: u64,
}

struct Run<'a, 'i, 'g> {
    started: Instant,
    limits: Limits,
    announcement: &'a MdnsAnnouncement,
    interface: &'i VerifiedInterface,
    cancelled: &'i AtomicBool,
    guard: &'g mut dyn FnMut() -> Result<()>,
}

/// Synchronous borrowed guards surround each active check; no callback is retained in a handle.
pub(super) fn guarded_check<T>(
    guard: &mut dyn FnMut() -> Result<()>,
    body: impl FnOnce() -> Result<T>,
) -> Result<T> {
    guard()?;
    let result = body()?;
    guard()?;
    Ok(result)
}

impl Run<'_, '_, '_> {
    fn poll(&mut self, io: &mut ProbeIo, requested: Duration) -> Result<Vec<Ingress>> {
        let sample = self.check()?;
        let wait = self.limits.wait_bound(requested, sample.elapsed)?;
        io.receive_ready(wait)
    }

    fn check(&mut self) -> Result<Sample> {
        let Self {
            started,
            limits,
            announcement,
            interface,
            cancelled: cancellation,
            guard,
        } = self;
        cancelled(cancellation)?;
        guarded_check(*guard, || {
            let elapsed = started.elapsed();
            let wall = wall_time()?;
            limits.check(elapsed, wall)?;
            announcement.probe_remaining(interface.local_address(), wall.as_secs())?;
            interface.verify_unchanged()?;
            cancelled(cancellation)?;
            Ok(Sample {
                elapsed,
                wall: wall.as_secs(),
            })
        })
    }

    fn observe(&mut self, batch: Vec<Ingress>, sent: u8) -> Result<Option<ProbeRunObservation>> {
        let sample = self.check()?;
        // Every consumed datagram across BOTH channels spends the same quota, including rejects.
        for packet in &batch {
            if !matches!(packet.received, Ok(None)) {
                self.limits.admit(sample.elapsed)?;
            }
        }
        let mut outcome = None;
        for packet in batch {
            if let Some(received) = packet.received? {
                let observation =
                    classify(self.announcement, packet.channel, sent, &received.bytes)?;
                if outcome.is_none() {
                    outcome = observation;
                }
            }
        }
        self.check()?;
        Ok(outcome)
    }

    fn drain(&mut self, io: &mut ProbeIo, sent: u8) -> Result<Option<ProbeRunObservation>> {
        loop {
            self.check()?;
            let batch = self.poll(io, Duration::ZERO)?;
            if batch.is_empty() {
                return Ok(None);
            }
            if let Some(observation) = self.observe(batch, sent)? {
                return Ok(Some(observation));
            }
        }
    }

    fn wait_after(
        &mut self,
        io: &mut ProbeIo,
        last_sent: Option<Duration>,
        sent: u8,
    ) -> Result<Option<ProbeRunObservation>> {
        loop {
            let remaining = remaining_wait(last_sent, self.check()?.elapsed)?;
            if remaining.is_zero() {
                return Ok(None);
            }
            let batch = self.poll(io, remaining.min(Duration::from_millis(50)))?;
            if let Some(observation) = self.observe(batch, sent)? {
                return Ok(Some(observation));
            }
        }
    }
}

fn remaining_wait(last_sent: Option<Duration>, elapsed: Duration) -> Result<Duration> {
    let Some(last_sent) = last_sent else {
        return Ok(Duration::ZERO);
    };
    let since = elapsed
        .checked_sub(last_sent)
        .ok_or(MdnsError::ClockInvalid)?;
    Ok(Duration::from_millis(250)
        .checked_sub(since)
        .unwrap_or(Duration::ZERO))
}

fn classify(
    announcement: &MdnsAnnouncement,
    channel: Channel,
    sent: u8,
    bytes: &[u8],
) -> Result<Option<ProbeRunObservation>> {
    // Pre-first-send packets are drained but cannot be evidence of a reply to this attempt.
    if sent == 0 {
        return Ok(None);
    }
    // Within the <=2s attempt, unicast is admissible only as a response to our ANY/QU names.
    if channel == Channel::Unicast && bytes.get(2).is_none_or(|byte| byte & 0x80 == 0) {
        return Err(MdnsError::InvalidPacket);
    }
    Ok(match announcement.observe_probe_packet(bytes)? {
        ProbeObservation::NoRelevantClaim => None,
        ProbeObservation::ExistingResponse => Some(ProbeRunObservation::ExistingResponse),
        ProbeObservation::CompetingProbe => Some(ProbeRunObservation::CompetingProbe),
    })
}

/// Three actual proposals, >=250ms apart, then >=250ms receive-only; never advertises a service.
/// Requires 1..=2s timeout and an announcement matching the selected local IPv4. Both channels
/// must open before any send. All errors/cancellation/conflicts drop both owned sockets on return.
/// This strict profile conservatively stops on competing proposals or rejected post-send packets;
/// it does not implement a tiebreak, retry, rename, continuous defense or availability guarantee.
pub fn probe_once(
    config: &MdnsConfig,
    announcement: &MdnsAnnouncement,
    cancellation: &AtomicBool,
) -> Result<ProbeRunObservation> {
    probe_guarded(config, announcement, cancellation, &mut || Ok(()))
}

// Only trusted sibling composition can add stricter cross-attempt constraints. This is not an
// exported callback/input surface, and an error can never become a quiet observation.
pub(super) fn probe_guarded(
    config: &MdnsConfig,
    announcement: &MdnsAnnouncement,
    cancellation: &AtomicBool,
    guard: &mut dyn FnMut() -> Result<()>,
) -> Result<ProbeRunObservation> {
    // Historical APIs still drop both original sockets before returning. In particular a local
    // controller guard never escapes into a retained handle after its callback has gone away.
    Ok(run_probe(config, announcement, cancellation, guard)?.observation())
}

// The stack-borrowed guard protects active probing only. Its owner separately transfers concrete
// original bounds into a quiet handle after the controller's final completion checks.
pub(super) fn probe_retained_guarded<'a>(
    config: &MdnsConfig,
    announcement: &'a MdnsAnnouncement,
    cancellation: &AtomicBool,
    guard: &mut dyn FnMut() -> Result<()>,
) -> Result<RetainedProbeOutcome<'a>> {
    run_probe(config, announcement, cancellation, guard)
}

/// One explicit attempt whose quiet branch retains the ORIGINAL two sockets and original bounds.
/// The opaque context permits only a short receive-only review, not an announcement or retry.
/// This separate one-attempt entry does not inherit any F1h controller's retry history/guard.
pub fn probe_retained_once<'a>(
    config: &MdnsConfig,
    announcement: &'a MdnsAnnouncement,
    cancellation: &AtomicBool,
) -> Result<RetainedProbeOutcome<'a>> {
    cancelled(cancellation)?;
    if !(Duration::from_secs(1)..=Duration::from_secs(2)).contains(&config.timeout())
        || !announcement.matches_probe_source(config.local_address())
    {
        return Err(MdnsError::InvalidConfig);
    }
    run_probe(config, announcement, cancellation, &mut || Ok(()))
}

fn run_probe<'a>(
    config: &MdnsConfig,
    announcement: &'a MdnsAnnouncement,
    cancellation: &AtomicBool,
    guard: &mut dyn FnMut() -> Result<()>,
) -> Result<RetainedProbeOutcome<'a>> {
    cancelled(cancellation)?;
    guard()?;
    let started = Instant::now();
    let wall = wall_time()?;
    let remaining = announcement.probe_remaining(config.local_address(), wall.as_secs())?;
    let limits = Limits::new(config.timeout(), wall, remaining)?;
    let delay = jitter()?;
    cancelled(cancellation)?;
    guard()?;
    let interface = VerifiedInterface::inspect(config)?;
    let mut run = Run {
        started,
        limits,
        announcement,
        interface: &interface,
        cancelled: cancellation,
        guard,
    };
    run.check()?;
    let mut io = ProbeIo::open(&interface)?;
    let base = run.check()?.elapsed;
    let mut schedule = ProbeSchedule::new(delay)?;
    let mut last_sent = None;
    loop {
        let sample = run.check()?;
        let elapsed = sample
            .elapsed
            .checked_sub(base)
            .ok_or(MdnsError::ClockInvalid)?;
        let milliseconds =
            u64::try_from(elapsed.as_millis()).map_err(|_| MdnsError::ClockInvalid)?;
        match schedule.step(milliseconds)? {
            Step::Wait(remaining) => {
                let batch = run.poll(&mut io, Duration::from_millis(remaining.min(50)))?;
                if let Some(observation) = run.observe(batch, schedule.sent_count())? {
                    return retained::conflict(observation);
                }
            }
            step => {
                // The integer schedule cannot round down an actual sub-millisecond completion.
                if let Some(observation) =
                    run.wait_after(&mut io, last_sent, schedule.sent_count())?
                {
                    return retained::conflict(observation);
                }
                // Bound/fairly drain both queues at send and terminal boundaries; a flood fails.
                if let Some(observation) = run.drain(&mut io, schedule.sent_count())? {
                    return retained::conflict(observation);
                }
                let sample = run.check()?;
                if matches!(step, Step::Complete) {
                    return Ok(RetainedProbeOutcome::Quiet(MdnsRetainedProbe::from_run(
                        run,
                        io,
                        sample.elapsed,
                    )?));
                }
                io.send_probe(announcement, sample.wall)?;
                let completed = run.check()?.elapsed;
                last_sent = Some(completed);
                let after = completed.checked_sub(base).ok_or(MdnsError::ClockInvalid)?;
                let milliseconds =
                    u64::try_from(after.as_millis()).map_err(|_| MdnsError::ClockInvalid)?;
                schedule.sent(milliseconds)?;
            }
        }
    }
}
