use crate::{fixture::*, *};
use futures_util::FutureExt;
use sha2::Digest;
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin, time::Duration};
pub const PUB_MARKER: &str = "collaboration_file_publication_schema";
pub const PUB_ROWS: &str = "collaboration_file_publications";
pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub fn auth(f: &Fixture) -> &identity_fixture::Fixture {
    &f.sharing.base.grants.auth
}
pub async fn checked<F>(mut f: Fixture, work: F)
where
    F: for<'a> FnOnce(&'a mut Fixture) -> Pin<Box<dyn Future<Output = ()> + 'a>>,
{
    let outcome = AssertUnwindSafe(work(&mut f)).catch_unwind().await;
    f.cleanup().await;
    if let Err(p) = outcome {
        std::panic::resume_unwind(p);
    }
}
pub fn request_body(token: &str, uri: &str, body: Body) -> Request<Body> {
    let mut r = request(token, uri);
    *r.body_mut() = body;
    r
}
pub fn request_method(method: &str, uri: &str, body: Body) -> Request<Body> {
    let mut r = request_body(TOKEN, uri, body);
    *r.method_mut() = method.parse().unwrap();
    r
}
pub fn unread() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("cheap rejection must not poll request bytes")
        },
    ))
}
pub fn headers(r: &axum::response::Response) {
    assert_eq!(r.headers()["cache-control"], "no-store");
    assert_eq!(r.headers()["pragma"], "no-cache");
    assert_eq!(r.headers()["x-content-type-options"], "nosniff");
    for h in [
        "set-cookie",
        "retry-after",
        "location",
        "access-control-allow-origin",
        "content-range",
        "etag",
        "last-modified",
    ] {
        assert!(!r.headers().contains_key(h), "private header {h}");
    }
}
pub async fn error_response(
    r: axum::response::Response,
    status: StatusCode,
    code: &str,
    outcome: &str,
) {
    assert_eq!(r.status(), status);
    headers(&r);
    assert_eq!(r.headers()["content-type"], "application/json");
    for h in [
        "x-cyanrex-file-id",
        "x-cyanrex-chunk-sha256",
        "content-disposition",
    ] {
        assert!(!r.headers().contains_key(h));
    }
    let data = r.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&data).unwrap(),
        serde_json::json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}
