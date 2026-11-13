use super::{endpoint, *};

pub(super) fn distinct<T: PartialEq>(values: &[T], limit: usize) -> bool {
    values.len() <= limit
        && values
            .iter()
            .enumerate()
            .all(|(index, value)| !values[..index].contains(value))
}

pub(super) fn capabilities_valid(offered: &[QualifiedName], required: &[QualifiedName]) -> bool {
    distinct(offered, MAX_CAPABILITIES)
        && distinct(required, MAX_CAPABILITIES)
        && required.iter().all(|name| offered.contains(name))
}

impl DiscoveryPolicy {
    /// Defaults to RFC1918/ULA candidates only. This value performs no connection or discovery.
    pub fn new(
        local_node: NodeFingerprint,
        protocol: ProtocolRange,
        supported: Vec<QualifiedName>,
        required: Vec<QualifiedName>,
    ) -> Result<Self> {
        if !capabilities_valid(&supported, &required) {
            return Err(DiscoveryError::InvalidPolicy);
        }
        Ok(Self {
            local_node,
            protocol,
            supported,
            required,
            public_direct: false,
            relays: Vec::new(),
        })
    }

    /// Explicitly admit eligible public literals as inert candidates, not reachable/trusted peers.
    pub fn with_public_direct(mut self) -> Self {
        self.public_direct = true;
        self
    }

    /// Local symbolic allowlist only. No peer-supplied relay URL or resolution is accepted.
    pub fn with_relays(mut self, relays: Vec<QualifiedName>) -> Result<Self> {
        if !distinct(&relays, MAX_HINTS) {
            return Err(DiscoveryError::InvalidPolicy);
        }
        self.relays = relays;
        Ok(self)
    }
}

impl PeerAdvertisement {
    /// Use the supplied observation time once; the caller owns clock freshness and later expiry.
    pub fn select(&self, policy: &DiscoveryPolicy, now: u64) -> Result<UnverifiedPeerCandidate> {
        if now == 0 || now > MAX_SAFE_TIME || self.issued_at > now + MAX_FUTURE_SKEW_SECONDS {
            return Err(DiscoveryError::InvalidTime);
        }
        if now >= self.expires_at {
            return Err(DiscoveryError::Expired);
        }
        if self.node == policy.local_node {
            return Err(DiscoveryError::SelfAdvertisement);
        }
        let min = self.protocol.min.max(policy.protocol.min);
        let max = self.protocol.max.min(policy.protocol.max);
        if min > max {
            return Err(DiscoveryError::UnsupportedProtocol);
        }
        if policy
            .required
            .iter()
            .any(|name| !self.offered.contains(name))
            || self
                .required
                .iter()
                .any(|name| !policy.supported.contains(name))
        {
            return Err(DiscoveryError::MissingCapability);
        }
        let mut hints = Vec::new();
        for class in [endpoint::Class::Private, endpoint::Class::Public] {
            if class == endpoint::Class::Public && !policy.public_direct {
                continue;
            }
            hints.extend(self.hints.iter().filter_map(|hint| match hint {
                ConnectionHint::Direct(address) if endpoint::classify(*address) == Some(class) => {
                    Some(hint.clone())
                }
                _ => None,
            }));
        }
        hints.extend(self.hints.iter().filter_map(|hint| match hint {
            ConnectionHint::Relay(name) if policy.relays.contains(name) => Some(hint.clone()),
            _ => None,
        }));
        if hints.is_empty() {
            return Err(DiscoveryError::NoAllowedHint);
        }
        Ok(UnverifiedPeerCandidate {
            node: self.node.clone(),
            authority: self.authority,
            protocol: max,
            capabilities: self
                .offered
                .iter()
                .filter(|name| policy.supported.contains(name))
                .cloned()
                .collect(),
            hints,
            expires_at: self.expires_at,
        })
    }
}
