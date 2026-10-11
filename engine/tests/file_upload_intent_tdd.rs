#[path = "file_upload_intent/mod.rs"]
mod cases;

mod defaults {
    use super::cases::fixture::*;
    use cyanrex_engine::{
        models::collaboration::{FileUploadId, MAX_FILE_TRANSFER_BYTES},
        services::file_upload_store::{
            FileUploadLimits, FileUploadStore, FileUploadStoreError, MAX_FILE_UPLOAD_INTENTS,
        },
    };
    use sqlx_postgres::PgPoolOptions;
    use uuid::Uuid;

    #[test]
    fn quota_configuration_is_bounded_and_allows_zero_byte_budgets() {
        assert_eq!(MAX_FILE_UPLOAD_INTENTS, 256);
        let maximum = MAX_FILE_TRANSFER_BYTES * u64::from(MAX_FILE_UPLOAD_INTENTS);
        for (total_bytes, owner_bytes, total_uploads, owner_uploads) in [
            (0, 0, 1, 1),
            (1024, 512, 16, 8),
            (maximum, maximum, 256, 256),
        ] {
            let limits =
                FileUploadLimits::new(total_bytes, owner_bytes, total_uploads, owner_uploads)
                    .unwrap();
            assert_eq!(limits.total_bytes(), total_bytes);
            assert_eq!(limits.owner_bytes(), owner_bytes);
            assert_eq!(limits.total_uploads(), total_uploads);
            assert_eq!(limits.owner_uploads(), owner_uploads);
        }
        for values in [
            (0, 1, 1, 1),
            (maximum + 1, 0, 1, 1),
            (u64::MAX, 0, 1, 1),
            (1, 1, 0, 0),
            (1, 1, 1, 0),
            (1, 1, 1, 2),
            (1, 1, 257, 1),
            (1, 1, u32::MAX, u32::MAX),
        ] {
            assert_eq!(
                FileUploadLimits::new(values.0, values.1, values.2, values.3),
                Err(FileUploadStoreError::InvalidInput)
            );
        }
    }

    #[test]
    fn upload_identity_is_distinct_nonnil_and_strictly_serialized() {
        let identity = upload(1);
        assert_eq!(identity.as_uuid(), Uuid::from_u128(301));
        assert_eq!(
            serde_json::from_value::<FileUploadId>(serde_json::json!(identity)).unwrap(),
            identity
        );
        assert!(FileUploadId::try_from(Uuid::nil()).is_err());
        for value in [
            serde_json::Value::Null,
            serde_json::json!(1),
            serde_json::json!({"id": identity}),
            serde_json::json!(Uuid::nil().to_string()),
            serde_json::json!("not-a-uuid"),
        ] {
            assert!(serde_json::from_value::<FileUploadId>(value).is_err());
        }
    }

    #[tokio::test]
    async fn installation_constructor_requires_random_rfc_uuid_without_io() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://fixture:fixture@127.0.0.1:1/unreachable")
            .unwrap();
        assert!(FileUploadStore::new(
            pool.clone(),
            scope(),
            Uuid::new_v4(),
            "private_uploads",
            limits()
        )
        .is_ok());
        for invalid in [
            Uuid::nil(),
            Uuid::from_u128(1),
            Uuid::parse_str("01234567-89ab-4cde-0123-456789abcdef").unwrap(),
            Uuid::parse_str("01234567-89ab-1cde-8123-456789abcdef").unwrap(),
        ] {
            assert!(matches!(
                FileUploadStore::new(pool.clone(), scope(), invalid, "private_uploads", limits()),
                Err(FileUploadStoreError::InvalidInput)
            ));
        }
        pool.close().await;
    }

    #[tokio::test]
    async fn closed_storage_rejects_operations_without_fallback_and_scope_precedes_io() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://fixture:fixture@127.0.0.1:1/unreachable")
            .unwrap();
        pool.close().await;
        let store =
            FileUploadStore::new(pool, scope(), Uuid::new_v4(), "private_uploads", limits())
                .unwrap();
        let body = manifest(1, 1);
        assert_eq!(
            store.install_empty_namespace().await,
            Err(FileUploadStoreError::StorageUnavailable)
        );
        assert_eq!(
            store.reserve(upload(1), owner(1), account(1), &body).await,
            Err(FileUploadStoreError::StorageUnavailable)
        );
        assert_eq!(
            store.get(upload(1), owner(1), account(1)).await,
            Err(FileUploadStoreError::StorageUnavailable)
        );
        let mut foreign = owner(1);
        foreign.authority_id = Uuid::from_u128(901).try_into().unwrap();
        assert_eq!(
            store.get(upload(1), foreign, account(1)).await,
            Err(FileUploadStoreError::ScopeMismatch)
        );
        assert_eq!(
            store.reserve(upload(1), foreign, account(1), &body).await,
            Err(FileUploadStoreError::ScopeMismatch)
        );
        let mut foreign_scope = scope();
        foreign_scope.workspace_id = Uuid::from_u128(902).try_into().unwrap();
        let foreign_body = changed(&body, "workspace", serde_json::json!(foreign_scope));
        assert_eq!(
            store
                .reserve(upload(1), owner(1), account(1), &foreign_body)
                .await,
            Err(FileUploadStoreError::ScopeMismatch)
        );
    }
}

mod baseline {
    use super::cases::fixture::*;
    use cyanrex_engine::services::file_upload_store::FileUploadStore;
    use sqlx_core::{query::query, row::Row};

    #[tokio::test]
    #[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_first_reservation_persists_and_reopens() {
        let fixture = Fixture::new(limits()).await;
        fixture.store.install_empty_namespace().await.unwrap();
        let expected = manifest(11, 1);
        let registered = fixture
            .store
            .reserve(upload(1), owner(1), account(1), &expected)
            .await;
        let reopened = FileUploadStore::new(
            fixture.pool.clone(),
            scope(),
            fixture.installation_id,
            fixture.schema.clone(),
            limits(),
        )
        .unwrap();
        let observed = reopened.get(upload(1), owner(1), account(1)).await;
        let table_count: i64 =
            query("SELECT count(*) AS n FROM pg_catalog.pg_tables WHERE schemaname=$1")
                .bind(&fixture.schema)
                .fetch_one(&fixture.admin)
                .await
                .unwrap()
                .get("n");
        fixture.cleanup().await;
        let registered = registered.expect("first valid reservation must persist");
        assert_eq!(registered.upload_id(), upload(1));
        assert_eq!(registered.owner(), owner(1));
        assert_eq!(registered.account_id(), account(1));
        assert_eq!(registered.manifest(), &expected);
        assert_eq!(observed.unwrap(), Some(registered));
        assert_eq!(table_count, 2);
    }
}
