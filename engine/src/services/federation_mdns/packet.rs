//! Conservative single-packet DNS-SD profile; never resolve, cache or authenticate a peer.
use super::MdnsError;
use crate::services::federation_discovery::lan::{decode_txt, LanDiscoveryError, LanSource};
use std::net::Ipv4Addr;

pub const MAX_MDNS_PACKET_BYTES: usize = 4_096;
pub const MAX_MDNS_RECORDS: usize = 64;
const MAX_EVENTS: usize = 8;
const MAX_ADDRESSES: usize = 8;
const MAX_NAME_JUMPS: usize = 128;
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

#[derive(Clone, PartialEq, Eq)]
struct Name(Vec<String>);

struct Record {
    owner: Name,
    kind: u16,
    class: u16,
    ttl: u32,
    start: usize,
    end: usize,
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

fn invalid<T>() -> Result<T> {
    Err(MdnsError::InvalidPacket)
}

fn word(bytes: &[u8], offset: usize) -> Result<u16> {
    let field = bytes
        .get(offset..offset + 2)
        .ok_or(MdnsError::InvalidPacket)?;
    Ok(u16::from_be_bytes([field[0], field[1]]))
}

fn name(bytes: &[u8], offset: &mut usize) -> Result<Name> {
    let mut cursor = *offset;
    let mut original_end = None;
    let mut labels = Vec::new();
    let mut expanded = 1_usize;
    let mut jumps = 0;
    loop {
        let length = *bytes.get(cursor).ok_or(MdnsError::InvalidPacket)?;
        if length & 0xc0 == 0xc0 {
            let target = usize::from(word(bytes, cursor)? & 0x3fff);
            // This profile accepts only a prior occurrence, not forward references or cycles.
            if target >= cursor || jumps >= MAX_NAME_JUMPS {
                return invalid();
            }
            original_end.get_or_insert(cursor + 2);
            cursor = target;
            jumps += 1;
            continue;
        }
        if length & 0xc0 != 0 {
            return invalid();
        }
        cursor += 1;
        if length == 0 {
            *offset = original_end.unwrap_or(cursor);
            return Ok(Name(labels));
        }
        expanded += 1 + usize::from(length);
        if expanded > 255 {
            return invalid();
        }
        let label = bytes
            .get(cursor..cursor + usize::from(length))
            .ok_or(MdnsError::InvalidPacket)?;
        if !label
            .iter()
            .all(|byte| byte.is_ascii() && !byte.is_ascii_control())
        {
            return invalid();
        }
        // Keep labels separate: a dot inside a label must not turn into a matching DNS suffix.
        labels.push(
            String::from_utf8(label.to_ascii_lowercase()).map_err(|_| MdnsError::InvalidPacket)?,
        );
        cursor += usize::from(length);
    }
}

fn read_records(bytes: &[u8]) -> Result<Vec<Record>> {
    if bytes.len() > MAX_MDNS_PACKET_BYTES {
        return Err(MdnsError::TooLarge);
    }
    if bytes.len() < 12 {
        return invalid();
    }
    let flags = word(bytes, 2)?;
    let questions = usize::from(word(bytes, 4)?);
    let count =
        usize::from(word(bytes, 6)?) + usize::from(word(bytes, 8)?) + usize::from(word(bytes, 10)?);
    if word(bytes, 0)? != 0
        || flags & 0x8000 == 0
        || flags & 0x7800 != 0
        || flags & 0x0200 != 0
        || flags & 0x000f != 0
        || questions > 8
        || count > MAX_MDNS_RECORDS
    {
        return invalid();
    }
    let mut offset = 12;
    for _ in 0..questions {
        name(bytes, &mut offset)?;
        offset = offset.checked_add(4).ok_or(MdnsError::InvalidPacket)?;
        if offset > bytes.len() {
            return invalid();
        }
    }
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        let owner = name(bytes, &mut offset)?;
        let kind = word(bytes, offset)?;
        let class = word(bytes, offset + 2)? & 0x7fff;
        let ttl_bytes = bytes
            .get(offset + 4..offset + 8)
            .ok_or(MdnsError::InvalidPacket)?;
        let ttl = u32::from_be_bytes([ttl_bytes[0], ttl_bytes[1], ttl_bytes[2], ttl_bytes[3]]);
        let length = usize::from(word(bytes, offset + 8)?);
        let start = offset + 10;
        let end = start.checked_add(length).ok_or(MdnsError::InvalidPacket)?;
        if end > bytes.len() {
            return invalid();
        }
        records.push(Record {
            owner,
            kind,
            class,
            ttl,
            start,
            end,
        });
        offset = end;
    }
    if offset != bytes.len() {
        return invalid();
    }
    Ok(records)
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
        .filter(|record| record.class == 1 && record.kind == kind && record.owner == *owner);
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
        .filter(|record| record.class == 1 && record.kind == 1 && record.owner == *target)
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
        .filter(|record| record.class == 1 && record.kind == 12 && record.owner == service_name())
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
