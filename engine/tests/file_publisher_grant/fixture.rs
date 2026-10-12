use super::*;

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
pub fn revision(value: u64) -> RevisionNumber {
    value.try_into().unwrap()
}
pub fn validity() -> FileGrantValidity {
    FileGrantValidity::new(100, 200).unwrap()
}
pub async fn pool_for(url: &str, schema: &str) -> PgPool {
    let selected = schema.to_owned();
    PgPoolOptions::new().max_connections(4).after_connect(move|connection,_|{
        let selected=selected.clone();Box::pin(async move{
            query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)")
                .bind(selected).execute(connection).await?;Ok(())
        })
    }).connect(url).await.unwrap()
}
pub struct Fixture {
    pub admin: PgPool,
    pub pool: PgPool,
    pub store: FilePublisherGrantStore,
    pub schema: String,
    pub installation_id: Uuid,
    pub max_grants: u32,
    url: String,
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
        let schema = format!("file_publisher_grant_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let pool = pool_for(&url, &schema).await;
        let installation_id = Uuid::new_v4();
        let store = FilePublisherGrantStore::new(
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
        }
    }
    pub async fn reopen(&self) -> (FilePublisherGrantStore, PgPool) {
        let pool = pool_for(&self.url, &self.schema).await;
        let store = FilePublisherGrantStore::new(
            pool.clone(),
            scope(),
            self.installation_id,
            &self.schema,
            self.max_grants,
        )
        .unwrap();
        (store, pool)
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
