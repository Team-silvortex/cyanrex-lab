//! Strict whole-file framing; decoding and DER/key interpretation remain in rustls.
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};

pub(super) fn certificates(bytes: &[u8]) -> Result<Vec<CertificateDer<'static>>, ()> {
    frames(bytes, false)?
        .into_iter()
        .map(|frame| CertificateDer::from_pem_slice(frame).map_err(|_| ()))
        .collect()
}

pub(super) fn private_key(bytes: &[u8]) -> Result<PrivateKeyDer<'static>, ()> {
    let frames = frames(bytes, true)?;
    PrivateKeyDer::from_pem_slice(frames[0]).map_err(|_| ())
}

fn line(bytes: &[u8]) -> (&[u8], usize) {
    let consumed = bytes
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |index| index + 1);
    let line = &bytes[..consumed];
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    (line.strip_suffix(b"\r").unwrap_or(line), consumed)
}

fn frames(mut bytes: &[u8], key: bool) -> Result<Vec<&[u8]>, ()> {
    let mut frames = Vec::new();
    loop {
        bytes = bytes.trim_ascii_start();
        if bytes.is_empty() {
            break;
        }
        let (header, mut consumed) = line(bytes);
        let footer: &[u8] = match header {
            b"-----BEGIN CERTIFICATE-----" if !key => b"-----END CERTIFICATE-----",
            b"-----BEGIN PRIVATE KEY-----" if key => b"-----END PRIVATE KEY-----",
            b"-----BEGIN RSA PRIVATE KEY-----" if key => b"-----END RSA PRIVATE KEY-----",
            b"-----BEGIN EC PRIVATE KEY-----" if key => b"-----END EC PRIVATE KEY-----",
            _ => return Err(()),
        };
        let mut has_body = false;
        loop {
            if consumed == bytes.len() {
                return Err(());
            }
            let (body, length) = line(&bytes[consumed..]);
            consumed += length;
            if body == footer {
                if !has_body {
                    return Err(());
                }
                break;
            }
            if body.is_empty()
                || !body
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'+' | b'/' | b'='))
            {
                return Err(());
            }
            has_body = true;
        }
        frames.push(&bytes[..consumed]);
        if frames.len() > if key { 1 } else { 8 } {
            return Err(());
        }
        bytes = &bytes[consumed..];
    }
    if frames.is_empty() {
        Err(())
    } else {
        Ok(frames)
    }
}
