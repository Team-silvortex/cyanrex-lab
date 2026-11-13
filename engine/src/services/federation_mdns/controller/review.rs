//! Concrete continuation of original controller clocks, never a fresh lease or sending permit.
use super::{gate::MAX_LIFETIME, lifetime::Lifetime};
use crate::services::federation_mdns::{MdnsError, Result};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Only the controller's completed quiet path supplies these owned, original clock bounds.
pub(in crate::services::federation_mdns) struct ControllerReviewBounds {
    origin: Instant,
    lifetime: Lifetime,
    watermark: Duration,
    closed: bool,
}

impl ControllerReviewBounds {
    pub(super) fn from_controller(
        origin: Instant,
        lifetime: Lifetime,
        completed: Duration,
    ) -> Result<Self> {
        if completed >= MAX_LIFETIME {
            return Err(MdnsError::DeadlineExceeded);
        }
        Ok(Self {
            origin,
            lifetime,
            watermark: completed,
            closed: false,
        })
    }

    pub(in crate::services::federation_mdns) fn check(&mut self) -> Result<()> {
        if self.closed {
            return Err(MdnsError::Closed);
        }
        let elapsed = self.origin.elapsed();
        let wall = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(wall) => wall,
            Err(_) => return self.fail(MdnsError::ClockInvalid),
        };
        self.check_at(elapsed, wall)
    }

    fn check_at(&mut self, elapsed: Duration, wall: Duration) -> Result<()> {
        if self.closed {
            return Err(MdnsError::Closed);
        }
        if elapsed < self.watermark {
            return self.fail(MdnsError::ClockInvalid);
        }
        // Preserve F1h's error order: the original F0 lifetime precedes the controller deadline.
        if let Err(error) = self.lifetime.check(elapsed, wall) {
            return self.fail(error);
        }
        if elapsed >= MAX_LIFETIME {
            return self.fail(MdnsError::DeadlineExceeded);
        }
        self.watermark = elapsed;
        Ok(())
    }

    fn fail<T>(&mut self, error: MdnsError) -> Result<T> {
        self.closed = true;
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::super::lifetime::Lifetime;
    use super::*;
    use crate::services::federation_mdns::MdnsError;
    use std::time::{Duration, Instant};

    fn bounds(
        initial_wall: Duration,
        remaining: u64,
        completed: Duration,
        final_wall: Duration,
    ) -> ControllerReviewBounds {
        let mut lifetime = Lifetime::new(initial_wall, remaining).unwrap();
        lifetime.check(completed, final_wall).unwrap();
        ControllerReviewBounds::from_controller(Instant::now(), lifetime, completed).unwrap()
    }

    fn long_lived(completed: Duration) -> ControllerReviewBounds {
        bounds(
            Duration::from_secs(100),
            60,
            completed,
            Duration::from_secs(129),
        )
    }

    #[test]
    fn continuation_keeps_the_original_monotonic_origin() {
        let origin = Instant::now();
        let lifetime = Lifetime::new(Duration::from_secs(100), 60).unwrap();
        let bound =
            ControllerReviewBounds::from_controller(origin, lifetime, Duration::ZERO).unwrap();
        assert_eq!(bound.origin, origin);
    }

    #[test]
    fn original_thirty_second_deadline_is_exact_and_terminal() {
        let mut bound = long_lived(Duration::from_millis(29_900));
        assert_eq!(
            bound.check_at(
                Duration::from_secs(30) - Duration::from_nanos(1),
                Duration::from_secs(129)
            ),
            Ok(())
        );
        assert_eq!(
            bound.check_at(Duration::from_secs(30), Duration::from_secs(129)),
            Err(MdnsError::DeadlineExceeded)
        );
        assert_eq!(
            bound.check_at(Duration::from_millis(29_950), Duration::from_secs(129)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn handoff_cannot_start_a_fresh_thirty_second_window() {
        let mut bound = long_lived(Duration::from_millis(29_999));
        assert_eq!(
            bound.check_at(Duration::from_millis(30_001), Duration::from_secs(129)),
            Err(MdnsError::DeadlineExceeded)
        );
    }

    #[test]
    fn repeated_valid_reviews_never_move_the_controller_deadline() {
        let mut bound = long_lived(Duration::from_millis(29_800));
        for elapsed in [29_800, 29_850, 29_900, 29_950, 29_999] {
            assert_eq!(
                bound.check_at(Duration::from_millis(elapsed), Duration::from_secs(129)),
                Ok(())
            );
        }
        assert_eq!(
            bound.check_at(Duration::from_secs(30), Duration::from_secs(129)),
            Err(MdnsError::DeadlineExceeded)
        );
    }

    #[test]
    fn final_controller_watermark_rejects_one_nanosecond_rollback() {
        let completed = Duration::from_secs(20);
        let mut bound = long_lived(completed);
        assert_eq!(
            bound.check_at(
                completed - Duration::from_nanos(1),
                Duration::from_secs(129)
            ),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(
            bound.check_at(completed, Duration::from_secs(129)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn successful_review_advances_the_controller_watermark() {
        let mut bound = long_lived(Duration::from_secs(20));
        assert_eq!(
            bound.check_at(Duration::from_millis(20_100), Duration::from_secs(129)),
            Ok(())
        );
        assert_eq!(
            bound.check_at(
                Duration::from_millis(20_100) - Duration::from_nanos(1),
                Duration::from_secs(129)
            ),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn moved_lifetime_keeps_its_pre_handoff_elapsed_watermark() {
        let mut lifetime = Lifetime::new(Duration::from_secs(100), 60).unwrap();
        lifetime
            .check(Duration::from_secs(20), Duration::from_secs(120))
            .unwrap();
        let mut bound = ControllerReviewBounds::from_controller(
            Instant::now(),
            lifetime,
            Duration::from_secs(19),
        )
        .unwrap();
        assert_eq!(
            bound.check_at(Duration::from_secs(19), Duration::from_secs(120)),
            Err(MdnsError::ClockInvalid)
        );
    }

    #[test]
    fn moved_lifetime_keeps_full_precision_wall_watermark() {
        let final_wall = Duration::from_secs(120) + Duration::from_nanos(1);
        let mut bound = bounds(
            Duration::from_secs(100),
            60,
            Duration::from_secs(20),
            final_wall,
        );
        assert_eq!(
            bound.check_at(
                Duration::from_secs(20),
                final_wall - Duration::from_nanos(1)
            ),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(
            bound.check_at(Duration::from_secs(20), final_wall),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn stopped_wall_does_not_renew_original_fractional_monotonic_lease() {
        let wall = Duration::from_millis(100_900);
        let mut bound = bounds(wall, 2, Duration::from_secs(1), Duration::from_secs(101));
        assert_eq!(
            bound.check_at(
                Duration::from_millis(1_100) - Duration::from_nanos(1),
                Duration::from_secs(101)
            ),
            Ok(())
        );
        assert_eq!(
            bound.check_at(Duration::from_millis(1_100), Duration::from_secs(101)),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(
            bound.check_at(Duration::from_secs(1), Duration::from_secs(101)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn original_absolute_f0_expiry_rejects_a_wall_jump() {
        let mut bound = bounds(
            Duration::from_millis(100_900),
            10,
            Duration::from_secs(1),
            Duration::from_secs(101),
        );
        assert_eq!(
            bound.check_at(Duration::from_millis(1_001), Duration::from_secs(110)),
            Err(MdnsError::ClockInvalid)
        );
        assert_eq!(
            bound.check_at(Duration::from_millis(1_002), Duration::from_secs(102)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn an_original_closed_lifetime_cannot_be_revived_at_handoff() {
        let mut lifetime = Lifetime::new(Duration::from_secs(100), 60).unwrap();
        assert_eq!(
            lifetime.check(Duration::ZERO, Duration::from_secs(99)),
            Err(MdnsError::ClockInvalid)
        );
        let mut bound =
            ControllerReviewBounds::from_controller(Instant::now(), lifetime, Duration::ZERO)
                .unwrap();
        assert_eq!(
            bound.check_at(Duration::ZERO, Duration::from_secs(100)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn a_controller_already_at_its_deadline_cannot_supply_bounds() {
        let lifetime = Lifetime::new(Duration::from_secs(100), 60).unwrap();
        assert!(matches!(
            ControllerReviewBounds::from_controller(
                Instant::now(),
                lifetime,
                Duration::from_secs(30)
            ),
            Err(MdnsError::DeadlineExceeded)
        ));
    }

    #[test]
    fn active_guard_rejects_a_deadline_crossed_inside_the_checked_operation() {
        use std::cell::Cell;
        let mut bound = long_lived(Duration::from_millis(29_999));
        let elapsed = Cell::new(Duration::from_millis(29_999));
        let calls = Cell::new(0);
        let body_ran = Cell::new(false);
        let mut guard = || {
            calls.set(calls.get() + 1);
            bound.check_at(elapsed.get(), Duration::from_secs(129))
        };
        let result = super::super::super::probing::guarded_check(&mut guard, || {
            body_ran.set(true);
            elapsed.set(Duration::from_secs(30));
            Ok("quiet")
        });
        assert_eq!(result, Err(MdnsError::DeadlineExceeded));
        assert!(body_ran.get());
        assert_eq!(calls.get(), 2);
        assert_eq!(
            bound.check_at(Duration::from_millis(29_999), Duration::from_secs(129)),
            Err(MdnsError::Closed)
        );
    }

    #[test]
    fn active_guard_rejects_invalid_entry_before_the_checked_operation() {
        use std::cell::Cell;
        let mut bound = long_lived(Duration::from_secs(20));
        let calls = Cell::new(0);
        let body_ran = Cell::new(false);
        let mut guard = || {
            calls.set(calls.get() + 1);
            bound.check_at(Duration::from_secs(19), Duration::from_secs(129))
        };
        let result = super::super::super::probing::guarded_check(&mut guard, || {
            body_ran.set(true);
            Ok("quiet")
        });
        assert_eq!(result, Err(MdnsError::ClockInvalid));
        assert!(!body_ran.get());
        assert_eq!(calls.get(), 1);
    }
}
