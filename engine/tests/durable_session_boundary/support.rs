use super::*;

pub async fn scoped_pool(path: &str, application: &str) -> PgPool {
    let path = path.to_owned();
    let application = application.to_owned();
    PgPoolOptions::new().max_connections(1).after_connect(move |connection, _| {
        let path = path.clone();
        let application = application.clone();
        Box::pin(async move {
            query("SELECT set_config('search_path', $1, false), set_config('application_name', $2, false)")
                .bind(path).bind(application).execute(connection).await?;
            Ok(())
        })
    }).connect(&std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable test database required")).await.unwrap()
}

pub async fn fault(f: &Fixture, timing: &str, body: &str) {
    f.sql("CREATE SEQUENCE session_boundary_calls").await;
    f.sql(&format!(
        "CREATE FUNCTION session_boundary_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.session_boundary_calls'); {body} END $$",
        f.schema
    ))
    .await;
    f.sql(&format!(
        "CREATE TRIGGER session_boundary_fault {timing} ON sessions
        FOR EACH ROW EXECUTE FUNCTION session_boundary_fault()"
    ))
    .await;
}

pub async fn calls(f: &Fixture) -> i64 {
    let row = query("SELECT last_value, is_called FROM session_boundary_calls")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert!(
        row.get::<bool, _>("is_called"),
        "operation did not reach its synthetic post-write fault"
    );
    row.get("last_value")
}

pub async fn source_unchanged(f: &Fixture) {
    let row = query("SELECT version, authority_id FROM collaboration_auth_source_schema")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(row.get::<i32, _>("version"), 2);
    assert_eq!(row.get::<Uuid, _>("authority_id"), authority().as_uuid());
}
