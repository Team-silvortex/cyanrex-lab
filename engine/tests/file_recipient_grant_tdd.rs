//! Trusted recipient ledger metadata, not current authority, publication validation or delivery.
use cyanrex_engine::{models::collaboration::*, services::file_recipient_grant_store::*};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;
#[path = "file_recipient_grant/mod.rs"]
mod cases;

mod baseline {
    use super::{cases::fixture::*, *};
    #[tokio::test]
    #[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_recipient_grant_records_exact_file_and_independent_accounts() {
        let f = Fixture::new(16).await;
        let installed = f.store.install_empty_namespace().await;
        let initial = f.snapshot().await;
        let issued = f
            .store
            .issue(
                grant(1),
                account(1),
                file(1),
                account(2),
                account(3),
                validity(),
            )
            .await;
        let after_issue = f.snapshot().await;
        let observed = f.store.get(grant(1), account(3)).await;
        let after_get = f.snapshot().await;
        let revoked = f.store.revoke(grant(1), account(4), revision(1)).await;
        let after_revoke = f.snapshot().await;
        let (reopened, pool) = f.reopen().await;
        let reopened_row = reopened.get(grant(1), account(3)).await;
        let final_snapshot = f.snapshot().await;
        pool.close().await;
        f.cleanup().await;
        assert!(
            installed.is_ok(),
            "a fresh trusted recipient ledger must acknowledge explicit installation"
        );
        let original=issued.expect("the exact claimed file and independent publisher/recipient/issuer generations must persist");
        assert_eq!(original.issuer(), account(1));
        assert_eq!(original.grant().id(), grant(1));
        assert_eq!(original.grant().revision(), revision(1));
        assert_eq!(original.grant().file(), &file(1));
        assert_eq!(original.grant().publisher(), account(2));
        assert_eq!(original.grant().recipient(), account(3));
        assert_eq!(original.grant().validity(), validity());
        assert_eq!(original.grant().state(), FileGrantState::Active);
        assert_eq!(observed.unwrap(), Some(original.clone()));
        assert_eq!(after_get, after_issue);
        let retired=revoked.expect("trusted attribution records a different revoking actor without changing file or parties");
        assert_eq!(retired.issuer(), original.issuer());
        assert_eq!(retired.grant().revision(), revision(2));
        assert_eq!(retired.grant().state(), FileGrantState::Revoked);
        assert_eq!(retired.grant().file(), original.grant().file());
        assert_eq!(retired.grant().publisher(), original.grant().publisher());
        assert_eq!(retired.grant().recipient(), original.grant().recipient());
        assert_eq!(retired.grant().validity(), original.grant().validity());
        assert_eq!(reopened_row.unwrap(), Some(retired));
        assert_eq!(final_snapshot, after_revoke);
        assert_eq!(initial.oids, after_issue.oids);
        assert_eq!(initial.oids, after_revoke.oids);
        for (name, old) in &initial.rows {
            let issued = rows(&after_issue, name);
            let revoked = rows(&after_revoke, name);
            match name.as_str() {
                HEADS => {
                    assert!(old.is_empty());
                    assert_eq!(issued.len(), 1);
                    assert_eq!(revoked.len(), 1);
                }
                CHANGES => {
                    assert!(old.is_empty());
                    assert_eq!(issued.len(), 1);
                    assert_eq!(revoked.len(), 2);
                    assert!(revoked.contains(&issued[0]));
                    let entry: serde_json::Value =
                        serde_json::from_str(revoked.iter().find(|r| !issued.contains(r)).unwrap())
                            .unwrap();
                    assert_eq!(
                        entry["actor_id"],
                        serde_json::json!(account(4).principal().principal_id)
                    );
                    assert_eq!(
                        entry["actor_account_id"],
                        serde_json::json!(account(4).account_id())
                    );
                }
                _ => {
                    assert_eq!(issued, old);
                    assert_eq!(revoked, old);
                }
            }
        }
    }
}
