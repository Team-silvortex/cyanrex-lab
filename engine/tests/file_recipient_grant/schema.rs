use super::cleanup::checked;
use super::{fixture::*, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_recipient_namespace_scope_incarnation_limits_and_relation_shape_are_not_adopted()
{
    let f = Fixture::new(8).await;
    sql(&f, "CREATE TABLE retained(value INTEGER NOT NULL)").await;
    sql(&f, "INSERT INTO retained VALUES(17)").await;
    let before = f.snapshot().await;
    let failed = f.store.install_empty_namespace().await;
    let after = f.snapshot().await;
    f.cleanup().await;
    assert_eq!(failed, Err(FileRecipientGrantStoreError::NamespaceNotEmpty));
    assert_eq!(after, before);
    checked(ready(8).await,|f|Box::pin(async move{let before=f.snapshot().await;assert_eq!(f.store.install_empty_namespace().await,Err(FileRecipientGrantStoreError::NamespaceNotEmpty));assert_eq!(f.snapshot().await,before);
    let wrong=FileRecipientGrantStore::new(f.pool.clone(),scope(),Uuid::new_v4(),&f.schema,8).unwrap();assert_eq!(wrong.get(grant(1),account(3)).await,Err(FileRecipientGrantStoreError::IncarnationMismatch));
    let wrong=FileRecipientGrantStore::new(f.pool.clone(),scope(),f.installation_id,&f.schema,9).unwrap();assert_eq!(wrong.get(grant(1),account(3)).await,Err(FileRecipientGrantStoreError::UnsupportedSchema));
    for foreign in[WorkspaceRef{workspace_id:Uuid::from_u128(9001).try_into().unwrap(),..scope()},WorkspaceRef{authority_id:Uuid::from_u128(9002).try_into().unwrap(),..scope()}]{let wrong=FileRecipientGrantStore::new(f.pool.clone(),foreign,f.installation_id,&f.schema,8).unwrap();let recipient=FileAccountRef::new(PrincipalRef{authority_id:foreign.authority_id,..account(3).principal()},account(3).account_id());assert_eq!(wrong.get(grant(1),recipient).await,Err(FileRecipientGrantStoreError::ScopeMismatch));}
    assert_eq!(f.snapshot().await,before);
    for column in["namespace_oid","schema_oid","grants_oid","changes_oid"]{sql(&f,&format!("UPDATE {MARKER} SET {column}={column}+1")).await;assert_eq!(get(&f,1,account(3)).await,Err(FileRecipientGrantStoreError::UnsupportedSchema));assert_eq!(issue(&f,1,account(1),account(2),account(3),validity()).await,Err(FileRecipientGrantStoreError::UnsupportedSchema));sql(&f,&format!("UPDATE {MARKER} SET {column}={column}-1")).await;assert_eq!(get(&f,1,account(3)).await.unwrap(),None);}
    sql(&f,&format!("ALTER TABLE {HEADS} ALTER COLUMN publisher_account_id DROP NOT NULL")).await;assert_eq!(get(&f,1,account(3)).await,Err(FileRecipientGrantStoreError::UnsupportedSchema));sql(&f,&format!("ALTER TABLE {HEADS} ALTER COLUMN publisher_account_id SET NOT NULL")).await;
    sql(&f,&format!("ALTER TABLE {CHANGES} DROP COLUMN recorded_at")).await;assert_eq!(get(&f,1,account(3)).await,Err(FileRecipientGrantStoreError::UnsupportedSchema));sql(&f,&format!("ALTER TABLE {CHANGES} ADD recorded_at BIGINT NOT NULL CHECK(recorded_at BETWEEN 0 AND 253402300799)")).await;
    sql(&f,&format!("ALTER TABLE {HEADS} ADD invented_file_id UUID")).await;assert_eq!(get(&f,1,account(3)).await,Err(FileRecipientGrantStoreError::UnsupportedSchema));sql(&f,&format!("ALTER TABLE {HEADS} DROP COLUMN invented_file_id")).await;issue(&f,1,account(1),account(2),account(3),validity()).await.unwrap();})).await;
}
