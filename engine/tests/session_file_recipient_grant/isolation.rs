use crate::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_recipient_metadata_uses_fresh_rc_without_overriding_readonly_or_pool_settings(
) {
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let original = f.issue(1, f.recipient()).await;
            configure(f, false).await;
            let mut held = f.auth.base.admin.begin().await.unwrap();
            query(&format!(
                "LOCK TABLE {}.{HEADS} IN ACCESS EXCLUSIVE MODE",
                f.schema
            ))
            .execute(&mut *held)
            .await
            .unwrap();
            let pid = backend(&mut held).await;
            let mut pending = Box::pin(read_direct(f, &f.auth.learner_token, 1));
            let mut early = None;
            let waited = tokio::select! {
                result=&mut pending => { early=Some(result); false },
                yes=blocked(f,pid) => yes,
            };
            let changed = FileRecipientGrant::new(
                grant(1),
                revision(2),
                original.grant().file().clone(),
                original.grant().publisher(),
                original.grant().recipient(),
                original.grant().validity(),
                FileGrantState::Revoked,
            )
            .unwrap();
            let raw = serde_json::to_vec(&changed).unwrap();
            assert_eq!(
                query(&format!(
                    "UPDATE {}.{HEADS} SET revision=2,grant_payload=$1 WHERE grant_id=$2",
                    f.schema
                ))
                .bind(&raw)
                .bind(grant(1).as_uuid())
                .execute(&mut *held)
                .await
                .unwrap()
                .rows_affected(),
                1
            );
            assert_eq!(
                query(&format!(
                    "INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,
            not_before,expires_at,recorded_at,grant_payload)
            SELECT grant_id,revision,issuer_id,issuer_account_id,not_before,expires_at,
                floor(extract(epoch FROM clock_timestamp()))::bigint,grant_payload
            FROM {}.{HEADS} WHERE grant_id=$1",
                    f.schema, f.schema
                ))
                .bind(grant(1).as_uuid())
                .execute(&mut *held)
                .await
                .unwrap()
                .rows_affected(),
                1
            );
            let injected = snapshot_in_tx(f, &mut held).await;
            held.commit().await.unwrap();
            let result = match early {
                Some(result) => result,
                None => pending.as_mut().await,
            };
            drop(pending);
            let after = f.snapshot().await;
            settings(f, false).await;
            let observed = result.unwrap().unwrap();
            assert!(
                waited,
                "the target relation must really block this source-owned read"
            );
            assert_eq!(
                observed.grant(),
                &changed,
                "the legal revocation committed after initial source SELECT must be read freshly"
            );
            assert_eq!(observed.issuer(), original.issuer());
            assert_eq!(
                after, injected,
                "expected data was frozen before holder COMMIT"
            );
            assert_eq!(
                read(f, &f.auth.learner_token, 1).await.unwrap(),
                Some(observed)
            );
            configure(f, true).await;
            let before = f.snapshot().await;
            for seed in [1, 99] {
                assert_eq!(
                    read(f, &f.auth.learner_token, seed).await,
                    Err(SessionFileRecipientGrantError::Session(
                        SessionCommandError::Authentication(DurableAuthError::StorageUnavailable)
                    )),
                    "configured READ ONLY cannot become READ WRITE to admit locking authorization"
                );
            }
            assert_eq!(f.snapshot().await, before);
            settings(f, true).await;
            configure(f, false).await;
            assert!(read(f, &f.auth.learner_token, 1).await.unwrap().is_some());
            settings(f, false).await;
            drain(f).await;
        })
    })
    .await;
}
