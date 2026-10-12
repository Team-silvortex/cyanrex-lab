use super::support::*;
use crate::{fixture::*, *};

fn changed_history(original: &StoredFileRecipientGrant, field: &str) -> Vec<u8> {
    let mut file = serde_json::to_value(original.grant().file()).unwrap();
    match field {
        "workspace" => file["workspace"]["workspace_id"] = serde_json::json!(id::<WorkspaceId>()),
        "file_id" => file["file_id"] = serde_json::json!(id::<FileId>()),
        "version_id" => file["version_id"] = serde_json::json!(id::<FileVersionId>()),
        "byte_length" => file["byte_length"] = serde_json::json!(12),
        "sha256" => file["sha256"] = serde_json::json!("b".repeat(64)),
        _ => panic!("unknown immutable file coordinate"),
    }
    let file: FileVersionRef = serde_json::from_value(file).unwrap();
    let grant = FileRecipientGrant::new(
        original.grant().id(),
        revision(1),
        file,
        original.grant().publisher(),
        original.grant().recipient(),
        original.grant().validity(),
        FileGrantState::Active,
    )
    .unwrap();
    serde_json::to_vec(&grant).unwrap()
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_recipient_selected_canonical_and_history_faults_do_not_leak_foreign_records(
) {
    for mode in [
        "headpayload",
        "noncanonical",
        "oversize",
        "issuer",
        "publisher",
        "revision",
        "validity",
        "historypayload",
        "historyactor",
        "missinghistory",
        "extrahistory",
        "workspace",
        "file_id",
        "version_id",
        "byte_length",
        "sha256",
    ] {
        checked(Fixture::ready().await, move |f| Box::pin(async move {
            let original = f.issue(1, f.recipient()).await;
            let foreign = f.issue(2, f.issuer()).await;
            assert_eq!(read(f, &f.auth.learner_token, 1).await.unwrap(), Some(original.clone()));
            assert_eq!(read(f, &f.auth.manager_token, 2).await.unwrap(), Some(foreign.clone()));
            let raw = serde_json::to_vec(original.grant()).unwrap();
            let recorded_at: i64 = query(&format!(
                "SELECT recorded_at FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=1", f.schema))
                .bind(grant(1).as_uuid()).fetch_one(&f.auth.base.admin).await.unwrap().get("recorded_at");
            let mut removed_check = None;
            if mode == "oversize" {
                let constraint = query("SELECT conname AS n,pg_get_constraintdef(oid) AS d
                    FROM pg_constraint WHERE conrelid=to_regclass($1) AND contype='c'
                    AND pg_get_constraintdef(oid) LIKE '%octet_length%'")
                    .bind(format!("{}.{HEADS}", f.schema)).fetch_one(&f.auth.base.admin).await.unwrap();
                let name: String = constraint.get("n");
                let definition: String = constraint.get("d");
                query(&format!("ALTER TABLE {}.{HEADS} DROP CONSTRAINT {name}", f.schema))
                    .execute(&f.auth.base.admin).await.unwrap();
                removed_check = Some((name, definition));
            }
            let affected = match mode {
                "headpayload" | "noncanonical" | "oversize" => {
                    let bad = match mode {
                        "noncanonical" => [b" ".as_slice(), raw.as_slice()].concat(),
                        "oversize" => vec![b' '; MAX_FILE_GRANT_JSON_BYTES + 1],
                        _ => b"{".to_vec(),
                    };
                    query(&format!("UPDATE {}.{HEADS} SET grant_payload=$1 WHERE grant_id=$2", f.schema))
                        .bind(bad).bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap().rows_affected()
                }
                "issuer" | "publisher" => {
                    let column = if mode == "issuer" { "issuer_account_id" } else { "publisher_account_id" };
                    query(&format!("UPDATE {}.{HEADS} SET {column}=$1 WHERE grant_id=$2", f.schema))
                        .bind(Uuid::new_v4()).bind(grant(1).as_uuid())
                        .execute(&f.auth.base.admin).await.unwrap().rows_affected()
                }
                "revision" | "validity" => {
                    let assignment = if mode == "revision" { "revision=2" } else { "not_before=1" };
                    query(&format!("UPDATE {}.{HEADS} SET {assignment} WHERE grant_id=$1", f.schema))
                        .bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap().rows_affected()
                }
                "historyactor" => query(&format!(
                    "UPDATE {}.{CHANGES} SET actor_account_id=$1 WHERE grant_id=$2", f.schema))
                    .bind(Uuid::new_v4()).bind(grant(1).as_uuid())
                    .execute(&f.auth.base.admin).await.unwrap().rows_affected(),
                "missinghistory" => query(&format!("DELETE FROM {}.{CHANGES} WHERE grant_id=$1", f.schema))
                    .bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap().rows_affected(),
                "extrahistory" => query(&format!(
                    "INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,
                        not_before,expires_at,recorded_at,grant_payload)
                    SELECT grant_id,2,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload
                    FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=1", f.schema, f.schema))
                    .bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap().rows_affected(),
                field => {
                    let bad = if field == "historypayload" { b"{".to_vec() } else { changed_history(&original, field) };
                    query(&format!("UPDATE {}.{CHANGES} SET grant_payload=$1 WHERE grant_id=$2", f.schema))
                        .bind(bad).bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap().rows_affected()
                }
            };
            assert_eq!(affected, 1, "fault {mode} must alter the intended selected row");
            let injected = f.snapshot().await;
            assert_eq!(read(f, &f.auth.learner_token, 1).await,
                Err(SessionFileRecipientGrantError::Grant(FileRecipientGrantStoreError::InvalidRecord)), "{mode}");
            assert_eq!(read(f, &f.auth.manager_token, 1).await.unwrap(), None,
                "a foreign corrupt payload/history must be filtered before decoding");
            assert_eq!(read(f, &f.auth.learner_token, 2).await.unwrap(), None);
            assert_eq!(read(f, &f.auth.learner_token, 99).await.unwrap(), None);
            assert_eq!(f.snapshot().await, injected);
            // Restore the exact first record/history, not a new claim or adopted revision.
            query(&format!("UPDATE {}.{HEADS} SET issuer_id=$1,issuer_account_id=$2,
                publisher_id=$3,publisher_account_id=$4,revision=1,not_before=$5,expires_at=$6,
                grant_payload=$7 WHERE grant_id=$8", f.schema))
                .bind(f.issuer().principal().principal_id.as_uuid()).bind(f.issuer().account_id().as_uuid())
                .bind(f.publisher().principal().principal_id.as_uuid()).bind(f.publisher().account_id().as_uuid())
                .bind(validity().not_before() as i64).bind(validity().expires_at() as i64)
                .bind(&raw).bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap();
            if mode == "missinghistory" {
                query(&format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,
                    not_before,expires_at,recorded_at,grant_payload) VALUES($1,1,$2,$3,$4,$5,$6,$7)", f.schema))
                    .bind(grant(1).as_uuid()).bind(f.issuer().principal().principal_id.as_uuid())
                    .bind(f.issuer().account_id().as_uuid()).bind(validity().not_before() as i64)
                    .bind(validity().expires_at() as i64).bind(recorded_at).bind(&raw)
                    .execute(&f.auth.base.admin).await.unwrap();
            } else {
                query(&format!("DELETE FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=2", f.schema))
                    .bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap();
                query(&format!("UPDATE {}.{CHANGES} SET actor_id=$1,actor_account_id=$2,grant_payload=$3
                    WHERE grant_id=$4 AND revision=1", f.schema))
                    .bind(f.issuer().principal().principal_id.as_uuid()).bind(f.issuer().account_id().as_uuid())
                    .bind(&raw).bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap();
            }
            if let Some((name, definition)) = removed_check {
                query(&format!("ALTER TABLE {}.{HEADS} ADD CONSTRAINT {name} {definition}", f.schema))
                    .execute(&f.auth.base.admin).await.unwrap();
            }
            assert_eq!(read(f, &f.auth.learner_token, 1).await.unwrap(), Some(original));
            assert_eq!(read(f, &f.auth.manager_token, 2).await.unwrap(), Some(foreign));
            drain(f).await;
        })).await;
    }
}
