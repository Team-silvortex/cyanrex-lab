//! Linear, unannounced receive-only continuation; not an enduring uniqueness or sending permit.
use super::{
    cancelled, Limits, MdnsAnnouncement, MdnsError, ProbeIo, ProbeRunObservation, Result, Run,
    VerifiedInterface,
};
use crate::services::federation_mdns::controller::ControllerReviewBounds;
use std::{sync::atomic::AtomicBool, time::Instant};

mod window;
use window::ReviewWindow;
mod bounds;
use bounds::ReviewBounds;

/// Only a quiet live attempt can supply a context. Conflicts never contain a sender or descriptor.
pub enum RetainedProbeOutcome<'a> {
    Quiet(MdnsRetainedProbe<'a>),
    ExistingResponse,
    CompetingProbe,
}

impl RetainedProbeOutcome<'_> {
    pub(super) fn observation(&self) -> ProbeRunObservation {
        match self {
            Self::Quiet(_) => ProbeRunObservation::NoConflictObserved,
            Self::ExistingResponse => ProbeRunObservation::ExistingResponse,
            Self::CompetingProbe => ProbeRunObservation::CompetingProbe,
        }
    }
}

pub(super) fn conflict(observation: ProbeRunObservation) -> Result<RetainedProbeOutcome<'static>> {
    match observation {
        ProbeRunObservation::ExistingResponse => Ok(RetainedProbeOutcome::ExistingResponse),
        ProbeRunObservation::CompetingProbe => Ok(RetainedProbeOutcome::CompetingProbe),
        // Even an invalid internal quiet conversion cannot create a handle or a permit.
        ProbeRunObservation::NoConflictObserved => Err(MdnsError::Closed),
    }
}

/// Holds the same two probed sockets and fixed bounds. No public constructor, Clone, FD export,
/// reconfiguration or send operation exists. Idle handles are not monitored: close/Drop releases
/// descriptors; every active review checks the original timeout/lease plus a fixed 250ms window.
pub struct MdnsRetainedProbe<'a> {
    live: Option<Live<'a>>,
}

struct Live<'a> {
    started: Instant,
    announcement: &'a MdnsAnnouncement,
    interface: VerifiedInterface,
    io: ProbeIo,
    bounds: ReviewBounds,
    controller: Option<ControllerReviewBounds>,
}

impl<'a> MdnsRetainedProbe<'a> {
    pub(super) fn from_run(
        run: Run<'a, '_, '_>,
        io: ProbeIo,
        quiet_at: std::time::Duration,
    ) -> Result<Self> {
        Ok(Self {
            live: Some(Live {
                started: run.started,
                announcement: run.announcement,
                interface: run.interface.clone(),
                io,
                bounds: ReviewBounds::from_probe(run.limits, quiet_at)?,
                controller: None,
            }),
        })
    }

    /// Only a completed quiet controller attempt can add its owned original bounds and first pin.
    pub(in crate::services::federation_mdns) fn bind_controller(
        &mut self,
        bounds: ControllerReviewBounds,
        interface: VerifiedInterface,
    ) -> Result<()> {
        let mut live = self.live.take().ok_or(MdnsError::Closed)?;
        if live.controller.is_some() {
            return Err(MdnsError::Closed);
        }
        live.controller = Some(bounds);
        live.interface = interface;
        self.live = Some(live);
        Ok(())
    }

    /// Drain the original two queues without waiting or sending. Quiet remains historical evidence,
    /// never permission to publish. Conflict/error/cancellation closes and drops both sockets.
    /// Repeated review cannot renew the window, original lease, timeout or packet allowance.
    pub fn review(&mut self, cancellation: &AtomicBool) -> Result<ProbeRunObservation> {
        // Take ownership first: every early return drops both descriptors and leaves self closed.
        let mut live = self.live.take().ok_or(MdnsError::Closed)?;
        cancelled(cancellation)?;
        let outcome = {
            let started = live.started;
            let mut guard = || {
                live.bounds.window.check(started.elapsed())?;
                if let Some(controller) = &mut live.controller {
                    controller.check()?;
                }
                Ok(())
            };
            let mut run = Run {
                started,
                limits: live.bounds.limits,
                announcement: live.announcement,
                interface: &live.interface,
                cancelled: cancellation,
                guard: &mut guard,
            };
            // Every review remains post-send, including packets queued after quiet completion.
            let outcome = run.drain(&mut live.io, 3).and_then(|value| {
                // An empty nonblocking poll still needs a fresh final cancellation/time check.
                run.check()?;
                Ok(value)
            });
            live.bounds.limits = run.limits;
            outcome
        };
        let observation = outcome?.unwrap_or(ProbeRunObservation::NoConflictObserved);
        if observation == ProbeRunObservation::NoConflictObserved {
            self.live = Some(live);
        }
        Ok(observation)
    }

    /// Idempotent local release only. Nothing was announced, so no goodbye is sent.
    pub fn close(&mut self) {
        self.live = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_observation_can_never_manufacture_a_quiet_live_context() {
        assert!(matches!(
            conflict(ProbeRunObservation::NoConflictObserved),
            Err(MdnsError::Closed)
        ));
    }

    #[test]
    fn conflict_outcomes_have_no_context_and_keep_the_original_classification() {
        for observation in [
            ProbeRunObservation::ExistingResponse,
            ProbeRunObservation::CompetingProbe,
        ] {
            let result = conflict(observation).unwrap();
            assert_eq!(result.observation(), observation);
            assert!(!matches!(result, RetainedProbeOutcome::Quiet(_)));
        }
    }

    #[test]
    fn closing_an_already_closed_handle_is_idempotent_and_cannot_revive_it() {
        let mut handle = MdnsRetainedProbe { live: None };
        for cancelled in [false, true, false] {
            handle.close();
            assert_eq!(
                handle.review(&AtomicBool::new(cancelled)),
                Err(MdnsError::Closed)
            );
            assert!(handle.live.is_none());
        }
    }
}
