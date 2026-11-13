use super::MdnsError;
use std::time::Duration;

/// A fixed post-quiet gate; successful reviews never move its deadline.
pub(super) struct ReviewWindow {
    last: Duration,
    deadline: Duration,
    closed: bool,
}

impl ReviewWindow {
    pub(super) fn new(quiet_at: Duration) -> Result<Self, MdnsError> {
        let deadline = quiet_at
            .checked_add(Duration::from_millis(250))
            .ok_or(MdnsError::ClockInvalid)?;
        Ok(Self {
            last: quiet_at,
            deadline,
            closed: false,
        })
    }

    pub(super) fn check(&mut self, elapsed: Duration) -> Result<(), MdnsError> {
        if self.closed {
            return Err(MdnsError::Closed);
        }
        let error = if elapsed < self.last {
            Some(MdnsError::ClockInvalid)
        } else if elapsed >= self.deadline {
            Some(MdnsError::DeadlineExceeded)
        } else {
            None
        };
        if let Some(error) = error {
            self.closed = true;
            return Err(error);
        }
        self.last = elapsed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{MdnsError, ReviewWindow};
    use std::time::Duration;

    const WINDOW: Duration = Duration::from_millis(250);
    const TICK: Duration = Duration::from_nanos(1);

    #[test]
    fn zero_quiet_time_is_an_open_window() {
        let mut window = ReviewWindow::new(Duration::ZERO).unwrap();
        assert_eq!(window.check(Duration::ZERO), Ok(()));
    }

    #[test]
    fn quiet_time_itself_is_admitted() {
        let quiet = Duration::from_secs(12) + Duration::from_nanos(7);
        let mut window = ReviewWindow::new(quiet).unwrap();
        assert_eq!(window.check(quiet), Ok(()));
    }

    #[test]
    fn one_nanosecond_before_deadline_is_admitted() {
        let quiet = Duration::from_secs(5);
        let mut window = ReviewWindow::new(quiet).unwrap();
        assert_eq!(window.check(quiet + WINDOW - TICK), Ok(()));
    }

    #[test]
    fn exact_deadline_expires_the_window() {
        let quiet = Duration::from_secs(5);
        let mut window = ReviewWindow::new(quiet).unwrap();
        assert_eq!(
            window.check(quiet + WINDOW),
            Err(MdnsError::DeadlineExceeded)
        );
    }

    #[test]
    fn one_nanosecond_past_deadline_expires_the_window() {
        let quiet = Duration::from_secs(5);
        let mut window = ReviewWindow::new(quiet).unwrap();
        assert_eq!(
            window.check(quiet + WINDOW + TICK),
            Err(MdnsError::DeadlineExceeded)
        );
    }

    #[test]
    fn a_far_past_first_check_expires_the_window() {
        let mut window = ReviewWindow::new(Duration::ZERO).unwrap();
        assert_eq!(
            window.check(Duration::MAX),
            Err(MdnsError::DeadlineExceeded)
        );
    }

    #[test]
    fn successful_checks_never_renew_the_deadline() {
        let quiet = Duration::from_secs(1);
        let mut window = ReviewWindow::new(quiet).unwrap();
        for milliseconds in [0, 50, 100, 150, 200, 249] {
            assert_eq!(
                window.check(quiet + Duration::from_millis(milliseconds)),
                Ok(())
            );
        }
        assert_eq!(
            window.check(quiet + WINDOW),
            Err(MdnsError::DeadlineExceeded)
        );
    }

    #[test]
    fn submillisecond_quiet_time_is_not_rounded() {
        let quiet = Duration::from_secs(4) + Duration::from_nanos(123_456);
        let mut window = ReviewWindow::new(quiet).unwrap();
        assert_eq!(window.check(quiet + WINDOW - TICK), Ok(()));
        assert_eq!(
            window.check(quiet + WINDOW),
            Err(MdnsError::DeadlineExceeded)
        );
    }

    #[test]
    fn repeated_equal_checks_are_admitted() {
        let quiet = Duration::from_secs(2);
        let mut window = ReviewWindow::new(quiet).unwrap();
        let sample = quiet + Duration::from_millis(100);
        assert_eq!(window.check(sample), Ok(()));
        assert_eq!(window.check(sample), Ok(()));
    }

    #[test]
    fn time_before_quiet_is_invalid_and_terminal() {
        let quiet = Duration::from_secs(2);
        let mut window = ReviewWindow::new(quiet).unwrap();
        assert_eq!(window.check(quiet - TICK), Err(MdnsError::ClockInvalid));
        assert_eq!(window.check(quiet), Err(MdnsError::Closed));
        assert_eq!(window.check(quiet + WINDOW), Err(MdnsError::Closed));
    }

    #[test]
    fn time_before_last_success_is_invalid_and_terminal() {
        let quiet = Duration::from_secs(2);
        let sample = quiet + Duration::from_millis(100);
        let mut window = ReviewWindow::new(quiet).unwrap();
        assert_eq!(window.check(sample), Ok(()));
        assert_eq!(window.check(sample - TICK), Err(MdnsError::ClockInvalid));
        assert_eq!(window.check(sample), Err(MdnsError::Closed));
        assert_eq!(window.check(quiet + WINDOW), Err(MdnsError::Closed));
    }

    #[test]
    fn deadline_failure_is_terminal_for_all_later_checks() {
        let quiet = Duration::from_secs(2);
        let mut window = ReviewWindow::new(quiet).unwrap();
        assert_eq!(
            window.check(quiet + WINDOW),
            Err(MdnsError::DeadlineExceeded)
        );
        for sample in [Duration::ZERO, quiet, quiet + WINDOW, Duration::MAX] {
            assert_eq!(window.check(sample), Err(MdnsError::Closed));
        }
    }

    #[test]
    fn maximum_quiet_time_rejects_deadline_overflow() {
        assert!(matches!(
            ReviewWindow::new(Duration::MAX),
            Err(MdnsError::ClockInvalid)
        ));
    }

    #[test]
    fn one_nanosecond_above_latest_quiet_time_rejects_overflow() {
        let quiet = Duration::MAX - WINDOW + TICK;
        assert!(matches!(
            ReviewWindow::new(quiet),
            Err(MdnsError::ClockInvalid)
        ));
    }

    #[test]
    fn latest_representable_deadline_obeys_exact_boundary() {
        let quiet = Duration::MAX - WINDOW;
        let mut window = ReviewWindow::new(quiet).unwrap();
        assert_eq!(window.check(quiet), Ok(()));
        assert_eq!(window.check(Duration::MAX - TICK), Ok(()));
        assert_eq!(
            window.check(Duration::MAX),
            Err(MdnsError::DeadlineExceeded)
        );
        assert_eq!(window.check(quiet), Err(MdnsError::Closed));
    }
}
