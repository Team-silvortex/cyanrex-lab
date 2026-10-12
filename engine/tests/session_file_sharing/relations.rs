use super::support::*;
use crate::{fixture::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_sharing_original_source_registry_publication_and_grant_relations_are_not_adopted(
) {
    for mode in [
        "source",
        "registry",
        "publication",
        "publishers",
        "recipients",
    ] {
        checked(Fixture::ready().await,|f|Box::pin(async move{
            let(schema,tables,marker,rewrites,expected)=match mode{
                "source"=>(&f.base.grants.auth.base.schema,vec!["users"],None,vec![],session(SessionCommandError::Authentication(DurableAuthError::SourceMismatch))),
                "registry"=>(&f.base.grants.auth.base.schema,vec!["collaboration_identity_schema","collaboration_principals"],None,vec![],session(SessionCommandError::Authentication(DurableAuthError::SourceMismatch))),
                "publication"=>(&f.base.publication_schema,vec![PUB_MARKER,PUB_ROWS],Some(PUB_MARKER),vec![("schema_oid",PUB_MARKER),("publications_oid",PUB_ROWS)],publication(FileUploadPublicationStoreError::UnsupportedSchema)),
                "publishers"=>(&f.base.grants.schema,vec![grant_fixture::MARKER,grant_fixture::HEADS],Some(grant_fixture::MARKER),vec![("schema_oid",grant_fixture::MARKER),("grants_oid",grant_fixture::HEADS)],publisher(FilePublisherGrantStoreError::UnsupportedSchema)),
                _=>(&f.schema,vec![MARKER,CHANGES],Some(MARKER),vec![("schema_oid",MARKER),("changes_oid",CHANGES)],recipient(FileRecipientGrantStoreError::UnsupportedSchema)),
            };
            let mut body=String::new();for table in tables{body.push_str(&format!("ALTER TABLE {schema}.{table} RENAME TO original_{table}; CREATE TABLE {schema}.{table}(LIKE {schema}.original_{table} INCLUDING ALL); INSERT INTO {schema}.{table} SELECT * FROM {schema}.original_{table};"));}
            if let Some(marker)=marker{let fields=rewrites.into_iter().map(|(column,table)|format!("{column}='{schema}.{table}'::regclass::oid::bigint")).collect::<Vec<_>>().join(",");body.push_str(&format!("UPDATE {schema}.{marker} SET {fields};"));}body.push_str("RETURN NEW;");
            // These valid ordinary clones rewrite marker OIDs and preserve row contents; a
            // recapture could adopt them. The ORIGINAL independent pins must reject them.
            fault(f,HEADS,"INSERT",&body,true).await;let before=f.snapshot().await;let result=issue(f,1).await;let reached=hit(f).await;assert_eq!(f.snapshot().await,before);remove_fault(f,HEADS).await;issue(f,1).await.unwrap();assert!(reached);assert_eq!(result,Err(expected),"original {mode} pin");
        })).await;
    }
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let ghost = format!("sharing_absent_{}", Uuid::new_v4().simple());
            let absent: bool = query("SELECT to_regnamespace($1) IS NULL AS yes")
                .bind(&ghost)
                .fetch_one(&f.base.grants.auth.base.admin)
                .await
                .unwrap()
                .get("yes");
            assert!(absent);
            fault(
                f,
                CHANGES,
                "INSERT",
                &format!(
                    "PERFORM set_config('search_path','{},{}',true); RETURN NEW;",
                    f.schema, ghost
                ),
                true,
            )
            .await;
            let result = issue(f, 1).await;
            let reached = hit(f).await;
            remove_fault(f, CHANGES).await;
            issue(f, 1).await.unwrap();
            assert!(reached);
            assert_eq!(
                result,
                Err(recipient(FileRecipientGrantStoreError::UnsupportedSchema)),
                "effective schema is unchanged but raw extra entry is forbidden"
            );
        })
    })
    .await;
}
