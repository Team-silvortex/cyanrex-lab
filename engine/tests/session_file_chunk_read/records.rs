use crate::{fixture::Fixture, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_read_rejects_selected_canonical_history_and_hidden_binding_corruption() {
    checked(Fixture::ready(1).await,|f|Box::pin(async move {
        healthy(f).await;let original=f.sharing.snapshot().await;
        let record=f.sharing.store.get(f.request().download.grant_id,f.sharing.recipient).await.unwrap().unwrap();
        let raw=serde_json::to_vec(record.grant()).unwrap();let g=record.grant();
        let recorded:i64=query(&format!("SELECT recorded_at FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=1",f.sharing.schema)).bind(g.id().as_uuid()).fetch_one(&auth(f).base.admin).await.unwrap().get("recorded_at");
        for mode in ["headpayload","noncanonical","historypayload","actor","missinghistory","extrahistory"] {
            match mode {
                "headpayload"|"noncanonical"|"historypayload"=>{let table=if mode=="historypayload" {CHANGES}else{HEADS};let bad=if mode=="noncanonical" {[b" ".as_slice(),raw.as_slice()].concat()}else{b"{".to_vec()};
                    query(&format!("UPDATE {}.{table} SET grant_payload=$1 WHERE grant_id=$2",f.sharing.schema)).bind(bad).bind(g.id().as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                "actor"=>{query(&format!("UPDATE {}.{CHANGES} SET actor_account_id=$1 WHERE grant_id=$2",f.sharing.schema)).bind(Uuid::new_v4()).bind(g.id().as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                "missinghistory"=>{query(&format!("DELETE FROM {}.{CHANGES} WHERE grant_id=$1",f.sharing.schema)).bind(g.id().as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                _=>{query(&format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) SELECT grant_id,2,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=1",f.sharing.schema,f.sharing.schema)).bind(g.id().as_uuid()).execute(&auth(f).base.admin).await.unwrap();}
            }
            assert_eq!(read(f,&f.request()).await,Err(recipient(FileRecipientGrantStoreError::InvalidRecord)),"{mode}");
            assert_eq!(read_as(f,&auth(f).source,&auth(f).manager_token,&f.workspace,&f.request()).await,Err(denied()),"foreign damaged grant stays excluded before decode");
            query(&format!("UPDATE {}.{HEADS} SET grant_payload=$1 WHERE grant_id=$2",f.sharing.schema)).bind(&raw).bind(g.id().as_uuid()).execute(&auth(f).base.admin).await.unwrap();
            query(&format!("DELETE FROM {}.{CHANGES} WHERE grant_id=$1",f.sharing.schema)).bind(g.id().as_uuid()).execute(&auth(f).base.admin).await.unwrap();
            query(&format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) VALUES($1,1,$2,$3,$4,$5,$6,$7)",f.sharing.schema))
                .bind(g.id().as_uuid()).bind(record.issuer().principal().principal_id.as_uuid()).bind(record.issuer().account_id().as_uuid())
                .bind(g.validity().not_before() as i64).bind(g.validity().expires_at() as i64).bind(recorded).bind(&raw).execute(&auth(f).base.admin).await.unwrap();
            assert_eq!(f.sharing.snapshot().await,original);healthy(f).await;
        }
        let row=query(&format!("SELECT manifest,source_key,root_key,journal_key,nonce FROM {}.{PUB_ROWS} WHERE upload_id=$1",f.sharing.base.publication_schema)).bind(f.request().download.upload_id.as_uuid()).fetch_one(&auth(f).base.admin).await.unwrap();
        for mode in ["manifest","noncanonical","source_key","root_key","journal_key","nonce","valid_source_key","valid_root_key"] {
            let mut removed=None;
            if ["source_key","root_key","journal_key","nonce"].contains(&mode) {
                let constraint=query("SELECT conname AS n,pg_get_constraintdef(oid) AS d FROM pg_constraint WHERE conrelid=to_regclass($1) AND contype='c' AND pg_get_constraintdef(oid) LIKE $2")
                    .bind(format!("{}.{PUB_ROWS}",f.sharing.base.publication_schema)).bind(format!("%{mode}%")).fetch_one(&auth(f).base.admin).await.unwrap();
                let name:String=constraint.get("n");let definition:String=constraint.get("d");sql(f,&format!("ALTER TABLE {}.{PUB_ROWS} DROP CONSTRAINT {name}",f.sharing.base.publication_schema)).await;removed=Some((name,definition));
            }
            match mode {
                "nonce"=>{query(&format!("UPDATE {}.{PUB_ROWS} SET nonce=$1 WHERE upload_id=$2",f.sharing.base.publication_schema)).bind(Uuid::nil()).bind(f.request().download.upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                "source_key"|"root_key"|"journal_key"|"valid_source_key"|"valid_root_key"=>{let column=mode.strip_prefix("valid_").unwrap_or(mode);let length=if mode.starts_with("valid_") {32}else{31};
                    query(&format!("UPDATE {}.{PUB_ROWS} SET {column}=$1 WHERE upload_id=$2",f.sharing.base.publication_schema)).bind(vec![37u8;length]).bind(f.request().download.upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();},
                _=>{let original:Vec<u8>=row.get("manifest");let bad=if mode=="noncanonical" {[b" ".as_slice(),original.as_slice()].concat()}else{b"{".to_vec()};
                    query(&format!("UPDATE {}.{PUB_ROWS} SET manifest=$1 WHERE upload_id=$2",f.sharing.base.publication_schema)).bind(bad).bind(f.request().download.upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();}
            }
            let error=if mode.starts_with("valid_") {denied()}else{publication(FileUploadPublicationStoreError::InvalidRecord)};
            assert_eq!(read(f,&f.request()).await,Err(error),"{mode}");
            assert_eq!(read(f,&request(f,99)).await,Err(denied()));
            query(&format!("UPDATE {}.{PUB_ROWS} SET manifest=$1,source_key=$2,root_key=$3,journal_key=$4,nonce=$5 WHERE upload_id=$6",f.sharing.base.publication_schema))
                .bind(row.get::<Vec<u8>,_>("manifest")).bind(row.get::<Vec<u8>,_>("source_key")).bind(row.get::<Vec<u8>,_>("root_key")).bind(row.get::<Vec<u8>,_>("journal_key"))
                .bind(row.get::<Uuid,_>("nonce")).bind(f.request().download.upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();
            if let Some((name,definition))=removed {sql(f,&format!("ALTER TABLE {}.{PUB_ROWS} ADD CONSTRAINT {name} {definition}",f.sharing.base.publication_schema)).await;}
            assert_eq!(f.sharing.snapshot().await,original);healthy(f).await;
        }drain(f).await;
    })).await;
}
