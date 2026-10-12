pub use super::identity_fixture::scope;
use super::*;
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStaging,
    file_upload_chunk_journal::FileUploadChunkJournal,
    file_upload_publication::FileUploadPublicationStore,
    file_upload_staging::{FileUploadPublisher, FileUploadStaging, JournaledFileUploadStaging},
    file_upload_store::{FileUploadLimits, FileUploadStore},
};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
};

pub fn upload(seed: u128) -> FileUploadId {
    Uuid::from_u128(seed + 300).try_into().unwrap()
}
#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub database: grant_fixture::Snapshot,
    pub root: bytes::Entry,
    pub files: Vec<bytes::Entry>,
}
pub struct Fixture {
    pub grants: grant_fixture::Fixture,
    pub upload_schema: String,
    pub journal_schema: String,
    pub publication_schema: String,
    pub upload_pool: PgPool,
    pub journal_pool: PgPool,
    pub publication_pool: PgPool,
    pub uploads: FileUploadStore,
    pub journal: FileUploadChunkJournal,
    pub publication: FileUploadPublicationStore,
    pub local: FileUploadStaging,
    pub journaled: JournaledFileUploadStaging,
    pub publisher: FileUploadPublisher,
    pub workspace: SessionFilePublishingWorkspace,
    pub directory: PathBuf,
    pub root: PathBuf,
    pub manifest: FileTransferManifest,
}
impl Fixture {
    pub async fn ready(length: u64) -> Self {
        Self::with_limits(length, 4, 8).await
    }
    pub async fn with_limits(length: u64, controls: usize, max_publications: u32) -> Self {
        let grants = grant_fixture::Fixture::ready().await;
        let upload_schema = format!("session_pub_upload_{}", Uuid::new_v4().simple());
        let journal_schema = format!("session_pub_journal_{}", Uuid::new_v4().simple());
        let publication_schema = format!("session_pub_target_{}", Uuid::new_v4().simple());
        for schema in [&upload_schema, &journal_schema, &publication_schema] {
            query(&format!("CREATE SCHEMA {schema}"))
                .execute(&grants.auth.base.admin)
                .await
                .unwrap();
        }
        let upload_pool = grant_fixture::pool_for(&upload_schema).await;
        let journal_pool = grant_fixture::pool_for(&journal_schema).await;
        let publication_pool = grant_fixture::pool_for(&publication_schema).await;
        let limits =
            FileUploadLimits::new(MAX_FILE_TRANSFER_BYTES, MAX_FILE_TRANSFER_BYTES, 16, 8).unwrap();
        let uploads = FileUploadStore::new(
            upload_pool.clone(),
            scope(),
            Uuid::new_v4(),
            &upload_schema,
            limits,
        )
        .unwrap();
        uploads.install_empty_namespace().await.unwrap();
        let journal = FileUploadChunkJournal::new(
            journal_pool.clone(),
            scope(),
            Uuid::new_v4(),
            &journal_schema,
            16,
        )
        .unwrap();
        journal.install_empty_namespace().await.unwrap();
        let publication = FileUploadPublicationStore::new(
            publication_pool.clone(),
            scope(),
            Uuid::new_v4(),
            &publication_schema,
            max_publications,
        )
        .unwrap();
        publication.install_empty_namespace().await.unwrap();
        let directory =
            std::env::temp_dir().join(format!("cyanrex-session-publication-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let root = directory.join("chunks");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let staging = ChunkStaging::open(scope(), &root, 2).unwrap();
        let local = FileUploadStaging::new(uploads.clone(), staging, controls).unwrap();
        let journaled = JournaledFileUploadStaging::new(local.clone(), journal.clone()).unwrap();
        let publisher =
            FileUploadPublisher::new(local.clone(), journal.clone(), publication.clone()).unwrap();
        let actor = grants.subject();
        let manifest = bytes::manifest(length, 1);
        uploads
            .reserve(upload(1), actor.principal(), actor.account_id(), &manifest)
            .await
            .unwrap();
        let path = directory.join("real-source.bin");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::copy(&mut bytes::Pattern::new(0, length), &mut file).unwrap();
        drop(file);
        for index in 0..manifest.chunk_sha256().len() as u32 {
            let mut reader = fs::File::open(&path).unwrap();
            reader
                .seek(SeekFrom::Start(index as u64 * FILE_TRANSFER_CHUNK_BYTES))
                .unwrap();
            let mut attempt = journaled
                .prepare(upload(1), actor.principal(), actor.account_id(), index)
                .await
                .unwrap();
            drop(
                journaled
                    .stage_chunk(
                        &mut attempt,
                        reader.take(manifest.chunk(index).unwrap().byte_length),
                    )
                    .await
                    .unwrap(),
            );
        }
        let workspace = grants
            .auth
            .source
            .file_publishing_workspace(publisher.clone(), grants.workspace.clone())
            .unwrap();
        Self {
            grants,
            upload_schema,
            journal_schema,
            publication_schema,
            upload_pool,
            journal_pool,
            publication_pool,
            uploads,
            journal,
            publication,
            local,
            journaled,
            publisher,
            workspace,
            directory,
            root,
            manifest,
        }
    }
    pub fn actor(&self) -> FileAccountRef {
        self.grants.subject()
    }
    pub async fn stage_additional(
        &self,
        seed: u128,
        length: u64,
        actor: FileAccountRef,
    ) -> FileTransferManifest {
        let manifest = bytes::manifest(length, seed);
        self.uploads
            .reserve(
                upload(seed),
                actor.principal(),
                actor.account_id(),
                &manifest,
            )
            .await
            .unwrap();
        for index in 0..manifest.chunk_sha256().len() as u32 {
            let chunk = manifest.chunk(index).unwrap();
            let mut attempt = self
                .journaled
                .prepare(upload(seed), actor.principal(), actor.account_id(), index)
                .await
                .unwrap();
            drop(
                self.journaled
                    .stage_chunk(
                        &mut attempt,
                        bytes::Pattern::new(
                            index as u64 * FILE_TRANSFER_CHUNK_BYTES,
                            chunk.byte_length,
                        ),
                    )
                    .await
                    .unwrap(),
            );
        }
        manifest
    }
    pub async fn snapshot(&self) -> Snapshot {
        let mut database = self.grants.snapshot().await;
        for schema in [
            &self.upload_schema,
            &self.journal_schema,
            &self.publication_schema,
        ] {
            let tables=query("SELECT relname,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname").bind(schema).fetch_all(&self.grants.auth.base.admin).await.unwrap();
            for t in tables {
                let name: String = t.get("relname");
                assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
                let rows=query(&format!("SELECT to_jsonb(t)::text AS row FROM {schema}.{name} t ORDER BY to_jsonb(t)::text"))
                .fetch_all(&self.grants.auth.base.admin).await.unwrap().into_iter().map(|r|r.get("row")).collect();
                database
                    .0
                    .push((format!("{schema}.{name}"), t.get("oid"), rows));
            }
        }
        Snapshot {
            database,
            root: bytes::snapshot(&self.root),
            files: bytes::tree(&self.root),
        }
    }
    pub async fn cleanup(self) {
        self.upload_pool.close().await;
        self.journal_pool.close().await;
        self.publication_pool.close().await;
        for schema in [
            &self.publication_schema,
            &self.journal_schema,
            &self.upload_schema,
        ] {
            query(&format!("DROP SCHEMA {schema} CASCADE"))
                .execute(&self.grants.auth.base.admin)
                .await
                .unwrap();
        }
        self.grants.cleanup().await;
        fs::remove_dir_all(&self.directory)
            .expect("remove only the exclusive synthetic publication directory");
    }
}
