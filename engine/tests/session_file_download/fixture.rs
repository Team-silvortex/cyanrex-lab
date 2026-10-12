pub use super::identity_fixture::scope;
use super::*;

pub struct Fixture {
    pub sharing: sharing_fixture::Fixture,
    pub workspace: SessionFileDownloadWorkspace,
    pub issued: StoredFileRecipientGrant,
}

impl Fixture {
    pub async fn ready() -> Self {
        // Existing helpers create real bytes, confirmed chunks, current Session publication,
        // an active publisher grant and a separately audited role-free Human recipient.
        let sharing = sharing_fixture::Fixture::ready().await;
        let issued = sharing
            .base
            .grants
            .auth
            .source
            .issue_session_file_recipient_grant(
                &sharing.base.grants.auth.learner_token,
                &sharing.workspace,
                &sharing.command(1),
            )
            .await
            .expect("real Session sharing must prepare the download-manifest fixture");
        let workspace = sharing
            .base
            .grants
            .auth
            .source
            .file_download_workspace(
                sharing.base.publication.clone(),
                sharing.recipient_workspace.clone(),
            )
            .unwrap();
        Self {
            sharing,
            workspace,
            issued,
        }
    }

    pub fn request(&self) -> SessionFileDownloadRequest {
        SessionFileDownloadRequest {
            grant_id: sharing_fixture::recipient_grant(1),
            upload_id: publication_fixture::upload(1),
            expected_file: self.sharing.expected_file.clone(),
        }
    }

    pub async fn cleanup(self) {
        self.sharing.cleanup().await;
    }
}
