use super::*;

/// Service-created draft only: no client-supplied digest, path, owner, parent or deserialization.
pub struct ArtifactDraft {
    pub(super) kind: QualifiedName,
    pub(super) title: String,
    pub(super) media_type: String,
    pub(super) bytes: Vec<u8>,
}
impl ArtifactDraft {
    pub fn new(
        kind: QualifiedName,
        title: impl Into<String>,
        media_type: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self> {
        let draft = Self {
            kind,
            title: title.into(),
            media_type: media_type.into(),
            bytes,
        };
        validate_metadata(&draft.title, &draft.media_type, draft.bytes.len() as u64)?;
        Ok(draft)
    }
}
pub(super) fn validate_metadata(title: &str, media: &str, length: u64) -> Result<()> {
    let token = |value: &str| {
        !value.is_empty()
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"!#$&^_.+-".contains(&b))
    };
    let valid_media = media.len() <= 127
        && media
            .split_once('/')
            .is_some_and(|(kind, subtype)| token(kind) && token(subtype));
    if title.trim().is_empty()
        || title.len() > 256
        || title.chars().any(char::is_control)
        || length > MAX_CONTENT as u64
        || !valid_media
    {
        return Err(ArtifactStoreError::InvalidInput);
    }
    Ok(())
}
