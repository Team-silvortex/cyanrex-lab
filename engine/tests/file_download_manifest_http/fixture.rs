use crate::*;
pub use crate::{download_fixture::Fixture, identity_fixture::scope};
use futures_util::FutureExt;
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin};

pub const ORIGIN: &str = "https://platform.example";
pub const COOKIE: &str = "cyanrex_platform_session";
pub const MARKER: &str = "x-cyanrex-platform-request";
pub const ENDPOINT: &str = "/platform/v1/file-manifests";
pub const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";
pub fn path(request: &SessionFileDownloadRequest) -> String {
    let file = &request.expected_file;
    format!(
        "{ENDPOINT}?grant_id={}&upload_id={}&file_id={}&version_id={}&byte_length={}&sha256={}",
        request.grant_id,
        request.upload_id,
        file.file_id(),
        file.version_id(),
        file.byte_length(),
        file.sha256()
    )
}
pub fn request(token: &str, path: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header(MARKER, "1")
        .header("origin", ORIGIN)
        .header("cookie", format!("{COOKIE}={token}"))
        .body(Body::empty())
        .unwrap()
}
pub fn app(f: &Fixture) -> Router {
    build_file_download_manifest_router(
        FileDownloadManifestHttpState::new(
            f.sharing.base.grants.auth.source.clone(),
            f.workspace.clone(),
            ORIGIN,
        )
        .unwrap(),
    )
}
pub async fn checked<T, F>(mut f: Fixture, work: F) -> T
where
    F: for<'a> FnOnce(&'a mut Fixture) -> Pin<Box<dyn Future<Output = T> + 'a>>,
{
    let outcome = AssertUnwindSafe(work(&mut f)).catch_unwind().await;
    f.cleanup().await;
    match outcome {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
