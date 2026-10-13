use crate::{fixture::*, support::*, *};
use std::fs;

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_manifest_http_uses_only_original_source_transaction_without_detached_store_or_file_reads(
) {
    checked(Fixture::ready().await, |f| Box::pin(async move {
        let route = app(f);
        healthy(f, &route).await;
        f.sharing.pool.close().await;
        f.sharing.base.publication_pool.close().await;
        f.sharing.base.upload_pool.close().await;
        f.sharing.base.journal_pool.close().await;
        f.sharing.base.grants.pool.close().await;
        let retained = f.sharing.base.directory.join("retained-source-chunks");
        fs::rename(&f.sharing.base.root, &retained).unwrap();
        f.sharing.base.root = retained;
        let before = f.sharing.snapshot().await;
        healthy(f, &route).await;
        assert_eq!(f.sharing.snapshot().await, before,
            "manifest transport neither inspects missing configured staging paths nor borrows detached store pools");
        download_support::drain(f).await;
    })).await;
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_manifest_http_sanitizes_selected_corruption_and_original_pinned_configuration_errors(
) {
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let route = app(f);
            let uri = path(&f.request());
            for recipient in [true, false] {
                let (schema, table, column, selector, id) = if recipient {
                    (
                        &f.sharing.schema,
                        sharing_fixture::HEADS,
                        "grant_payload",
                        "grant_id",
                        f.request().grant_id.as_uuid(),
                    )
                } else {
                    (
                        &f.sharing.base.publication_schema,
                        download_support::PUB_ROWS,
                        "manifest",
                        "upload_id",
                        f.request().upload_id.as_uuid(),
                    )
                };
                let select =
                    format!("SELECT {column} AS payload FROM {schema}.{table} WHERE {selector}=$1");
                let original: Vec<u8> = query(&select)
                    .bind(id)
                    .fetch_one(&auth(f).base.admin)
                    .await
                    .unwrap()
                    .get("payload");
                let update = format!("UPDATE {schema}.{table} SET {column}=$1 WHERE {selector}=$2");
                query(&update)
                    .bind(b"{".as_slice())
                    .bind(id)
                    .execute(&auth(f).base.admin)
                    .await
                    .unwrap();
                denied(
                    f,
                    &route,
                    request(&auth(f).target_token, &uri),
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                )
                .await;
                denied(
                    f,
                    &route,
                    request(&auth(f).manager_token, &uri),
                    StatusCode::NOT_FOUND,
                    "not_found",
                )
                .await;
                query(&update)
                    .bind(original)
                    .bind(id)
                    .execute(&auth(f).base.admin)
                    .await
                    .unwrap();
                healthy(f, &route).await;
            }
            let schema = &f.sharing.base.publication_schema;
            let original: Uuid = query(&format!(
                "SELECT installation_id FROM {schema}.{}",
                download_support::PUB_MARKER
            ))
            .fetch_one(&auth(f).base.admin)
            .await
            .unwrap()
            .get("installation_id");
            let update = format!(
                "UPDATE {schema}.{} SET installation_id=$1",
                download_support::PUB_MARKER
            );
            query(&update)
                .bind(Uuid::new_v4())
                .execute(&auth(f).base.admin)
                .await
                .unwrap();
            denied(
                f,
                &route,
                request(&auth(f).target_token, &uri),
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
            )
            .await;
            query(&update)
                .bind(original)
                .execute(&auth(f).base.admin)
                .await
                .unwrap();
            healthy(f, &route).await;
            let wrong = DurableAuthSource::new(
                auth(f).base.pool.clone(),
                Uuid::new_v4().try_into().unwrap(),
            );
            let wrong = build_file_download_manifest_router(
                FileDownloadManifestHttpState::new(wrong, f.workspace.clone(), ORIGIN).unwrap(),
            );
            denied(
                f,
                &wrong,
                request(&auth(f).target_token, &uri),
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
            )
            .await;
            healthy(f, &route).await;
            download_support::drain(f).await;
        })
    })
    .await;
}
