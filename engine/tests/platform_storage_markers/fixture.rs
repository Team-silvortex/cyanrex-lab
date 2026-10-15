pub(super) use crate::catalog_fixture::{within, Fixture};
use sqlx_core::{query::query, row::Row};

const MARKERS: [&str; 5] = [
    "collaboration_auth_source_schema",
    "collaboration_identity_schema",
    "collaboration_access_schema",
    "collaboration_task_schema",
    "collaboration_artifact_schema",
];

async fn template(fixture: &Fixture, schema: &str, text: &str, delimiter: &str) {
    let mut connection = within(fixture.owner().acquire())
        .await
        .unwrap_or_else(|_| panic!("owned template preparation connection unavailable"));
    within(
        query("SELECT pg_catalog.set_config('search_path', $1, false)")
            .bind(schema)
            .execute(&mut *connection),
    )
    .await
    .unwrap_or_else(|_| panic!("owned template namespace selection failed"));
    for statement in text
        .split(delimiter)
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
    {
        within(query(statement).execute(&mut *connection))
            .await
            .unwrap_or_else(|_| panic!("explicit fresh test template installation failed"));
    }
}

pub(super) async fn prepare(fixture: &Fixture) {
    // Explicit trusted test setup, not migration through the API being exercised. These are
    // the real templates; registry business graphs remain empty and are not audited here.
    for (text, delimiter) in [
        (
            include_str!("../../migrations/0001_auth_users_sessions.sql"),
            ";",
        ),
        (
            include_str!("../../migrations/0010_durable_auth_source.sql"),
            "-- cyanrex-statement",
        ),
        (
            include_str!("../../migrations/0015_durable_otp_consumption.sql"),
            ";",
        ),
        (
            include_str!("../../migrations/0006_collaboration_identity.sql"),
            ";",
        ),
        (
            include_str!("../../migrations/0007_collaboration_access.sql"),
            ";",
        ),
        (
            include_str!("../../migrations/0008_collaboration_policy_audit.sql"),
            "-- cyanrex-statement",
        ),
        (
            include_str!("../../migrations/0009_collaboration_identity_audit.sql"),
            "-- cyanrex-statement",
        ),
    ] {
        template(fixture, &fixture.source, text, delimiter).await;
    }
    template(
        fixture,
        &fixture.task,
        include_str!("../../migrations/0014_collaboration_task_content.sql"),
        ";",
    )
    .await;
    template(
        fixture,
        &fixture.artifacts,
        include_str!("../../migrations/0012_collaboration_artifacts.sql"),
        ";",
    )
    .await;

    let scope = fixture.config().scope();
    within(query(&format!("INSERT INTO {}.collaboration_auth_source_schema (singleton,version,authority_id) VALUES (TRUE,2,$1)", fixture.source))
        .bind(scope.authority_id.as_uuid()).execute(fixture.owner())).await
        .unwrap_or_else(|_| panic!("synthetic Source marker preparation failed"));
    for table in [
        "collaboration_identity_schema",
        "collaboration_access_schema",
    ] {
        fixture
            .sql(&format!(
                "UPDATE {}.{table} SET version=2 WHERE singleton",
                fixture.source
            ))
            .await;
    }
    for (schema, table, version) in [
        (&fixture.task, "collaboration_task_schema", 3),
        (&fixture.artifacts, "collaboration_artifact_schema", 1),
    ] {
        within(query(&format!("INSERT INTO {schema}.{table} (singleton,version,authority_id,workspace_id) VALUES (TRUE,$1,$2,$3)"))
            .bind(version).bind(scope.authority_id.as_uuid()).bind(scope.workspace_id.as_uuid())
            .execute(fixture.owner())).await
            .unwrap_or_else(|_| panic!("synthetic scoped marker preparation failed"));
    }
    // Nonempty synthetic text makes independent no-mutation digests meaningful. It is not a
    // valid credential, Task JSON, Artifact publication or claim that business content is healthy.
    fixture.sql(&format!("INSERT INTO {}.users (username,password_salt,password_hash,totp_secret,account_id)
        VALUES ('marker-fixture','synthetic-salt','synthetic-noncredential','synthetic-nonenrollment','00000000-0000-0000-0000-000000008211')", fixture.source)).await;
    fixture.sql(&format!("INSERT INTO {}.collaboration_tasks (task_id,owner_id,revision,snapshot,manifest)
        VALUES ('00000000-0000-0000-0000-000000008212','00000000-0000-0000-0000-000000008213',1,'synthetic Task body','synthetic manifest')", fixture.task)).await;
    fixture.sql(&format!("INSERT INTO {}.collaboration_artifacts (artifact_id,owner_id,head_revision_id,sequence)
        VALUES ('00000000-0000-0000-0000-000000008214','00000000-0000-0000-0000-000000008213','00000000-0000-0000-0000-000000008215',1)", fixture.artifacts)).await;
    fixture.sql(&format!("INSERT INTO {}.collaboration_artifact_revisions (revision_id,artifact_id,sequence,snapshot)
        VALUES ('00000000-0000-0000-0000-000000008215','00000000-0000-0000-0000-000000008214',1,'synthetic Artifact body')", fixture.artifacts)).await;

    for schema in [&fixture.source, &fixture.task, &fixture.artifacts] {
        fixture
            .sql(&format!(
                "REVOKE ALL ON ALL TABLES IN SCHEMA {schema} FROM PUBLIC, {}",
                fixture.role
            ))
            .await;
    }
    // Qualified ACCESS SHARE locks and actual tableoid metadata need relation-level SELECT.
    // Only these five marker tables receive it; all their user columns are required metadata.
    // No business table or business column receives any SELECT privilege.
    for (schema, table) in [
        (&fixture.source, MARKERS[0]),
        (&fixture.source, MARKERS[1]),
        (&fixture.source, MARKERS[2]),
        (&fixture.task, MARKERS[3]),
        (&fixture.artifacts, MARKERS[4]),
    ] {
        fixture
            .sql(&format!(
                "GRANT SELECT ON {schema}.{table} TO {}",
                fixture.role
            ))
            .await;
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct Snapshot {
    catalog: crate::catalog_fixture::Snapshot,
    tables: Vec<(i64, String, String, String)>,
}

impl Snapshot {
    pub(super) fn same_rows(&self, other: &Self) -> bool {
        self.tables.len() == other.tables.len()
            && self
                .tables
                .iter()
                .zip(&other.tables)
                .all(|(before, after)| {
                    before.1 == after.1 && before.2 == after.2 && before.3 == after.3
                })
    }
}

pub(super) async fn snapshot(fixture: &Fixture) -> Snapshot {
    let catalog = fixture.snapshot().await;
    let relations = within(
        query(
            "SELECT c.oid::bigint AS oid, n.nspname::text AS schema, c.relname::text AS name
        FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=ANY($1) AND c.relkind='r' ORDER BY n.nspname,c.relname",
        )
        .bind(vec![
            fixture.source.clone(),
            fixture.task.clone(),
            fixture.artifacts.clone(),
        ])
        .fetch_all(fixture.owner()),
    )
    .await
    .unwrap_or_else(|_| panic!("independent table identity snapshot failed"));
    let mut tables = Vec::new();
    for relation in relations {
        let schema: String = relation.get("schema");
        let table: String = relation.get("name");
        let digest: String = within(query(&format!("SELECT md5(coalesce(string_agg(to_jsonb(t)::text,'|' ORDER BY to_jsonb(t)::text),'')) AS digest FROM {schema}.{table} t"))
            .fetch_one(fixture.owner())).await
            .unwrap_or_else(|_| panic!("independent stored-data digest failed")).get("digest");
        tables.push((relation.get("oid"), schema, table, digest));
    }
    Snapshot { catalog, tables }
}

pub(super) async fn only_marker_columns_are_readable(fixture: &Fixture) -> bool {
    let row = within(query("SELECT
        count(*) FILTER (WHERE c.relname=ANY($2) AND pg_catalog.has_column_privilege(current_user,c.oid,a.attnum,'SELECT')) AS marker_columns,
        count(*) FILTER (WHERE NOT c.relname=ANY($2) AND pg_catalog.has_column_privilege(current_user,c.oid,a.attnum,'SELECT')) AS business_columns,
        current_setting('transaction_read_only')='on' AS readonly
        FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped
        WHERE n.nspname=ANY($1) AND c.relkind='r'")
        .bind(vec![fixture.source.clone(), fixture.task.clone(), fixture.artifacts.clone()])
        .bind(MARKERS.map(str::to_owned).to_vec()).fetch_one(fixture.runtime())).await
        .unwrap_or_else(|_| panic!("independent runtime column-privilege oracle failed"));
    row.get::<i64, _>("marker_columns") == 15
        && row.get::<i64, _>("business_columns") == 0
        && row.get::<bool, _>("readonly")
}

pub(super) async fn forbidden_reads_are_denied(fixture: &Fixture) -> Vec<bool> {
    let mut connection = within(fixture.runtime().acquire())
        .await
        .unwrap_or_else(|_| panic!("runtime negative-permission connection unavailable"));
    let mut denied = Vec::new();
    for (schema, table, columns) in [
        (
            &fixture.source,
            "users",
            "password_hash,totp_secret,otp_last_counter",
        ),
        (&fixture.source, "sessions", "token,expires_at"),
        (&fixture.task, "collaboration_tasks", "snapshot,manifest"),
        (
            &fixture.task,
            "collaboration_task_outbox",
            "snapshot,manifest",
        ),
        (
            &fixture.artifacts,
            "collaboration_artifacts",
            "owner_id,head_revision_id",
        ),
        (
            &fixture.artifacts,
            "collaboration_artifact_revisions",
            "snapshot",
        ),
        (
            &fixture.artifacts,
            "collaboration_artifact_outbox",
            "snapshot",
        ),
    ] {
        let result = within(
            query(&format!("SELECT {columns} FROM {schema}.{table} LIMIT 0"))
                .execute(&mut *connection),
        )
        .await;
        denied.push(
            result
                .as_ref()
                .err()
                .and_then(|error| error.as_database_error())
                .is_some_and(|error| error.code().as_deref() == Some("42501")),
        );
    }
    denied
}
