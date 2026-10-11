use super::behavior::{counts, decode, failure, reference, revision};
use super::*;

pub(super) type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub(super) struct Pending(Option<tokio::task::JoinHandle<axum::response::Response>>);
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(handle) = &self.0 {
            handle.abort();
        }
    }
}
impl Pending {
    pub(super) fn start(route: Router, req: Request<Body>) -> Self {
        Self(Some(tokio::spawn(async move {
            route.oneshot(req).await.unwrap()
        })))
    }
    pub(super) fn is_finished(&self) -> bool {
        self.0.as_ref().unwrap().is_finished()
    }
    pub(super) async fn response(mut self) -> axum::response::Response {
        // Keep the handle in the abort-on-drop guard across timeout/panic, never detach it.
        let result = tokio::time::timeout(Duration::from_secs(3), self.0.as_mut().unwrap())
            .await
            .expect("bounded response after releasing the exact blocker")
            .unwrap();
        self.0.take();
        result
    }
    pub(super) async fn cancel(mut self) {
        self.0.as_ref().unwrap().abort();
        let result = self.0.as_mut().unwrap().await;
        self.0.take();
        assert!(result.unwrap_err().is_cancelled());
    }
}
pub(super) async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}
pub(super) async fn wait_for(f: &ContentFixture, blocker: i32) -> i32 {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let rows = query("SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid))")
                .bind(blocker).fetch_all(&f.auth.base.admin).await.unwrap();
            if rows.len() == 1 { return rows[0].get("pid"); }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("request did not reach its fixture-owned lock")
}
pub(super) async fn pause_event(f: &ContentFixture) -> Tx<'_> {
    f.sql_artifact("CREATE SEQUENCE text_http_calls").await;
    f.sql_artifact(&format!(
        "CREATE FUNCTION text_http_pause() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.text_http_calls');
        PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 994); RETURN NEW; END $$",
        f.artifact_schema
    ))
    .await;
    f.sql_artifact("CREATE TRIGGER text_http_pause BEFORE INSERT ON collaboration_artifact_outbox FOR EACH ROW EXECUTE FUNCTION text_http_pause()").await;
    let mut holder = f.artifact_pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext($1),994)")
        .bind(&f.artifact_schema)
        .execute(&mut *holder)
        .await
        .unwrap();
    holder
}
pub(super) async fn calls(f: &ContentFixture) -> i64 {
    let row = query("SELECT last_value,is_called FROM text_http_calls")
        .fetch_one(&f.artifact_pool)
        .await
        .unwrap();
    assert!(
        row.get::<bool, _>("is_called"),
        "intended trigger did not execute"
    );
    row.get("last_value")
}
fn unread_body() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("saturated request must not consume its body")
        },
    ))
}
async fn saturated(route: &Router, req: Request<Body>) {
    tokio::time::timeout(
        Duration::from_secs(1),
        error(
            route.clone(),
            req,
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        ),
    )
    .await
    .expect("saturated admission queued");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_text_artifact_http_duplicate_concurrent_ids_conflict_without_adoption() {
    let f = ContentFixture::ready().await;
    let artifact_id = id();
    let revision_id = id();
    let route = app(&f);
    let (a, b) = tokio::join!(
        route.clone().oneshot(request(
            "POST",
            TEXT_COLLECTION,
            &f.auth.learner_token,
            create_body(artifact_id, revision_id, "Same identity", "winner A")
        )),
        route.clone().oneshot(request(
            "POST",
            TEXT_COLLECTION,
            &f.auth.learner_token,
            create_body(artifact_id, revision_id, "Same identity", "winner B")
        ))
    );
    let mut winner = None;
    let mut conflicts = 0;
    for result in [a.unwrap(), b.unwrap()] {
        if result.status() == StatusCode::CREATED {
            assert!(winner.is_none());
            winner = Some(revision(&decode(result, StatusCode::CREATED).await, false));
        } else {
            failure(result, StatusCode::CONFLICT, "conflict").await;
            conflicts += 1;
        }
    }
    assert_eq!(conflicts, 1);
    let winner = winner.unwrap();
    let read = response(
        route.clone(),
        request(
            "GET",
            &text_path(&winner.reference),
            &f.auth.learner_token,
            Body::empty(),
        ),
        StatusCode::OK,
    )
    .await;
    let text = read["text"].as_str().unwrap();
    assert!(matches!(text, "winner A" | "winner B"));
    assert_eq!(
        winner.reference,
        reference(artifact_id, revision_id, text.as_bytes())
    );
    error(
        route.clone(),
        request(
            "POST",
            TEXT_COLLECTION,
            &f.auth.learner_token,
            create_body(artifact_id, revision_id, "Same identity", text),
        ),
        StatusCode::CONFLICT,
        "conflict",
        "unconfirmed",
    )
    .await;
    counts(&f, 1).await;
    assert_eq!(f.files(), 1);
    f.assert_pool_restored().await;
    drop(route);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_text_artifact_http_capacity_covers_sql_wait_and_response_without_cancel_cleanup()
{
    use axum::body::HttpBody;
    for cancel in [false, true] {
        let f = ContentFixture::ready().await;
        let mut holder = pause_event(&f).await;
        let blocker = backend(&mut holder).await;
        let route = build_text_artifact_router(
            TextArtifactHttpState::with_capacity(
                f.auth.source.clone(),
                f.artifact_workspace.clone(),
                ORIGIN,
                1,
            )
            .unwrap(),
        );
        let pin = reference(id(), id(), b"Bounded publication");
        let pending = Pending::start(
            route.clone(),
            request(
                "POST",
                TEXT_COLLECTION,
                &f.auth.learner_token,
                create_body(
                    pin.artifact_id,
                    pin.revision_id,
                    "Bounded content",
                    "Bounded publication",
                ),
            ),
        );
        wait_for(&f, blocker).await;
        assert!(!pending.is_finished());
        assert_eq!(calls(&f).await, 1);
        saturated(
            &route,
            request(
                "POST",
                TEXT_COLLECTION,
                &f.auth.learner_token,
                unread_body(),
            ),
        )
        .await;
        saturated(
            &route,
            request(
                "GET",
                &text_path(&pin),
                &f.auth.learner_token,
                unread_body(),
            ),
        )
        .await;
        if cancel {
            pending.cancel().await;
            f.assert_pool_restored().await;
            holder.rollback().await.unwrap();
            // Only the subsequent metadata/byte assertions prove this fixture's rollback.
            counts(&f, 0).await;
            error(
                route.clone(),
                request(
                    "GET",
                    &text_path(&pin),
                    &f.auth.learner_token,
                    Body::empty(),
                ),
                StatusCode::NOT_FOUND,
                "not_found",
                "unconfirmed",
            )
            .await;
        } else {
            holder.rollback().await.unwrap();
            let created = pending.response().await;
            assert_eq!(created.status(), StatusCode::CREATED);
            headers(&created);
            saturated(
                &route,
                request(
                    "GET",
                    &text_path(&pin),
                    &f.auth.learner_token,
                    unread_body(),
                ),
            )
            .await;
            let mut body = created.into_body();
            let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
            assert!(body.is_end_stream());
            let saved = revision(&serde_json::from_slice(&frame).unwrap(), false);
            assert_eq!(saved.reference, pin);
            let read = route
                .clone()
                .oneshot(request(
                    "GET",
                    &text_path(&pin),
                    &f.auth.learner_token,
                    Body::empty(),
                ))
                .await
                .unwrap();
            assert_eq!(read.status(), StatusCode::OK);
            saturated(
                &route,
                request(
                    "GET",
                    &text_path(&pin),
                    &f.auth.learner_token,
                    unread_body(),
                ),
            )
            .await;
            drop(read);
            let actual = response(
                route.clone(),
                request(
                    "GET",
                    &text_path(&pin),
                    &f.auth.learner_token,
                    Body::empty(),
                ),
                StatusCode::OK,
            )
            .await;
            assert_eq!(actual["text"], "Bounded publication");
            counts(&f, 1).await;
            drop(body);
        }
        // File publication is separate: cancelling SQL never implicitly removes its blob.
        assert_eq!(f.files(), 1);
        assert_eq!(fs::read(f.blob(&pin)).unwrap(), b"Bounded publication");
        assert_eq!(f.count_task("collaboration_tasks").await, 0);
        f.assert_pool_restored().await;
        drop(route);
        f.cleanup().await;
    }
}
