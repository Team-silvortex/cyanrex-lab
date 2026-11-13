use super::super::MdnsError;
use std::time::Duration;

/// Frozen advertisement lifetime, checked against caller-supplied clock samples.
/// Neither successful checks nor a stopped wall clock can renew this lease.
#[derive(Debug)]
pub(super) struct Lifetime {
    expiry: Duration,
    lease: Duration,
    last_wall: Duration,
    last_elapsed: Duration,
    closed: bool,
}

impl Lifetime {
    pub(super) fn new(initial_wall: Duration, remaining_seconds: u64) -> Result<Self, MdnsError> {
        if remaining_seconds == 0 {
            return Err(MdnsError::InvalidConfig);
        }
        let expiry_seconds = initial_wall
            .as_secs()
            .checked_add(remaining_seconds)
            .ok_or(MdnsError::ClockInvalid)?;
        let expiry = Duration::from_secs(expiry_seconds);
        let lease = expiry
            .checked_sub(initial_wall)
            .ok_or(MdnsError::ClockInvalid)?;
        Ok(Self {
            expiry,
            lease,
            last_wall: initial_wall,
            last_elapsed: Duration::ZERO,
            closed: false,
        })
    }

    pub(super) fn check(&mut self, elapsed: Duration, wall: Duration) -> Result<(), MdnsError> {
        if self.closed {
            return Err(MdnsError::Closed);
        }
        if elapsed < self.last_elapsed
            || wall < self.last_wall
            || elapsed >= self.lease
            || wall >= self.expiry
        {
            self.closed = true;
            return Err(MdnsError::ClockInvalid);
        }
        self.last_elapsed = elapsed;
        self.last_wall = wall;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    fn milliseconds(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    fn lifetime(initial_wall: Duration, remaining: u64) -> Lifetime {
        Lifetime::new(initial_wall, remaining).expect("valid frozen lifetime")
    }

    #[test]
    fn zero_remaining_is_invalid_configuration() {
        assert!(matches!(
            Lifetime::new(seconds(10), 0),
            Err(MdnsError::InvalidConfig)
        ));
    }

    #[test]
    fn expiry_seconds_are_checked_before_constructing_duration() {
        for initial in [seconds(u64::MAX), Duration::new(u64::MAX, 999_999_999)] {
            assert!(matches!(
                Lifetime::new(initial, 1),
                Err(MdnsError::ClockInvalid)
            ));
        }
        assert!(matches!(
            Lifetime::new(seconds(1), u64::MAX),
            Err(MdnsError::ClockInvalid)
        ));
    }

    #[test]
    fn maximum_representable_expiry_is_not_arbitrarily_capped() {
        let mut lease = lifetime(seconds(0), u64::MAX);
        assert_eq!(lease.check(seconds(30), seconds(30)), Ok(()));
        assert_eq!(
            lease.check(seconds(u64::MAX), seconds(u64::MAX)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn equal_initial_watermarks_are_valid() {
        let initial = Duration::new(10, 123_456_789);
        let mut lease = lifetime(initial, 1);
        assert_eq!(lease.check(Duration::ZERO, initial), Ok(()));
        assert_eq!(lease.check(Duration::ZERO, initial), Ok(()));
    }

    #[test]
    fn fractional_initial_wall_keeps_exact_whole_second_expiry() {
        let initial = Duration::new(10, 900_000_000);
        let mut lease = lifetime(initial, 1);
        assert_eq!(
            lease.check(Duration::new(0, 99_999_999), Duration::new(10, 999_999_999)),
            Ok(())
        );
        assert_eq!(
            lease.check(milliseconds(100), seconds(11)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn wall_jump_to_or_past_expiry_closes_immediately() {
        for wall in [seconds(12), seconds(40)] {
            let mut lease = lifetime(seconds(10), 2);
            assert_eq!(
                lease.check(milliseconds(1), wall),
                Err(MdnsError::ClockInvalid)
            );
            assert_eq!(lease.check(milliseconds(2), wall), Err(MdnsError::Closed));
        }
    }

    #[test]
    fn stopped_wall_cannot_outlive_frozen_monotonic_lease() {
        let initial = Duration::new(10, 750_000_000);
        let mut lease = lifetime(initial, 2);
        assert_eq!(lease.check(milliseconds(1249), initial), Ok(()));
        assert_eq!(
            lease.check(milliseconds(1250), initial),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(
            lease.check(milliseconds(1251), initial),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn elapsed_rollback_closes_even_when_wall_moves_forward() {
        let mut lease = lifetime(seconds(10), 10);
        assert_eq!(lease.check(seconds(2), seconds(12)), Ok(()));
        assert_eq!(
            lease.check(seconds(1), seconds(13)),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(lease.check(seconds(3), seconds(14)), Err(MdnsError::Closed));
    }

    #[test]
    fn wall_rollback_closes_even_when_elapsed_moves_forward() {
        let mut lease = lifetime(seconds(10), 10);
        assert_eq!(lease.check(seconds(2), seconds(12)), Ok(()));
        assert_eq!(
            lease.check(seconds(3), seconds(11)),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(lease.check(seconds(4), seconds(13)), Err(MdnsError::Closed));
    }

    #[test]
    fn wall_rollback_before_first_check_is_rejected() {
        let mut lease = lifetime(Duration::new(10, 1), 1);
        assert_eq!(
            lease.check(Duration::ZERO, seconds(10)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn repeated_checks_do_not_renew_initial_expiry_or_elapsed_lease() {
        let mut lease = lifetime(milliseconds(10500), 4);
        assert_eq!(lease.check(seconds(1), milliseconds(11500)), Ok(()));
        assert_eq!(lease.check(seconds(1), milliseconds(11500)), Ok(()));
        assert_eq!(lease.check(seconds(3), milliseconds(13500)), Ok(()));
        assert_eq!(
            lease.check(milliseconds(3500), milliseconds(13500)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn wall_expiry_is_not_moved_by_successful_elapsed_checks() {
        let mut lease = lifetime(milliseconds(10500), 4);
        assert_eq!(lease.check(seconds(1), seconds(13)), Ok(()));
        assert_eq!(
            lease.check(seconds(2), seconds(14)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn near_maximum_fractional_wall_preserves_nanosecond_lease() {
        let initial = Duration::new(u64::MAX - 1, 999_999_999);
        let mut lease = lifetime(initial, 1);
        assert_eq!(lease.check(Duration::ZERO, initial), Ok(()));
        assert_eq!(
            lease.check(Duration::from_nanos(1), initial),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn closure_is_permanent_even_if_clocks_return_inside_valid_window() {
        let initial = seconds(10);
        let mut lease = lifetime(initial, 2);
        assert_eq!(
            lease.check(seconds(2), initial),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(lease.check(Duration::ZERO, initial), Err(MdnsError::Closed));
    }
}
