//! Conservative single-packet DNS-SD profile; never resolve, cache or authenticate a peer.
use super::{
    wire::{invalid, name, read_message, word, Name, Record},
    MdnsError,
};
use crate::services::federation_discovery::lan::{decode_txt, LanDiscoveryError, LanSource};
use std::net::Ipv4Addr;

pub const MAX_MDNS_PACKET_BYTES: usize = 4_096;
pub const MAX_MDNS_RECORDS: usize = 64;
const MAX_EVENTS: usize = 8;
const MAX_ADDRESSES: usize = 8;
type Result<T> = std::result::Result<T, MdnsError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdnsEvent {
    Resolved {
        instance: String,
        advertisement: Vec<u8>,
        addresses: Vec<Ipv4Addr>,
        port: u16,
        ttl: u32,
    },
    Removed {
        instance: String,
    },
}

struct Service {
    target: Name,
    port: u16,
    ttl: u32,
}

/// One ID-zero PTR/IN question. The caller owns explicit interface selection and all socket work.
pub fn browse_query() -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    bytes.extend_from_slice(b"\x08_cyanrex\x04_udp\x05local\0\0\x0c\0\x01");
    bytes
}

fn read_records(bytes: &[u8]) -> Result<Vec<Record>> {
    if bytes.len() > MAX_MDNS_PACKET_BYTES {
        return Err(MdnsError::TooLarge);
    }
    if bytes.len() < 12 {
        return invalid();
    }
    let flags = word(bytes, 2)?;
    if word(bytes, 0)? != 0
        || flags & 0x8000 == 0
        || flags & 0x7800 != 0
        || flags & 0x0200 != 0
        || flags & 0x000f != 0
    {
        return invalid();
    }
    // Candidate assembly intentionally keeps its pre-existing flattened-section semantics.
    Ok(read_message(bytes)?.records)
}

fn service_name() -> Name {
    Name(vec!["_cyanrex".into(), "_udp".into(), "local".into()])
}

fn instance_name(target: &Name) -> Result<String> {
    if target.0.len() != 4 || target.0[1..] != service_name().0 {
        return invalid();
    }
    let instance = &target.0[0];
    LanSource::new(1, instance).map_err(|_| MdnsError::InvalidPacket)?;
    Ok(instance.clone())
}

fn rdata_name(bytes: &[u8], record: &Record, start: usize) -> Result<Name> {
    let mut offset = start;
    let result = name(bytes, &mut offset)?;
    if offset != record.end {
        return invalid();
    }
    Ok(result)
}

fn unique_record<'a>(records: &'a [Record], owner: &Name, kind: u16) -> Result<Option<&'a Record>> {
    let mut matching = records
        .iter()
        .filter(|record| record.class() == 1 && record.kind == kind && record.owner == *owner);
    let first = matching.next();
    if matching.next().is_some() {
        return invalid();
    }
    Ok(first)
}

fn srv(bytes: &[u8], record: &Record) -> Result<Service> {
    if record.end - record.start < 7
        || word(bytes, record.start)? != 0
        || word(bytes, record.start + 2)? != 0
    {
        return invalid();
    }
    let port = word(bytes, record.start + 4)?;
    let target = rdata_name(bytes, record, record.start + 6)?;
    let alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    if port == 0
        || target.0.len() < 2
        || target.0.last().is_none_or(|label| label != "local")
        || target.0[..target.0.len() - 1].iter().any(|label| {
            !label.bytes().next().is_some_and(alphanumeric)
                || !label.bytes().last().is_some_and(alphanumeric)
                || !label.bytes().all(|byte| alphanumeric(byte) || byte == b'-')
        })
    {
        return invalid();
    }
    Ok(Service {
        target,
        port,
        ttl: record.ttl,
    })
}

fn addresses(records: &[Record], bytes: &[u8], target: &Name) -> Result<(Vec<Ipv4Addr>, u32)> {
    let mut addresses = Vec::new();
    let mut minimum_ttl = u32::MAX;
    for record in records
        .iter()
        .filter(|record| record.class() == 1 && record.kind == 1 && record.owner == *target)
    {
        let data = &bytes[record.start..record.end];
        if data.len() != 4 {
            return invalid();
        }
        let address = Ipv4Addr::new(data[0], data[1], data[2], data[3]);
        if addresses.contains(&address) || addresses.len() >= MAX_ADDRESSES {
            return invalid();
        }
        addresses.push(address);
        minimum_ttl = minimum_ttl.min(record.ttl);
    }
    Ok((addresses, minimum_ttl))
}

/// Assemble only complete bounded records from this packet. This never imports prior DNS state.
pub fn parse_response(bytes: &[u8]) -> Result<Vec<MdnsEvent>> {
    let records = read_records(bytes)?;
    let mut targets = Vec::new();
    for record in records
        .iter()
        .filter(|record| record.class() == 1 && record.kind == 12 && record.owner == service_name())
    {
        let target = rdata_name(bytes, record, record.start)?;
        let instance = instance_name(&target)?;
        if targets.iter().any(|(existing, _, _)| existing == &target) {
            return invalid();
        }
        targets.push((target, instance, record.ttl));
    }
    let mut events = Vec::new();
    for (target, instance, ptr_ttl) in targets {
        let srv_record = unique_record(&records, &target, 33)?;
        let txt_record = unique_record(&records, &target, 16)?;
        if ptr_ttl == 0 {
            events.push(MdnsEvent::Removed { instance });
        } else if let (Some(srv_record), Some(txt_record)) = (srv_record, txt_record) {
            let service = srv(bytes, srv_record)?;
            let (addresses, a_ttl) = addresses(&records, bytes, &service.target)?;
            if addresses.is_empty() {
                continue;
            }
            let advertisement = decode_txt(&bytes[txt_record.start..txt_record.end])
                .map_err(MdnsError::RejectedTxt)?
                .encode()
                .map_err(|error| {
                    MdnsError::RejectedTxt(LanDiscoveryError::RejectedAdvertisement(error))
                })?;
            let ttl = ptr_ttl
                .min(service.ttl)
                .min(txt_record.ttl)
                .min(a_ttl)
                .min(120);
            if ttl == 0 {
                events.push(MdnsEvent::Removed { instance });
            } else {
                events.push(MdnsEvent::Resolved {
                    instance,
                    advertisement,
                    addresses,
                    port: service.port,
                    ttl,
                });
            }
        }
        if events.len() > MAX_EVENTS {
            return invalid();
        }
    }
    Ok(events)
}
