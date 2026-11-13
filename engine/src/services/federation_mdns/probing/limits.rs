use super::super::{MdnsError, MAX_PACKETS_PER_RUN, MAX_PACKETS_PER_SECOND};
use std::time::Duration;

type Result<T> = std::result::Result<T, MdnsError>;

/// Local per-attempt bounds, not DNS ownership, a lease renewal or a retry policy.
#[derive(Debug)]
pub(super) struct Limits {
    timeout: Duration,
    expires_wall: Duration,
    lease: Duration,
    elapsed: Duration,
    wall: Duration,
    window: u64,
    in_window: usize,
    total: usize,
    closed: bool,
}

impl Limits {
    pub(super) fn new(
        timeout: Duration,
        initial_wall: Duration,
        remaining_seconds: u64,
    ) -> Result<Self> {
        if !(Duration::from_secs(1)..=Duration::from_secs(2)).contains(&timeout)
            || remaining_seconds == 0
        {
            return Err(MdnsError::InvalidConfig);
        }
        let expires_wall = Duration::from_secs(
            initial_wall
                .as_secs()
                .checked_add(remaining_seconds)
                .ok_or(MdnsError::ClockInvalid)?,
        );
        let lease = expires_wall
            .checked_sub(initial_wall)
            .ok_or(MdnsError::ClockInvalid)?;
        Ok(Self {
            timeout,
            expires_wall,
            lease,
            elapsed: Duration::ZERO,
            wall: initial_wall,
            window: 0,
            in_window: 0,
            total: 0,
            closed: false,
        })
    }

    pub(super) fn check(&mut self, elapsed: Duration, wall: Duration) -> Result<()> {
        self.check_elapsed(elapsed)?;
        if wall < self.wall {
            return self.fail(MdnsError::ClockInvalid);
        }
        self.wall = wall;
        if elapsed >= self.timeout {
            return self.fail(MdnsError::DeadlineExceeded);
        }
        if elapsed >= self.lease || wall >= self.expires_wall {
            return self.fail(MdnsError::ClockInvalid);
        }
        Ok(())
    }

    /// The caller freshly checks wall time before bounding a blocking wait.
    pub(super) fn wait_bound(
        &mut self,
        requested: Duration,
        elapsed: Duration,
    ) -> Result<Duration> {
        self.check_elapsed(elapsed)?;
        if elapsed >= self.timeout {
            return self.fail(MdnsError::DeadlineExceeded);
        }
        if elapsed >= self.lease {
            return self.fail(MdnsError::ClockInvalid);
        }
        Ok(requested
            .min(self.timeout - elapsed)
            .min(self.lease - elapsed))
    }

    /// Charge every received datagram before parsing; the caller checks time separately.
    pub(super) fn admit(&mut self, elapsed: Duration) -> Result<()> {
        self.check_elapsed(elapsed)?;
        let window = elapsed.as_secs();
        if window != self.window {
            self.window = window;
            self.in_window = 0;
        }
        if self.total == MAX_PACKETS_PER_RUN || self.in_window == MAX_PACKETS_PER_SECOND {
            return self.fail(MdnsError::BudgetExceeded);
        }
        self.total += 1;
        self.in_window += 1;
        Ok(())
    }

    fn check_elapsed(&mut self, elapsed: Duration) -> Result<()> {
        if self.closed {
            return Err(MdnsError::Closed);
        }
        if elapsed < self.elapsed {
            return self.fail(MdnsError::ClockInvalid);
        }
        self.elapsed = elapsed;
        Ok(())
    }

    fn fail<T>(&mut self, error: MdnsError) -> Result<T> {
        self.closed = true;
        Err(error)
    }
}

