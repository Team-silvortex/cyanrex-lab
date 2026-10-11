use super::{fixture::*, support::*, *};
use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
pub enum Selected {
    Upload(Option<FileUploadReservation>),
    Publication(Option<FileUploadPublicationRecord>),
}
impl Selected {
    pub fn manifest(&self) -> &FileTransferManifest {
        match self {
            Self::Upload(Some(v)) => v.manifest(),
            Self::Publication(Some(v)) => v.manifest(),
            _ => panic!("expected an owned selected row"),
        }
    }
}
pub async fn read(f: &Fixture, publication: bool) -> Result<Selected, SessionFileMetadataError> {
    if publication {
        f.auth
            .source
            .get_session_file_publication(
                &f.auth.learner_token,
                &f.publication_workspace,
                upload(1),
            )
            .await
            .map(Selected::Publication)
    } else {
        f.auth
            .source
            .get_session_file_upload(&f.auth.learner_token, &f.upload_workspace, upload(1))
            .await
            .map(Selected::Upload)
    }
}
pub async fn holder(f: &Fixture, publication: bool, marker: bool) -> Tx<'_> {
    let schema = if publication {
        &f.publication_schema
    } else {
        &f.upload_schema
    };
    let table = match (publication, marker) {
        (true, true) => PUBLICATION_MARKER,
        (true, false) => PUBLICATION_ROWS,
        (false, true) => UPLOAD_MARKER,
        (false, false) => UPLOAD_ROWS,
    };
    let mut tx = f.auth.base.admin.begin().await.unwrap();
    if marker {
        query(&format!(
            "SELECT * FROM {schema}.{table} WHERE singleton FOR UPDATE"
        ))
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    } else {
        query(&format!(
            "SELECT * FROM {schema}.{table} WHERE upload_id=$1 FOR UPDATE"
        ))
        .bind(upload(1).as_uuid())
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    }
    tx
}
pub async fn expire_soon(f: &Fixture) {
    query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '1.3 seconds' WHERE token=$1")
        .bind(source_fixture::token_hash(&f.auth.learner_token))
        .execute(&f.auth.base.pool)
        .await
        .unwrap();
}
pub async fn expired(f: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let elapsed: bool = query(&format!(
                "SELECT expires_at<=clock_timestamp() AS yes FROM {}.sessions WHERE token=$1",
                f.auth.base.schema
            ))
            .bind(source_fixture::token_hash(&f.auth.learner_token))
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("yes");
            if elapsed {
                return true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or(false)
}
pub async fn refresh_expiry(f: &Fixture) {
    query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '12 hours' WHERE token=$1")
        .bind(source_fixture::token_hash(&f.auth.learner_token))
        .execute(&f.auth.base.pool)
        .await
        .unwrap();
}
pub async fn drain_publication(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut tx = f.auth.base.admin.begin().await.unwrap();
        query(&format!(
            "LOCK TABLE {}.{PUBLICATION_MARKER},{}.{PUBLICATION_ROWS} IN ACCESS EXCLUSIVE MODE",
            f.publication_schema, f.publication_schema
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        f.pool_restored().await;
    })
    .await
    .expect("publication read retained a source transaction after cancellation");
}
