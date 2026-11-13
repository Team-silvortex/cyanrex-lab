//! Pure frozen DNS-SD wire preparation, not an announcer or permission to send these bytes.
//! Names and addresses remain claims; probing, conflict handling and sender lifecycle are separate.
use super::{MdnsError, MAX_MDNS_PACKET_BYTES};
use crate::services::federation_discovery::{
    lan::{encode_txt, LanSource, MAX_LAN_TTL_SECONDS},
    PeerAdvertisement,
};
use std::net::{Ipv4Addr, SocketAddrV4};

mod probe;
pub use probe::ProbeObservation;

type Result<T> = std::result::Result<T, MdnsError>;
const SERVICE: &[u8] = b"\x08_cyanrex\x04_udp\x05local\0";

/// Caller-owned immutable record data, without a name claim, delivery receipt or live lease.
/// No interface is checked. A later sender must establish uniqueness and recheck its actual egress.
pub struct MdnsAnnouncement {
    instance: Vec<u8>,
    host: Vec<u8>,
    endpoint: SocketAddrV4,
    advertisement: PeerAdvertisement,
    txt: Vec<u8>,
    prepared_at: u64,
    ttl: u32,
}

impl MdnsAnnouncement {
    /// Require one exact private IPv4 hint and independently canonical single-label names.
    /// Time is caller supplied; this neither reads a clock nor issues/renews an F0 declaration.
    pub fn new(
        instance: &str,
        host_label: &str,
        endpoint: SocketAddrV4,
        advertisement: &PeerAdvertisement,
        now: u64,
        ttl: u32,
    ) -> Result<Self> {
        if !(1..=MAX_LAN_TTL_SECONDS).contains(&ttl) {
            return Err(MdnsError::InvalidConfig);
        }
        // Reuse only canonical label validation, not a real interface incarnation or identity.
        LanSource::new(1, instance).map_err(|_| MdnsError::InvalidConfig)?;
        LanSource::new(1, host_label).map_err(|_| MdnsError::InvalidConfig)?;
        advertisement.lan_announcement_remaining(endpoint, now)?;
        let txt = encode_txt(advertisement)?;
        let mut instance_name = vec![instance.len() as u8];
        instance_name.extend_from_slice(instance.as_bytes());
        instance_name.extend_from_slice(SERVICE);
        let mut host = vec![host_label.len() as u8];
        host.extend_from_slice(host_label.as_bytes());
        host.extend_from_slice(b"\x05local\0");
        Ok(Self {
            instance: instance_name,
            host,
            endpoint,
            advertisement: advertisement.clone(),
            txt,
            prepared_at: now,
            ttl,
        })
    }

    /// Encode four Answer records with one fresh supplied time sample and bounded remaining TTL.
    /// Samples before construction fail; successive calls do NOT provide clock-rollback detection.
    /// Returned bytes can become stale immediately: they are not reusable sending authorization.
    pub fn encode_at(&self, now: u64) -> Result<Vec<u8>> {
        self.packet(self.ttl_at(now)?)
    }

    fn ttl_at(&self, now: u64) -> Result<u32> {
        if now < self.prepared_at {
            return Err(MdnsError::ClockInvalid);
        }
        let remaining = self
            .advertisement
            .lan_announcement_remaining(self.endpoint, now)?;
        Ok(u64::from(self.ttl).min(remaining) as u32)
    }

    pub(super) fn probe_remaining(&self, local: Ipv4Addr, now: u64) -> Result<u64> {
        if !self.matches_probe_source(local) {
            return Err(MdnsError::InvalidConfig);
        }
        if now < self.prepared_at {
            return Err(MdnsError::ClockInvalid);
        }
        Ok(self
            .advertisement
            .lan_announcement_remaining(self.endpoint, now)?)
    }

    pub(super) fn matches_probe_source(&self, local: Ipv4Addr) -> bool {
        *self.endpoint.ip() == local
    }

