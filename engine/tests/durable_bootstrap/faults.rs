use super::*;

/// DDL-time injection is required because production bootstrap rejects preinstalled objects.
/// These event triggers are confined to one random fixture schema on the disposable test DB.
pub struct Fault {
    namespace: String,
    table: String,
}
impl Fault {
    pub async fn install(
        f: &Fixture,
        table: &str,
        timing: &str,
        body: &str,
        deferred: bool,
    ) -> Self {
        let namespace = format!("bootstrap_fault_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {namespace}"))
            .execute(&f.admin)
            .await
            .unwrap();
        query(&format!("CREATE SEQUENCE {namespace}.hits"))
            .execute(&f.admin)
            .await
            .unwrap();
        query(&format!("CREATE FUNCTION {namespace}.row_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('{namespace}.hits'); {body} END $$"))
            .execute(&f.admin).await.unwrap();
        let kind = if deferred { "CONSTRAINT " } else { "" };
        let defer = if deferred {
            "DEFERRABLE INITIALLY DEFERRED "
        } else {
            ""
        };
        query(&format!("CREATE FUNCTION {namespace}.on_table() RETURNS event_trigger LANGUAGE plpgsql AS $event$
            DECLARE item record; BEGIN
                FOR item IN SELECT * FROM pg_event_trigger_ddl_commands() LOOP
                    IF item.schema_name = '{schema}' AND item.object_type = 'table'
                        AND item.object_identity = '{schema}.{table}'
                        AND NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgrelid = item.objid AND tgname = 'bootstrap_fault') THEN
                        EXECUTE 'CREATE {kind}TRIGGER bootstrap_fault {timing} ON {schema}.{table} {defer}FOR EACH ROW EXECUTE FUNCTION {namespace}.row_fault()';
                    END IF;
                END LOOP;
            END $event$", schema = f.schema)).execute(&f.admin).await.unwrap();
        query(&format!("CREATE EVENT TRIGGER {namespace} ON ddl_command_end WHEN TAG IN ('CREATE TABLE') EXECUTE FUNCTION {namespace}.on_table()"))
            .execute(&f.admin).await.unwrap();
        Self {
            namespace,
            table: table.to_string(),
        }
    }
    pub async fn assert_fired(&self, f: &Fixture) {
        // Sequence increments survive rollback: prove the intended row boundary was exercised,
        // instead of accepting an unrelated earlier DDL failure as a passing fault test.
        assert!(
            query(&format!("SELECT is_called FROM {}.hits", self.namespace))
                .fetch_one(&f.admin)
                .await
                .unwrap()
                .get::<bool, _>("is_called"),
            "intended row fault for {} was not reached",
            self.table
        );
    }
    pub async fn remove(self, f: &Fixture) {
        query(&format!("DROP EVENT TRIGGER {}", self.namespace))
            .execute(&f.admin)
            .await
            .unwrap();
        query(&format!("DROP SCHEMA {} CASCADE", self.namespace))
            .execute(&f.admin)
            .await
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL with event-trigger privileges"]
async fn postgres_bootstrap_suppressed_graph_writes_leave_no_schema_or_manager() {
    let _guard = fixture_guard().await;
    for (table, timing) in [
        ("collaboration_auth_source_schema", "BEFORE INSERT"),
        ("users", "BEFORE INSERT"),
        ("collaboration_authorities", "BEFORE INSERT"),
        ("collaboration_workspaces", "BEFORE INSERT"),
        ("collaboration_legacy_workspaces", "BEFORE INSERT"),
        ("collaboration_principals", "BEFORE INSERT"),
        ("collaboration_legacy_identities", "BEFORE INSERT"),
        ("collaboration_memberships", "BEFORE INSERT"),
        ("collaboration_deployment_grants", "BEFORE INSERT"),
        ("collaboration_policy_audit", "BEFORE INSERT"),
        ("collaboration_identity_audit", "BEFORE INSERT"),
        ("collaboration_identity_schema", "BEFORE UPDATE"),
        ("collaboration_access_schema", "BEFORE UPDATE"),
    ] {
        let f = Fixture::new().await;
        let fault = Fault::install(&f, table, timing, "RETURN NULL;", false).await;
        assert!(
            f.source
                .bootstrap_empty_authority(scope(), &username(), PASSWORD)
                .await
                .is_err(),
            "{table}"
        );
        f.drain().await;
        fault.assert_fired(&f).await;
        assert_eq!(objects(&f).await, 0, "{table}");
        fault.remove(&f).await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL with event-trigger privileges"]
async fn postgres_bootstrap_post_write_checks_reject_tampered_graph_and_credentials() {
    let _guard = fixture_guard().await;
    for (table, timing, body) in [
        ("users", "BEFORE INSERT", "NEW.totp_secret = 'JBSWY3DPEHPK3PXP'; RETURN NEW;"),
        ("collaboration_memberships", "BEFORE INSERT", "NEW.role_refs = ARRAY['cyanrex.teaching.learner']; RETURN NEW;"),
        ("collaboration_deployment_grants", "BEFORE INSERT", "NEW.status = 'revoked'; RETURN NEW;"),
        ("collaboration_policy_audit", "AFTER INSERT", "UPDATE users SET password_hash = '$argon2-corrupt'; RETURN NEW;"),
        ("collaboration_identity_audit", "AFTER INSERT", "DELETE FROM users; RETURN NEW;"),
        ("collaboration_identity_schema", "AFTER UPDATE", "UPDATE collaboration_principals SET status = 'disabled'; RETURN NEW;"),
        ("collaboration_identity_schema", "AFTER UPDATE", "UPDATE collaboration_workspaces SET status = 'archived'; RETURN NEW;"),
        ("collaboration_identity_schema", "AFTER UPDATE", "UPDATE collaboration_auth_source_schema SET authority_id = gen_random_uuid(); RETURN NEW;"),
        ("collaboration_access_schema", "BEFORE UPDATE", "NEW.version = 1; RETURN NEW;"),
        ("collaboration_identity_schema", "AFTER UPDATE", "INSERT INTO sessions (token, username, account_id, expires_at) SELECT repeat('a',64), username, account_id, clock_timestamp() + INTERVAL '1 hour' FROM users; RETURN NEW;"),
        ("collaboration_identity_schema", "AFTER UPDATE", "INSERT INTO collaboration_authorities (authority_id) VALUES (gen_random_uuid()); RETURN NEW;"),
    ] {
        let f = Fixture::new().await;
        let fault = Fault::install(&f, table, timing, body, false).await;
        assert!(f.source.bootstrap_empty_authority(scope(), &username(), PASSWORD).await.is_err(), "{body}");
        f.drain().await;
        fault.assert_fired(&f).await;
        assert_eq!(objects(&f).await, 0, "{table}");
        fault.remove(&f).await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL with event-trigger privileges"]
async fn postgres_bootstrap_deferred_commit_failure_rolls_back_ddl_and_authority() {
    let _guard = fixture_guard().await;
    for table in ["users", "collaboration_identity_audit"] {
        let f = Fixture::new().await;
        let fault = Fault::install(
            &f,
            table,
            "AFTER INSERT",
            "RAISE EXCEPTION 'synthetic bootstrap commit failure';",
            true,
        )
        .await;
        assert!(matches!(
            f.source
                .bootstrap_empty_authority(scope(), &username(), PASSWORD)
                .await,
            Err(BootstrapError::Authentication(
                DurableAuthError::StorageUnavailable
            ))
        ));
        fault.assert_fired(&f).await;
        assert_eq!(objects(&f).await, 0);
        fault.remove(&f).await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL with event-trigger privileges"]
async fn postgres_bootstrap_cancelled_write_is_not_published_and_confirmed_rollback_can_retry() {
    let _guard = fixture_guard().await;
    let f = Fixture::new().await;
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 71001)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let fault = Fault::install(
        &f,
        "collaboration_identity_audit",
        "AFTER INSERT",
        "PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 71001); RETURN NEW;",
        false,
    )
    .await;
    let source = f.source.clone();
    let pending = tokio::spawn(async move {
        source
            .bootstrap_empty_authority(scope(), &username(), PASSWORD)
            .await
    });
    f.wait_blocked().await;
    assert_eq!(
        objects(&f).await,
        0,
        "uncommitted DDL/credentials must not be visible"
    );
    pending.abort();
    assert!(pending.await.is_err_and(|e| e.is_cancelled()));
    blocker.rollback().await.unwrap();
    f.drain().await;
    fault.assert_fired(&f).await;
    assert_eq!(objects(&f).await, 0);
    fault.remove(&f).await;
    // This retry is deliberate only AFTER confirming rollback of this synthetic transaction.
    f.source
        .bootstrap_empty_authority(scope(), &username(), PASSWORD)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_bootstrap_installer_lock_timeout_never_seeds_state() {
    let _guard = fixture_guard().await;
    let f = Fixture::new().await;
    let mut blocker = f.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.auth-source.' || current_schema()))").execute(&mut *blocker).await.unwrap();
    assert!(matches!(
        f.source
            .bootstrap_empty_authority(scope(), &username(), PASSWORD)
            .await,
        Err(BootstrapError::Authentication(
            DurableAuthError::StorageUnavailable
        ))
    ));
    assert_eq!(objects(&f).await, 0);
    blocker.rollback().await.unwrap();
    f.cleanup().await;
}
