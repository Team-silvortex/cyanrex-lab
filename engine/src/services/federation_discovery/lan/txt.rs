//! A compact application envelope inside one raw DNS-SD TXT RDATA, not a DNS packet decoder.
//! Never concatenate different records, resolve addresses or truncate a valid F0 announcement.
use super::LanDiscoveryError;
use crate::services::federation_discovery::PeerAdvertisement;

pub const MAX_TXT_RECORD_BYTES: usize = 1_200;
pub const MAX_TXT_CHUNK_BYTES: usize = 200;
pub const MAX_TXT_CHUNKS: usize = 6;
type Result<T> = std::result::Result<T, LanDiscoveryError>;

fn append(record: &mut Vec<u8>, value: &[u8]) -> Result<()> {
    if value.len() > 255 || record.len() + 1 + value.len() > MAX_TXT_RECORD_BYTES {
        return Err(LanDiscoveryError::TooLarge);
    }
    record.push(value.len() as u8);
    record.extend_from_slice(value);
    Ok(())
}

/// Canonical lower-case keys; oversized F0 values fail instead of losing capabilities or hints.
pub fn encode_txt(advertisement: &PeerAdvertisement) -> Result<Vec<u8>> {
    let bytes = advertisement
        .encode()
        .map_err(LanDiscoveryError::RejectedAdvertisement)?;
    let count = bytes.len().div_ceil(MAX_TXT_CHUNK_BYTES);
    if count == 0 || count > MAX_TXT_CHUNKS {
        return Err(LanDiscoveryError::TooLarge);
    }
    let mut record = Vec::new();
    append(&mut record, b"v=1")?;
    append(&mut record, &[b'c', b'=', b'0' + count as u8])?;
    for (index, bytes) in bytes.chunks(MAX_TXT_CHUNK_BYTES).enumerate() {
        let mut field = vec![b'd', b'0' + index as u8, b'='];
        field.extend_from_slice(bytes);
        append(&mut record, &field)?;
    }
    Ok(record)
}

/// Accept field order and ASCII key case, reject duplicates before any map can erase them.
pub fn decode_txt(record: &[u8]) -> Result<PeerAdvertisement> {
    if record.len() > MAX_TXT_RECORD_BYTES {
        return Err(LanDiscoveryError::TooLarge);
    }
    let invalid = || LanDiscoveryError::InvalidTxt;
    let mut version = false;
    let mut count = None;
    let mut chunks = [None; MAX_TXT_CHUNKS];
    let mut offset = 0;
    let mut fields = 0;
    while offset < record.len() {
        fields += 1;
        if fields > MAX_TXT_CHUNKS + 2 {
            return Err(invalid());
        }
        let length = record[offset] as usize;
        offset += 1;
        let field = record.get(offset..offset + length).ok_or_else(invalid)?;
        offset += length;
        let equal = field
            .iter()
            .position(|byte| *byte == b'=')
            .ok_or_else(invalid)?;
        if !(1..=2).contains(&equal) {
            return Err(invalid());
        }
        let key = field[..equal].to_ascii_lowercase();
        let value = &field[equal + 1..];
        match key.as_slice() {
            b"v" if !version && value == b"1" => version = true,
            b"c" if count.is_none() && value.len() == 1 && (b'1'..=b'6').contains(&value[0]) => {
                count = Some((value[0] - b'0') as usize);
            }
            [b'd', index @ b'0'..=b'5']
                if !value.is_empty() && value.len() <= MAX_TXT_CHUNK_BYTES =>
            {
                let slot = &mut chunks[(index - b'0') as usize];
                if slot.replace(value).is_some() {
                    return Err(invalid());
                }
            }
            _ => return Err(invalid()),
        }
    }
    let count = count.filter(|_| version).ok_or_else(invalid)?;
    let mut bytes = Vec::new();
    for (index, chunk) in chunks.into_iter().enumerate() {
        if index >= count {
            if chunk.is_some() {
                return Err(invalid());
            }
            continue;
        }
        let chunk = chunk.ok_or_else(invalid)?;
        if index + 1 < count && chunk.len() != MAX_TXT_CHUNK_BYTES {
            return Err(invalid());
        }
        bytes.extend_from_slice(chunk);
    }
    PeerAdvertisement::parse(&bytes).map_err(LanDiscoveryError::RejectedAdvertisement)
}
