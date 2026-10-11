pub use super::bytes::manifest;
use super::bytes::Pattern;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_transfer::staging::ChunkStaging,
        file_upload_chunk_journal::FileUploadChunkJournal,
        file_upload_staging::{FileUploadStaging, JournaledFileUploadStaging},
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

pub const SOURCE_MARKER: &str = "collaboration_file_upload_schema";
pub const SOURCE_ROWS: &str = "collaboration_file_uploads";
pub const JOURNAL_MARKER: &str = "collaboration_file_upload_chunk_schema";
pub const JOURNAL_ROWS: &str = "collaboration_file_upload_chunks";

pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "8b075b0b-8118-4c37-9224-337ba22f40c6".parse().unwrap(),
        workspace_id: "267c6b0f-182c-4980-a925-cdc967327c57".parse().unwrap(),
    }
}
pub fn owner(seed: u128) -> PrincipalRef {
    PrincipalRef {
        authority_id: scope().authority_id,
        principal_id: Uuid::from_u128(seed + 100).try_into().unwrap(),
    }
}
pub fn account(seed: u128) -> LegacyAccountId {
    Uuid::from_u128(seed + 200).try_into().unwrap()
}
pub fn upload(seed: u128) -> FileUploadId {
    Uuid::from_u128(seed + 300).try_into().unwrap()
}
pub fn limits() -> FileUploadLimits {
    FileUploadLimits::new(MAX_FILE_TRANSFER_BYTES, MAX_FILE_TRANSFER_BYTES, 16, 8).unwrap()
}

pub async fn pool_for(url: &str, schema: &str, max: u32) -> PgPool {
    let selected = schema.to_owned();
    PgPoolOptions::new().max_connections(max).after_connect(move |connection,_| {
        let selected=selected.clone();Box::pin(async move {
            query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)")
                .bind(selected).execute(connection).await?;Ok(())
        })
    }).connect(url).await.unwrap()
}

pub struct Fixture {
    pub source_admin: PgPool,
    pub source_pool: PgPool,
    pub source_schema: String,
    pub source_installation_id: Uuid,
    pub source: FileUploadStore,
    pub journal_admin: PgPool,
    pub journal_pool: PgPool,
    pub journal_schema: String,
    pub journal_installation_id: Uuid,
    pub journal: FileUploadChunkJournal,
    pub local: FileUploadStaging,
    pub journaled: JournaledFileUploadStaging,
    pub staging: ChunkStaging,
    pub root: PathBuf,
    pub directory: PathBuf,
    pub manifest: FileTransferManifest,
    pub entries: u32,
}

