use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanObservation {
    Added,
    Duplicate,
}

/// A read-time snapshot, not live registration, peer authentication or a later-use permission.
#[derive(Debug, Clone)]
pub struct LanDiscoveredCandidate {
    source: LanSource,
    candidate: UnverifiedPeerCandidate,
    deadline_monotonic: u64,
}

impl LanDiscoveredCandidate {
    pub fn source(&self) -> &LanSource {
        &self.source
    }
    pub fn candidate(&self) -> &UnverifiedPeerCandidate {
        &self.candidate
    }
    pub fn deadline_monotonic(&self) -> u64 {
        self.deadline_monotonic
    }
}

#[derive(Debug)]
struct Entry {
    source: LanSource,
    advertisement: PeerAdvertisement,
    canonical: Vec<u8>,
    deadline_unix: u64,
    deadline_monotonic: u64,
}

/// Bounded caller-owned state. No eviction, background work, automatic renewal or restart.
#[derive(Debug)]
pub struct LanCandidateCache {
    policy: DiscoveryPolicy,
    approved_epochs: Vec<u64>,
    capacity: usize,
    entries: Vec<Entry>,
    last_time: Option<CacheTime>,
    event_second: Option<u64>,
    event_count: u32,
    closed: bool,
}

impl LanCandidateCache {
    pub fn new(
        policy: DiscoveryPolicy,
        approved_epochs: Vec<u64>,
        capacity: usize,
    ) -> Result<Self> {
        if !(1..=MAX_LAN_CANDIDATES).contains(&capacity)
            || approved_epochs.is_empty()
            || approved_epochs.contains(&0)
            || !super::super::policy::distinct(&approved_epochs, MAX_LAN_INTERFACES)
        {
            return Err(LanDiscoveryError::InvalidConfig);
        }
        Ok(Self {
            policy,
            approved_epochs,
            capacity,
            entries: Vec::new(),
            last_time: None,
            event_second: None,
            event_count: 0,
            closed: false,
        })
    }

    pub fn observe(
        &mut self,
        source: &LanSource,
        raw_f0: &[u8],
        ttl: u32,
        time: CacheTime,
    ) -> Result<LanObservation> {
        self.advance(time)?;
        self.event(time)?;
        self.require_source(source)?;
        if !(1..=MAX_LAN_TTL_SECONDS).contains(&ttl) {
            return Err(LanDiscoveryError::InvalidTtl);
        }
        let advertisement = PeerAdvertisement::parse(raw_f0)?;
        let canonical = advertisement.encode()?;
        if let Some(existing) = self.entries.iter().find(|entry| &entry.source == source) {
            if existing.canonical != canonical {
                return Err(LanDiscoveryError::SourceConflict);
            }
            // The original wall/monotonic deadlines are never extended or shortened by a repeat.
            return Ok(LanObservation::Duplicate);
        }
        let candidate = advertisement.select(&self.policy, time.unix)?;
        let duration = u64::from(ttl).min(candidate.expires_at() - time.unix);
        let deadline_monotonic = time
            .monotonic
            .checked_add(duration)
            .ok_or(LanDiscoveryError::InvalidTime)?;
        let deadline_unix = time
            .unix
            .checked_add(duration)
            .ok_or(LanDiscoveryError::InvalidTime)?;
        if self.entries.len() == self.capacity {
            return Err(LanDiscoveryError::Capacity);
        }
        self.entries.push(Entry {
            source: source.clone(),
            advertisement,
            canonical,
            deadline_unix,
            deadline_monotonic,
        });
        Ok(LanObservation::Added)
    }

    pub fn snapshot(&mut self, time: CacheTime) -> Result<Vec<LanDiscoveredCandidate>> {
        self.advance(time)?;
        self.entries
            .iter()
            .map(|entry| {
                Ok(LanDiscoveredCandidate {
                    source: entry.source.clone(),
                    candidate: entry.advertisement.select(&self.policy, time.unix)?,
                    deadline_monotonic: entry.deadline_monotonic,
                })
            })
            .collect()
    }

    pub fn remove(&mut self, source: &LanSource, time: CacheTime) -> Result<bool> {
        self.advance(time)?;
        self.event(time)?;
        self.require_source(source)?;
        let previous = self.entries.len();
        self.entries.retain(|entry| &entry.source != source);
        Ok(previous != self.entries.len())
    }

    /// Permanent closure. Retained snapshots stay inert metadata, never live cache authority.
    pub fn stop(&mut self) {
        self.entries.clear();
        self.closed = true;
    }

    fn advance(&mut self, time: CacheTime) -> Result<()> {
        if self.closed {
            return Err(LanDiscoveryError::Closed);
        }
        if self.last_time.is_some_and(|previous| {
            time.unix < previous.unix || time.monotonic < previous.monotonic
        }) {
            self.stop();
            return Err(LanDiscoveryError::ClockRollback);
        }
        // A valid sample is observed even if later input or deadline admission fails. Never
        // rewind this watermark (or the event bucket) to make a rejected event retryable.
        self.last_time = Some(time);
        self.entries.retain(|entry| {
            time.unix < entry.deadline_unix && time.monotonic < entry.deadline_monotonic
        });
        Ok(())
    }

    fn event(&mut self, time: CacheTime) -> Result<()> {
        if self.event_second != Some(time.monotonic) {
            self.event_second = Some(time.monotonic);
            self.event_count = 0;
        }
        if self.event_count >= MAX_LAN_EVENTS_PER_SECOND {
            return Err(LanDiscoveryError::RateLimited);
        }
        self.event_count += 1;
        Ok(())
    }

    fn require_source(&self, source: &LanSource) -> Result<()> {
        if !self.approved_epochs.contains(&source.interface_epoch()) {
            return Err(LanDiscoveryError::InvalidSource);
        }
        Ok(())
    }
}
