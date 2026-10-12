use crate::{fixture::*, support::*, *};
use std::{
    fs::{self, OpenOptions, Permissions},
    io::Write,
    os::unix::fs::PermissionsExt,
};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_http_selected_storage_corruption_rejects_without_partial_disclosure_or_repairs(
) {
    checked(Fixture::ready(1).await, |f| {
        Box::pin(async move {
            let route = app(f);
            let uri = path(&f.request());
            healthy(f, &route).await;
            let entry = bytes::tree(&f.sharing.base.root).remove(0);
            let chunk = f.sharing.base.root.join(entry.name);
            for damage in ["wrongdigest", "short", "long"] {
                fs::set_permissions(&chunk, Permissions::from_mode(0o600)).unwrap();
                let length = if damage == "short" {
                    16
                } else if damage == "long" {
                    18
                } else {
                    17
                };
                let mut data = (0..length).map(bytes::byte).collect::<Vec<_>>();
                if damage == "wrongdigest" {
                    data[0] ^= 1;
                }
                OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&chunk)
                    .unwrap()
                    .write_all(&data)
                    .unwrap();
                fs::set_permissions(&chunk, Permissions::from_mode(0o400)).unwrap();
                denied(
                    f,
                    &route,
                    request(&auth(f).target_token, &uri),
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                )
                .await;
                fs::set_permissions(&chunk, Permissions::from_mode(0o600)).unwrap();
                let mut restore = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&chunk)
                    .unwrap();
                std::io::copy(&mut bytes::Pattern::new(0, 17), &mut restore).unwrap();
                drop(restore);
                fs::set_permissions(&chunk, Permissions::from_mode(0o400)).unwrap();
                healthy(f, &route).await;
            }
            let old = serde_json::to_vec(&f.sharing.base.manifest).unwrap();
            query(&format!(
                "UPDATE {}.{PUB_ROWS} SET manifest=$1 WHERE upload_id=$2",
                f.sharing.base.publication_schema
            ))
            .bind(b"{".to_vec())
            .bind(f.request().download.upload_id.as_uuid())
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
            query(&format!(
                "UPDATE {}.{PUB_ROWS} SET manifest=$1 WHERE upload_id=$2",
                f.sharing.base.publication_schema
            ))
            .bind(old)
            .bind(f.request().download.upload_id.as_uuid())
            .execute(&auth(f).base.admin)
            .await
            .unwrap();
            healthy(f, &route).await;
            query(&format!(
                "UPDATE {}.{PUB_MARKER} SET publications_oid=publications_oid+1",
                f.sharing.base.publication_schema
            ))
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
            query(&format!(
                "UPDATE {}.{PUB_MARKER} SET publications_oid=publications_oid-1",
                f.sharing.base.publication_schema
            ))
            .execute(&auth(f).base.admin)
            .await
            .unwrap();
            healthy(f, &route).await;
            drain(f).await;
        })
    })
    .await;
}
