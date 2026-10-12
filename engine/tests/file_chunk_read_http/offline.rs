use crate::{fixture::*, support::*, *};
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};
pub struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("cyanrex-http-chunk-default-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
        Self(p)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove only the exclusive synthetic HTTP root");
    }
}
pub struct Offline {
    pub source: DurableAuthSource,
    pub workspace: SessionFileChunkReadWorkspace,
    pub app: Router,
    pub _root: Root,
}
impl Offline {
    pub async fn new() -> Self {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://synthetic@127.0.0.1/unused")
            .unwrap();
        pool.close().await;
        let source = DurableAuthSource::new(pool.clone(), authority());
        let root = Root::new();
        let pubstore = FileUploadPublicationStore::new(
            pool,
            scope(),
            Uuid::new_v4(),
            "chunk_http_publications",
            16,
        )
        .unwrap();
        let recipients = source
            .file_recipient_grant_workspace("chunk_http_recipients", scope(), Uuid::new_v4(), 16)
            .unwrap();
        let downloads = source
            .file_download_workspace(pubstore, recipients)
            .unwrap();
        let workspace = source
            .file_chunk_read_workspace(
                downloads,
                ChunkStaging::open(scope(), &root.0, 1).unwrap(),
                1,
            )
            .unwrap();
        let app = build_file_chunk_read_router(
            FileChunkReadHttpState::new(source.clone(), workspace.clone(), ORIGIN).unwrap(),
        );
        Self {
            source,
            workspace,
            app,
            _root: root,
        }
    }
    pub fn state(&self, n: usize) -> FileChunkReadHttpState {
        FileChunkReadHttpState::with_capacity(
            self.source.clone(),
            self.workspace.clone(),
            ORIGIN,
            n,
        )
        .unwrap()
    }
}
pub fn typed() -> SessionFileChunkReadRequest {
    SessionFileChunkReadRequest {
        download: SessionFileDownloadRequest {
            grant_id: sharing_fixture::recipient_grant(1),
            upload_id: publication_fixture::upload(1),
            expected_file: FileVersionRef::from_manifest(&bytes::manifest(17, 1)),
        },
        chunk_index: 0,
    }
}
pub fn uri() -> String {
    path(&typed())
}
pub async fn rejected(f: &Offline, r: Request<Body>, s: StatusCode, c: &str) {
    error(f.app.clone(), r, s, c, "not_attempted").await;
}
pub async fn available(route: Router) {
    error(
        route,
        request(TOKEN, &uri()),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}
