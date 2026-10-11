use cyanrex_engine::{models::collaboration::*, services::file_upload_store::*};
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions, Postgres};
use std::time::Duration;
use uuid::Uuid;

pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, Postgres>;
pub const MARKER: &str = "collaboration_file_upload_schema";
pub const LEDGER: &str = "collaboration_file_uploads";

pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "8b075b0b-8118-4c37-9224-337ba22f40c6".parse().unwrap(),
        workspace_id: "267c6b0f-182c-4980-a925-cdc967327c57".parse().unwrap(),
    }
}

pub fn owner(value: u128) -> PrincipalRef {
    PrincipalRef {
        authority_id: scope().authority_id,
        principal_id: Uuid::from_u128(value + 100).to_string().parse().unwrap(),
    }
}

pub fn account(value: u128) -> LegacyAccountId {
    Uuid::from_u128(value + 200).to_string().parse().unwrap()
}

pub fn upload(value: u128) -> FileUploadId {
    Uuid::from_u128(value + 300).to_string().parse().unwrap()
}

pub fn digest(bytes: &[u8]) -> Sha256Digest {
    format!("{:x}", Sha256::digest(bytes)).parse().unwrap()
}

// Claimed metadata only: these hashes do not establish that any file bytes exist.
pub fn manifest(length: u64, variant: u128) -> FileTransferManifest {
    let count = (length / FILE_TRANSFER_CHUNK_BYTES
        + u64::from(length % FILE_TRANSFER_CHUNK_BYTES != 0)) as usize;
    let whole = if length == 0 {
        digest(b"")
    } else {
        digest(format!("claimed-file-{variant}").as_bytes())
    };
    FileTransferManifest::new(
        scope(),
        Uuid::from_u128(variant + 400).try_into().unwrap(),
        Uuid::from_u128(variant + 500).try_into().unwrap(),
        "作业.bin",
        length,
        vec![whole.clone(); count],
        whole,
    )
    .unwrap()
}

pub fn quota_limits(
    total_bytes: u64,
    owner_bytes: u64,
    total_uploads: u32,
    owner_uploads: u32,
) -> FileUploadLimits {
    FileUploadLimits::new(total_bytes, owner_bytes, total_uploads, owner_uploads).unwrap()
}

pub fn limits() -> FileUploadLimits {
    quota_limits(1024, 512, 16, 8)
}

pub fn changed(
    original: &FileTransferManifest,
    key: &str,
    value: serde_json::Value,
) -> FileTransferManifest {
    let mut wire = serde_json::to_value(original).unwrap();
    wire[key] = value;
    FileTransferManifest::parse_json(&serde_json::to_vec(&wire).unwrap()).unwrap()
}

pub async fn pool_for(url: &str, schema: &str, max: u32) -> PgPool {
    let selected = schema.to_owned();
    PgPoolOptions::new().max_connections(max).after_connect(move |connection, _| {
        let selected = selected.clone();
        Box::pin(async move {
            query("SELECT set_config('search_path', $1, false), set_config('application_name', $1, false)")
                .bind(selected).execute(connection).await?;
            Ok(())
        })
    }).connect(url).await.unwrap()
}

pub struct Fixture {
    pub admin: PgPool,
    pub pool: PgPool,
    pub store: FileUploadStore,
    pub schema: String,
    pub installation_id: Uuid,
    pub limits: FileUploadLimits,
    url: String,
}

impl Fixture {
    pub async fn new(limits: FileUploadLimits) -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("explicit disposable database required");
        let admin = PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .unwrap();
        let schema = format!("file_upload_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let pool = pool_for(&url, &schema, 4).await;
        let installation = Uuid::new_v4();
        let store = FileUploadStore::new(
            pool.clone(),
            scope(),
            installation,
            schema.clone(),
            limits.clone(),
        )
        .unwrap();
        Self {
            admin,
            pool,
            store,
            schema,
            installation_id: installation,
            limits,
            url,
        }
    }

    pub async fn ready(limits: FileUploadLimits) -> Self {
        let fixture = Self::new(limits).await;
        fixture.store.install_empty_namespace().await.unwrap();
        fixture
    }

    pub async fn reopen(&self) -> (FileUploadStore, PgPool) {
        let pool = pool_for(&self.url, &self.schema, 4).await;
        let store = FileUploadStore::new(
            pool.clone(),
            scope(),
            self.installation_id,
            self.schema.clone(),
            self.limits.clone(),
        )
        .unwrap();
        (store, pool)
    }

    pub async fn sql(&self, statement: &str) {
        query(statement).execute(&self.pool).await.unwrap();
    }

    pub async fn rows(&self) -> Vec<Vec<String>> {
        let mut all = Vec::new();
        for relation in [MARKER, LEDGER] {
            let mut rows: Vec<String> = query(&format!(
                "SELECT row_to_json(t)::text AS value FROM {relation} t"
            ))
            .fetch_all(&self.pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.get("value"))
            .collect();
            rows.sort();
            all.push(rows);
        }
        all
    }

