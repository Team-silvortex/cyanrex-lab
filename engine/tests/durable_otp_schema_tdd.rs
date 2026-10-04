//! Explicit schema-2 initialization and rejection tests; never migrations of a prepared source.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::{AuthorityId, LegacyUsername, WorkspaceRef},
    services::auth_service::durable_source::{BootstrapError, DurableAuthError, DurableAuthSource},
};
use sqlx_core::{query::query, row::Row, transaction::Transaction};
use sqlx_postgres::{PgPool, PgPoolOptions, Postgres};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::*;

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: authority(),
        workspace_id: id(),
    }
}

async fn bootstrap_guard() -> Transaction<'static, Postgres> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&std::env::var("CYANREX_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut guard = pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.test.bootstrap.event-hooks'))")
        .execute(&mut *guard).await.unwrap();
    guard
}

async fn snapshot(f: &Fixture) -> Vec<String> {
    let mut state = Vec::new();
    for table in ["users", "sessions", "collaboration_auth_source_schema"] {
        state.push(query(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb)::text AS state FROM {table} t"))
            .fetch_one(&f.pool).await.unwrap().get("state"));
    }
    for projection in [
        "SELECT t.oid, t.relname, t.relkind, t.relpersistence FROM pg_class t WHERE t.relnamespace=current_schema()::regnamespace AND t.relname IN ('users','sessions','collaboration_auth_source_schema')",
        "SELECT a.attrelid, a.attnum, a.attname, a.atttypid, a.attnotnull, a.attidentity, a.attgenerated, pg_get_expr(d.adbin,d.adrelid) AS default_value FROM pg_attribute a JOIN pg_class t ON t.oid=a.attrelid LEFT JOIN pg_attrdef d ON d.adrelid=a.attrelid AND d.adnum=a.attnum WHERE t.relnamespace=current_schema()::regnamespace AND a.attnum>0 AND NOT a.attisdropped",
        "SELECT c.oid,c.conrelid,c.conname,c.convalidated,pg_get_constraintdef(c.oid) AS definition FROM pg_constraint c WHERE c.connamespace=current_schema()::regnamespace",
        "SELECT t.oid,pg_get_triggerdef(t.oid) AS definition FROM pg_trigger t JOIN pg_class r ON r.oid=t.tgrelid WHERE r.relnamespace=current_schema()::regnamespace AND NOT t.tgisinternal",
        "SELECT p.oid,pg_get_functiondef(p.oid) AS definition FROM pg_proc p WHERE p.pronamespace=current_schema()::regnamespace",
    ] {
        state.push(query(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text), '[]'::jsonb)::text AS state FROM ({projection}) s"))
            .fetch_one(&f.pool).await.unwrap().get("state"));
    }
    state
}

