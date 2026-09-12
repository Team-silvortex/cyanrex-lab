use super::support::*;
use cyanrex_engine::services::script_store::ScriptStore;
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::PgPoolOptions;

#[tokio::test]
#[ignore = "requires CYANREX_TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn scripts_use_postgres_without_file_fallback_and_keep_owner_boundaries_after_reload() {
    let _guard = ENV_LOCK.lock().await;
    let mut f = Fixture::new();
    let url = std::env::var("CYANREX_TEST_DATABASE_URL")
        .expect("explicit disposable test database URL required");
    let admin = PgPoolOptions::new().connect(&url).await.unwrap();
    let schema = format!("scripts_boundary_{}", uuid::Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let mut isolated = reqwest::Url::parse(&url).unwrap();
    isolated
        .query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    f.set_env("DATABASE_URL", isolated.as_str());
    f.set_env("CYANREX_DB_FALLBACK", "true");
    let root = f.root.clone();
    let check_pool = admin.clone();
    let check_schema = schema.clone();
    // Join before cleanup so a failing assertion cannot leave a fixture schema behind.
    let outcome = tokio::spawn(async move {
        let store = ScriptStore::default();
        let first = store
            .save_for_user("network-alice", " first ", SOURCE)
            .await
            .unwrap();
        assert_eq!(first.title, "first");
        let second = store
            .save_for_user("network-bob", "private", "bob source")
            .await
            .unwrap();
        assert!(store
            .save_for_user("../escape", "invalid", SOURCE)
            .await
            .is_err());
        assert!(store.list_for_user("../escape").await.is_empty());
        let durable_count: i64 = query(&format!(
            "SELECT count(*) AS count FROM {check_schema}.user_scripts"
        ))
        .fetch_one(&check_pool)
        .await
        .unwrap()
        .get("count");
        assert_eq!(
            durable_count, 2,
            "both records must really reach PostgreSQL"
        );
        assert!(
            !root.join("scripts").exists(),
            "a file fallback cannot stand in for SQL persistence"
        );
        let reloaded = ScriptStore::default();
        assert_eq!(
            reloaded.list_for_user("network-alice").await[0].id,
            first.id
        );
        assert_eq!(reloaded.list_for_user("network-bob").await[0].id, second.id);
        reloaded
            .delete_for_user("network-bob", &first.id)
            .await
            .unwrap();
        assert_eq!(store.list_for_user("network-alice").await.len(), 1);
        let (left, right) = tokio::join!(
            store.save_for_user("network-alice", "concurrent one", SOURCE),
            reloaded.save_for_user("network-alice", "concurrent two", SOURCE)
        );
        assert!(left.is_ok() && right.is_ok());
        assert_eq!(
            ScriptStore::default()
                .list_for_user("network-alice")
                .await
                .len(),
            3
        );
        store
            .delete_for_user("network-alice", &first.id)
            .await
            .unwrap();
        assert_eq!(
            ScriptStore::default()
                .list_for_user("network-alice")
                .await
                .len(),
            2
        );
        assert_eq!(
            ScriptStore::default()
                .list_for_user("network-bob")
                .await
                .len(),
            1
        );
    })
    .await;
    query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    outcome.expect("PostgreSQL script boundary failed; owned schema was cleaned up");
}
