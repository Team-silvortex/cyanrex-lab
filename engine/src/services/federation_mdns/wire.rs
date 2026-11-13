//! Shared bounded DNS framing, without response, candidate or name-ownership semantics.
use super::{MdnsError, MAX_MDNS_PACKET_BYTES, MAX_MDNS_RECORDS};

const MAX_NAME_JUMPS: usize = 128;
type Result<T> = std::result::Result<T, MdnsError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Name(pub(super) Vec<String>);

pub(super) struct Question {
    pub(super) name: Name,
    pub(super) kind: u16,
    pub(super) raw_class: u16,
}

impl Question {
    pub(super) fn class(&self) -> u16 {
        self.raw_class & 0x7fff
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Answer,
    Authority,
    Additional,
}

pub(super) struct Record {
    pub(super) owner: Name,
    pub(super) kind: u16,
    pub(super) raw_class: u16,
    pub(super) ttl: u32,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) section: Section,
}

impl Record {
    pub(super) fn class(&self) -> u16 {
        self.raw_class & 0x7fff
    }
}

pub(super) struct Message {
    pub(super) id: u16,
    pub(super) flags: u16,
    pub(super) questions: Vec<Question>,
    pub(super) records: Vec<Record>,
}

pub(super) fn invalid<T>() -> Result<T> {
    Err(MdnsError::InvalidPacket)
}

pub(super) fn word(bytes: &[u8], offset: usize) -> Result<u16> {
    let field = bytes
        .get(offset..offset + 2)
        .ok_or(MdnsError::InvalidPacket)?;
    Ok(u16::from_be_bytes([field[0], field[1]]))
}

pub(super) fn name(bytes: &[u8], offset: &mut usize) -> Result<Name> {
    let mut cursor = *offset;
    let mut original_end = None;
    let mut labels = Vec::new();
    let mut expanded = 1_usize;
    let mut jumps = 0;
    loop {
        let length = *bytes.get(cursor).ok_or(MdnsError::InvalidPacket)?;
        if length & 0xc0 == 0xc0 {
            let target = usize::from(word(bytes, cursor)? & 0x3fff);
            // Accept only a prior occurrence, not forward references or cycles.
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
        // Keep labels separate: a dot in one label must not become a matching DNS suffix.
        labels.push(
            String::from_utf8(label.to_ascii_lowercase()).map_err(|_| MdnsError::InvalidPacket)?,
        );
        cursor += usize::from(length);
    }
}

/// Keep all sections and high class bits while imposing only the existing framing profile.
/// Header protocol semantics and known-type RDATA validation belong to the consuming profile.
pub(super) fn read_message(bytes: &[u8]) -> Result<Message> {
    if bytes.len() > MAX_MDNS_PACKET_BYTES {
        return Err(MdnsError::TooLarge);
    }
    if bytes.len() < 12 {
        return invalid();
    }
    let id = word(bytes, 0)?;
    let flags = word(bytes, 2)?;
    let question_count = usize::from(word(bytes, 4)?);
    let sections = [
        (Section::Answer, usize::from(word(bytes, 6)?)),
        (Section::Authority, usize::from(word(bytes, 8)?)),
        (Section::Additional, usize::from(word(bytes, 10)?)),
    ];
    let record_count = sections.iter().map(|(_, count)| count).sum::<usize>();
    if question_count > 8 || record_count > MAX_MDNS_RECORDS {
        return invalid();
    }
    let mut offset = 12;
    let mut questions = Vec::with_capacity(question_count);
    for _ in 0..question_count {
        let name = name(bytes, &mut offset)?;
        let kind = word(bytes, offset)?;
        let class = word(bytes, offset + 2)?;
        offset = offset.checked_add(4).ok_or(MdnsError::InvalidPacket)?;
        questions.push(Question {
            name,
            kind,
            raw_class: class,
        });
    }
    let mut records = Vec::with_capacity(record_count);
    for (section, count) in sections {
        for _ in 0..count {
            let owner = name(bytes, &mut offset)?;
            let kind = word(bytes, offset)?;
            let class = word(bytes, offset + 2)?;
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
                raw_class: class,
                ttl,
                start,
                end,
                section,
            });
            offset = end;
        }
    }
    if offset != bytes.len() {
        return invalid();
    }
    Ok(Message {
        id,
        flags,
        questions,
        records,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message() -> Vec<u8> {
        let mut bytes = vec![0, 7, 0x80, 0, 0, 1, 0, 1, 0, 1, 0, 1];
        bytes.extend_from_slice(b"\x01h\0\0\xff\x80\x01");
        for class in [0x8001_u16, 3, 0x8001] {
            bytes.extend_from_slice(b"\x01h\0\0\x01");
            bytes.extend_from_slice(&class.to_be_bytes());
            // An opaque one-byte A RDATA is framed here, not semantically admitted.
            bytes.extend_from_slice(&[0, 0, 0, 1, 0, 1, 42]);
        }
        bytes
    }

    #[test]
    fn framing_retains_headers_questions_raw_class_bits_and_all_record_sections() {
        let message = read_message(&message()).unwrap();
        assert_eq!((message.id, message.flags), (7, 0x8000));
        assert_eq!(message.questions.len(), 1);
        let question = &message.questions[0];
        assert_eq!(question.name, Name(vec!["h".into()]));
        assert_eq!(
            (question.kind, question.raw_class, question.class()),
            (255, 0x8001, 1)
        );
        assert_eq!(message.records.len(), 3);
        assert_eq!(
            message
                .records
                .iter()
                .map(|record| record.section)
                .collect::<Vec<_>>(),
            [Section::Answer, Section::Authority, Section::Additional]
        );
        assert_eq!(
            message
                .records
                .iter()
                .map(|record| record.raw_class)
                .collect::<Vec<_>>(),
            [0x8001, 3, 0x8001]
        );
        assert_eq!(
            message
                .records
                .iter()
                .map(Record::class)
                .collect::<Vec<_>>(),
            [1, 3, 1]
        );
    }

    #[test]
    fn later_truncation_never_returns_an_earlier_valid_record_prefix() {
        let mut bytes = message();
        bytes.pop();
        assert!(matches!(
            read_message(&bytes),
            Err(MdnsError::InvalidPacket)
        ));
    }
}
