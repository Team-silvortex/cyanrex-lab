//! Pure per-controller admission only: no clock, sleep, I/O or publication authority.
use super::ProbeControllerState;
use crate::services::federation_mdns::MdnsError;
use std::time::Duration;

pub(super) const MAX_ATTEMPTS: u8 = 4;
pub(super) const MAX_LIFETIME: Duration = Duration::from_secs(30);
const RETRY_DELAY: Duration = Duration::from_secs(5);

type Result<T> = std::result::Result<T, MdnsError>;

#[derive(Debug)]
enum Phase {
    Ready,
    InFlight,
    CoolingDown { until: Duration },
    Finished,
    Closed,
}

/// The owner supplies elapsed samples from its one real, private monotonic origin.
/// Independent gates have independent histories; this is not a process-wide limiter.
#[derive(Debug)]
pub(super) struct RetryGate {
    phase: Phase,
    attempts: u8,
    watermark: Duration,
}

impl RetryGate {
    pub(super) fn new() -> Self {
        Self {
            phase: Phase::Ready,
            attempts: 0,
            watermark: Duration::ZERO,
        }
    }

    /// An early cooldown check advances time but cannot spend or extend an attempt.
    pub(super) fn begin(&mut self, now: Duration) -> Result<()> {
        match self.phase {
            Phase::Closed | Phase::Finished => return Err(MdnsError::Closed),
            Phase::InFlight => return self.fail(MdnsError::Closed),
            Phase::Ready | Phase::CoolingDown { .. } => {}
        }
        self.observe_time(now)?;
        self.check_lifetime(now)?;
        if let Phase::CoolingDown { until } = self.phase {
            if now < until {
                return Err(MdnsError::CoolingDown);
            }
        }
        if self.attempts == MAX_ATTEMPTS {
            return self.fail(MdnsError::Closed);
        }
        self.attempts += 1;
        self.phase = Phase::InFlight;
        Ok(())
    }

    /// The private live adapter checks the same lifetime around every I/O boundary.
    pub(super) fn check_in_flight(&mut self, now: Duration) -> Result<()> {
        match self.phase {
            Phase::Closed | Phase::Finished => return Err(MdnsError::Closed),
            Phase::Ready | Phase::CoolingDown { .. } => return self.fail(MdnsError::Closed),
            Phase::InFlight => {}
        }
        self.observe_time(now)?;
        self.check_lifetime(now)
    }

    /// A quiet observation is terminal, not an enduring permission to announce.
    /// Conservative conflicts wait five seconds from completion, never from begin.
    pub(super) fn finish(&mut self, now: Duration, conflict: bool) -> Result<()> {
        match self.phase {
            Phase::Closed | Phase::Finished => return Err(MdnsError::Closed),
            Phase::Ready | Phase::CoolingDown { .. } => return self.fail(MdnsError::Closed),
            Phase::InFlight => {}
        }
        self.observe_time(now)?;
        let next = if conflict {
            let Some(until) = now.checked_add(RETRY_DELAY) else {
                return self.fail(MdnsError::ClockInvalid);
            };
            Some(until)
        } else {
            None
        };
        self.check_lifetime(now)?;
        self.phase = match next {
            None => Phase::Finished,
            Some(_) if self.attempts == MAX_ATTEMPTS => Phase::Closed,
            Some(until) => Phase::CoolingDown { until },
        };
        Ok(())
    }

    pub(super) fn close(&mut self) {
        self.phase = Phase::Closed;
    }

    pub(super) fn state(&self) -> ProbeControllerState {
        match self.phase {
            Phase::Ready => ProbeControllerState::Ready,
            Phase::CoolingDown { .. } => ProbeControllerState::CoolingDown,
            Phase::Finished => ProbeControllerState::Finished,
            // The synchronous public wrapper never exposes an in-flight admission.
            Phase::InFlight | Phase::Closed => ProbeControllerState::Closed,
        }
    }

    pub(super) fn attempts_started(&self) -> u8 {
        self.attempts
    }

    /// Read-only hint: even zero remaining never authorizes work or advances state/time.
    pub(super) fn remaining(&self, now: Duration) -> Option<Duration> {
        match self.phase {
            Phase::CoolingDown { until } => Some(until.saturating_sub(now)),
            _ => None,
        }
    }

    fn observe_time(&mut self, now: Duration) -> Result<()> {
        if now < self.watermark {
            return self.fail(MdnsError::ClockInvalid);
        }
        self.watermark = now;
        Ok(())
    }

    fn check_lifetime(&mut self, now: Duration) -> Result<()> {
        if now >= MAX_LIFETIME {
            return self.fail(MdnsError::DeadlineExceeded);
        }
        Ok(())
    }

    fn fail<T>(&mut self, error: MdnsError) -> Result<T> {
        self.close();
        Err(error)
    }
}

#[cfg(test)]
#[path = "gate/tests.rs"]
mod tests;
