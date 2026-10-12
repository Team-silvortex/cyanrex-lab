use crate::{fixture::Fixture, support::*, *};

async fn restore_grant(f: &Fixture, recorded: i64) {
    let g = f.issued.grant();
    let raw = serde_json::to_vec(g).unwrap();
    let issuer = f.issued.issuer();
    query(&format!("UPDATE {}.{HEADS} SET publisher_id=$1,publisher_account_id=$2,issuer_id=$3,issuer_account_id=$4,revision=1,not_before=$5,expires_at=$6,grant_payload=$7 WHERE grant_id=$8",f.sharing.schema))
        .bind(g.publisher().principal().principal_id.as_uuid()).bind(g.publisher().account_id().as_uuid())
        .bind(issuer.principal().principal_id.as_uuid()).bind(issuer.account_id().as_uuid())
        .bind(g.validity().not_before() as i64).bind(g.validity().expires_at() as i64).bind(&raw).bind(g.id().as_uuid()).execute(&auth(f).base.admin).await.unwrap();
    query(&format!(
        "DELETE FROM {}.{CHANGES} WHERE grant_id=$1",
        f.sharing.schema
    ))
    .bind(g.id().as_uuid())
    .execute(&auth(f).base.admin)
    .await
    .unwrap();
    query(&format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) VALUES($1,1,$2,$3,$4,$5,$6,$7)",f.sharing.schema))
        .bind(g.id().as_uuid()).bind(issuer.principal().principal_id.as_uuid()).bind(issuer.account_id().as_uuid())
        .bind(g.validity().not_before() as i64).bind(g.validity().expires_at() as i64).bind(recorded).bind(raw).execute(&auth(f).base.admin).await.unwrap();
}
async fn remove_check(f: &Fixture, table: &str, column: &str) -> (String, String) {
    let row=query("SELECT conname AS n,pg_get_constraintdef(oid) AS d FROM pg_constraint WHERE conrelid=to_regclass($1) AND contype='c' AND pg_get_constraintdef(oid) LIKE $2")
        .bind(format!("{}.{table}",f.sharing.base.publication_schema)).bind(format!("%{column}%"))
        .fetch_one(&auth(f).base.admin).await.unwrap();
    let name: String = row.get("n");
    let def: String = row.get("d");
    sql(
        f,
        &format!(
            "ALTER TABLE {}.{table} DROP CONSTRAINT {name}",
            f.sharing.base.publication_schema
        ),
    )
    .await;
    (name, def)
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_download_rejects_selected_recipient_history_and_hidden_publication_corruption() {
    checked(Fixture::ready().await, |f| Box::pin(async move {
        healthy(f).await;
        let original=f.sharing.snapshot().await;
        let raw=serde_json::to_vec(f.issued.grant()).unwrap();
        let recorded:i64=query(&format!("SELECT recorded_at FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=1",f.sharing.schema))
            .bind(f.request().grant_id.as_uuid()).fetch_one(&auth(f).base.admin).await.unwrap().get("recorded_at");
        for mode in ["headpayload","noncanonical","issuer","publisher","revision","historypayload","historyactor","missinghistory","extrahistory","historyfile"] {
            match mode {
                "headpayload"|"noncanonical"=>{let bad=if mode=="noncanonical" {[b" ".as_slice(),raw.as_slice()].concat()}else{b"{".to_vec()};
                    query(&format!("UPDATE {}.{HEADS} SET grant_payload=$1 WHERE grant_id=$2",f.sharing.schema)).bind(bad).bind(f.request().grant_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                "issuer"|"publisher"=>{let column=if mode=="issuer" {"issuer_account_id"}else{"publisher_account_id"};
                    query(&format!("UPDATE {}.{HEADS} SET {column}=$1 WHERE grant_id=$2",f.sharing.schema)).bind(Uuid::new_v4()).bind(f.request().grant_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                "revision"=>{sql(f,&format!("UPDATE {}.{HEADS} SET revision=2 WHERE grant_id='{}'",f.sharing.schema,f.request().grant_id)).await;},
                "missinghistory"=>{query(&format!("DELETE FROM {}.{CHANGES} WHERE grant_id=$1",f.sharing.schema)).bind(f.request().grant_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                "extrahistory"=>{query(&format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) SELECT grant_id,2,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=1",f.sharing.schema,f.sharing.schema)).bind(f.request().grant_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                "historyactor"=>{query(&format!("UPDATE {}.{CHANGES} SET actor_account_id=$1 WHERE grant_id=$2",f.sharing.schema)).bind(Uuid::new_v4()).bind(f.request().grant_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                _=>{let bad=if mode=="historyfile" {let mut value=serde_json::to_value(f.issued.grant()).unwrap();value["file"]["sha256"]=serde_json::json!("b".repeat(64));
                    serde_json::to_vec(&serde_json::from_value::<FileRecipientGrant>(value).unwrap()).unwrap()}else{b"{".to_vec()};
                    query(&format!("UPDATE {}.{CHANGES} SET grant_payload=$1 WHERE grant_id=$2",f.sharing.schema)).bind(bad).bind(f.request().grant_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();}
            }
            assert_eq!(read(f,&f.request()).await,Err(recipient(FileRecipientGrantStoreError::InvalidRecord)),"selected {mode}");
            assert_eq!(read_as(f,&auth(f).manager_token,&f.workspace,&f.request()).await,Err(SessionFileDownloadError::PermissionDenied),"foreign corrupt grant stays filtered");
            restore_grant(f,recorded).await;assert_eq!(f.sharing.snapshot().await,original);healthy(f).await;
        }
        let row=query(&format!("SELECT manifest,source_key,root_key,journal_key,nonce FROM {}.{PUB_ROWS} WHERE upload_id=$1",f.sharing.base.publication_schema))
            .bind(f.request().upload_id.as_uuid()).fetch_one(&auth(f).base.admin).await.unwrap();
        let manifest:Vec<u8>=row.get("manifest");let nonce:Uuid=row.get("nonce");
        for mode in ["manifest","noncanonical","bodylength","source_key","root_key","journal_key","nonce"] {
            let mut removed=None;
            if ["source_key","root_key","journal_key","nonce"].contains(&mode) {removed=Some(remove_check(f,PUB_ROWS,mode).await);}
            match mode {
                "nonce"=>{query(&format!("UPDATE {}.{PUB_ROWS} SET nonce=$1 WHERE upload_id=$2",f.sharing.base.publication_schema))
                    .bind(Uuid::nil()).bind(f.request().upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                "source_key"|"root_key"|"journal_key"=>{query(&format!("UPDATE {}.{PUB_ROWS} SET {mode}=$1 WHERE upload_id=$2",f.sharing.base.publication_schema))
                    .bind(vec![0u8;31]).bind(f.request().upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                _=>{let bad=match mode {"noncanonical"=>[b" ".as_slice(),manifest.as_slice()].concat(),"bodylength"=>serde_json::to_vec(&bytes::manifest(18,1)).unwrap(),_=>b"{".to_vec()};
                    query(&format!("UPDATE {}.{PUB_ROWS} SET manifest=$1 WHERE upload_id=$2",f.sharing.base.publication_schema)).bind(bad).bind(f.request().upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();}
            }
            let expected=if mode=="bodylength" {SessionFileDownloadError::PermissionDenied}else{publication(FileUploadPublicationStoreError::InvalidRecord)};
            assert_eq!(read(f,&f.request()).await,Err(expected),"selected publication {mode}");
            let mut r=f.request();r.grant_id=sharing_fixture::recipient_grant(99);
            assert_eq!(read(f,&r).await,Err(SessionFileDownloadError::PermissionDenied),"without grant no Published decoding is attempted");
            query(&format!("UPDATE {}.{PUB_ROWS} SET manifest=$1,source_key=$2,root_key=$3,journal_key=$4,nonce=$5 WHERE upload_id=$6",f.sharing.base.publication_schema))
                .bind(&manifest).bind(row.get::<Vec<u8>,_>("source_key")).bind(row.get::<Vec<u8>,_>("root_key")).bind(row.get::<Vec<u8>,_>("journal_key"))
                .bind(nonce).bind(f.request().upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();
            if let Some((name,definition))=removed {sql(f,&format!("ALTER TABLE {}.{PUB_ROWS} ADD CONSTRAINT {name} {definition}",f.sharing.base.publication_schema)).await;}
            assert_eq!(f.sharing.snapshot().await,original);healthy(f).await;
        }drain(f).await;
    })).await;
}
