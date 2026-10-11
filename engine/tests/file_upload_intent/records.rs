use super::fixture::*;
use cyanrex_engine::{models::collaboration::*, services::file_upload_store::FileUploadStoreError};
use sqlx_core::query::query;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_corrupt_foreign_ledger_and_oversized_manifests_fail_closed_without_writes() {
    let fixture = Fixture::ready(limits()).await;
    fixture
        .store
        .reserve(upload(1), owner(1), account(1), &manifest(11, 1))
        .await
        .unwrap();
    let foreign = manifest(11, 2);
    fixture
        .store
        .reserve(upload(2), owner(2), account(2), &foreign)
        .await
        .unwrap();
    let size_checks = fixture.drop_constraints(LEDGER, "manifest", "c").await;
    let canonical = serde_json::to_vec(&foreign).unwrap();
    let mut damaged = vec![
        b"not-json".to_vec(),
        b"[]".to_vec(),
        b"{}".to_vec(),
        [b" \n".as_slice(), canonical.as_slice()].concat(),
        vec![b' '; MAX_FILE_TRANSFER_MANIFEST_BYTES + 1],
    ];
    let encoded = String::from_utf8(canonical.clone()).unwrap();
    damaged.push(format!("{{\"schema_version\":1,{}", &encoded[1..]).into_bytes());
    for (key, value) in [
        ("schema_version", serde_json::json!(2)),
        ("display_name", serde_json::json!("../not-a-path")),
        ("byte_length", serde_json::json!(0)),
        ("file_id", serde_json::json!(uuid::Uuid::nil().to_string())),
        (
            "workspace",
            serde_json::json!({"authority_id": scope().authority_id,
            "workspace_id": uuid::Uuid::from_u128(901).to_string()}),
        ),
        ("claimed_permission", serde_json::json!("not authority")),
    ] {
        let mut wire = serde_json::to_value(&foreign).unwrap();
        wire[key] = value;
        damaged.push(serde_json::to_vec(&wire).unwrap());
    }
    for bytes in damaged {
        query(&format!(
            "UPDATE {LEDGER} SET manifest=$1 WHERE upload_id=$2"
        ))
        .bind(bytes)
        .bind(upload(2).as_uuid())
        .execute(&fixture.pool)
        .await
        .unwrap();
        let before = fixture.rows().await;
        assert_eq!(
            fixture.store.get(upload(2), owner(2), account(2)).await,
            Err(FileUploadStoreError::InvalidRecord)
        );
        assert_eq!(
            fixture
                .store
                .reserve(upload(3), owner(1), account(3), &manifest(1, 3))
                .await,
            Err(FileUploadStoreError::InvalidRecord),
            "foreign corruption must not become zero quota usage"
        );
        assert_eq!(fixture.rows().await, before);
        query(&format!(
            "UPDATE {LEDGER} SET manifest=$1 WHERE upload_id=$2"
        ))
        .bind(&canonical)
        .bind(upload(2).as_uuid())
        .execute(&fixture.pool)
        .await
        .unwrap();
        let restored = fixture.rows().await;
        assert!(fixture
            .store
            .get(upload(2), owner(2), account(2))
            .await
            .unwrap()
            .is_some());
        assert_eq!(fixture.rows().await, restored);
    }
    for column in ["owner_id", "account_id"] {
        let original = if column == "owner_id" {
            owner(2).principal_id.as_uuid()
        } else {
            account(2).as_uuid()
        };
        query(&format!(
            "UPDATE {LEDGER} SET {column}=$1 WHERE upload_id=$2"
        ))
        .bind(uuid::Uuid::nil())
        .bind(upload(2).as_uuid())
        .execute(&fixture.pool)
        .await
        .unwrap();
        let before = fixture.rows().await;
        assert_eq!(
            fixture
                .store
                .reserve(upload(3), owner(1), account(3), &manifest(1, 3))
                .await,
            Err(FileUploadStoreError::InvalidRecord)
        );
        assert_eq!(fixture.rows().await, before);
        query(&format!(
            "UPDATE {LEDGER} SET {column}=$1 WHERE upload_id=$2"
        ))
        .bind(original)
        .bind(upload(2).as_uuid())
        .execute(&fixture.pool)
        .await
        .unwrap();
        assert!(fixture
            .store
            .get(upload(2), owner(2), account(2))
            .await
            .unwrap()
            .is_some());
    }
    fixture.restore_constraints(LEDGER, size_checks).await;
    // A different Principal's valid canonical records still must obey that Principal's budgets.
    query(&format!(
        "UPDATE {LEDGER} SET manifest=$1 WHERE upload_id=$2"
    ))
    .bind(serde_json::to_vec(&manifest(513, 2)).unwrap())
    .bind(upload(2).as_uuid())
    .execute(&fixture.pool)
    .await
    .unwrap();
    let before = fixture.rows().await;
    assert_eq!(
        fixture
            .store
            .reserve(upload(3), owner(1), account(3), &manifest(1, 3))
            .await,
        Err(FileUploadStoreError::InvalidRecord),
        "foreign owner exceeds 512 bytes but global usage remains below 1024"
    );
    assert_eq!(fixture.rows().await, before);
    query(&format!(
        "UPDATE {LEDGER} SET manifest=$1 WHERE upload_id=$2"
    ))
    .bind(&canonical)
    .bind(upload(2).as_uuid())
    .execute(&fixture.pool)
    .await
    .unwrap();
    for seed in 100..108 {
        query(&format!(
            "INSERT INTO {LEDGER}(upload_id,owner_id,account_id,manifest) VALUES($1,$2,$3,$4)"
        ))
        .bind(upload(seed).as_uuid())
        .bind(owner(2).principal_id.as_uuid())
        .bind(account(seed).as_uuid())
        .bind(serde_json::to_vec(&manifest(0, seed)).unwrap())
        .execute(&fixture.pool)
        .await
        .unwrap();
    }
    let before = fixture.rows().await;
    assert_eq!(
        fixture
            .store
            .reserve(upload(3), owner(1), account(3), &manifest(1, 3))
            .await,
        Err(FileUploadStoreError::InvalidRecord),
        "foreign owner has nine intents across generations, global count is only ten"
    );
    assert_eq!(fixture.rows().await, before);
    for seed in 100..108 {
        query(&format!("DELETE FROM {LEDGER} WHERE upload_id=$1"))
            .bind(upload(seed).as_uuid())
            .execute(&fixture.pool)
            .await
            .unwrap();
    }
    let before = fixture.rows().await;
    assert!(fixture
        .store
        .get(upload(2), owner(2), account(2))
        .await
        .unwrap()
        .is_some());
    assert_eq!(fixture.rows().await, before);
    fixture
        .store
        .reserve(upload(3), owner(1), account(3), &manifest(1, 3))
        .await
        .unwrap();
    fixture.cleanup().await;

    let maximum = Fixture::ready(quota_limits(
        MAX_FILE_TRANSFER_BYTES,
        MAX_FILE_TRANSFER_BYTES,
        1,
        1,
    ))
    .await;
    let body = manifest(MAX_FILE_TRANSFER_BYTES, 900);
    assert_eq!(body.chunk_sha256().len(), 16_384);
    assert!(serde_json::to_vec(&body).unwrap().len() < MAX_FILE_TRANSFER_MANIFEST_BYTES);
    let reserved = maximum
        .store
        .reserve(upload(900), owner(1), account(1), &body)
        .await
        .unwrap();
    let before = maximum.rows().await;
    assert_eq!(
        maximum
            .store
            .get(upload(900), owner(1), account(1))
            .await
            .unwrap(),
        Some(reserved)
    );
    assert_eq!(maximum.rows().await, before);
    maximum.cleanup().await;

    let overflow = Fixture::ready(quota_limits(0, 0, 256, 256)).await;
    for seed in 1..=257 {
        query(&format!(
            "INSERT INTO {LEDGER}(upload_id,owner_id,account_id,manifest) VALUES($1,$2,$3,$4)"
        ))
        .bind(upload(seed).as_uuid())
        .bind(owner(1).principal_id.as_uuid())
        .bind(account(1).as_uuid())
        .bind(serde_json::to_vec(&manifest(0, seed)).unwrap())
        .execute(&overflow.pool)
        .await
        .unwrap();
    }
    let before = overflow.rows().await;
    assert_eq!(
        overflow
            .store
            .reserve(upload(900), owner(2), account(2), &manifest(0, 900))
            .await,
        Err(FileUploadStoreError::InvalidRecord),
        "257th scalar ID must fail before unbounded ledger decode"
    );
    assert_eq!(overflow.rows().await, before);
    query(&format!("DELETE FROM {LEDGER} WHERE upload_id=$1"))
        .bind(upload(257).as_uuid())
        .execute(&overflow.pool)
        .await
        .unwrap();
    let before = overflow.rows().await;
    assert_eq!(
        overflow
            .store
            .reserve(upload(900), owner(2), account(2), &manifest(0, 900))
            .await,
        Err(FileUploadStoreError::QuotaExceeded)
    );
    assert_eq!(overflow.rows().await, before);
    assert!(overflow
        .store
        .get(upload(1), owner(1), account(1))
        .await
        .unwrap()
        .is_some());
    overflow.cleanup().await;
}
