use super::*;
pub const MARKER: &str = "collaboration_file_recipient_grant_schema";
pub const HEADS: &str = "collaboration_file_recipient_grants";
pub const CHANGES: &str = "collaboration_file_recipient_grant_changes";
pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "8b075b0b-8118-4c37-9224-337ba22f40c6".parse().unwrap(),
        workspace_id: "267c6b0f-182c-4980-a925-cdc967327c57".parse().unwrap(),
    }
}
pub fn account(seed: u128) -> FileAccountRef {
    FileAccountRef::new(
        PrincipalRef {
            authority_id: scope().authority_id,
            principal_id: Uuid::from_u128(seed + 100).try_into().unwrap(),
        },
        Uuid::from_u128(seed + 200).try_into().unwrap(),
    )
}
pub fn grant(seed: u128) -> FileGrantId {
    Uuid::from_u128(seed + 300).try_into().unwrap()
}
pub fn revision(n: u64) -> RevisionNumber {
    n.try_into().unwrap()
}
pub fn validity() -> FileGrantValidity {
    FileGrantValidity::new(100, 200).unwrap()
}
pub fn file(seed: u128) -> FileVersionRef {
    file_for(scope(), seed)
}
pub fn file_for(scope: WorkspaceRef, seed: u128) -> FileVersionRef {
    // Only valid claimed metadata. No filesystem bytes or Published provenance is asserted.
    let hash: Sha256Digest = "a".repeat(64).parse().unwrap();
    let manifest = FileTransferManifest::new(
        scope,
        Uuid::from_u128(seed + 400).try_into().unwrap(),
        Uuid::from_u128(seed + 500).try_into().unwrap(),
        "claimed.bin",
        11,
        vec![hash.clone()],
        hash,
    )
    .unwrap();
    FileVersionRef::from_manifest(&manifest)
}
#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub rows: Vec<(String, Vec<String>)>,
    pub oids: Vec<(String, i64)>,
}
pub fn rows<'a>(s: &'a Snapshot, table: &str) -> &'a Vec<String> {
    &s.rows.iter().find(|(n, _)| n == table).unwrap().1
}
pub async fn pool_for(url: &str, schema: &str) -> PgPool {
    let selected = schema.to_owned();
    PgPoolOptions::new().max_connections(4).after_connect(move|c,_|{let selected=selected.clone();Box::pin(async move{query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)").bind(selected).execute(c).await?;Ok(())})}).connect(url).await.unwrap()
}
pub struct Fixture {
    pub admin: PgPool,
    pub pool: PgPool,
    pub store: FileRecipientGrantStore,
    pub schema: String,
    pub installation_id: Uuid,
    pub max_grants: u32,
    url: String,
    auxiliary_pools: std::sync::Mutex<Vec<PgPool>>,
    auxiliary_schemas: std::sync::Mutex<Vec<String>>,
}
impl Fixture {
    pub async fn new(max_grants: u32) -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("explicit disposable database required");
        let admin = PgPoolOptions::new()
            .max_connections(3)
            .connect(&url)
            .await
            .unwrap();
        let schema = format!("file_recipient_grant_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let pool = pool_for(&url, &schema).await;
        let installation_id = Uuid::new_v4();
        let store = FileRecipientGrantStore::new(
            pool.clone(),
            scope(),
            installation_id,
            &schema,
            max_grants,
        )
        .unwrap();
        Self {
            admin,
            pool,
            store,
            schema,
            installation_id,
            max_grants,
            url,
            auxiliary_pools: std::sync::Mutex::new(Vec::new()),
            auxiliary_schemas: std::sync::Mutex::new(Vec::new()),
        }
    }
    pub async fn reopen(&self) -> (FileRecipientGrantStore, PgPool) {
        let pool = pool_for(&self.url, &self.schema).await;
        self.auxiliary_pools.lock().unwrap().push(pool.clone());
        let store = FileRecipientGrantStore::new(
            pool.clone(),
            scope(),
            self.installation_id,
            &self.schema,
            self.max_grants,
        )
        .unwrap();
        (store, pool)
    }
    pub async fn side_namespace(&self) -> (String, PgPool) {
        let schema = format!("recipient_publisher_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&self.admin)
            .await
            .unwrap();
        self.auxiliary_schemas.lock().unwrap().push(schema.clone());
        let pool = pool_for(&self.url, &schema).await;
        self.auxiliary_pools.lock().unwrap().push(pool.clone());
        (schema, pool)
    }
    pub async fn snapshot(&self) -> Snapshot {
        let oids:Vec<(String,i64)>=query("SELECT relname AS n,oid::bigint AS oid FROM pg_class WHERE relnamespace=to_regnamespace($1) AND relkind='r' ORDER BY relname").bind(&self.schema).fetch_all(&self.admin).await.unwrap().into_iter().map(|r|(r.get("n"),r.get("oid"))).collect();
        let mut rows = Vec::new();
        for (table, _) in &oids {
            assert!(table
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_'));
            let mut values: Vec<String> = query(&format!(
                "SELECT row_to_json(t)::text AS v FROM {}.{table} t",
                self.schema
            ))
            .fetch_all(&self.admin)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.get("v"))
            .collect();
            values.sort();
            rows.push((table.clone(), values));
        }
        Snapshot { rows, oids }
    }
    pub async fn cleanup(self) {
        self.pool.close().await;
        for pool in self.auxiliary_pools.into_inner().unwrap() {
            pool.close().await;
        }
        for schema in self.auxiliary_schemas.into_inner().unwrap() {
            query(&format!("DROP SCHEMA {schema} CASCADE"))
                .execute(&self.admin)
                .await
                .unwrap();
        }
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}
