use super::{
    fixture::*,
    publisher_fixture::{grant, validity, CHANGES, HEADS, MARKER},
    support::*,
    *,
};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_grants_reject_original_source_registry_and_target_relation_replacement(
) {
    for deferred in [false, true] {
        for mode in ["source", "registry", "target"] {
            let f = Fixture::ready().await;
            let mut body = String::new();
            let (schema, tables) = match mode {
                "source" => (&f.base.auth.base.schema, vec!["users"]),
                "registry" => (
                    &f.base.auth.base.schema,
                    vec!["collaboration_identity_schema", "collaboration_principals"],
                ),
                _ => (&f.base.schema, vec![MARKER, CHANGES]),
            };
            for table in tables {
                body.push_str(&format!("ALTER TABLE {schema}.{table} RENAME TO original_{table}; CREATE TABLE {schema}.{table}(LIKE {schema}.original_{table} INCLUDING ALL); INSERT INTO {schema}.{table} SELECT * FROM {schema}.original_{table};"));
            }
            if mode == "target" {
                body.push_str(&format!("UPDATE {}.{MARKER} SET schema_oid='{}.{MARKER}'::regclass::oid::bigint,changes_oid='{}.{CHANGES}'::regclass::oid::bigint;",f.base.schema,f.base.schema,f.base.schema));
            }
            body.push_str("RETURN NEW;");
            fault(&f, HEADS, "INSERT", &body, deferred).await;
            let before = f.base.snapshot().await;
            let result = f
                .base
                .auth
                .source
                .issue_session_file_publisher_grant(
                    &f.base.auth.learner_token,
                    &f.management,
                    grant(1),
                    f.target(),
                    validity(),
                )
                .await;
            let reached = hit(&f).await;
            let after = f.base.snapshot().await;
            remove_fault(&f, HEADS).await;
            let healthy = issue(&f, 1, f.target()).await;
            restored(&f).await;
            f.cleanup().await;
            assert!(
                reached,
                "actual original pin replacement {mode} deferred={deferred}"
            );
            assert_eq!(after, before);
            assert_eq!(
                result,
                Err(if mode == "target" {
                    grant_error(FilePublisherGrantStoreError::UnsupportedSchema)
                } else {
                    session_error(SessionCommandError::Authentication(
                        DurableAuthError::SourceMismatch,
                    ))
                })
            );
            assert!(healthy.is_ok());
        }
    }
}
