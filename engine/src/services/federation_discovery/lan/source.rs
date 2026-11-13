use super::{LanDiscoveryError, Result};

/// Caller-owned source coordinate. Epochs identify explicit interface incarnations, not ifindices.
/// A canonical instance label is a cache position, never node ownership or authentication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LanSource {
    epoch: u64,
    instance: String,
}

impl LanSource {
    pub fn new(epoch: u64, instance: &str) -> Result<Self> {
        let alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
        if epoch == 0
            || !(1..=63).contains(&instance.len())
            || !instance
                .bytes()
                .all(|byte| alphanumeric(byte) || byte == b'-')
            || !instance.bytes().next().is_some_and(alphanumeric)
            || !instance.bytes().last().is_some_and(alphanumeric)
        {
            return Err(LanDiscoveryError::InvalidSource);
        }
        Ok(Self {
            epoch,
            instance: instance.to_owned(),
        })
    }

    pub fn interface_epoch(&self) -> u64 {
        self.epoch
    }

    pub fn instance(&self) -> &str {
        &self.instance
    }
}

/// Explicit supplied samples only. Monotonic zero is valid; arithmetic is checked at admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheTime {
    pub(super) unix: u64,
    pub(super) monotonic: u64,
}

impl CacheTime {
    pub fn new(unix: u64, monotonic_seconds: u64) -> Result<Self> {
        if unix == 0 || unix > super::super::MAX_SAFE_TIME {
            return Err(LanDiscoveryError::InvalidTime);
        }
        Ok(Self {
            unix,
            monotonic: monotonic_seconds,
        })
    }
}
