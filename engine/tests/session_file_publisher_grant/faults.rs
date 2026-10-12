use super::support::*;
use crate::{fixture::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_publisher_grant_selected_corruption_fails_but_foreign_records_stay_hidden(
) {
    for mode in [
        "headpayload",
        "issuer",
        "issueraccount",
        "revision",
        "validity",
        "historypayload",
        "historyactor",
        "missinghistory",
    ] {
        let f = Fixture::ready().await;
        let original = f.issue(1, f.subject()).await;
        let foreign = f.issue(2, f.issuer()).await;
        assert_eq!(
            read(&f, &f.auth.learner_token, 1).await.unwrap(),
            Some(original.clone())
        );
        assert_eq!(
            read(&f, &f.auth.manager_token, 2).await.unwrap(),
            Some(foreign)
        );
        let recorded_at: i64 = query(&format!(
            "SELECT recorded_at FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=1",
            f.schema
        ))
        .bind(grant(1).as_uuid())
        .fetch_one(&f.auth.base.admin)
        .await
        .unwrap()
        .get("recorded_at");
        let affected = match mode {
            "headpayload" => query(&format!(
                "UPDATE {}.{HEADS} SET grant_payload=$1 WHERE grant_id=$2",
                f.schema
            ))
            .bind(b"{".as_slice())
            .bind(grant(1).as_uuid())
            .execute(&f.auth.base.admin)
            .await
            .unwrap()
            .rows_affected(),
            "issuer" => query(&format!(
                "UPDATE {}.{HEADS} SET issuer_id=$1 WHERE grant_id=$2",
                f.schema
            ))
            .bind(Uuid::new_v4())
            .bind(grant(1).as_uuid())
            .execute(&f.auth.base.admin)
            .await
            .unwrap()
            .rows_affected(),
            "issueraccount" => query(&format!(
                "UPDATE {}.{HEADS} SET issuer_account_id=$1 WHERE grant_id=$2",
                f.schema
            ))
            .bind(Uuid::new_v4())
            .bind(grant(1).as_uuid())
            .execute(&f.auth.base.admin)
            .await
            .unwrap()
            .rows_affected(),
            "revision" => query(&format!(
                "UPDATE {}.{HEADS} SET revision=2 WHERE grant_id=$1",
                f.schema
            ))
            .bind(grant(1).as_uuid())
            .execute(&f.auth.base.admin)
            .await
            .unwrap()
            .rows_affected(),
            "validity" => query(&format!(
                "UPDATE {}.{HEADS} SET not_before=1 WHERE grant_id=$1",
                f.schema
            ))
            .bind(grant(1).as_uuid())
            .execute(&f.auth.base.admin)
            .await
            .unwrap()
            .rows_affected(),
            "historypayload" => query(&format!(
                "UPDATE {}.{CHANGES} SET grant_payload=$1 WHERE grant_id=$2",
                f.schema
            ))
            .bind(b"{".as_slice())
            .bind(grant(1).as_uuid())
            .execute(&f.auth.base.admin)
            .await
            .unwrap()
            .rows_affected(),
            "historyactor" => query(&format!(
                "UPDATE {}.{CHANGES} SET actor_account_id=$1 WHERE grant_id=$2",
                f.schema
            ))
            .bind(Uuid::new_v4())
            .bind(grant(1).as_uuid())
            .execute(&f.auth.base.admin)
            .await
            .unwrap()
            .rows_affected(),
            _ => query(&format!(
                "DELETE FROM {}.{CHANGES} WHERE grant_id=$1",
                f.schema
            ))
            .bind(grant(1).as_uuid())
            .execute(&f.auth.base.admin)
            .await
            .unwrap()
            .rows_affected(),
        };
        assert_eq!(
            affected, 1,
            "fault {mode} must actually change the selected record"
        );
        let injected = f.snapshot().await;
        assert_eq!(
            read(&f, &f.auth.learner_token, 1).await,
            Err(SessionFilePublisherGrantError::Grant(
                FilePublisherGrantStoreError::InvalidRecord
            )),
            "{mode}"
        );
        assert_eq!(
            read(&f, &f.auth.manager_token, 1).await.unwrap(),
            None,
            "foreign corrupt bytes/history must not be decoded or disclosed"
        );
        assert_eq!(read(&f, &f.auth.learner_token, 2).await.unwrap(), None);
        assert_eq!(
            read(&f, &f.auth.learner_token, 99).await.unwrap(),
            None,
            "an absent selected head is not a global audit"
        );
        assert_eq!(f.snapshot().await, injected);
        query(&format!("UPDATE {}.{HEADS} SET issuer_id=$1,issuer_account_id=$2,revision=1,not_before=0,grant_payload=$3 WHERE grant_id=$4", f.schema))
            .bind(f.issuer().principal().principal_id.as_uuid()).bind(f.issuer().account_id().as_uuid()).bind(serde_json::to_vec(original.grant()).unwrap()).bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap();
        if mode == "missinghistory" {
            query(&format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) SELECT grant_id,revision,issuer_id,issuer_account_id,not_before,expires_at,$1,grant_payload FROM {}.{HEADS} WHERE grant_id=$2", f.schema, f.schema))
                .bind(recorded_at).bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap();
        } else {
            query(&format!("UPDATE {}.{CHANGES} SET actor_id=$1,actor_account_id=$2,grant_payload=$3 WHERE grant_id=$4", f.schema))
                .bind(f.issuer().principal().principal_id.as_uuid()).bind(f.issuer().account_id().as_uuid()).bind(serde_json::to_vec(original.grant()).unwrap()).bind(grant(1).as_uuid()).execute(&f.auth.base.admin).await.unwrap();
        }
        assert_eq!(
            read(&f, &f.auth.learner_token, 1).await.unwrap(),
            Some(original)
        );
        f.auth.base.drain().await;
        f.cleanup().await;
    }
}
