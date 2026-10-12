use super::support::*;
use crate::{fixture::*, *};

async fn configure(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.auth.base.pool.acquire().await.unwrap();
        for statement in [
            "SET default_transaction_isolation='repeatable read'",
            "SET synchronous_commit='off'",
            if readonly {
                "SET default_transaction_read_only='on'"
            } else {
                "SET default_transaction_read_only='off'"
            },
        ] {
            query(statement).execute(&mut *c).await.unwrap();
        }
        held.push(c);
    }
}
async fn settings(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.auth.base.pool.acquire().await.unwrap();
        let row = query("SELECT current_setting('default_transaction_isolation') AS i,current_setting('synchronous_commit') AS s,current_setting('default_transaction_read_only') AS r").fetch_one(&mut *c).await.unwrap();
        assert_eq!(row.get::<String, _>("i"), "repeatable read");
        assert_eq!(row.get::<String, _>("s"), "off");
        assert_eq!(
            row.get::<String, _>("r"),
            if readonly { "on" } else { "off" }
        );
        held.push(c);
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_publisher_grant_fresh_reads_override_rr_but_preserve_readonly_settings() {
    let f = Fixture::ready().await;
    let original = f.issue(1, f.subject()).await;
    configure(&f, false).await;
    let mut held = f.auth.base.admin.begin().await.unwrap();
    query(&format!(
        "LOCK TABLE {}.{HEADS} IN ACCESS EXCLUSIVE MODE",
        f.schema
    ))
    .execute(&mut *held)
    .await
    .unwrap();
    let pid = backend(&mut held).await;
    let mut pending = Box::pin(read_direct(&f, &f.auth.learner_token, 1));
    let mut early = None;
    let waited = tokio::select! { result=&mut pending=>{early=Some(result);false}, yes=blocked(&f,pid)=>yes };
    let changed = FilePublisherGrant::new(
        grant(1),
        2u64.try_into().unwrap(),
        scope(),
        f.subject(),
        original.grant().validity(),
        FileGrantState::Revoked,
    )
    .unwrap();
    let payload = serde_json::to_vec(&changed).unwrap();
    assert_eq!(
        query(&format!(
            "UPDATE {}.{HEADS} SET revision=2,grant_payload=$1 WHERE grant_id=$2",
            f.schema
        ))
        .bind(&payload)
        .bind(grant(1).as_uuid())
        .execute(&mut *held)
        .await
        .unwrap()
        .rows_affected(),
        1
    );
    assert_eq!(query(&format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) SELECT grant_id,revision,issuer_id,issuer_account_id,not_before,expires_at,floor(extract(epoch FROM clock_timestamp()))::bigint,grant_payload FROM {}.{HEADS} WHERE grant_id=$1", f.schema, f.schema))
        .bind(grant(1).as_uuid()).execute(&mut *held).await.unwrap().rows_affected(), 1);
    let injected = snapshot_in_tx(&f, &mut held).await;
    held.commit().await.unwrap();
    let result = match early {
        Some(r) => r,
        None => pending.as_mut().await,
    };
    drop(pending);
    let after = f.snapshot().await;
    settings(&f, false).await;
    let observed = result.unwrap().unwrap();
    assert!(
        waited,
        "source GET must really wait on this independent target table holder"
    );
    assert_eq!(observed.grant(), &changed, "RC must see the lawful metadata transition committed after the source's initial Session snapshot");
    assert_eq!(observed.issuer(), original.issuer());
    assert_eq!(
        after, injected,
        "the expected injected state is captured before releasing the read"
    );
    assert_eq!(
        read(&f, &f.auth.learner_token, 1).await.unwrap(),
        Some(observed)
    );
    configure(&f, true).await;
    let before = f.snapshot().await;
    for seed in [1, 99] {
        assert_eq!(
            read(&f, &f.auth.learner_token, seed).await,
            Err(SessionFilePublisherGrantError::Session(
                SessionCommandError::Authentication(DurableAuthError::StorageUnavailable)
            )),
            "configured READ ONLY must not be overridden to allow the locking Session guard"
        );
    }
    assert_eq!(f.snapshot().await, before);
    settings(&f, true).await;
    configure(&f, false).await;
    assert!(read(&f, &f.auth.learner_token, 1).await.unwrap().is_some());
    settings(&f, false).await;
    f.auth.base.drain().await;
    f.cleanup().await;
}