/// Rejection sampling keeps all 251 millisecond buckets equally likely.
pub(super) fn jitter_value(sample: u16) -> Option<u16> {
    (sample < 65_511).then_some(sample % 251)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn limits() -> Limits {
        Limits::new(Duration::from_secs(2), Duration::from_secs(100), 10).unwrap()
    }

    #[test]
    fn accepts_timeout_bounds_and_live_lease() {
        for timeout in [Duration::from_secs(1), Duration::from_secs(2)] {
            assert!(Limits::new(timeout, Duration::from_secs(100), 1).is_ok());
        }
    }

    #[test]
    fn rejects_timeout_below_one_second() {
        for timeout in [Duration::ZERO, Duration::from_nanos(999_999_999)] {
            assert!(matches!(
                Limits::new(timeout, Duration::from_secs(100), 10),
                Err(MdnsError::InvalidConfig)
            ));
        }
    }

    #[test]
    fn rejects_timeout_above_two_seconds() {
        assert!(matches!(
            Limits::new(
                Duration::from_secs(2) + Duration::from_nanos(1),
                Duration::from_secs(100),
                10
            ),
            Err(MdnsError::InvalidConfig)
        ));
    }

    #[test]
    fn rejects_non_live_lease() {
        assert!(matches!(
            Limits::new(Duration::from_secs(2), Duration::from_secs(100), 0),
            Err(MdnsError::InvalidConfig)
        ));
    }

    #[test]
    fn rejects_absolute_expiry_overflow() {
        assert!(matches!(
            Limits::new(Duration::from_secs(2), Duration::from_secs(u64::MAX), 1),
            Err(MdnsError::ClockInvalid)
        ));
    }

    #[test]
    fn subsecond_initial_wall_does_not_extend_lease() {
        let mut bound =
            Limits::new(Duration::from_secs(2), Duration::from_millis(100_999), 1).unwrap();
        assert_eq!(
            bound.check(Duration::from_micros(999), Duration::from_millis(100_999)),
            Ok(())
        );
        assert_eq!(
            bound.check(Duration::from_millis(1), Duration::from_millis(100_999)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn exact_lease_deadline_is_invalid() {
        let mut bound = Limits::new(Duration::from_secs(2), Duration::from_secs(100), 1).unwrap();
        assert_eq!(
            bound.check(Duration::from_secs(1), Duration::from_secs(100)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn wall_clock_forward_jump_cannot_keep_expired_lease_live() {
        assert_eq!(
            limits().check(Duration::ZERO, Duration::from_secs(110)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn exact_timeout_expires() {
        assert_eq!(
            limits().check(Duration::from_secs(2), Duration::from_secs(100)),
            Err(MdnsError::DeadlineExceeded)
        );
    }

    #[test]
    fn just_before_timeout_remains_live() {
        assert_eq!(
            limits().check(
                Duration::from_secs(2) - Duration::from_nanos(1),
                Duration::from_secs(100)
            ),
            Ok(())
        );
    }

    #[test]
    fn elapsed_watermark_cannot_move_backwards_in_check() {
        let mut bound = limits();
        bound
            .check(Duration::from_millis(10), Duration::from_secs(100))
            .unwrap();
        assert_eq!(
            bound.check(Duration::from_millis(9), Duration::from_secs(100)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn wall_watermark_cannot_move_backwards() {
        let mut bound = limits();
        bound
            .check(Duration::ZERO, Duration::from_secs(101))
            .unwrap();
        assert_eq!(
            bound.check(Duration::ZERO, Duration::from_secs(100)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn initial_wall_is_already_a_watermark() {
        assert_eq!(
            limits().check(Duration::ZERO, Duration::from_secs(99)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn check_and_admit_share_elapsed_watermark() {
        let mut bound = limits();
        bound.admit(Duration::from_millis(10)).unwrap();
        assert_eq!(
            bound.check(Duration::from_millis(9), Duration::from_secs(100)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn admit_checks_elapsed_watermark_from_check() {
        let mut bound = limits();
        bound
            .check(Duration::from_millis(10), Duration::from_secs(100))
            .unwrap();
        assert_eq!(
            bound.admit(Duration::from_millis(9)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn same_sample_may_be_checked_and_admitted_repeatedly() {
        let mut bound = limits();
        for _ in 0..64 {
            bound
                .check(Duration::from_millis(10), Duration::from_secs(100))
                .unwrap();
            bound.admit(Duration::from_millis(10)).unwrap();
        }
        assert_eq!(
            bound.check(Duration::from_millis(10), Duration::from_secs(100)),
            Ok(())
        );
    }

    #[test]
    fn sixty_fifth_packet_within_integer_second_exhausts_budget() {
        let mut bound = limits();
        for _ in 0..64 {
            bound.admit(Duration::ZERO).unwrap();
        }
        assert_eq!(
            bound.admit(Duration::from_nanos(999_999_999)),
            Err(MdnsError::BudgetExceeded)
        );
    }

    #[test]
    fn quota_window_resets_at_integer_second() {
        let mut bound = limits();
        for _ in 0..64 {
            bound.admit(Duration::from_nanos(999_999_999)).unwrap();
        }
        assert_eq!(bound.admit(Duration::from_secs(1)), Ok(()));
    }

    #[test]
    fn total_packet_budget_is_independent_of_window_resets() {
        let mut bound = limits();
        for second in 0..4 {
            for _ in 0..64 {
                bound.admit(Duration::from_secs(second)).unwrap();
            }
        }
        assert_eq!(
            bound.admit(Duration::from_secs(4)),
            Err(MdnsError::BudgetExceeded)
        );
    }

    #[test]
    fn admission_does_not_replace_explicit_runtime_deadline_check() {
        assert_eq!(limits().admit(Duration::from_secs(20)), Ok(()));
    }

    #[test]
    fn quota_failure_permanently_closes_both_methods() {
        let mut bound = limits();
        for _ in 0..64 {
            bound.admit(Duration::ZERO).unwrap();
        }
        assert_eq!(bound.admit(Duration::ZERO), Err(MdnsError::BudgetExceeded));
        assert_eq!(bound.admit(Duration::from_secs(1)), Err(MdnsError::Closed));
        assert_eq!(
            bound.check(Duration::ZERO, Duration::from_secs(100)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn clock_failure_permanently_closes_both_methods() {
        let mut bound = limits();
        assert_eq!(
            bound.check(Duration::ZERO, Duration::from_secs(99)),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(bound.admit(Duration::ZERO), Err(MdnsError::Closed));
        assert_eq!(
            bound.check(Duration::ZERO, Duration::from_secs(100)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn deadline_failure_permanently_closes_both_methods() {
        let mut bound = limits();
        assert_eq!(
            bound.check(Duration::from_secs(2), Duration::from_secs(100)),
            Err(MdnsError::DeadlineExceeded)
        );
        assert_eq!(bound.admit(Duration::from_secs(2)), Err(MdnsError::Closed));
        assert_eq!(
            bound.check(Duration::from_secs(2), Duration::from_secs(100)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn wait_keeps_shorter_requested_duration() {
        assert_eq!(
            limits().wait_bound(Duration::from_millis(300), Duration::from_millis(10)),
            Ok(Duration::from_millis(300))
        );
    }

    #[test]
    fn wait_is_capped_at_remaining_deadline_without_polling_cap() {
        assert_eq!(
            limits().wait_bound(Duration::from_secs(5), Duration::from_millis(1_100)),
            Ok(Duration::from_millis(900))
        );
    }

    #[test]
    fn wait_is_capped_at_remaining_subsecond_lease() {
        let mut bound =
            Limits::new(Duration::from_secs(2), Duration::from_millis(100_750), 1).unwrap();
        assert_eq!(
            bound.wait_bound(Duration::from_secs(5), Duration::from_millis(100)),
            Ok(Duration::from_millis(150))
        );
    }

    #[test]
    fn wait_at_exact_deadline_closes_permanently() {
        let mut bound = limits();
        assert_eq!(
            bound.wait_bound(Duration::ZERO, Duration::from_secs(2)),
            Err(MdnsError::DeadlineExceeded)
        );
        assert_eq!(
            bound.wait_bound(Duration::ZERO, Duration::from_secs(2)),
            Err(MdnsError::Closed)
        );
        assert_eq!(bound.admit(Duration::from_secs(2)), Err(MdnsError::Closed));
    }

    #[test]
    fn wait_at_exact_lease_expiry_closes_permanently() {
        let mut bound =
            Limits::new(Duration::from_secs(2), Duration::from_millis(100_750), 1).unwrap();
        assert_eq!(
            bound.wait_bound(Duration::ZERO, Duration::from_millis(250)),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(
            bound.wait_bound(Duration::ZERO, Duration::from_millis(250)),
            Err(MdnsError::Closed)
        );
        assert_eq!(
            bound.check(Duration::from_millis(250), Duration::from_millis(100_750)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn wait_rejects_elapsed_regression_and_closes() {
        let mut bound = limits();
        bound.admit(Duration::from_millis(10)).unwrap();
        assert_eq!(
            bound.wait_bound(Duration::ZERO, Duration::from_millis(9)),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(
            bound.wait_bound(Duration::ZERO, Duration::from_millis(10)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn wait_updates_shared_elapsed_watermark() {
        let mut bound = limits();
        bound
            .wait_bound(Duration::from_millis(10), Duration::from_millis(20))
            .unwrap();
        assert_eq!(
            bound.admit(Duration::from_millis(19)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn zero_requested_wait_is_legal_while_live() {
        let mut bound = limits();
        for _ in 0..2 {
            assert_eq!(
                bound.wait_bound(Duration::ZERO, Duration::ZERO),
                Ok(Duration::ZERO)
            );
        }
    }

    #[test]
    fn jitter_accepts_exactly_equal_sized_buckets_and_rejects_tail() {
        let mut buckets = [0_u16; 251];
        let mut rejected = 0;
        for sample in 0..=u16::MAX {
            match jitter_value(sample) {
                Some(value) => {
                    assert!(sample < 65_511);
                    buckets[usize::from(value)] += 1;
                }
                None => {
                    assert!(sample >= 65_511);
                    rejected += 1;
                }
            }
        }
        assert_eq!(buckets, [261; 251]);
        assert_eq!(rejected, 25);
    }
}
