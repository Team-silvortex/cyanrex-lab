use super::{fixture::*, http, identity_helpers};
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_loopback_host_running_host_rejects_root_and_non_source_marker_drift() {
    let (ready, issued, refused, unchanged, restored, healthy, closed, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let mut child = prepared.spawn(None);
                let ready = child.ready().await;
                let client = http::client();
                let mut issued = false;
                let mut refused = false;
                let mut unchanged = false;
                let mut restored = false;
                let mut healthy = false;
                if ready {
                    let token = http::token(&http::login(&client, &prepared.origin).await);
                    issued = token.is_some();
                    let token = token.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    let before = snapshot(f).await;
                    refused = true;
                    unchanged = true;
                    restored = true;
                    for root in [true, false] {
                        if root {
                            refused &= prepared.root().swap();
                        } else {
                            identity_helpers::replace_marker(f, 3).await;
                        }
                        let drifted = snapshot(f).await;
                        for response in [
                            http::login(&client, &prepared.origin).await,
                            http::get(&client, &prepared.origin, &token).await,
                            http::logout(&client, &prepared.origin, &token).await,
                        ] {
                            refused &=
                                http::denied(&response, 503, "service_unavailable", "unconfirmed");
                        }
                        unchanged &= drifted == snapshot(f).await && session_count(f).await == 1;
                        if root {
                            restored &= prepared.root().restore();
                        } else {
                            identity_helpers::restore_marker(f, 3).await;
                        }
                        restored &= before == snapshot(f).await;
                    }
                    healthy = http::current(&http::get(&client, &prepared.origin, &token).await);
                }
                let closed = stopped(&child.stop().await);
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    ready, issued, refused, unchanged, restored, healthy, closed, no_backend,
                    cleaned,
                )
            })
        })
        .await;
    assert!(
        ready
            && issued
            && refused
            && unchanged
            && restored
            && healthy
            && closed
            && no_backend
            && cleaned
    );
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_loopback_host_failed_logout_readonly_and_missing_delete_preserve_cookies(
) {
    let (ready, issued, refused, unchanged, healthy, closed, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let mut child = prepared.spawn(None);
            let ready = child.ready().await;
            let client = http::client();
            let mut issued = false;
            let mut refused = false;
            let mut unchanged = false;
            let mut healthy = false;
            if ready {
                let token = http::token(&http::login(&client,&prepared.origin).await);
                issued = token.is_some();
                let token = token.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                f.sql(&format!("REVOKE DELETE ON {}.sessions FROM {}",f.source,f.role)).await;
                let before = snapshot(f).await;
                refused = http::denied(&http::logout(&client,&prepared.origin,&token).await,503,"service_unavailable","unconfirmed");
                unchanged = before == snapshot(f).await && session_count(f).await == 1;
                healthy = http::current(&http::get(&client,&prepared.origin,&token).await);
                f.sql(&format!("GRANT DELETE ON {}.sessions TO {}",f.source,f.role)).await;
                read_only(f,true).await;
                let before = snapshot(f).await;
                for response in [
                    http::login(&client,&prepared.origin).await,
                    http::get(&client,&prepared.origin,&token).await,
                    http::logout(&client,&prepared.origin,&token).await,
                ] {
                    refused &= http::denied(&response,503,"service_unavailable","unconfirmed");
                }
                unchanged &= before == snapshot(f).await && session_count(f).await == 1;
                read_only(f,false).await;
                // Execute one real DELETE which is deliberately suppressed. Its unknown
                // result must not become a clear-Cookie success or an automatic retry.
                f.sql(&format!("CREATE SEQUENCE {}.loopback_suppress_hits",f.source)).await;
                f.sql(&format!("CREATE FUNCTION {}.loopback_suppress_delete() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.nextval('{}.loopback_suppress_hits'); RETURN NULL; END $$",f.source,f.source)).await;
                f.sql(&format!("CREATE TRIGGER loopback_suppress BEFORE DELETE ON {}.sessions FOR EACH ROW EXECUTE FUNCTION {}.loopback_suppress_delete()",f.source,f.source)).await;
                let before = snapshot(f).await;
                refused &= http::denied(&http::logout(&client,&prepared.origin,&token).await,503,"service_unavailable","unconfirmed");
                refused &= within(query(&format!("SELECT is_called AND last_value=1 AS hit FROM {}.loopback_suppress_hits",f.source)).fetch_one(f.owner())).await.unwrap().get::<bool,_>("hit");
                unchanged &= before == snapshot(f).await && session_count(f).await == 1;
                f.sql(&format!("DROP TRIGGER loopback_suppress ON {}.sessions",f.source)).await;
                f.sql(&format!("DROP SEQUENCE {}.loopback_suppress_hits",f.source)).await;
                f.sql(&format!("DROP FUNCTION {}.loopback_suppress_delete()",f.source)).await;
                healthy &= http::current(&http::get(&client,&prepared.origin,&token).await);
                let unknown = uuid::Uuid::new_v4().to_string();
                healthy &= http::cleared(&http::logout(&client,&prepared.origin,&unknown).await) && session_count(f).await == 1;
            }
            let closed = stopped(&child.stop().await);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (ready,issued,refused,unchanged,healthy,closed,no_backend,cleaned)
        })
    }).await;
    assert!(ready && issued && refused && unchanged && healthy && closed && no_backend && cleaned);
}
