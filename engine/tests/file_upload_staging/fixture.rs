pub use super::bytes::manifest;
use super::bytes::Pattern;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_transfer::staging::ChunkStaging,
        file_upload_staging::FileUploadStaging,
        file_upload_store::{FileUploadLimits, FileUploadStore},
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    time::Duration,
};
use uuid::Uuid;

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
        principal_id: Uuid::from_u128(value + 100).try_into().unwrap(),
    }
}
pub fn account(value: u128) -> LegacyAccountId {
    Uuid::from_u128(value + 200).try_into().unwrap()
}
pub fn upload(value: u128) -> FileUploadId {
    Uuid::from_u128(value + 300).try_into().unwrap()
}
pub fn limits() -> FileUploadLimits {
    FileUploadLimits::new(MAX_FILE_TRANSFER_BYTES, MAX_FILE_TRANSFER_BYTES, 16, 8).unwrap()
}

pub async fn pool_for(url: &str, schema: &str, max: u32) -> PgPool {
    let selected = schema.to_owned();
    PgPoolOptions::new().max_connections(max).after_connect(move |connection,_| {
        let selected = selected.clone();
        Box::pin(async move {
            query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)")
                .bind(selected).execute(connection).await?;
            Ok(())
        })
    }).connect(url).await.unwrap()
}

pub struct Fixture {
    pub admin: PgPool,
    pub pool: PgPool,
    pub schema: String,
    pub installation_id: Uuid,
    pub source: FileUploadStore,
    pub staging: ChunkStaging,
    pub adapter: FileUploadStaging,
    pub directory: PathBuf,
    pub root: PathBuf,
    pub manifest: FileTransferManifest,
}

impl Fixture {
    pub async fn new(controls: usize, workers: usize) -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("explicit disposable database required");
        let admin = PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .unwrap();
        let schema = format!("file_upload_stage_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let pool = pool_for(&url, &schema, 4).await;
        let installation_id = Uuid::new_v4();
        let source = FileUploadStore::new(
            pool.clone(),
            scope(),
            installation_id,
            schema.clone(),
            limits(),
        )
        .unwrap();
        source.install_empty_namespace().await.unwrap();
        let directory =
            std::env::temp_dir().join(format!("cyanrex-upload-stage-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let root = directory.join("chunks");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let staging = ChunkStaging::open(scope(), &root, workers).unwrap();
        let adapter = FileUploadStaging::new(source.clone(), staging.clone(), controls).unwrap();
        Self {
            admin,
            pool,
            schema,
            installation_id,
            source,
            staging,
            adapter,
            directory,
            root,
            manifest: manifest(17, 1),
        }
    }

    pub async fn ready(length: u64) -> Self {
        let mut fixture = Self::new(4, 2).await;
        fixture.manifest = manifest(length, 1);
        fixture
            .source
            .reserve(upload(1), owner(1), account(1), &fixture.manifest)
            .await
            .unwrap();
        fixture
    }

    pub fn real_file(&self, length: u64) -> File {
        let path = self
            .directory
            .join(format!("source-{}.bin", Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::copy(&mut Pattern::new(0, length), &mut file).unwrap();
        drop(file);
        File::open(path).unwrap()
    }

    pub async fn sql(&self, statement: &str) {
        query(statement).execute(&self.pool).await.unwrap();
    }

    pub async fn rows(&self) -> Vec<Vec<String>> {
        let mut result = Vec::new();
        for table in [MARKER, LEDGER] {
            let mut rows: Vec<String> = query(&format!(
                "SELECT row_to_json(t)::text AS value FROM {table} t"
            ))
            .fetch_all(&self.pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.get("value"))
            .collect();
            rows.sort();
            result.push(rows);
        }
        result
    }

    pub async fn oids(&self) -> Vec<i64> {
        let mut values = Vec::new();
        for table in [MARKER, LEDGER] {
            values.push(
                query("SELECT to_regclass($1)::oid::bigint AS value")
                    .bind(format!("{}.{table}", self.schema))
                    .fetch_one(&self.admin)
                    .await
                    .unwrap()
                    .get("value"),
            );
        }
        values
    }

    pub async fn wait_blocked(&self, blocker: i32) {
        tokio::time::timeout(Duration::from_secs(2),async {
            loop {
                let amount:i64 = query("SELECT count(*) AS n FROM pg_stat_activity WHERE application_name=$1 AND $2=ANY(pg_blocking_pids(pid))")
                    .bind(&self.schema).bind(blocker).fetch_one(&self.admin).await.unwrap().get("n");
                if amount>=1 { return; } tokio::task::yield_now().await;
            }
        }).await.expect("operation did not reach its intended real SQL wait");
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
        .expect("adapter leaked a pooled connection");
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
        fs::remove_dir_all(&self.directory).expect("remove only this exclusive fixture");
    }
}
