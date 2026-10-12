pub use super::chunk_fixture::Fixture;
pub use super::identity_fixture::scope;
use super::*;
pub const ORIGIN: &str = "https://platform.example";
pub const COOKIE: &str = "cyanrex_platform_session";
pub const MARKER: &str = "x-cyanrex-platform-request";
pub const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";
pub const ENDPOINT: &str = "/platform/v1/file-chunks";
pub fn path(request: &SessionFileChunkReadRequest) -> String {
    let file = &request.download.expected_file;
    format!("/platform/v1/file-chunks?grant_id={}&upload_id={}&file_id={}&version_id={}&byte_length={}&sha256={}&chunk_index={}",
        request.download.grant_id,request.download.upload_id,file.file_id(),file.version_id(),file.byte_length(),file.sha256(),request.chunk_index)
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
    build_file_chunk_read_router(
        FileChunkReadHttpState::new(
            f.sharing.base.grants.auth.source.clone(),
            f.workspace.clone(),
            ORIGIN,
        )
        .unwrap(),
    )
}
