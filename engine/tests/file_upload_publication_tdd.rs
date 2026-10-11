#![cfg(unix)]
#[path = "file_upload_publication/mod.rs"]
mod cases;
#[path = "file_upload_publication/defaults.rs"]
mod defaults;

mod baseline {
    use cyanrex_engine::{
        models::collaboration::WorkspaceRef,
        services::file_upload_publication::FileUploadPublicationStore,
    };
    use sqlx_core::{query::query, row::Row};
    use sqlx_postgres::PgPoolOptions;
    use uuid::Uuid;

    #[tokio::test]
    #[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_publication_installer_acknowledges_one_fresh_empty_namespace() {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("explicit disposable database required");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        let schema = format!("file_publication_install_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let selected = schema.clone();
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .after_connect(move |connection, _| {
                let selected = selected.clone();
                Box::pin(async move {
                    query("SELECT set_config('search_path',$1,false)")
                        .bind(selected)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap();
        let scope = WorkspaceRef {
            authority_id: "8b075b0b-8118-4c37-9224-337ba22f40c6".parse().unwrap(),
            workspace_id: "267c6b0f-182c-4980-a925-cdc967327c57".parse().unwrap(),
        };
        let store =
            FileUploadPublicationStore::new(pool.clone(), scope, Uuid::new_v4(), schema.clone(), 8)
                .unwrap();
        let result = store.install_empty_namespace().await;
        let mut tables: Vec<String> = query(
            "SELECT c.relname AS name FROM pg_catalog.pg_class c
             JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
             WHERE n.nspname=$1 AND c.relkind='r'",
        )
        .bind(&schema)
        .fetch_all(&admin)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get("name"))
        .collect();
        tables.sort();
        let counts = if result.is_ok() {
            let row = query(
                "SELECT
                (SELECT count(*) FROM collaboration_file_publication_schema) AS markers,
                (SELECT count(*) FROM collaboration_file_publications) AS records",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            Some((row.get::<i64, _>("markers"), row.get::<i64, _>("records")))
        } else {
            None
        };
        pool.close().await;
        query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;
        result.expect("a fresh publication installer must acknowledge its empty immutable store");
        assert_eq!(
            tables,
            [
                "collaboration_file_publication_schema",
                "collaboration_file_publications"
            ]
        );
        assert_eq!(counts, Some((1, 0)));
    }

    #[tokio::test]
    #[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_real_confirmed_file_first_publication_returns_receipt_and_persists_metadata()
    {
        use super::cases::{bytes::*, fixture::*};
        use cyanrex_engine::services::file_upload_staging::FileUploadPublicationState;
        let fixture = Fixture::ready(17).await;
        let mut chunk = fixture
            .journaled
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        drop(
            fixture
                .journaled
                .stage_chunk(&mut chunk, Pattern::new(0, 17))
                .await
                .unwrap(),
        );
        let source = fixture.rows(Store::Source).await;
        let journal = fixture.rows(Store::Journal).await;
        let files = tree(&fixture.root);
        let mut attempt = fixture
            .publisher
            .prepare(upload(1), owner(1), account(1))
            .await
            .unwrap();
        let result = fixture.publisher.publish(&mut attempt).await;
        let state = attempt.state();
        let stored = fixture
            .publication
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap();
        let after_source = fixture.rows(Store::Source).await;
        let after_journal = fixture.rows(Store::Journal).await;
        let after_files = tree(&fixture.root);
        let expected = fixture.manifest.clone();
        fixture.cleanup().await;
        let receipt = result
            .expect("a real confirmed file must receive its first publication acknowledgement");
        assert_eq!(state, FileUploadPublicationState::Confirmed);
        assert_eq!(receipt.upload_id(), upload(1));
        assert_eq!(receipt.owner(), owner(1));
        assert_eq!(receipt.account_id(), account(1));
        assert_eq!(receipt.manifest(), &expected);
        let stored = stored.expect("published immutable metadata must survive the acknowledgement");
        assert_eq!(stored.upload_id(), upload(1));
        assert_eq!(stored.manifest(), &expected);
        assert_eq!(after_source, source);
        assert_eq!(after_journal, journal);
        assert_eq!(after_files, files);
    }
}
