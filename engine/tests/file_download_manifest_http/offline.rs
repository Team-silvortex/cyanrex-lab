use crate::{fixture::*, support::*, *};

pub struct Offline {
    pub source: DurableAuthSource,
    pub workspace: SessionFileDownloadWorkspace,
    pub app: Router,
}
impl Offline {
    pub async fn new() -> Self {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://synthetic@127.0.0.1/unused")
            .unwrap();
        pool.close().await;
        let source = DurableAuthSource::new(pool.clone(), authority());
        let published = FileUploadPublicationStore::new(
            pool,
            scope(),
            Uuid::new_v4(),
            "manifest_http_published",
            16,
        )
        .unwrap();
        let recipients = source
            .file_recipient_grant_workspace("manifest_http_recipients", scope(), Uuid::new_v4(), 16)
            .unwrap();
        let workspace = source
            .file_download_workspace(published, recipients)
            .unwrap();
        let app = build_file_download_manifest_router(
            FileDownloadManifestHttpState::new(source.clone(), workspace.clone(), ORIGIN).unwrap(),
        );
        Self {
            source,
            workspace,
            app,
        }
    }
    pub fn state(&self, capacity: usize) -> FileDownloadManifestHttpState {
        FileDownloadManifestHttpState::with_capacity(
            self.source.clone(),
            self.workspace.clone(),
            ORIGIN,
            capacity,
        )
        .unwrap()
    }
}
pub fn typed() -> SessionFileDownloadRequest {
    SessionFileDownloadRequest {
        grant_id: sharing_fixture::recipient_grant(1),
        upload_id: publication_fixture::upload(1),
        expected_file: FileVersionRef::from_manifest(&bytes::manifest(17, 0xab)),
    }
}
pub fn uri() -> String {
    path(&typed())
}
pub async fn rejected(f: &Offline, request: Request<Body>, status: StatusCode, code: &str) {
    error(&f.app, request, status, code, "not_attempted").await;
}
pub async fn available(route: &Router) {
    error(
        route,
        request(TOKEN, &uri()),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}