async fn assert_unconsumed(f: &Fixture) {
    let row = query("SELECT otp_last_counter, (SELECT count(*) FROM sessions) AS sessions, (SELECT version FROM collaboration_auth_source_schema) AS version FROM users")
        .fetch_one(&f.pool).await.unwrap();
    assert_eq!(row.get::<i64, _>("otp_last_counter"), -1);
    assert_eq!(row.get::<i64, _>("sessions"), 0);
    assert_eq!(row.get::<i32, _>("version"), 2);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_schema_one_is_rejected_without_adoption_or_repair() {
    for populated in [false, true] {
        let f = Fixture::new().await;
        // Build the actual old template; dropping a v2 column is not evidence of v1 compatibility.
        for (template, separator) in [
            (
                include_str!("../migrations/0001_auth_users_sessions.sql"),
                ";",
            ),
            (
                include_str!("../migrations/0010_durable_auth_source.sql"),
                "-- cyanrex-statement",
            ),
        ] {
            for statement in template
                .split(separator)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                f.sql(statement).await;
            }
        }
        query("INSERT INTO collaboration_auth_source_schema (singleton,version,authority_id) VALUES (TRUE,1,$1)")
            .bind(authority().as_uuid()).execute(&f.pool).await.unwrap();
        let token = Uuid::new_v4().to_string();
        if populated {
            query("INSERT INTO users (username,account_id,password_salt,password_hash,totp_secret) VALUES ($1,$2,'synthetic-v1-salt','synthetic-v1-hash','JBSWY3DPEHPK3PXP')")
                .bind(username().as_str()).bind(Uuid::new_v4()).execute(&f.pool).await.unwrap();
            query("INSERT INTO sessions (token,username,account_id,expires_at) SELECT $1,username,account_id,clock_timestamp()+INTERVAL '1 hour' FROM users")
                .bind(token_hash(&token)).execute(&f.pool).await.unwrap();
        }
        let before = snapshot(&f).await;
        assert_eq!(
            f.source.install_empty_schema().await,
            Err(DurableAuthError::UnsupportedSchema)
        );
        assert!(matches!(
            f.source
                .register(&"new-account".parse().unwrap(), PASSWORD)
                .await,
            Err(DurableAuthError::UnsupportedSchema)
        ));
        assert!(matches!(
            f.source.login(&username(), PASSWORD, "000000").await,
            Err(DurableAuthError::UnsupportedSchema)
        ));
        assert_eq!(
            f.source.validate_session(&token).await,
            Err(DurableAuthError::UnsupportedSchema)
        );
        assert_eq!(
            f.source.logout(&token).await,
            Err(DurableAuthError::UnsupportedSchema)
        );
        assert_eq!(
            f.source
                .change_session_password(&token, PASSWORD, "synthetic-replacement", "000000")
                .await,
            Err(DurableAuthError::UnsupportedSchema)
        );
        assert_eq!(snapshot(&f).await, before, "populated={populated}");
        let column: bool = query("SELECT EXISTS (SELECT 1 FROM pg_attribute WHERE attrelid='users'::regclass AND attname='otp_last_counter' AND NOT attisdropped) AS present")
            .fetch_one(&f.pool).await.unwrap().get("present");
        assert!(!column, "rejected calls must not add consumption state");
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_schema_two_requires_exact_counter_shape_without_repair() {
    for damage in [
        "ALTER TABLE users DROP COLUMN otp_last_counter CASCADE",
        "ALTER TABLE users DROP CONSTRAINT collaboration_auth_otp_last_counter; ALTER TABLE users ALTER COLUMN otp_last_counter DROP DEFAULT; ALTER TABLE users ALTER COLUMN otp_last_counter TYPE TEXT USING otp_last_counter::TEXT",
        "ALTER TABLE users ALTER COLUMN otp_last_counter DROP NOT NULL; UPDATE users SET otp_last_counter=NULL",
        "ALTER TABLE users ALTER COLUMN otp_last_counter DROP DEFAULT",
        "ALTER TABLE users ALTER COLUMN otp_last_counter SET DEFAULT 0",
        "ALTER TABLE users DROP CONSTRAINT collaboration_auth_otp_last_counter",
        "ALTER TABLE users DROP CONSTRAINT collaboration_auth_otp_last_counter; ALTER TABLE users ADD CONSTRAINT collaboration_auth_otp_last_counter CHECK (TRUE)",
        "ALTER TABLE users DROP CONSTRAINT collaboration_auth_otp_last_counter; ALTER TABLE users ADD CONSTRAINT collaboration_auth_otp_last_counter CHECK (otp_last_counter >= '-2'::BIGINT)",
        "ALTER TABLE users DROP CONSTRAINT collaboration_auth_otp_last_counter; ALTER TABLE users ADD CONSTRAINT collaboration_auth_otp_last_counter CHECK (otp_last_counter >= '-1'::BIGINT) NOT VALID",
        "ALTER TABLE users DROP CONSTRAINT collaboration_auth_otp_last_counter; ALTER TABLE users ADD CONSTRAINT collaboration_auth_otp_last_counter CHECK (length(username)>0)",
    ] {
        let f = Fixture::ready().await;
        let (account, session) = f.seed_account_session().await;
        for statement in damage.split(';') { f.sql(statement).await; }
        let before = snapshot(&f).await;
        assert_eq!(f.source.install_empty_schema().await, Err(DurableAuthError::InvalidRecord), "{damage}");
        assert!(matches!(f.source.register(&"new-account".parse().unwrap(), PASSWORD).await,
            Err(DurableAuthError::InvalidRecord)), "{damage}");
        assert!(matches!(f.source.login(&username(), PASSWORD, &otp(&account.bootstrap.secret)).await,
            Err(DurableAuthError::InvalidRecord)), "{damage}");
        assert_eq!(f.source.validate_session(&session.token).await, Err(DurableAuthError::InvalidRecord), "{damage}");
        assert_eq!(f.source.logout(&session.token).await, Err(DurableAuthError::InvalidRecord), "{damage}");
        assert_eq!(snapshot(&f).await, before, "schema/data repaired by a rejected call: {damage}");
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_schema_fresh_registration_and_bootstrap_start_unconsumed() {
    let _guard = bootstrap_guard().await;
    for bootstrap in [false, true] {
        let f = Fixture::new().await;
        let account = if bootstrap {
            f.source
                .bootstrap_empty_authority(scope(), &username(), PASSWORD)
                .await
                .unwrap()
                .registration
        } else {
            f.source.install_empty_schema().await.unwrap();
            f.source.register(&username(), PASSWORD).await.unwrap()
        };
        assert_unconsumed(&f).await;
        let login = f
            .source
            .login(&username(), PASSWORD, &otp(&account.bootstrap.secret))
            .await
            .unwrap();
        assert_eq!(login.session.account, account.account);
        let consumed: i64 = query("SELECT otp_last_counter FROM users")
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get("otp_last_counter");
        assert!(consumed >= 0);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_schema_initial_counter_faults_are_rechecked_and_rolled_back() {
    let _guard = bootstrap_guard().await;
    for bootstrap in [false, true] {
        let f = Fixture::new().await;
        let trace = format!("otp_schema_trace_{}", Uuid::new_v4().simple());
        let hook = format!("otp_schema_hook_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {trace}"))
            .execute(&f.admin)
            .await
            .unwrap();
        query(&format!("CREATE SEQUENCE {trace}.calls"))
            .execute(&f.admin)
            .await
            .unwrap();
        query(&format!(
            "CREATE FUNCTION {trace}.row_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{trace}.calls'); NEW.otp_last_counter=37; RETURN NEW; END $$"
        ))
        .execute(&f.admin)
        .await
        .unwrap();
        if bootstrap {
            // Keep the bootstrap target empty. A scoped external event hook attaches the row
            // fault only when this exact target's users table is created.
            query(&format!("CREATE FUNCTION {trace}.ddl_fault() RETURNS event_trigger LANGUAGE plpgsql AS $$ DECLARE item record; BEGIN
                FOR item IN SELECT * FROM pg_event_trigger_ddl_commands() LOOP
                    IF item.schema_name='{schema}' AND item.object_type='table' AND item.object_identity='{schema}.users' AND item.command_tag='CREATE TABLE' THEN
                        EXECUTE 'CREATE TRIGGER otp_initial_fault BEFORE INSERT ON {schema}.users FOR EACH ROW EXECUTE FUNCTION {trace}.row_fault()';
                    END IF;
                END LOOP; END $$", schema=f.schema)).execute(&f.admin).await.unwrap();
            query(&format!("CREATE EVENT TRIGGER {hook} ON ddl_command_end EXECUTE FUNCTION {trace}.ddl_fault()"))
                .execute(&f.admin).await.unwrap();
            let result = f
                .source
                .bootstrap_empty_authority(scope(), &username(), PASSWORD)
                .await;
            query(&format!("DROP EVENT TRIGGER {hook}"))
                .execute(&f.admin)
                .await
                .unwrap();
            assert!(matches!(
                result,
                Err(BootstrapError::Authentication(
                    DurableAuthError::StorageUnavailable
                ))
            ));
            let objects: i64 = query("SELECT (SELECT count(*) FROM pg_class WHERE relnamespace=$1::regnamespace) + (SELECT count(*) FROM pg_proc WHERE pronamespace=$1::regnamespace) AS count")
                .bind(&f.schema).fetch_one(&f.admin).await.unwrap().get("count");
            assert_eq!(
                objects, 0,
                "failed bootstrap must leave no initialized source or registry"
            );
        } else {
            f.source.install_empty_schema().await.unwrap();
            f.sql(&format!("CREATE TRIGGER otp_initial_fault BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION {trace}.row_fault()")).await;
            let before = snapshot(&f).await;
            assert!(matches!(
                f.source.register(&username(), PASSWORD).await,
                Err(DurableAuthError::StorageUnavailable)
            ));
            assert_eq!(snapshot(&f).await, before);
        }
        let row = query(&format!("SELECT last_value,is_called FROM {trace}.calls"))
            .fetch_one(&f.admin)
            .await
            .unwrap();
        assert!(
            row.get::<bool, _>("is_called"),
            "rejection must happen after the intended insert fault"
        );
        assert_eq!(row.get::<i64, _>("last_value"), 1);
        query(&format!("DROP SCHEMA {trace} CASCADE"))
            .execute(&f.admin)
            .await
            .unwrap();
        f.cleanup().await;
    }
}
