use super::{fixture::*, support::*, *};
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStaging,
    file_upload_chunk_journal::FileUploadChunkJournal,
    file_upload_staging::{FileUploadPublisher, FileUploadStaging},
};
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};

pub struct Published {
    pub schema: String,
    pub pool: PgPool,
    pub directory: PathBuf,
    pub root: PathBuf,
    pub manifest: FileTransferManifest,
}
impl Published {
    /// A real trusted empty-file publication setup, not a synthetic row or Session publication.
    pub async fn empty(f: &Fixture, token: &str, seed: u128) -> Self {
        let manifest = manifest(0, seed);
        reserve(f, token, seed, &manifest).await.unwrap();
        let schema = format!("session_metadata_journal_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&f.auth.base.admin)
            .await
            .unwrap();
        let pool = pool_for(&schema).await;
        let journal =
            FileUploadChunkJournal::new(pool.clone(), scope(), Uuid::new_v4(), &schema, 16)
                .unwrap();
        journal.install_empty_namespace().await.unwrap();
        let directory =
            std::env::temp_dir().join(format!("cyanrex-session-metadata-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let root = directory.join("chunks");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let staging = ChunkStaging::open(scope(), &root, 2).unwrap();
        let local = FileUploadStaging::new(f.uploads.clone(), staging, 4).unwrap();
        let publisher = FileUploadPublisher::new(local, journal, f.publications.clone()).unwrap();
        let session = f
            .auth
            .source
            .validate_session(token)
            .await
            .unwrap()
            .unwrap();
        let owner = if token == f.auth.learner_token {
            f.auth.learner.principal.reference
        } else {
            f.auth.manager.principal.reference
        };
        let mut attempt = publisher
            .prepare(upload(seed), owner, session.account.account_id)
            .await
            .unwrap();
        let receipt = publisher.publish(&mut attempt).await.unwrap();
        assert_eq!(receipt.manifest(), &manifest);
        drop(receipt);
        Self {
            schema,
            pool,
            directory,
            root,
            manifest,
        }
    }
    pub async fn rows(&self, f: &Fixture) -> Vec<(String, Vec<String>)> {
        f.rows(&self.schema).await
    }
    pub async fn cleanup(self, f: &Fixture) {
        self.pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&f.auth.base.admin)
            .await
            .unwrap();
        fs::remove_dir_all(&self.directory)
            .expect("only remove this exclusive private publication fixture");
    }
}
