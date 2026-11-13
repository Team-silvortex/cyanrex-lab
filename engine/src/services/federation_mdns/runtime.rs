use super::*;
use crate::services::federation_discovery::{
    lan::{CacheTime, LanCandidateCache, LanDiscoveredCandidate, LanSource},
    ConnectionHint, DiscoveryPolicy, PeerAdvertisement,
};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;

fn sample(started: Instant) -> Result<(CacheTime, u64)> {
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| MdnsError::ClockInvalid)?;
    let time = CacheTime::new(wall.as_secs(), started.elapsed().as_secs())
        .map_err(|_| MdnsError::ClockInvalid)?;
    Ok((time, wall.as_secs()))
}

struct Intake {
    cache: LanCandidateCache,
    origins: HashMap<String, std::net::Ipv4Addr>,
    policy: DiscoveryPolicy,
    epoch: u64,
}

impl Intake {
    fn new(policy: DiscoveryPolicy, epoch: u64) -> Result<Self> {
        Ok(Self {
            cache: LanCandidateCache::new(policy.clone(), vec![epoch], 64)?,
            origins: HashMap::new(),
            policy,
            epoch,
        })
    }
    fn apply(
        &mut self,
        event: MdnsEvent,
        sender: std::net::Ipv4Addr,
        time: CacheTime,
        wall: u64,
    ) -> Result<()> {
        let live = self.cache.snapshot(time)?;
        self.origins
            .retain(|instance, _| live.iter().any(|peer| peer.source().instance() == instance));
        let result = match event {
            MdnsEvent::Removed { instance } => {
                if self.origins.get(&instance) != Some(&sender) {
                    return Ok(());
                }
                let source = LanSource::new(self.epoch, &instance)?;
                let removed = self.cache.remove(&source, time);
                if removed.is_ok() {
                    self.origins.remove(&instance);
                }
                removed.map(|_| ())
            }
            MdnsEvent::Resolved {
                instance,
                advertisement,
                addresses,
                port,
                ttl,
            } => {
                let peer = PeerAdvertisement::parse(&advertisement)?;
                let selected = match peer.select(&self.policy, wall) {
                    Ok(candidate) => candidate,
                    Err(_) => return Ok(()),
                };
                if selected.hints().is_empty() || selected.hints().iter().any(|hint| !matches!(hint,
                    ConnectionHint::Direct(std::net::SocketAddr::V4(address))
                    if *address.ip() == sender && address.port() == port && addresses.contains(address.ip()))) {
                    return Ok(());
                }
                let source = LanSource::new(self.epoch, &instance)?;
                let admitted = self.cache.observe(&source, &advertisement, ttl, time);
                if matches!(
                    admitted,
                    Ok(crate::services::federation_discovery::lan::LanObservation::Added)
                ) {
                    self.origins.insert(instance, sender);
                }
                admitted.map(|_| ())
            }
        };
        match result {
            Ok(())
            | Err(
                LanDiscoveryError::SourceConflict
                | LanDiscoveryError::Capacity
                | LanDiscoveryError::RejectedAdvertisement(_),
            ) => Ok(()),
            Err(LanDiscoveryError::RateLimited) => Err(MdnsError::BudgetExceeded),
            Err(error) => Err(error.into()),
        }
    }
}

/// Send exactly one fixed multicast PTR query, collect bounded replies and close before returning.
/// The returned values are expired-at-some-future-time metadata, never connected or verified peers.
pub fn discover_once(
    config: &MdnsConfig,
    policy: DiscoveryPolicy,
    cancelled: &AtomicBool,
) -> Result<Vec<LanDiscoveredCandidate>> {
    if cancelled.load(Ordering::Acquire) {
        return Err(MdnsError::Cancelled);
    }
    let started = Instant::now();
    let (initial_time, _) = sample(started)?;
    let interface = VerifiedInterface::inspect(config)?;
    let epoch = NEXT_EPOCH
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| MdnsError::Closed)?;
    let mut intake = Intake::new(policy, epoch)?;
    intake.cache.snapshot(initial_time)?;
    if cancelled.load(Ordering::Acquire) {
        intake.cache.stop();
        return Err(MdnsError::Cancelled);
    }
    let mut socket = MdnsSocket::open(&interface)?;
    let result = (|| {
        if cancelled.load(Ordering::Acquire) {
            return Err(MdnsError::Cancelled);
        }
        socket.send_query(&browse_query())?;
        let mut total = 0;
        let mut window = 0;
        let mut in_window = 0;
        while started.elapsed() < config.timeout() {
            if cancelled.load(Ordering::Acquire) {
                return Err(MdnsError::Cancelled);
            }
            interface.verify_unchanged()?;
            let received = socket.receive();
            if cancelled.load(Ordering::Acquire) {
                return Err(MdnsError::Cancelled);
            }
            if matches!(received, Ok(None)) {
                continue;
            }
            total += 1;
            let second = started.elapsed().as_secs();
            if second != window {
                window = second;
                in_window = 0;
            }
            in_window += 1;
            if total > MAX_PACKETS_PER_RUN || in_window > MAX_PACKETS_PER_SECOND {
                return Err(MdnsError::BudgetExceeded);
            }
            let received = match received {
                Ok(Some(packet)) => packet,
                Err(MdnsError::InvalidPacket | MdnsError::TooLarge) => continue,
                Err(error) => return Err(error),
                Ok(None) => unreachable!(),
            };
            let events = match parse_response(&received.bytes) {
                Ok(events) => events,
                Err(MdnsError::InvalidPacket | MdnsError::TooLarge | MdnsError::RejectedTxt(_)) => {
                    continue
                }
                Err(error) => return Err(error),
            };
            for event in events {
                if cancelled.load(Ordering::Acquire) {
                    return Err(MdnsError::Cancelled);
                }
                let (time, wall) = sample(started)?;
                intake.apply(event, received.source, time, wall)?;
            }
        }
        if cancelled.load(Ordering::Acquire) {
            return Err(MdnsError::Cancelled);
        }
        interface.verify_unchanged()?;
        intake
            .cache
            .snapshot(sample(started)?.0)
            .map_err(Into::into)
    })();
    intake.cache.stop();
    drop(socket);
    result
}