    pub async fn oids(&self) -> Vec<i64> {
        let mut values = Vec::new();
        for relation in [MARKER, LEDGER] {
            values.push(
                query("SELECT to_regclass($1)::oid::bigint AS oid")
                    .bind(format!("{}.{}", self.schema, relation))
                    .fetch_one(&self.admin)
                    .await
                    .unwrap()
                    .get("oid"),
            );
        }
        values
    }

    pub async fn fault(&self, body: &str, deferred: bool, after: bool) {
        self.sql("CREATE SEQUENCE upload_fault_hits").await;
        self.sql(&format!(
            "CREATE FUNCTION upload_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.upload_fault_hits'); {body} RETURN NEW; END $$",
            self.schema
        ))
        .await;
        self.sql(&format!("CREATE {}TRIGGER upload_fault {} INSERT ON {LEDGER} {}FOR EACH ROW EXECUTE FUNCTION upload_fault()",
            if deferred { "CONSTRAINT " } else { "" }, if deferred || after { "AFTER" } else { "BEFORE" },
            if deferred { "DEFERRABLE INITIALLY DEFERRED " } else { "" })).await;
    }

    pub async fn hit(&self) -> bool {
        query("SELECT is_called FROM upload_fault_hits")
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .get("is_called")
    }

    pub async fn remove_fault(&self) {
        self.sql(&format!("DROP TRIGGER upload_fault ON {LEDGER}"))
            .await;
        self.sql("DROP FUNCTION upload_fault()").await;
        self.sql("DROP SEQUENCE upload_fault_hits").await;
    }

    pub async fn drop_constraints(
        &self,
        relation: &str,
        column: &str,
        kind: &str,
    ) -> Vec<(String, String)> {
        assert!([MARKER, LEDGER].contains(&relation));
        let rows = query(
            "SELECT p.conname::text AS name, pg_get_constraintdef(p.oid) AS definition
            FROM pg_catalog.pg_constraint p JOIN pg_catalog.pg_attribute a
            ON a.attrelid=p.conrelid AND a.attnum=ANY(p.conkey)
            WHERE p.conrelid=to_regclass($1) AND a.attname=$2 AND p.contype::text=$3",
        )
        .bind(relation)
        .bind(column)
        .bind(kind)
        .fetch_all(&self.pool)
        .await
        .unwrap();
        let saved: Vec<(String, String)> = rows
            .into_iter()
            .map(|row| (row.get("name"), row.get("definition")))
            .collect();
        assert!(
            !saved.is_empty(),
            "fault fixture must find the exact structural constraint"
        );
        for (name, _) in &saved {
            self.sql(&format!(
                "ALTER TABLE {relation} DROP CONSTRAINT \"{}\"",
                name.replace('"', "\"\"")
            ))
            .await;
        }
        saved
    }

    pub async fn restore_constraints(&self, relation: &str, saved: Vec<(String, String)>) {
        assert!([MARKER, LEDGER].contains(&relation));
        for (name, definition) in saved {
            self.sql(&format!(
                "ALTER TABLE {relation} ADD CONSTRAINT \"{}\" {definition}",
                name.replace('"', "\"\"")
            ))
            .await;
        }
    }

    pub async fn backend(holder: &mut Tx<'_>) -> i32 {
        query("SELECT pg_backend_pid() AS pid")
            .fetch_one(&mut **holder)
            .await
            .unwrap()
            .get("pid")
    }

    pub async fn wait_blocked(&self, blocker: i32, amount: i64) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let blocked: i64 = query("SELECT count(*) AS n FROM pg_stat_activity WHERE application_name = $1 AND $2 = ANY(pg_blocking_pids(pid))")
                    .bind(&self.schema).bind(blocker).fetch_one(&self.admin).await.unwrap().get("n");
                if blocked >= amount { return; }
                tokio::task::yield_now().await;
            }
        }).await.expect("reservation did not reach the intended real lock");
    }

    pub async fn drain(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut tx = self.admin.begin().await.unwrap();
            query(&format!(
                "LOCK TABLE {}.{MARKER}, {}.{LEDGER} IN ACCESS EXCLUSIVE MODE",
                self.schema, self.schema
            ))
            .execute(&mut *tx)
            .await
            .unwrap();
            tx.rollback().await.unwrap();
        })
        .await
        .expect("cancelled transaction did not release its locks");
    }

    pub async fn pool_restored(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut held = Vec::new();
            for _ in 0..4 {
                let mut connection = self.pool.acquire().await.unwrap();
                let selected: String = query("SELECT current_schema() AS selected")
                    .fetch_one(&mut *connection)
                    .await
                    .unwrap()
                    .get("selected");
                assert_eq!(selected, self.schema);
                held.push(connection);
            }
        })
        .await
        .expect("upload store leaked a pooled connection");
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}
