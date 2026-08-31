use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::models::settings::{CompilerOperationMetricsResponse, PerformanceMetricsResponse};

#[derive(Debug, Default)]
struct OperationMetrics {
    total_requests: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    errors: AtomicU64,
    rejected: AtomicU64,
    in_flight: AtomicUsize,
    in_flight_peak: AtomicUsize,
    total_duration_nanos: AtomicU64,
}

impl OperationMetrics {
    fn start(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        let in_flight = self.in_flight.fetch_add(1, Ordering::Relaxed) + 1;
        update_peak(&self.in_flight_peak, in_flight);
    }

    fn finish(&self, duration_nanos: u64, cache_hit: Option<bool>, ok: bool, rejected: bool) {
        match cache_hit {
            Some(true) => {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
            }
            Some(false) => {
                self.cache_misses.fetch_add(1, Ordering::Relaxed);
            }
            None => {}
        }
        if !ok {
            self.errors.fetch_add(1, Ordering::Relaxed);
        }
        if rejected {
            self.rejected.fetch_add(1, Ordering::Relaxed);
        }
        self.total_duration_nanos
            .fetch_add(duration_nanos, Ordering::Relaxed);
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> CompilerOperationMetricsResponse {
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let total_duration_nanos = self.total_duration_nanos.load(Ordering::Relaxed);

        CompilerOperationMetricsResponse {
            total_requests,
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
            in_flight: self.in_flight.load(Ordering::Relaxed) as u64,
            in_flight_peak: self.in_flight_peak.load(Ordering::Relaxed) as u64,
            avg_duration_ms: average_duration_ms(total_duration_nanos, total_requests),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct PerformanceMetrics {
    check: OperationMetrics,
    completion: OperationMetrics,
}

impl PerformanceMetrics {
    pub(crate) fn start_check(&self) {
        self.check.start();
    }

    pub(crate) fn finish_check(
        &self,
        duration_nanos: u64,
        cache_hit: Option<bool>,
        ok: bool,
        rejected: bool,
    ) {
        self.check.finish(duration_nanos, cache_hit, ok, rejected);
    }

    pub(crate) fn start_completion(&self) {
        self.completion.start();
    }

    pub(crate) fn finish_completion(
        &self,
        duration_nanos: u64,
        cache_hit: Option<bool>,
        ok: bool,
        rejected: bool,
    ) {
        self.completion
            .finish(duration_nanos, cache_hit, ok, rejected);
    }

    pub(crate) fn snapshot(&self) -> PerformanceMetricsResponse {
        PerformanceMetricsResponse {
            check: self.check.snapshot(),
            completion: self.completion.snapshot(),
        }
    }
}

fn average_duration_ms(total_duration_nanos: u64, total_requests: u64) -> f64 {
    if total_requests == 0 {
        0.0
    } else {
        total_duration_nanos as f64 / total_requests as f64 / 1_000_000.0
    }
}

fn update_peak(peak: &AtomicUsize, current: usize) {
    let mut previous = peak.load(Ordering::Relaxed);
    while current > previous {
        match peak.compare_exchange_weak(previous, current, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => previous = next,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PerformanceMetrics;

    #[test]
    fn records_check_and_completion_metrics_independently() {
        let metrics = PerformanceMetrics::default();

        metrics.start_check();
        metrics.finish_check(2_000_000, Some(true), true, false);
        metrics.start_completion();
        metrics.finish_completion(4_000_000, Some(false), false, true);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.check.total_requests, 1);
        assert_eq!(snapshot.check.cache_hits, 1);
        assert_eq!(snapshot.check.cache_misses, 0);
        assert_eq!(snapshot.check.errors, 0);
        assert_eq!(snapshot.check.rejected, 0);
        assert_eq!(snapshot.check.in_flight, 0);
        assert_eq!(snapshot.check.in_flight_peak, 1);
        assert_eq!(snapshot.check.avg_duration_ms, 2.0);

        assert_eq!(snapshot.completion.total_requests, 1);
        assert_eq!(snapshot.completion.cache_hits, 0);
        assert_eq!(snapshot.completion.cache_misses, 1);
        assert_eq!(snapshot.completion.errors, 1);
        assert_eq!(snapshot.completion.rejected, 1);
        assert_eq!(snapshot.completion.in_flight, 0);
        assert_eq!(snapshot.completion.in_flight_peak, 1);
        assert_eq!(snapshot.completion.avg_duration_ms, 4.0);
    }
}
