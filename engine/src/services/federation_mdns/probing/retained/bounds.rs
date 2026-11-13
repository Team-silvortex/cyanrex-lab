use super::{Limits, MdnsError, ReviewWindow};
use std::time::Duration;

/// Transfer the original attempt's accounting without renewing any existing bound.
pub(super) struct ReviewBounds {
    pub(super) limits: Limits,
    pub(super) window: ReviewWindow,
}

impl ReviewBounds {
    pub(super) fn from_probe(limits: Limits, quiet_at: Duration) -> Result<Self, MdnsError> {
        Ok(Self {
            limits,
            window: ReviewWindow::new(quiet_at)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_limits() -> Limits {
        Limits::new(Duration::from_secs(2), Duration::from_secs(100), 10).unwrap()
    }

    #[test]
    fn handoff_preserves_exhausted_same_second_packet_budget() {
        let quiet = Duration::from_millis(100);
        let mut original = probe_limits();
        for _ in 0..64 {
            original.admit(quiet).unwrap();
        }
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        assert_eq!(bounds.window.check(quiet), Ok(()));
        assert_eq!(bounds.limits.admit(quiet), Err(MdnsError::BudgetExceeded));
        assert_eq!(bounds.limits.admit(quiet), Err(MdnsError::Closed));
    }

    #[test]
    fn handoff_preserves_partial_budget_and_original_second_boundary() {
        let quiet = Duration::from_millis(999);
        let mut original = probe_limits();
        for _ in 0..63 {
            original.admit(quiet).unwrap();
        }
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        assert_eq!(bounds.limits.admit(quiet), Ok(()));
        let next_second = Duration::from_secs(1);
        assert_eq!(bounds.window.check(next_second), Ok(()));
        for _ in 0..64 {
            bounds.limits.admit(next_second).unwrap();
        }
        assert_eq!(
            bounds.limits.admit(next_second),
            Err(MdnsError::BudgetExceeded)
        );
    }

    #[test]
    fn handoff_preserves_total_packet_budget_across_second_boundaries() {
        let mut original = probe_limits();
        // Admission accounts packets independently of the caller's separate deadline checks.
        for second in 0..4 {
            for _ in 0..64 {
                original.admit(Duration::from_secs(second)).unwrap();
            }
        }
        let quiet = Duration::from_millis(3_999);
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        let next_second = Duration::from_secs(4);
        assert_eq!(bounds.window.check(next_second), Ok(()));
        assert_eq!(
            bounds.limits.admit(next_second),
            Err(MdnsError::BudgetExceeded)
        );
    }

    #[test]
    fn original_two_second_deadline_can_expire_before_review_window() {
        let quiet = Duration::from_millis(1_900);
        let mut original = probe_limits();
        original.check(quiet, Duration::from_secs(101)).unwrap();
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        let deadline = Duration::from_secs(2);
        assert_eq!(bounds.window.check(deadline), Ok(()));
        assert_eq!(
            bounds
                .limits
                .check(deadline - Duration::from_nanos(1), Duration::from_secs(101)),
            Ok(())
        );
        assert_eq!(
            bounds.limits.check(deadline, Duration::from_secs(102)),
            Err(MdnsError::DeadlineExceeded)
        );
    }

    #[test]
    fn handoff_preserves_a_shorter_configured_timeout() {
        let quiet = Duration::from_millis(900);
        let mut original =
            Limits::new(Duration::from_secs(1), Duration::from_secs(100), 10).unwrap();
        original.check(quiet, Duration::from_secs(100)).unwrap();
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        assert_eq!(bounds.window.check(Duration::from_secs(1)), Ok(()));
        assert_eq!(
            bounds
                .limits
                .check(Duration::from_secs(1), Duration::from_secs(101)),
            Err(MdnsError::DeadlineExceeded)
        );
    }

    #[test]
    fn handoff_preserves_full_precision_original_lease() {
        let initial_wall = Duration::from_millis(100_900);
        let quiet = Duration::from_millis(50);
        let mut original = Limits::new(Duration::from_secs(2), initial_wall, 1).unwrap();
        original.check(quiet, initial_wall).unwrap();
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        let expiry = Duration::from_millis(100);
        assert_eq!(bounds.window.check(expiry), Ok(()));
        assert_eq!(
            bounds
                .limits
                .check(expiry - Duration::from_nanos(1), initial_wall),
            Ok(())
        );
        assert_eq!(
            bounds.limits.check(expiry, initial_wall),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn handoff_preserves_original_absolute_wall_lease_expiry() {
        let initial_wall = Duration::from_millis(100_900);
        let quiet = Duration::from_millis(50);
        let mut original = Limits::new(Duration::from_secs(2), initial_wall, 1).unwrap();
        original.check(quiet, initial_wall).unwrap();
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        assert_eq!(bounds.window.check(quiet), Ok(()));
        assert_eq!(
            bounds.limits.check(quiet, Duration::from_secs(101)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn handoff_preserves_wall_clock_watermark() {
        let quiet = Duration::from_millis(100);
        let mut original = probe_limits();
        original.check(quiet, Duration::from_secs(101)).unwrap();
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        assert_eq!(
            bounds
                .limits
                .check(quiet, Duration::from_secs(101) - Duration::from_nanos(1)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn handoff_preserves_elapsed_watermark_from_clock_check() {
        let quiet = Duration::from_millis(100);
        let mut original = probe_limits();
        original.check(quiet, Duration::from_secs(100)).unwrap();
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        assert_eq!(
            bounds
                .limits
                .check(quiet - Duration::from_nanos(1), Duration::from_secs(100)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn handoff_preserves_elapsed_watermark_from_packet_admission() {
        let quiet = Duration::from_millis(100);
        let mut original = probe_limits();
        original.admit(quiet).unwrap();
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        assert_eq!(
            bounds.limits.admit(quiet - Duration::from_nanos(1)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn handoff_preserves_elapsed_watermark_from_wait_check() {
        let quiet = Duration::from_millis(100);
        let mut original = probe_limits();
        original.wait_bound(Duration::ZERO, quiet).unwrap();
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        assert_eq!(
            bounds.limits.admit(quiet - Duration::from_nanos(1)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn handed_off_wait_remains_capped_by_original_timeout() {
        let quiet = Duration::from_millis(1_900);
        let mut original = probe_limits();
        original.check(quiet, Duration::from_secs(101)).unwrap();
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        assert_eq!(
            bounds.limits.wait_bound(Duration::from_millis(250), quiet),
            Ok(Duration::from_millis(100))
        );
    }

    #[test]
    fn handed_off_wait_remains_capped_by_original_subsecond_lease() {
        let quiet = Duration::from_millis(50);
        let mut original =
            Limits::new(Duration::from_secs(2), Duration::from_millis(100_900), 1).unwrap();
        original
            .check(quiet, Duration::from_millis(100_950))
            .unwrap();
        let mut bounds = ReviewBounds::from_probe(original, quiet).unwrap();
        assert_eq!(
            bounds.limits.wait_bound(Duration::from_millis(250), quiet),
            Ok(Duration::from_millis(50))
        );
    }

    #[test]
    fn handoff_cannot_revive_closed_original_limits() {
        let mut original = probe_limits();
        assert_eq!(
            original.check(Duration::ZERO, Duration::from_secs(99)),
            Err(MdnsError::ClockInvalid)
        );
        let mut bounds = ReviewBounds::from_probe(original, Duration::ZERO).unwrap();
        assert_eq!(
            bounds
                .limits
                .check(Duration::ZERO, Duration::from_secs(100)),
            Err(MdnsError::Closed)
        );
        assert_eq!(bounds.limits.admit(Duration::ZERO), Err(MdnsError::Closed));
    }

    #[test]
    fn handoff_creates_one_fixed_post_quiet_window() {
        let quiet = Duration::from_millis(100);
        let mut bounds = ReviewBounds::from_probe(probe_limits(), quiet).unwrap();
        assert_eq!(bounds.window.check(quiet), Ok(()));
        assert_eq!(bounds.window.check(Duration::from_millis(349)), Ok(()));
        assert_eq!(
            bounds.window.check(Duration::from_millis(350)),
            Err(MdnsError::DeadlineExceeded)
        );
        assert_eq!(
            bounds.window.check(Duration::from_millis(350)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn handoff_rejects_post_quiet_deadline_overflow() {
        assert!(matches!(
            ReviewBounds::from_probe(probe_limits(), Duration::MAX),
            Err(MdnsError::ClockInvalid)
        ));
    }
}
