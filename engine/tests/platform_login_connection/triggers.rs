use super::{
    catalog_fixture::{within, Fixture},
    fixture::{otp, password, session_count, snapshot, username, watermark, Prepared},
    storage_helpers::no_connections,
};
use cyanrex_engine::services::auth_service::durable_source::DurableAuthError;
use sqlx_core::{query::query, row::Row};

#[derive(Clone, Copy)]
enum Fault {
    Credentials,
    ExtraSession,
    InsertFailure,
}

impl Fault {
    fn error(self) -> DurableAuthError {
        match self {
            Self::Credentials => DurableAuthError::InvalidCredentials,
            Self::ExtraSession | Self::InsertFailure => DurableAuthError::StorageUnavailable,
        }
    }
}

async fn exercise(
    deferred: bool,
    fault: Fault,
) -> (Result<(), DurableAuthError>, i64, bool, bool, bool) {
    Fixture::checked(|f|Box::pin(async move {
        let prepared=Prepared::new(f).await;
        let source=&f.source;
        f.sql(&format!("CREATE SEQUENCE {source}.platform_login_fault_hits")).await;
        let body=match fault {
            Fault::Credentials=>format!("UPDATE {source}.users SET password_salt=password_salt||'-changed' WHERE username=NEW.username;"),
            Fault::ExtraSession=>format!("INSERT INTO {source}.sessions(token,username,account_id,expires_at) VALUES(repeat('e',64),NEW.username,NEW.account_id,NEW.expires_at);"),
            Fault::InsertFailure=>"RAISE EXCEPTION 'synthetic issuance refusal';".to_owned(),
        };
        f.sql(&format!("CREATE FUNCTION {source}.platform_login_insert_fault() RETURNS trigger
            LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $body$ BEGIN
            IF NEW.token<>repeat('e',64) THEN
                PERFORM pg_catalog.nextval('{source}.platform_login_fault_hits'); {body}
            END IF; RETURN NEW; END $body$")).await;
        let trigger=if deferred {"CONSTRAINT TRIGGER"} else {"TRIGGER"};
        let deferred_clause=if deferred {"DEFERRABLE INITIALLY DEFERRED"} else {""};
        f.sql(&format!("CREATE {trigger} platform_login_fault AFTER INSERT ON {source}.sessions
            {deferred_clause} FOR EACH ROW EXECUTE FUNCTION {source}.platform_login_insert_fault()")).await;
        let connector=prepared.connector().await;
        let before=snapshot(f).await;
        let failed=connector.login(&username(),password(),&otp()).await.map(|_|());
        let hits=within(query(&format!("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits FROM {source}.platform_login_fault_hits"))
            .fetch_one(f.owner())).await.unwrap().get("hits");
        let unchanged=before==snapshot(f).await && watermark(f).await== -1 && session_count(f).await==0;
        drop(connector);
        let no_backend=no_connections(f).await;
        let cleaned=prepared.finish();
        (failed,hits,unchanged,no_backend,cleaned)
    })).await
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_insert_side_effects_never_publish_or_consume_otp() {
    for fault in [
        Fault::Credentials,
        Fault::ExtraSession,
        Fault::InsertFailure,
    ] {
        let (failed, hits, unchanged, no_backend, cleaned) = exercise(false, fault).await;
        // Fixture/role/database/root cleanup precedes every behavioral assertion.
        assert_eq!(failed, Err(fault.error()));
        assert_eq!(hits, 1);
        assert!(unchanged && no_backend && cleaned);
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_deferred_insert_side_effects_are_flushed_before_commit()
{
    for fault in [
        Fault::Credentials,
        Fault::ExtraSession,
        Fault::InsertFailure,
    ] {
        let (failed, hits, unchanged, no_backend, cleaned) = exercise(true, fault).await;
        assert_eq!(failed, Err(fault.error()));
        assert_eq!(hits, 1);
        assert!(unchanged && no_backend && cleaned);
    }
}
