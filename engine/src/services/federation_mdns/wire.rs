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

/// Decode exactly one encoded interval while retaining full-packet compression offsets.
pub(super) fn complete_name(bytes: &[u8], start: usize, end: usize) -> Result<Name> {
    if start >= end || end > bytes.len() {
        return invalid();
    }
    let mut offset = start;
    let result = name(bytes, &mut offset)?;
    if offset != end {
        return invalid();
    }
    Ok(result)
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

    #[test]
    fn complete_name_accepts_root_casefolding_and_distinct_labels() {
        for (bytes, labels) in [
            (b"\0".as_slice(), vec![]),
            (b"\x04NoDe\x05LoCaL\0".as_slice(), vec!["node", "local"]),
            (b"\x03a.b\x05local\0".as_slice(), vec!["a.b", "local"]),
        ] {
            assert_eq!(
                complete_name(bytes, 0, bytes.len()),
                Ok(Name(labels.into_iter().map(str::to_owned).collect()))
            );
        }
    }

    #[test]
    fn complete_name_accepts_an_exact_nonzero_absolute_interval() {
        let bytes = b"unrelated\x04node\x05local\0trailing";
        assert_eq!(
            complete_name(bytes, 9, 21),
            Ok(Name(vec!["node".into(), "local".into()]))
        );
    }

    #[test]
    fn complete_name_backward_compression_uses_full_packet_and_original_encoded_end() {
        let mut bytes = vec![0; 12];
        bytes.extend_from_slice(b"\x05LoCaL\0\x04NoDe\xc0\x0c\0");
        assert_eq!(
            complete_name(&bytes, 19, 26),
            Ok(Name(vec!["node".into(), "local".into()]))
        );
        assert_eq!(
            complete_name(&bytes, 24, 26),
            Ok(Name(vec!["local".into()]))
        );
    }

    #[test]
    fn complete_name_requires_exact_plain_and_compressed_endpoints() {
        let bytes = b"\x05local\0\x04node\xc0\0\0";
        for (start, end) in [(0, 7), (7, 14)] {
            assert!(complete_name(bytes, start, end).is_ok());
            for wrong_end in [end - 1, end + 1] {
                assert_eq!(
                    complete_name(bytes, start, wrong_end),
                    Err(MdnsError::InvalidPacket)
                );
            }
        }
    }

    #[test]
    fn complete_name_rejects_every_truncated_label_or_pointer_prefix() {
        for (bytes, start) in [
            (b"\x04node\x05local\0".as_slice(), 0),
            (b"\x01a\0\xc0\0".as_slice(), 3),
        ] {
            for end in start..bytes.len() {
                assert_eq!(
                    complete_name(&bytes[..end], start, end),
                    Err(MdnsError::InvalidPacket),
                    "accepted truncated interval {start}..{end}"
                );
            }
        }
    }

    #[test]
    fn complete_name_rejects_multiple_names_in_one_interval() {
        let bytes = b"\x01a\0\x01b\0";
        assert_eq!(complete_name(bytes, 0, 3), Ok(Name(vec!["a".into()])));
        assert_eq!(complete_name(bytes, 3, 6), Ok(Name(vec!["b".into()])));
        assert_eq!(
            complete_name(bytes, 0, bytes.len()),
            Err(MdnsError::InvalidPacket)
        );
    }

    #[test]
    fn complete_name_rejects_forward_self_outside_and_cyclic_pointers() {
        for bytes in [
            b"\xc0\0".as_slice(),
            b"\xc0\x02\0".as_slice(),
            b"\xc0\xff".as_slice(),
            b"\x01a\xc0\0".as_slice(),
        ] {
            assert_eq!(
                complete_name(bytes, 0, bytes.len()),
                Err(MdnsError::InvalidPacket)
            );
        }
    }

    #[test]
    fn complete_name_preserves_exact_expanded_dns_name_bound() {
        for (last, valid) in [(61, true), (62, false)] {
            let mut bytes = Vec::new();
            for length in [63, 63, 63, last] {
                bytes.push(length);
                bytes.extend(vec![b'a'; usize::from(length)]);
            }
            bytes.push(0);
            let result = complete_name(&bytes, 0, bytes.len());
            if valid {
                assert_eq!(bytes.len(), 255);
                assert_eq!(
                    result
                        .unwrap()
                        .0
                        .iter()
                        .map(String::len)
                        .collect::<Vec<_>>(),
                    [63, 63, 63, 61]
                );
            } else {
                assert_eq!(result, Err(MdnsError::InvalidPacket));
            }
        }
    }

    #[test]
    fn complete_name_rejects_control_nonascii_and_reserved_label_encodings() {
        for bytes in [
            b"\x01\x1f\0".as_slice(),
            b"\x01\x80\0".as_slice(),
            b"\x40\0".as_slice(),
            b"\x80\0".as_slice(),
        ] {
            assert_eq!(
                complete_name(bytes, 0, bytes.len()),
                Err(MdnsError::InvalidPacket)
            );
        }
    }

    #[test]
    fn complete_name_rejects_empty_reversed_or_outside_intervals_without_panicking() {
        let bytes = b"\x01a\0";
        for (start, end) in [
            (0, 0),
            (1, 0),
            (3, 3),
            (4, 3),
            (0, 4),
            (usize::MAX, 3),
            (0, usize::MAX),
            (usize::MAX, usize::MAX),
        ] {
            assert_eq!(
                complete_name(bytes, start, end),
                Err(MdnsError::InvalidPacket),
                "accepted invalid interval {start}..{end}"
            );
        }
        assert_eq!(complete_name(&[], 0, 0), Err(MdnsError::InvalidPacket));
    }

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
