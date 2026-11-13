//! Caller-owned finite retry history; retained quiet review is explicit, never a sending permit.
use super::{
    MdnsAnnouncement, MdnsConfig, MdnsError, ProbeRunObservation, Result, RetainedProbeOutcome,
    VerifiedInterface,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod gate;
mod lifetime;
mod review;
#[cfg(test)]
mod tests;
use gate::RetryGate;
use lifetime::Lifetime;
pub(super) use review::ControllerReviewBounds;

/// Last observed state, not current lease validity or permission to send a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeControllerState {
    Ready,
    CoolingDown,
    Finished,
    Closed,
}

/// Frozen per-object history: at most four explicit attempts in thirty seconds after first use.
/// Conflict outcomes allow another attempt only after five seconds from completion. All errors
/// except CoolingDown close permanently; silence finishes without granting publication authority.
/// Independent controllers and the lower-level probe_once do not share this history.
pub struct MdnsProbeController {
    config: MdnsConfig,
    announcement: MdnsAnnouncement,
    gate: RetryGate,
    origin: Option<Instant>,
    lifetime: Option<Lifetime>,
    interface: Option<VerifiedInterface>,
}

impl MdnsProbeController {
    /// Pure preparation: no clocks, entropy, interface lookup, descriptor or process is created.
    pub fn new(config: MdnsConfig, announcement: MdnsAnnouncement) -> Result<Self> {
        if !(Duration::from_secs(1)..=Duration::from_secs(2)).contains(&config.timeout())
            || !announcement.matches_probe_source(config.local_address())
        {
            return Err(MdnsError::InvalidConfig);
        }
        Ok(Self {
            config,
            announcement,
            gate: RetryGate::new(),
            origin: None,
            lifetime: None,
            interface: None,
        })
    }

    pub fn state(&self) -> ProbeControllerState {
        self.gate.state()
    }

    /// Admitted attempts, including later interface/socket failure; early cooldown does not count.
    pub fn attempts_started(&self) -> u8 {
        self.gate.attempts_started()
    }

    /// Scheduling hint only. Some(ZERO) still requires all checks in a fresh explicit attempt.
    /// No background clock/expiry/cancellation monitoring or state transition is performed here.
    pub fn retry_after(&self) -> Option<Duration> {
        self.gate.remaining(self.origin?.elapsed())
    }

    pub fn attempt(&mut self, cancellation: &AtomicBool) -> Result<ProbeRunObservation> {
        let result = self.attempt_inner(cancellation);
        if matches!(result, Err(error) if error != MdnsError::CoolingDown) {
            self.gate.close();
        }
        result
    }