pub async fn error(route: Router, r: Request<Body>, status: StatusCode, code: &str, outcome: &str) {
    error_response(route.oneshot(r).await.unwrap(), status, code, outcome).await;
}
pub async fn denied(f: &Fixture, route: &Router, r: Request<Body>, status: StatusCode, code: &str) {
    let before = f.sharing.snapshot().await;
    error(route.clone(), r, status, code, "unconfirmed").await;
    assert_eq!(f.sharing.snapshot().await, before);
}
pub fn state(f: &Fixture, http: usize) -> FileChunkReadHttpState {
    FileChunkReadHttpState::with_capacity(auth(f).source.clone(), f.workspace.clone(), ORIGIN, http)
        .unwrap()
}
pub fn bounded(f: &Fixture) -> Router {
    build_file_chunk_read_router(state(f, 1))
}
pub async fn binary(f: &Fixture, route: &Router, r: &SessionFileChunkReadRequest) -> Vec<u8> {
    let before = f.sharing.snapshot().await;
    let response = route
        .clone()
        .oneshot(request(&auth(f).target_token, &path(r)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    headers(&response);
    assert_eq!(
        response.headers()["content-type"],
        "application/octet-stream"
    );
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=\"chunk.bin\""
    );
    assert_eq!(response.headers()["x-cyanrex-file-transfer-version"], "1");
    assert_eq!(
        response.headers()["x-cyanrex-file-id"],
        r.download.expected_file.file_id().to_string()
    );
    assert_eq!(
        response.headers()["x-cyanrex-file-version-id"],
        r.download.expected_file.version_id().to_string()
    );
    assert_eq!(
        response.headers()["x-cyanrex-chunk-index"],
        r.chunk_index.to_string()
    );
    assert_eq!(
        response.headers()["x-cyanrex-file-sha256"],
        r.download.expected_file.sha256().to_string()
    );
    let offset = r.chunk_index as u64 * FILE_TRANSFER_CHUNK_BYTES;
    let expected_length =
        (r.download.expected_file.byte_length() - offset).min(FILE_TRANSFER_CHUNK_BYTES);
    let expected = (offset..offset + expected_length)
        .map(bytes::byte)
        .collect::<Vec<_>>();
    assert_eq!(
        response.headers()["x-cyanrex-chunk-sha256"],
        format!("{:x}", sha2::Sha256::digest(&expected))
    );
    let length = response.headers()["content-length"]
        .to_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let data = response.into_body().collect().await.unwrap().to_bytes();
    let actual = data.to_vec();
    drop(data);
    assert_eq!(actual.len(), length);
    assert_eq!(f.sharing.snapshot().await, before);
    actual
}
pub async fn healthy(f: &Fixture, route: &Router) {
    assert_eq!(
        binary(f, route, &f.request()).await,
        (0..17).map(bytes::byte).collect::<Vec<_>>()
    );
}
pub async fn saturated(route: &Router, uri: &str) {
    tokio::time::timeout(
        Duration::from_secs(1),
        error(
            route.clone(),
            request_body(TOKEN, uri, unread()),
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        ),
    )
    .await
    .expect("HTTP budget must fail-fast before body or source");
}
pub async fn source_full(f: &Fixture) {
    let r = f.request();
    let result = auth(f)
        .source
        .read_session_file_chunk(&auth(f).target_token, &f.workspace, &r)
        .await;
    assert_eq!(result.err(),Some(cyanrex_engine::services::auth_service::durable_source::SessionFileChunkReadError::CapacityExhausted));
}
pub async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS n")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("n")
}
pub async fn blocked(f: &Fixture, pid: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(1),async {loop {
        let yes:bool=query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND state='active' AND wait_event_type='Lock' AND $2=ANY(pg_blocking_pids(pid))) AS yes")
            .bind(&auth(f).base.schema).bind(pid).fetch_one(&auth(f).base.admin).await.unwrap().get("yes");
        if yes{return true;}tokio::task::yield_now().await;
    }}).await.unwrap_or(false)
}
pub async fn holder(f: &Fixture) -> Tx<'_> {
    let mut tx = auth(f).base.admin.begin().await.unwrap();
    query(&format!(
        "SELECT * FROM {}.{PUB_MARKER} FOR UPDATE",
        f.sharing.base.publication_schema
    ))
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    tx
}
pub async fn drain(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut tx = auth(f).base.admin.begin().await.unwrap();
        query(&format!(
            "LOCK TABLE {}.{},{}.{},{}.{},{}.{PUB_MARKER},{}.{PUB_ROWS} IN ACCESS EXCLUSIVE MODE",
            f.sharing.schema,
            sharing_fixture::MARKER,
            f.sharing.schema,
            sharing_fixture::HEADS,
            f.sharing.schema,
            sharing_fixture::CHANGES,
            f.sharing.base.publication_schema,
            f.sharing.base.publication_schema
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        auth(f).base.drain().await;
    })
    .await
    .expect("HTTP cancellation must drain source/selected table locks");
}
pub async fn snapshot_in_tx(f: &Fixture, tx: &mut Tx<'_>) -> sharing_fixture::Snapshot {
    let mut database = Vec::new();
    for schema in [
        &auth(f).base.schema,
        &f.sharing.base.grants.schema,
        &f.sharing.base.upload_schema,
        &f.sharing.base.journal_schema,
        &f.sharing.base.publication_schema,
    ] {
        database.extend(rows(schema, tx).await);
    }
    sharing_fixture::Snapshot {
        base: publication_fixture::Snapshot {
            database: grant_fixture::Snapshot(database),
            root: bytes::snapshot(&f.sharing.base.root),
            files: bytes::tree(&f.sharing.base.root),
        },
        recipient: rows(&f.sharing.schema, tx).await,
    }
}
async fn rows(schema: &str, tx: &mut Tx<'_>) -> Vec<(String, i64, Vec<String>)> {
    let tables=query("SELECT relname AS n,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname").bind(schema).fetch_all(&mut **tx).await.unwrap();
    let mut result = Vec::new();
    for t in tables {
        let n: String = t.get("n");
        assert!(n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        let rows = query(&format!(
            "SELECT to_jsonb(t)::text AS v FROM {schema}.{n} t ORDER BY to_jsonb(t)::text"
        ))
        .fetch_all(&mut **tx)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.get("v"))
        .collect();
        result.push((format!("{schema}.{n}"), t.get("oid"), rows));
    }
    result
}
