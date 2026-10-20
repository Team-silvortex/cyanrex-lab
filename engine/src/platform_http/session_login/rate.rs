//! One bounded state-wide window, never a peer-IP, request-header or username keyed registry.
use crate::platform_http::errors::HttpError;
use axum::http::StatusCode;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

const WINDOW: Duration = Duration::from_secs(60);
const MAX_ATTEMPTS: u8 = 64;

pub(super) struct LoginRate {
    window: Mutex<Window>,
}

struct Window {
    started: Instant,
    attempts: u8,
}

impl LoginRate {
    pub(super) fn new() -> Self {
        Self {
            window: Mutex::new(Window {
                started: Instant::now(),
                attempts: 0,
            }),
        }
    }

    pub(super) fn admit(&self) -> Result<(), HttpError> {
        let mut window = self
            .window
            .lock()
            .map_err(|_| HttpError::new(StatusCode::SERVICE_UNAVAILABLE, "service_unavailable"))?;
        if window.admit_at(Instant::now()) {
            Ok(())
        } else {
            Err(HttpError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
            ))
        }
    }
}

impl Window {
    fn admit_at(&mut self, now: Instant) -> bool {
        if now.saturating_duration_since(self.started) >= WINDOW {
            self.started = now;
            self.attempts = 0;
        }
        if self.attempts >= MAX_ATTEMPTS {
            return false;
        }
        // Every admitted request spends this count, even later capacity/body/backend failure.
        // Completion, cancellation and successful login have no refund or reset capability.
        self.attempts += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn global_login_rate_admits_exactly_64_without_refunds_or_success_resets() {
        let now = Instant::now();
        let mut window = Window {
            started: now,
            attempts: 0,
        };
        for _ in 0..MAX_ATTEMPTS {
            assert!(window.admit_at(now));
        }
        for _ in 0..10 {
            assert!(!window.admit_at(now));
        }
        assert_eq!(window.attempts, MAX_ATTEMPTS);
    }

    #[test]
    fn global_login_rate_resets_only_at_the_exact_monotonic_window_boundary() {
        let now = Instant::now();
        let mut window = Window {
            started: now,
            attempts: MAX_ATTEMPTS,
        };
        assert!(!window.admit_at(now + WINDOW - Duration::from_nanos(1)));
        assert!(window.admit_at(now + WINDOW));
        assert_eq!(window.attempts, 1);
        assert_eq!(window.started, now + WINDOW);
        for _ in 1..MAX_ATTEMPTS {
            assert!(window.admit_at(now + WINDOW));
        }
        assert!(!window.admit_at(now + WINDOW));
    }

    #[test]
    fn global_login_rate_does_not_reset_when_a_supplied_time_precedes_the_window() {
        let now = Instant::now();
        let mut window = Window {
            started: now + WINDOW,
            attempts: MAX_ATTEMPTS,
        };
        assert!(!window.admit_at(now));
        assert_eq!(window.attempts, MAX_ATTEMPTS);
    }

    #[test]
    fn global_login_rate_fails_closed_if_its_fixed_state_is_poisoned() {
        let rate = LoginRate::new();
        let _ = std::panic::catch_unwind(|| {
            let _guard = rate.window.lock().unwrap();
            panic!("synthetic rate-lock poison");
        });
        assert_eq!(
            rate.admit().err().unwrap().into_response().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