    /// Retain only a quiet attempt's original sockets and original controller bounds for review.
    /// The borrowed handle prevents another controller call until it is dropped. Quiet finishes
    /// retry history permanently; later review conflicts cannot reopen that history or send.
    pub fn attempt_retained(
        &mut self,
        cancellation: &AtomicBool,
    ) -> Result<RetainedProbeOutcome<'_>> {
        // Split fields before creating an outcome borrowing the frozen announcement. No callback
        // escapes: only concrete owned bounds are attached after all completion checks succeed.
        let Self {
            config,
            announcement,
            gate,
            origin,
            lifetime,
            interface,
        } = self;
        let result = (|| {
            check_cancelled(cancellation)?;
            if matches!(
                gate.state(),
                ProbeControllerState::Finished | ProbeControllerState::Closed
            ) {
                return Err(MdnsError::Closed);
            }
            let elapsed = sample_fields(config, announcement, origin, lifetime)?;
            gate.begin(elapsed)?;
            if let Some(pinned) = interface.as_ref() {
                pinned.verify_unchanged()?;
            } else {
                *interface = Some(VerifiedInterface::inspect(config)?);
            }
            check_cancelled(cancellation)?;
            let mut outcome = {
                let pinned = interface.as_ref().ok_or(MdnsError::Closed)?;
                let started = origin.ok_or(MdnsError::Closed)?;
                let frozen = lifetime.as_mut().ok_or(MdnsError::Closed)?;
                let mut guard = || {
                    let wall = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(|_| MdnsError::ClockInvalid)?;
                    let elapsed = started.elapsed();
                    frozen.check(elapsed, wall)?;
                    gate.check_in_flight(elapsed)?;
                    pinned.verify_unchanged()
                };
                super::probing::probe_retained_guarded(
                    config,
                    announcement,
                    cancellation,
                    &mut guard,
                )?
            };
            interface
                .as_ref()
                .ok_or(MdnsError::Closed)?
                .verify_unchanged()?;
            check_cancelled(cancellation)?;
            let completed = sample_fields(config, announcement, origin, lifetime)?;
            gate.finish(
                completed,
                !matches!(outcome, RetainedProbeOutcome::Quiet(_)),
            )?;
            if let RetainedProbeOutcome::Quiet(handle) = &mut outcome {
                let bounds = ControllerReviewBounds::from_controller(
                    origin.ok_or(MdnsError::Closed)?,
                    lifetime.take().ok_or(MdnsError::Closed)?,
                    completed,
                )?;
                handle.bind_controller(
                    bounds,
                    interface.as_ref().ok_or(MdnsError::Closed)?.clone(),
                )?;
            }
            Ok(outcome)
        })();
        if matches!(result, Err(error) if error != MdnsError::CoolingDown) {
            gate.close();
        }
        result
    }

    fn attempt_inner(&mut self, cancellation: &AtomicBool) -> Result<ProbeRunObservation> {
        check_cancelled(cancellation)?;
        if matches!(
            self.state(),
            ProbeControllerState::Finished | ProbeControllerState::Closed
        ) {
            return Err(MdnsError::Closed);
        }
        let elapsed = self.sample()?;
        // A cooling-down rejection never reaches entropy, interface lookup or socket creation.
        self.gate.begin(elapsed)?;
        if let Some(interface) = &self.interface {
            interface.verify_unchanged()?;
        } else {
            self.interface = Some(VerifiedInterface::inspect(&self.config)?);
        }
        check_cancelled(cancellation)?;
        // The original one-attempt runtime owns/drops all I/O. A result is never retained as a
        // sender capability; publication cannot be safely bridged by closing/reopening these FDs.
        let observation = {
            let interface = self.interface.as_ref().ok_or(MdnsError::Closed)?;
            let origin = self.origin.ok_or(MdnsError::Closed)?;
            let lifetime = self.lifetime.as_mut().ok_or(MdnsError::Closed)?;
            let gate = &mut self.gate;
            let mut guard = || {
                let wall = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| MdnsError::ClockInvalid)?;
                let elapsed = origin.elapsed();
                lifetime.check(elapsed, wall)?;
                gate.check_in_flight(elapsed)?;
                interface.verify_unchanged()
            };
            super::probing::probe_guarded(
                &self.config,
                &self.announcement,
                cancellation,
                &mut guard,
            )?
        };
        self.interface
            .as_ref()
            .ok_or(MdnsError::Closed)?
            .verify_unchanged()?;
        check_cancelled(cancellation)?;
        let completed = self.sample()?;
        self.gate.finish(
            completed,
            observation != ProbeRunObservation::NoConflictObserved,
        )?;
        Ok(observation)
    }

    fn sample(&mut self) -> Result<Duration> {
        sample_fields(
            &self.config,
            &self.announcement,
            &mut self.origin,
            &mut self.lifetime,
        )
    }
}

fn sample_fields(
    config: &MdnsConfig,
    announcement: &MdnsAnnouncement,
    origin: &mut Option<Instant>,
    lifetime: &mut Option<Lifetime>,
) -> Result<Duration> {
    let started = origin.unwrap_or_else(Instant::now);
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| MdnsError::ClockInvalid)?;
    let remaining = announcement.probe_remaining(config.local_address(), wall.as_secs())?;
    if lifetime.is_none() {
        *lifetime = Some(Lifetime::new(wall, remaining)?);
        *origin = Some(started);
    }
    let elapsed = started.elapsed();
    lifetime
        .as_mut()
        .ok_or(MdnsError::Closed)?
        .check(elapsed, wall)?;
    Ok(elapsed)
}

fn check_cancelled(cancellation: &AtomicBool) -> Result<()> {
    if cancellation.load(Ordering::Acquire) {
        Err(MdnsError::Cancelled)
    } else {
        Ok(())
    }
}
