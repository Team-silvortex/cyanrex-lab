use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_original_source_and_registry_relation_replacement_rolls_back_reservation() {
    for registry in [false, true] {
        let f = Fixture::ready().await;
        let tables = if registry {
            vec!["collaboration_identity_schema", "collaboration_principals"]
        } else {
            vec!["users"]
        };
        let mut body = String::new();
        for table in tables {
            body.push_str(&format!(
                "ALTER TABLE {}.{table} RENAME TO original_{table};
            CREATE TABLE {}.{table}(LIKE {}.original_{table} INCLUDING ALL);
            INSERT INTO {}.{table} SELECT * FROM {}.original_{table};",
                f.auth.base.schema,
                f.auth.base.schema,
                f.auth.base.schema,
                f.auth.base.schema,
                f.auth.base.schema
            ));
        }
        fault(&f, &body, false).await;
        let before = snapshot(&f).await;
        let result = f
            .auth
            .source
            .reserve_session_file_upload(
                &f.auth.learner_token,
                &f.upload_workspace,
                upload(1),
                &manifest(17, 1),
            )
            .await;
        let reached = hit(&f).await;
        let after = snapshot(&f).await;
        f.pool_restored().await;
        f.cleanup().await;
        assert!(
            reached,
            "source relation replacement must execute inside the actual Session upload transaction"
        );
        assert_eq!(
            result,
            Err(SessionFileMetadataError::Session(
                SessionCommandError::Authentication(DurableAuthError::SourceMismatch)
            )),
            "a legal shape/data clone must not replace the original source/registry RelationPin"
        );
        assert_eq!(
            after, before,
            "replacement DDL and tentative upload must all roll back on the same source connection"
        );
    }
    let f = Fixture::ready().await;
    reserve(&f, &f.auth.learner_token, 2, &manifest(17, 2))
        .await
        .unwrap();
    f.cleanup().await;
}
