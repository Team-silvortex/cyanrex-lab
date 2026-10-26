use crate::{
    models::collaboration::{
        ArtifactContent, ArtifactId, ArtifactRevision, ArtifactRevisionId, CoreSchemaVersion,
    },
    platform_http::errors::HttpError,
    services::{
        artifact_store::{validate_artifact_metadata, ArtifactDraft},
        task_content::validate_text_bytes,
    },
};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

pub(super) const TEXT_KIND: &str = "cyanrex.task-text";
pub(super) const TEXT_MEDIA: &str = "text/plain";
pub(in crate::platform_http) const MAX_WIRE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::platform_http) struct Create {
    pub schema_version: CoreSchemaVersion,
    pub artifact_id: ArtifactId,
    pub revision_id: ArtifactRevisionId,
    title: String,
    text: String,
}
impl Create {
    /// Shared wire profile, validated before either backend dispatch without cloning text bytes.
    pub(in crate::platform_http) fn into_text_parts(self) -> Result<(String, String), HttpError> {
        validate_text_bytes(self.text.as_bytes()).map_err(|_| HttpError::invalid_request())?;
        validate_artifact_metadata(&self.title, TEXT_MEDIA, self.text.len() as u64)
            .map_err(|_| HttpError::invalid_request())?;
        Ok((self.title, self.text))
    }

    pub(super) fn into_draft(self) -> Result<ArtifactDraft, HttpError> {
        validate_text_bytes(self.text.as_bytes()).map_err(|_| HttpError::invalid_request())?;
        ArtifactDraft::new(
            TEXT_KIND
                .parse()
                .map_err(|_| HttpError::invalid_request())?,
            self.title,
            TEXT_MEDIA,
            self.text.into_bytes(),
        )
        .map_err(|_| HttpError::invalid_request())
    }
}

#[derive(Serialize)]
pub(in crate::platform_http) struct TextResponse {
    schema_version: u8,
    artifact: ArtifactRevision,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}
impl From<ArtifactRevision> for TextResponse {
    fn from(artifact: ArtifactRevision) -> Self {
        Self {
            schema_version: 1,
            artifact,
            text: None,
        }
    }
}
impl TryFrom<ArtifactContent> for TextResponse {
    type Error = HttpError;
    fn try_from(content: ArtifactContent) -> Result<Self, Self::Error> {
        let invalid =
            || HttpError::dispatched(StatusCode::SERVICE_UNAVAILABLE, "service_unavailable");
        // Session command already checked exact reference/owner/digest and confirmed its commit.
        // This narrower projection does not relabel arbitrary generic bytes as a text Artifact.
        if content.revision.kind.as_str() != TEXT_KIND
            || content.revision.media_type != TEXT_MEDIA
            || content.revision.byte_length != content.bytes.len() as u64
            || validate_text_bytes(&content.bytes).is_err()
        {
            return Err(invalid());
        }
        let text = String::from_utf8(content.bytes).map_err(|_| invalid())?;
        Ok(Self {
            schema_version: 1,
            artifact: content.revision,
            text: Some(text),
        })
    }
}
