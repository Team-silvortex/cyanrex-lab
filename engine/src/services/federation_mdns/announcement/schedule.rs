//! Finite caller-driven preparation timing, never a probe, sender, lease renewal or permit.
use super::{MdnsAnnouncement, MdnsError, Result};
use std::time::Duration;

/// Historical planning output only. Prepare neither encodes bytes nor authorizes transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncementStep {
    Wait(Duration),
    Prepare { ordinal: u8 },
    Complete,
}

/// At most two preparation intents over one fixed caller-supplied clock interval.
/// No independent constructor, Clone, clock lookup, socket or live probing context is exposed.
pub struct MdnsAnnouncementSchedule<'a> {
    _announcement: &'a MdnsAnnouncement,
    timeout: Duration,
    expires_wall: Duration,
    lease: Duration,
    elapsed: Duration,
    wall: Duration,
    due: Duration,
    completed: u8,
    pending: bool,
    closed: bool,
}

impl MdnsAnnouncement {
    /// Borrow the frozen declaration and fix a two-to-five-second planning budget at elapsed zero.
    /// Short remaining F0 leases are allowed but may prevent even the second preparation intent.
    /// This accepts no quiet observation and establishes no right to announce either name.
    pub fn initial_schedule(
        &self,
        initial_wall: Duration,
        timeout: Duration,
    ) -> Result<MdnsAnnouncementSchedule<'_>> {
        if !(Duration::from_secs(2)..=Duration::from_secs(5)).contains(&timeout) {
            return Err(MdnsError::InvalidConfig);
        }
        let unix = initial_wall.as_secs();
        if unix < self.prepared_at {
            return Err(MdnsError::ClockInvalid);
        }
        let remaining = self
            .advertisement
            .lan_announcement_remaining(self.endpoint, unix)?;
        let expires_wall =
            Duration::from_secs(unix.checked_add(remaining).ok_or(MdnsError::ClockInvalid)?);
        let lease = expires_wall
            .checked_sub(initial_wall)
            .ok_or(MdnsError::ClockInvalid)?;
        Ok(MdnsAnnouncementSchedule {
            _announcement: self,
            timeout,
            expires_wall,
            lease,
            elapsed: Duration::ZERO,
            wall: initial_wall,
            due: Duration::ZERO,
            completed: 0,
            pending: false,
            closed: false,
        })
    }
}

impl MdnsAnnouncementSchedule<'_> {
    /// Return one intent at a time. Record its completion before requesting another step.
    /// Wait is bounded by all original deadlines, not a sleep operation or reusable validity.
    pub fn step(
        &mut self,
        elapsed: Duration,
        wall: Duration,
        cancelled: bool,
    ) -> Result<AnnouncementStep> {
        self.check(elapsed, wall, cancelled)?;
        if self.pending {
            return self.fail(MdnsError::Closed);
        }
        if self.completed == 2 {
            self.close();
            return Ok(AnnouncementStep::Complete);
        }
        if elapsed < self.due {
            return Ok(AnnouncementStep::Wait(
                (self.due - elapsed)
                    .min(self.timeout - elapsed)
                    .min(self.lease - elapsed)
                    .min(self.expires_wall - wall),
            ));
        }
        self.pending = true;
        Ok(AnnouncementStep::Prepare {
            ordinal: self.completed + 1,
        })
    }

    /// Record caller-reported attempt completion, not a send acknowledgement or delivery receipt.
    /// The next intent is at least one second after the first completion, never a catch-up burst.
    /// Neither progress nor completion renews the F0 declaration or any planning deadline.
    pub fn record_completion(
        &mut self,
        elapsed: Duration,
        wall: Duration,
        cancelled: bool,
    ) -> Result<()> {
        self.check(elapsed, wall, cancelled)?;
        if !self.pending {
            return self.fail(MdnsError::Closed);
        }
        if self.completed == 0 {
            let Some(due) = elapsed.checked_add(Duration::from_secs(1)) else {
                return self.fail(MdnsError::ClockInvalid);
            };
            self.due = due;
        }
        self.completed += 1;
        self.pending = false;
        Ok(())
    }

    /// Idempotent local model termination only; nothing is sent or withdrawn.
    pub fn close(&mut self) {
        self.pending = false;
        self.closed = true;
    }

    fn check(&mut self, elapsed: Duration, wall: Duration, cancelled: bool) -> Result<()> {
        if self.closed {
            return Err(MdnsError::Closed);
        }
        if cancelled {
            return self.fail(MdnsError::Cancelled);
        }
        if elapsed < self.elapsed || wall < self.wall {
            return self.fail(MdnsError::ClockInvalid);
        }
        self.elapsed = elapsed;
        self.wall = wall;
        if elapsed >= self.timeout {
            return self.fail(MdnsError::DeadlineExceeded);
        }
        if elapsed >= self.lease || wall >= self.expires_wall {
            return self.fail(MdnsError::ClockInvalid);
        }
        Ok(())
    }

    fn fail<T>(&mut self, error: MdnsError) -> Result<T> {
        self.close();
        Err(error)
    }
}
