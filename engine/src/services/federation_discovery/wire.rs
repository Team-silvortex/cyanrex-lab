//! Map-only strict raw decoding: never erase duplicate keys through `serde_json::Value`.
use super::*;
use serde::{
    de::{value::MapAccessDeserializer, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use std::marker::PhantomData;

#[derive(Serialize)]
#[serde(transparent)]
struct MapOnly<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for MapOnly<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct RecordVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for RecordVisitor<T> {
            type Value = MapOnly<T>;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object record")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(MapOnly)
            }
        }
        deserializer.deserialize_map(RecordVisitor(PhantomData))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawAdvertisement {
    schema_version: u8,
    node_fingerprint: String,
    authority_id: AuthorityId,
    protocol: MapOnly<RawProtocol>,
    offered_capabilities: Vec<QualifiedName>,
    required_capabilities: Vec<QualifiedName>,
    issued_at: u64,
    expires_at: u64,
    hints: Vec<MapOnly<RawHint>>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawProtocol {
    min: u16,
    max: u16,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RawHint {
    Direct { address: String },
    Relay { relay_id: QualifiedName },
}

impl PeerAdvertisement {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_ADVERTISEMENT_BYTES {
            return Err(DiscoveryError::InvalidAdvertisement);
        }
        let raw = serde_json::from_slice::<MapOnly<RawAdvertisement>>(bytes)
            .map_err(|_| DiscoveryError::InvalidAdvertisement)?
            .0;
        if raw.schema_version != 1
            || !policy::capabilities_valid(&raw.offered_capabilities, &raw.required_capabilities)
            || raw.issued_at == 0
            || raw.expires_at > MAX_SAFE_TIME
            || raw.expires_at <= raw.issued_at
            || raw.expires_at - raw.issued_at > MAX_LEASE_SECONDS
            || raw.hints.is_empty()
            || raw.hints.len() > MAX_HINTS
        {
            return Err(DiscoveryError::InvalidAdvertisement);
        }
        let protocol = ProtocolRange::new(raw.protocol.0.min, raw.protocol.0.max)
            .map_err(|_| DiscoveryError::InvalidAdvertisement)?;
        let hints = raw
            .hints
            .into_iter()
            .map(|hint| match hint.0 {
                RawHint::Direct { address } => {
                    endpoint::parse(&address).map(ConnectionHint::Direct)
                }
                RawHint::Relay { relay_id } => Ok(ConnectionHint::Relay(relay_id)),
            })
            .collect::<Result<Vec<_>>>()?;
        if !policy::distinct(&hints, MAX_HINTS) {
            return Err(DiscoveryError::InvalidAdvertisement);
        }
        Ok(Self {
            node: raw.node_fingerprint.parse()?,
            authority: raw.authority_id,
            protocol,
            offered: raw.offered_capabilities,
            required: raw.required_capabilities,
            issued_at: raw.issued_at,
            expires_at: raw.expires_at,
            hints,
        })
    }

    /// Re-encode validated claims without reallocating identity, refreshing time or doing I/O.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let raw = RawAdvertisement {
            schema_version: 1,
            node_fingerprint: self.node.to_string(),
            authority_id: self.authority,
            protocol: MapOnly(RawProtocol {
                min: self.protocol.min,
                max: self.protocol.max,
            }),
            offered_capabilities: self.offered.clone(),
            required_capabilities: self.required.clone(),
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            hints: self
                .hints
                .iter()
                .map(|hint| {
                    MapOnly(match hint {
                        ConnectionHint::Direct(address) => RawHint::Direct {
                            address: address.to_string(),
                        },
                        ConnectionHint::Relay(relay_id) => RawHint::Relay {
                            relay_id: relay_id.clone(),
                        },
                    })
                })
                .collect(),
        };
        let encoded = serde_json::to_vec(&raw).map_err(|_| DiscoveryError::InvalidAdvertisement)?;
        if encoded.len() > MAX_ADVERTISEMENT_BYTES {
            return Err(DiscoveryError::InvalidAdvertisement);
        }
        Ok(encoded)
    }
}