impl Fixture {
    pub async fn new(controls: usize, workers: usize, entries: u32) -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("explicit disposable database required");
        let source_admin = PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .unwrap();
        let journal_admin = PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .unwrap();
        let source_schema = format!("file_chunk_source_{}", Uuid::new_v4().simple());
        let journal_schema = format!("file_chunk_journal_{}", Uuid::new_v4().simple());
        for schema in [&source_schema, &journal_schema] {
            query(&format!("CREATE SCHEMA {schema}"))
                .execute(&source_admin)
                .await
                .unwrap();
        }
        let source_pool = pool_for(&url, &source_schema, 4).await;
        let journal_pool = pool_for(&url, &journal_schema, 4).await;
        let source_installation_id = Uuid::new_v4();
        let journal_installation_id = Uuid::new_v4();
        let source = FileUploadStore::new(
            source_pool.clone(),
            scope(),
            source_installation_id,
            source_schema.clone(),
            limits(),
        )
        .unwrap();
        source.install_empty_namespace().await.unwrap();
        let journal = FileUploadChunkJournal::new(
            journal_pool.clone(),
            scope(),
            journal_installation_id,
            journal_schema.clone(),
            entries,
        )
        .unwrap();
        let directory =
            std::env::temp_dir().join(format!("cyanrex-chunk-journal-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let root = directory.join("chunks");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let staging = ChunkStaging::open(scope(), &root, workers).unwrap();
        let local = FileUploadStaging::new(source.clone(), staging.clone(), controls).unwrap();
        let journaled = JournaledFileUploadStaging::new(local.clone(), journal.clone()).unwrap();
        Self {
            source_admin,
            source_pool,
            source_schema,
            source_installation_id,
            source,
            journal_admin,
            journal_pool,
            journal_schema,
            journal_installation_id,
            journal,
            local,
            journaled,
            staging,
            root,
            directory,
            manifest: manifest(17, 1),
            entries,
        }
    }

    pub async fn ready(length: u64) -> Self {
        let mut fixture = Self::new(4, 2, 16).await;
        fixture.journal.install_empty_namespace().await.unwrap();
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

    pub async fn sql(&self, journal: bool, statement: &str) {
        query(statement)
            .execute(if journal {
                &self.journal_pool
            } else {
                &self.source_pool
            })
            .await
            .unwrap();
    }
    pub async fn rows(&self, journal: bool) -> Vec<Vec<String>> {
        let tables = if journal {
            [JOURNAL_MARKER, JOURNAL_ROWS]
        } else {
            [SOURCE_MARKER, SOURCE_ROWS]
        };
        let pool = if journal {
            &self.journal_pool
        } else {
            &self.source_pool
        };
        let mut result = Vec::new();
        for table in tables {
            let mut rows: Vec<String> = query(&format!(
                "SELECT row_to_json(t)::text AS value FROM {table} t"
            ))
            .fetch_all(pool)
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
    pub async fn oids(&self, journal: bool) -> Vec<i64> {
        let tables = if journal {
            [JOURNAL_MARKER, JOURNAL_ROWS]
        } else {
            [SOURCE_MARKER, SOURCE_ROWS]
        };
        let schema = if journal {
            &self.journal_schema
        } else {
            &self.source_schema
        };
        let mut values = Vec::new();
        for table in tables {
            values.push(
                query("SELECT to_regclass($1)::oid::bigint AS value")
                    .bind(format!("{schema}.{table}"))
                    .fetch_one(&self.journal_admin)
                    .await
                    .unwrap()
                    .get("value"),
            );
        }
        values
    }
    pub async fn wait_blocked(&self, journal: bool, pid: i32) {
        let schema = if journal {
            &self.journal_schema
        } else {
            &self.source_schema
        };
        tokio::time::timeout(Duration::from_secs(2),async {loop {
            let count:i64=query("SELECT count(*) AS n FROM pg_stat_activity WHERE application_name=$1 AND $2=ANY(pg_blocking_pids(pid))")
                .bind(schema).bind(pid).fetch_one(&self.journal_admin).await.unwrap().get("n");
            if count>0 {return;}tokio::task::yield_now().await;
        }}).await.expect("operation did not reach its intended real SQL wait");
    }
    pub async fn pool_restored(&self, journal: bool) {
        let pool = if journal {
            &self.journal_pool
        } else {
            &self.source_pool
        };
        let schema = if journal {
            &self.journal_schema
        } else {
            &self.source_schema
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut held = Vec::new();
            for _ in 0..4 {
                let mut connection = pool.acquire().await.unwrap();
                let selected: String = query("SELECT current_schema() AS value")
                    .fetch_one(&mut *connection)
                    .await
                    .unwrap()
                    .get("value");
                assert_eq!(&selected, schema);
                held.push(connection);
            }
        })
        .await
        .expect("operation leaked pooled connections");
    }
    pub async fn cleanup(self) {
        self.source_pool.close().await;
        self.journal_pool.close().await;
        for schema in [&self.journal_schema, &self.source_schema] {
            query(&format!("DROP SCHEMA {schema} CASCADE"))
                .execute(&self.journal_admin)
                .await
                .unwrap();
        }
        self.source_admin.close().await;
        self.journal_admin.close().await;
        fs::remove_dir_all(&self.directory).expect("remove only this exclusive fixture");
    }
}
