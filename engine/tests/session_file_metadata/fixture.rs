use super::*;
use sha2::{Digest, Sha256};
use std::time::Duration;

pub const UPLOAD_MARKER: &str = "collaboration_file_upload_schema";
pub const UPLOAD_ROWS: &str = "collaboration_file_uploads";
pub const PUBLICATION_MARKER: &str = "collaboration_file_publication_schema";
pub const PUBLICATION_ROWS: &str = "collaboration_file_publications";

pub fn upload(seed: u128) -> FileUploadId {
    Uuid::from_u128(seed + 300).try_into().unwrap()
}
pub fn manifest(length: u64, seed: u128) -> FileTransferManifest {
    assert!(
        length <= 17,
        "this metadata fixture is not a maximum-size byte benchmark"
    );
    let digest: Sha256Digest = format!("{:x}", Sha256::digest(vec![0x63; length as usize]))
        .parse()
        .unwrap();
    FileTransferManifest::new(
        scope(),
        Uuid::from_u128(seed + 400).try_into().unwrap(),
        Uuid::from_u128(seed + 500).try_into().unwrap(),
        "metadata-only.bin",
        length,
        if length == 0 {
            vec![]
        } else {
            vec![digest.clone()]
        },
        digest,
    )
    .unwrap()
}
pub fn limits() -> FileUploadLimits {
    FileUploadLimits::new(1024, 1024, 16, 8).unwrap()
}
pub async fn pool_for(schema: &str) -> PgPool {
    let selected = schema.to_owned();
    PgPoolOptions::new().max_connections(4).after_connect(move|connection,_|{let selected=selected.clone();Box::pin(async move{
        query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)")
            .bind(selected).execute(connection).await?;Ok(())
    })}).connect(&std::env::var("CYANREX_TEST_DATABASE_URL").expect("explicit disposable database")).await.unwrap()
}
pub struct Fixture {
    pub auth: identity_fixture::Fixture,
    pub uploads: FileUploadStore,
    pub publications: FileUploadPublicationStore,
    pub upload_workspace: SessionFileUploadWorkspace,
    pub publication_workspace: SessionFilePublicationWorkspace,
    pub upload_schema: String,
    pub publication_schema: String,
    pub upload_pool: PgPool,
    pub publication_pool: PgPool,
    pub upload_installation_id: Uuid,
    pub publication_installation_id: Uuid,
}
impl Fixture {
    pub async fn ready() -> Self {
        Self::with_limits(limits()).await
    }
    pub async fn with_limits(budget: FileUploadLimits) -> Self {
        let auth = identity_fixture::Fixture::ready().await;
        let current = auth.current(auth.learner.binding.principal_id).await;
        let mut desired = current.policy;
        desired.membership.role_refs.clear();
        auth.source
            .apply_session_policy_command(
                &auth.manager_token,
                &SessionPolicyCommand {
                    command_id: id(),
                    expected_revision: Some(current.revision),
                    desired,
                },
            )
            .await
            .unwrap();
        let upload_schema = format!("session_file_upload_{}", Uuid::new_v4().simple());
        let publication_schema = format!("session_file_publication_{}", Uuid::new_v4().simple());
        for schema in [&upload_schema, &publication_schema] {
            query(&format!("CREATE SCHEMA {schema}"))
                .execute(&auth.base.admin)
                .await
                .unwrap();
        }
        let upload_pool = pool_for(&upload_schema).await;
        let publication_pool = pool_for(&publication_schema).await;
        let upload_installation_id = Uuid::new_v4();
        let publication_installation_id = Uuid::new_v4();
        let uploads = FileUploadStore::new(
            upload_pool.clone(),
            scope(),
            upload_installation_id,
            &upload_schema,
            budget,
        )
        .unwrap();
        uploads.install_empty_namespace().await.unwrap();
        let publications = FileUploadPublicationStore::new(
            publication_pool.clone(),
            scope(),
            publication_installation_id,
            &publication_schema,
            8,
        )
        .unwrap();
        publications.install_empty_namespace().await.unwrap();
        let upload_workspace = auth
            .source
            .file_upload_workspace(&upload_schema, scope(), upload_installation_id, budget)
            .unwrap();
        let publication_workspace = auth
            .source
            .file_publication_workspace(
                &publication_schema,
                scope(),
                publication_installation_id,
                8,
            )
            .unwrap();
        Self {
            auth,
            uploads,
            publications,
            upload_workspace,
            publication_workspace,
            upload_schema,
            publication_schema,
            upload_pool,
            publication_pool,
            upload_installation_id,
            publication_installation_id,
        }
    }
    /// Qualified snapshots of all ordinary tables in this exact synthetic namespace, not only the
    /// selected user. Auxiliary sequences/functions are deliberately not represented as rows.
    pub async fn rows(&self, schema: &str) -> Vec<(String, Vec<String>)> {
        let tables:Vec<String>=query("SELECT relname AS n FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname")
            .bind(schema).fetch_all(&self.auth.base.admin).await.unwrap().into_iter().map(|r|r.get("n")).collect();
        let mut result = Vec::new();
        for table in tables {
            assert!(table
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_'));
            let mut values: Vec<String> = query(&format!(
                "SELECT row_to_json(t)::text AS v FROM {schema}.{table} t"
            ))
            .fetch_all(&self.auth.base.admin)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.get("v"))
            .collect();
            values.sort();
            result.push((table, values));
        }
        result
    }
    pub async fn oids(&self, schema: &str) -> Vec<(String, i64)> {
        query("SELECT relname AS n,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname")
            .bind(schema).fetch_all(&self.auth.base.admin).await.unwrap().into_iter().map(|r|(r.get("n"),r.get("oid"))).collect()
    }
    pub async fn wait_blocked(&self, pid: i32) {
        tokio::time::timeout(Duration::from_secs(3),async{loop{
            let n:i64=query("SELECT count(*) AS n FROM pg_stat_activity WHERE application_name=$1 AND $2=ANY(pg_blocking_pids(pid))")
                .bind(&self.auth.base.schema).bind(pid).fetch_one(&self.auth.base.admin).await.unwrap().get("n");
            if n>0{break;}tokio::task::yield_now().await;
        }}).await.expect("metadata operation did not reach a real source-connection SQL wait");
    }
    pub async fn pool_restored(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut held = Vec::new();
            for _ in 0..4 {
                let mut c = self.auth.base.pool.acquire().await.unwrap();
                let row = query("SELECT current_schema() AS n,current_setting('search_path') AS p")
                    .fetch_one(&mut *c)
                    .await
                    .unwrap();
                assert_eq!(row.get::<String, _>("n"), self.auth.base.schema);
                assert_eq!(row.get::<String, _>("p"), self.auth.base.schema);
                held.push(c);
            }
        })
        .await
        .expect("metadata operation leaked or changed the original source pool");
    }
    pub async fn cleanup(self) {
        self.upload_pool.close().await;
        self.publication_pool.close().await;
        for schema in [&self.publication_schema, &self.upload_schema] {
            query(&format!("DROP SCHEMA {schema} CASCADE"))
                .execute(&self.auth.base.admin)
                .await
                .unwrap();
        }
        self.auth.cleanup().await;
    }
}
