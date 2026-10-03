//! Pure consistency checks for supplied Task content snapshots, never authorization or storage.
use crate::models::collaboration::{
    ArtifactContent, ContractError, PrincipalRef, TaskContentManifest, TaskSnapshot,
    TextPayloadBinding, MAX_TEXT_PAYLOAD_BYTES,
};
use sha2::{Digest, Sha256};

/// Validates supplied snapshots only; it does not establish provenance or a current Session.
/// A caller must still use the authorized Session transaction for reads and writes. Passing this
/// check does not prove persistence, current membership or approval, and cannot be cached as a grant.
/// Filenames and languages are inert labels, never paths or instructions to execute content.
pub fn validate_task_content_snapshot(
    manifest: &TaskContentManifest,
    task: &TaskSnapshot,
    contents: &[ArtifactContent],
) -> Result<(), ContractError> {
    manifest.validate_task_snapshot(task)?;
    if contents.len() != manifest.payload().len() {
        return Err(ContractError(
            "Task content must match every ordered payload binding",
        ));
    }
    for (item, content) in manifest.payload().iter().zip(contents) {
        validate_text_payload_content(item, task.owner, content)?;
    }
    Ok(())
}

/// One bounded item at a time for transaction-owned adapters; still not authorization by itself.
pub(crate) fn validate_text_payload_content(
    item: &TextPayloadBinding,
    owner: PrincipalRef,
    content: &ArtifactContent,
) -> Result<(), ContractError> {
    if content.revision.reference != *item.artifact() || content.revision.owner != owner {
        return Err(ContractError(
            "Task content must match its exact revision and owner",
        ));
    }
    if content.bytes.len() > MAX_TEXT_PAYLOAD_BYTES
        || content.revision.byte_length != content.bytes.len() as u64
    {
        return Err(ContractError(
            "Task text content has an invalid byte length",
        ));
    }
    let text = std::str::from_utf8(&content.bytes)
        .map_err(|_| ContractError("Task text content must be valid UTF-8"))?;
    if text
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\t' | '\n' | '\r'))
    {
        return Err(ContractError(
            "Task text content contains a forbidden control character",
        ));
    }
    if format!("{:x}", Sha256::digest(&content.bytes)) != item.artifact().sha256.as_str() {
        return Err(ContractError(
            "Task text content does not match its pinned digest",
        ));
    }
    Ok(())
}
