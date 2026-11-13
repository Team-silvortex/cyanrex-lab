//! Shared basic IN wire validation and frozen-record equality, without receive-side policy.
use super::{MdnsAnnouncement, MdnsError, Result, SERVICE};
use crate::services::federation_mdns::wire::{self, Name, Record};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecordKind {
    Ptr,
    Srv,
    Txt,
    A,
}

impl RecordKind {
    pub(super) fn code(self) -> u16 {
        match self {
            Self::Ptr => 12,
            Self::Srv => 33,
            Self::Txt => 16,
            Self::A => 1,
        }
    }

    pub(super) fn from_code(code: u16) -> Option<Self> {
        match code {
            12 => Some(Self::Ptr),
            33 => Some(Self::Srv),
            16 => Some(Self::Txt),
            1 => Some(Self::A),
            _ => None,
        }
    }
}

pub(super) enum MatchRecord {
    Unrelated,
    Equal(RecordKind),
    Different(RecordKind),
}

pub(super) struct FrozenRecords<'a> {
    announcement: &'a MdnsAnnouncement,
    service: Name,
    instance: Name,
    host: Name,
}

impl<'a> FrozenRecords<'a> {
    pub(super) fn new(announcement: &'a MdnsAnnouncement) -> Result<Self> {
        Ok(Self {
            announcement,
            service: wire::complete_name(SERVICE, 0, SERVICE.len())?,
            instance: wire::complete_name(&announcement.instance, 0, announcement.instance.len())?,
            host: wire::complete_name(&announcement.host, 0, announcement.host.len())?,
        })
    }

    pub(super) fn owner(&self, kind: RecordKind) -> &Name {
        match kind {
            RecordKind::Ptr => &self.service,
            RecordKind::Srv | RecordKind::Txt => &self.instance,
            RecordKind::A => &self.host,
        }
    }

    /// Validate all known IN data, including unrelated owners, before comparing the frozen tuple.
    pub(super) fn match_record(&self, bytes: &[u8], record: &Record) -> Result<MatchRecord> {
        let Some(kind) = RecordKind::from_code(record.kind).filter(|_| record.class() == 1) else {
            return Ok(MatchRecord::Unrelated);
        };
        let data = &bytes[record.start..record.end];
        let equal = match kind {
            RecordKind::A => {
                let address: [u8; 4] = data.try_into().map_err(|_| MdnsError::InvalidPacket)?;
                address == self.announcement.endpoint.ip().octets()
            }
            RecordKind::Ptr => {
                wire::complete_name(bytes, record.start, record.end)? == self.instance
            }
            RecordKind::Txt => {
                validate_txt(data)?;
                // TXT includes original string boundaries, order and case; no F0 decoding.
                data == self.announcement.txt.as_slice()
            }
            RecordKind::Srv => {
                // Every u16 priority/weight/port and every valid target, including root, is syntax.
                if data.len() < 7 {
                    return Err(MdnsError::InvalidPacket);
                }
                let target = wire::complete_name(bytes, record.start + 6, record.end)?;
                wire::word(bytes, record.start)? == 0
                    && wire::word(bytes, record.start + 2)? == 0
                    && wire::word(bytes, record.start + 4)? == self.announcement.endpoint.port()
                    && target == self.host
            }
        };
        Ok(if record.owner != *self.owner(kind) {
            MatchRecord::Unrelated
        } else if equal {
            MatchRecord::Equal(kind)
        } else {
            MatchRecord::Different(kind)
        })
    }
}

// Empty TXT is valid. Nonempty DNS strings must consume exactly the entire RDATA interval.
fn validate_txt(bytes: &[u8]) -> Result<()> {
    let mut offset = 0_usize;
    while offset < bytes.len() {
        offset = offset
            .checked_add(1 + usize::from(bytes[offset]))
            .ok_or(MdnsError::InvalidPacket)?;
        if offset > bytes.len() {
            return Err(MdnsError::InvalidPacket);
        }
    }
    Ok(())
}
