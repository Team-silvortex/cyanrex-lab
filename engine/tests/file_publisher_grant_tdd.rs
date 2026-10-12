//! Trusted publisher-grant ledger only: no registry authorization, recipients or download gate.
use cyanrex_engine::{models::collaboration::*, services::file_publisher_grant_store::*};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;

#[path = "file_publisher_grant/mod.rs"]
mod cases;

mod baseline {
    use super::cases::fixture::*;

    #[tokio::test]
    #[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_fresh_publisher_grant_issue_persists_and_reopens() {
        let f = Fixture::new(16).await;
        let installed = f.store.install_empty_namespace().await;
        let issued = f
            .store
            .issue(grant(1), account(1), account(2), validity())
            .await;
        let (reopened, pool) = f.reopen().await;
        let observed = reopened.get(grant(1), account(2)).await;
        pool.close().await;
        f.cleanup().await;
        assert!(
            installed.is_ok(),
            "fresh grant ledger must acknowledge explicit installation"
        );
        let record =
            issued.expect("first trusted publisher grant must commit its immutable issuer/history");
        assert_eq!(record.issuer(), account(1));
        assert_eq!(record.grant().id(), grant(1));
        assert_eq!(record.grant().revision(), revision(1));
        assert_eq!(record.grant().workspace(), scope());
        assert_eq!(record.grant().subject(), account(2));
        assert_eq!(record.grant().validity(), validity());
        assert_eq!(record.grant().state(), super::FileGrantState::Active);
        let reopened = observed
            .unwrap()
            .expect("committed grant metadata must survive an independent pool reopening");
        assert_eq!(reopened.issuer(), record.issuer());
        assert_eq!(reopened.grant(), record.grant());
    }
}
