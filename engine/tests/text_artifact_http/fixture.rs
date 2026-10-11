use super::*;
pub use content_fixture::Fixture as ContentFixture;
pub use http_fixture::{error, headers, request, response, COOKIE, MARKER, ORIGIN, TOKEN};

pub const TEXT_COLLECTION: &str = "/platform/v1/text-artifacts";
pub const TEXT_ARTIFACT: &str = "57a22739-8078-44e1-9be3-2f40cf19786c";
pub const TEXT_REVISION: &str = "a5493cc4-47a5-4c61-8c57-07ebf66f8470";
pub const TEXT_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub struct Offline {
    pub app: Router,
    pub source: DurableAuthSource,
    pub workspace: SessionArtifactWorkspace,
    root: PathBuf,
}
impl Offline {
    pub async fn new() -> Self {
        let source = input_fixture::unavailable_source().await;
        let root = input_fixture::private_directory();
        let workspace = source
            .open_artifact_workspace("private_text_artifacts", scope(), &root)
            .unwrap();
        let app = build_text_artifact_router(
            TextArtifactHttpState::new(source.clone(), workspace.clone(), ORIGIN).unwrap(),
        );
        Self {
            app,
            source,
            workspace,
            root,
        }
    }
    pub fn state(&self, capacity: usize) -> TextArtifactHttpState {
        TextArtifactHttpState::with_capacity(
            self.source.clone(),
            self.workspace.clone(),
            ORIGIN,
            capacity,
        )
        .unwrap()
    }
}
impl Drop for Offline {
    fn drop(&mut self) {
        assert_eq!(fs::read_dir(&self.root).unwrap().count(), 0);
        fs::remove_dir(&self.root).unwrap();
    }
}
pub fn app(f: &ContentFixture) -> Router {
    build_text_artifact_router(
        TextArtifactHttpState::new(f.auth.source.clone(), f.artifact_workspace.clone(), ORIGIN)
            .unwrap(),
    )
}
pub fn create_body(
    artifact: ArtifactId,
    revision: ArtifactRevisionId,
    title: &str,
    text: &str,
) -> String {
    json!({"schema_version":1,"artifact_id":artifact,"revision_id":revision,"title":title,"text":text}).to_string()
}
pub fn valid_body(text: &str) -> String {
    create_body(
        TEXT_ARTIFACT.parse().unwrap(),
        TEXT_REVISION.parse().unwrap(),
        "Private text",
        text,
    )
}
pub fn text_path(reference: &ArtifactRef) -> String {
    format!(
        "{TEXT_COLLECTION}/{}/revisions/{}/{}",
        reference.artifact_id, reference.revision_id, reference.sha256
    )
}
pub fn read_path() -> String {
    format!("{TEXT_COLLECTION}/{TEXT_ARTIFACT}/revisions/{TEXT_REVISION}/{TEXT_SHA}")
}
pub async fn offline_error(f: &Offline, req: Request<Body>, status: StatusCode, code: &str) {
    error(f.app.clone(), req, status, code, "not_attempted").await;
}
pub async fn dispatched(app: Router, req: Request<Body>) {
    error(
        app,
        req,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}
