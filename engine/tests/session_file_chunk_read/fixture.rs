pub use super::identity_fixture::scope;
use super::*;

pub struct Fixture {
    pub sharing: sharing_fixture::Fixture,
    pub workspace: SessionFileChunkReadWorkspace,
    pub staging: ChunkStaging,
    pub capacity: usize,
    pools: std::sync::Mutex<Vec<PgPool>>,
}

impl Fixture {
    pub async fn ready(capacity: usize) -> Self {
        let sharing = sharing_fixture::Fixture::ready().await;
        sharing
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
            .expect("real Session sharing must prepare the byte-read fixture");
        let metadata = sharing
            .base
            .grants
            .auth
            .source
            .file_download_workspace(
                sharing.base.publication.clone(),
                sharing.recipient_workspace.clone(),
            )
            .unwrap();
        let staging = ChunkStaging::open(scope(), &sharing.base.root, 2).unwrap();
        let workspace = sharing
            .base
            .grants
            .auth
            .source
            .file_chunk_read_workspace(metadata, staging.clone(), capacity)
            .unwrap();
        Self {
            sharing,
            workspace,
            staging,
            capacity,
            pools: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn request(&self) -> SessionFileChunkReadRequest {
        SessionFileChunkReadRequest {
            download: SessionFileDownloadRequest {
                grant_id: sharing_fixture::recipient_grant(1),
                upload_id: publication_fixture::upload(1),
                expected_file: self.sharing.expected_file.clone(),
            },
            chunk_index: 0,
        }
    }

    pub async fn cleanup(self) {
        for pool in self.pools.into_inner().unwrap() {
            pool.close().await;
        }
        self.sharing.cleanup().await;
    }
    pub fn track(&self, pool: PgPool) {
        self.pools.lock().unwrap().push(pool);
    }
}
