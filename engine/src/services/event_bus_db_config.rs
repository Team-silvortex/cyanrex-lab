use std::time::{Duration as StdDuration, Instant as StdInstant};

#[derive(Clone, Copy)]
pub(crate) struct PersistQueuePressureConfig {
    pub(crate) enabled: bool,
    pub(crate) warning_ratio_percent: u64,
    pub(crate) clear_ratio_percent: u64,
    pub(crate) warn_min_interval: StdDuration,
}

impl PersistQueuePressureConfig {
    pub(crate) fn from_env() -> Self {
        let warning_ratio_percent =
            parse_env_percent("CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT", 80);

        let clear_ratio_percent =
            parse_env_percent("CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT", 40)
                .min(warning_ratio_percent.saturating_sub(1).max(1));

        Self {
            enabled: parse_env_bool("CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED", true),
            warning_ratio_percent,
            clear_ratio_percent: clear_ratio_percent.max(1),
            warn_min_interval: StdDuration::from_millis(parse_env_u64(
                "CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS",
                10_000,
            )),
        }
    }

    pub(crate) fn warning_threshold(&self, capacity: usize) -> usize {
        let threshold = (capacity as u64).saturating_mul(self.warning_ratio_percent) / 100;
        threshold.max(1) as usize
    }

    pub(crate) fn recover_threshold(&self, capacity: usize) -> usize {
        let threshold = (capacity as u64).saturating_mul(self.clear_ratio_percent) / 100;
        threshold.max(1) as usize
    }

    pub(crate) fn should_emit_warning(&self, last_warning_time: Option<StdInstant>) -> bool {
        match last_warning_time {
            Some(previous) => previous.elapsed() >= self.warn_min_interval,
            None => true,
        }
    }
}

fn parse_env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.to_lowercase().as_str(),
                "1" | "true" | "on" | "yes" | "y" | "enabled",
            )
        })
        .unwrap_or(default)
}

fn parse_env_percent(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| (1..=100).contains(value))
        .unwrap_or(default)
}

fn parse_env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}