    /// Preserve the entire frozen record set, changing only TTLs to zero, even after F0 expiry.
    /// This does not establish that any record was sent, owned, removed or accepted by a peer.
    pub fn encode_goodbye(&self) -> Result<Vec<u8>> {
        self.packet(0)
    }

    fn packet(&self, ttl: u32) -> Result<Vec<u8>> {
        let mut packet = Packet(Vec::new());
        // Unsolicited-announcement layout (RFC 6762 section 8.3), not a PTR query-response layout.
        packet.append(&[0, 0, 0x84, 0, 0, 0, 0, 4, 0, 0, 0, 0])?;
        packet.record(SERVICE, 12, 1, ttl, &self.instance)?;
        self.unique_records(&mut packet, 0x8001, ttl)?;
        Ok(packet.0)
    }

    fn unique_records(&self, packet: &mut Packet, class: u16, ttl: u32) -> Result<()> {
        let mut srv = vec![0, 0, 0, 0];
        srv.extend_from_slice(&self.endpoint.port().to_be_bytes());
        srv.extend_from_slice(&self.host);
        // Cache-flush bits are only wire fields: no uniqueness probe has been performed here.
        packet.record(&self.instance, 33, class, ttl, &srv)?;
        packet.record(&self.instance, 16, class, ttl, &self.txt)?;
        packet.record(&self.host, 1, class, ttl, &self.endpoint.ip().octets())
    }
}

/// Check each append before reserve/copy. A failed packet never leaves this module as a prefix.
struct Packet(Vec<u8>);

impl Packet {
    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        if self
            .0
            .len()
            .checked_add(bytes.len())
            .is_none_or(|length| length > MAX_MDNS_PACKET_BYTES)
        {
            return Err(MdnsError::TooLarge);
        }
        self.0
            .try_reserve_exact(bytes.len())
            .map_err(|_| MdnsError::TooLarge)?;
        self.0.extend_from_slice(bytes);
        Ok(())
    }

    fn record(&mut self, owner: &[u8], kind: u16, class: u16, ttl: u32, data: &[u8]) -> Result<()> {
        let length = u16::try_from(data.len()).map_err(|_| MdnsError::TooLarge)?;
        self.append(owner)?;
        self.append(&kind.to_be_bytes())?;
        self.append(&class.to_be_bytes())?;
        self.append(&ttl.to_be_bytes())?;
        self.append(&length.to_be_bytes())?;
        self.append(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_accepts_exact_packet_bound_but_rejects_one_more_without_mutation() {
        let mut packet = Packet(Vec::new());
        packet.append(&vec![0; MAX_MDNS_PACKET_BYTES]).unwrap();
        assert_eq!(packet.0.len(), MAX_MDNS_PACKET_BYTES);
        assert_eq!(packet.append(&[1]), Err(MdnsError::TooLarge));
        assert_eq!(packet.0, vec![0; MAX_MDNS_PACKET_BYTES]);
    }

    #[test]
    fn oversized_first_append_does_not_reserve_or_copy_a_prefix() {
        let mut packet = Packet(Vec::new());
        assert_eq!(
            packet.append(&vec![0; MAX_MDNS_PACKET_BYTES + 1]),
            Err(MdnsError::TooLarge)
        );
        assert!(packet.0.is_empty());
        assert_eq!(packet.0.capacity(), 0);
    }

    #[test]
    fn incremental_fragments_share_one_total_packet_bound() {
        let mut packet = Packet(Vec::new());
        packet.append(&vec![0; MAX_MDNS_PACKET_BYTES - 1]).unwrap();
        packet.append(&[2]).unwrap();
        assert_eq!(packet.append(&[3]), Err(MdnsError::TooLarge));
        assert_eq!(packet.0.last(), Some(&2));
        assert_eq!(packet.0.len(), MAX_MDNS_PACKET_BYTES);
    }
}
