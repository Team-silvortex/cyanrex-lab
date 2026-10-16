use super::{
    catalog_fixture::{within, Fixture},
    http_helpers::token_hash,
    marker_fixture,
    storage_helpers::{self, Mode, Root},
};
use axum::Router;
use cyanrex_engine::{
    platform_host::PlatformSessionConnector,
    platform_http::{build_session_lifecycle_router, SessionLifecycleHttpState},
};
use sqlx_core::{query::query, row::Row};
use uuid::Uuid;

const MARKERS: [&str; 5] = [
    "collaboration_auth_source_schema",
    "collaboration_identity_schema",
    "collaboration_access_schema",
    "collaboration_task_schema",
    "collaboration_artifact_schema",
];

pub(super) struct Prepared<'a> {
    pub(super) owner: &'a Fixture,
    root: Option<Root>,
    pub(super) token: String,
    pub(super) other: String,
    pub(super) unknown: String,
}

impl<'a> Prepared<'a> {
    pub(super) async fn new(owner: &'a Fixture) -> Self {
        marker_fixture::prepare(owner).await;
        // Trusted fixture provisioning only. Production must never SET READ WRITE to
        // override the explicit role policy; a fresh connection inherits this setting.
        owner
            .sql(&format!(
                "ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=off",
                owner.role, owner.database
            ))
            .await;
        close_runtime(owner).await;
        for grant in [
            "SELECT (username,account_id), UPDATE (username) ON users",
            "SELECT (token,username,account_id,expires_at), UPDATE (token), DELETE ON sessions",
            "UPDATE (singleton) ON collaboration_auth_source_schema",
        ] {
            let mut connection = within(owner.owner().acquire()).await.unwrap();
            within(
                query("SELECT pg_catalog.set_config('search_path',$1,false)")
                    .bind(&owner.source)
                    .execute(&mut *connection),
            )
            .await
            .unwrap();
            within(query(&format!("GRANT {grant} TO {}", owner.role)).execute(&mut *connection))
                .await
                .unwrap();
        }
        let token = Uuid::new_v4().to_string();
        let other = Uuid::new_v4().to_string();
        // Already committed synthetic Sessions, not a login/OTP-issuance claim. The real
        // Source 2 template and immutable account-incarnation constraints enforce shape.
        for value in [&token, &other] {
            within(query(&format!("INSERT INTO {}.sessions (token,username,account_id,expires_at)
                VALUES ($1,'marker-fixture','00000000-0000-0000-0000-000000008211',clock_timestamp()+interval '12 hours')", owner.source))
                .bind(token_hash(value)).execute(owner.owner())).await.unwrap();
        }
        Self {
            owner,
            root: Some(Root::new()),
            token,
            other,
            unknown: Uuid::new_v4().to_string(),
        }
    }

    pub(super) fn root(&self) -> &Root {
        self.root.as_ref().unwrap()
    }
    pub(super) async fn connector(&self) -> PlatformSessionConnector {
        let storage = storage_helpers::binding(self.owner, self.root(), Mode::Expected)
            .await
            .unwrap();
        PlatformSessionConnector::new(storage)
    }
    pub(super) async fn app(&self) -> Router {
        let storage = storage_helpers::binding(self.owner, self.root(), Mode::Expected)
            .await
            .unwrap();
        build_session_lifecycle_router(
            SessionLifecycleHttpState::with_platform_storage(storage, 4).unwrap(),
        )
    }
    pub(super) fn finish(mut self) -> bool {
        self.root.take().unwrap().finish()
    }

    pub(super) async fn minimal_acl(&self) -> bool {
        let f = self.owner;
        let row = within(query("SELECT NOT r.rolsuper AND NOT r.rolinherit AND NOT r.rolcreatedb
            AND NOT r.rolcreaterole AND NOT r.rolreplication AND NOT r.rolbypassrls
            AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_auth_members WHERE member=r.oid)
            AND NOT pg_catalog.has_database_privilege(r.oid,current_database(),'CREATE')
            AND NOT pg_catalog.has_database_privilege(r.oid,current_database(),'TEMP')
            AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_namespace n WHERE n.nspname=ANY($2)
                AND (n.nspowner=r.oid OR pg_catalog.has_schema_privilege(r.oid,n.oid,'CREATE')))
            AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                WHERE n.nspname=ANY($2) AND c.relkind='r' AND (c.relowner=r.oid
                    OR pg_catalog.has_table_privilege(r.oid,c.oid,'INSERT')
                    OR pg_catalog.has_table_privilege(r.oid,c.oid,'UPDATE')
                    OR pg_catalog.has_table_privilege(r.oid,c.oid,'TRIGGER')
                    OR (NOT c.relname=ANY($3) AND pg_catalog.has_table_privilege(r.oid,c.oid,'SELECT'))))
            AND (SELECT count(*) FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped
                WHERE n.nspname=ANY($2) AND c.relkind='r'
                    AND pg_catalog.has_column_privilege(r.oid,c.oid,a.attnum,'SELECT'))=21
            AND (SELECT count(*) FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped
                WHERE n.nspname=ANY($2) AND c.relkind='r'
                    AND pg_catalog.has_column_privilege(r.oid,c.oid,a.attnum,'UPDATE'))=3
            AND pg_catalog.has_table_privilege(r.oid,$4,'DELETE')
            AND NOT pg_catalog.has_table_privilege(r.oid,$5,'DELETE') AS minimal
            FROM pg_catalog.pg_roles r WHERE r.rolname=$1")
            .bind(&f.role).bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()])
            .bind(MARKERS.map(str::to_owned).to_vec()).bind(format!("{}.sessions",f.source))
            .bind(format!("{}.users",f.source)).fetch_one(f.owner())).await.unwrap();
        let policy = within(
            query(
                "SELECT current_user::text=$1 AND session_user::text=$1
            AND current_setting('default_transaction_read_only')='off'
            AND current_setting('transaction_read_only')='off' AS runtime",
            )
            .bind(&f.role)
            .fetch_one(f.runtime()),
        )
        .await
        .unwrap()
        .get::<bool, _>("runtime");
        row.get::<bool, _>("minimal") && policy
    }

    pub(super) async fn credentials_denied(&self) -> bool {
        let mut denied = true;
        for column in [
            "password_salt",
            "password_hash",
            "totp_secret",
            "otp_last_counter",
        ] {
            let result = within(
                query(&format!(
                    "SELECT {column} FROM ONLY {}.users LIMIT 0",
                    self.owner.source
                ))
                .fetch_all(self.owner.runtime()),
            )
            .await;
            denied &= result
                .err()
                .and_then(|error| {
                    error
                        .as_database_error()
                        .and_then(|error| error.code().map(|code| code == "42501"))
                })
                .unwrap_or(false);
        }
        denied
    }
}

pub(super) async fn close_runtime(fixture: &Fixture) {
    let connection = within(fixture.runtime().acquire()).await.unwrap();
    within(connection.close()).await.unwrap();
}

#[derive(PartialEq, Eq)]
pub(super) struct Stable {
    catalog: super::catalog_fixture::Snapshot,
    users: String,
    markers: Vec<String>,
}
pub(super) async fn stable(fixture: &Fixture) -> Stable {
    async fn digest(fixture: &Fixture, relation: &str) -> String {
        within(query(&format!("SELECT md5(coalesce(string_agg(to_jsonb(t)::text,'|' ORDER BY to_jsonb(t)::text),'')) AS digest FROM {relation} t"))
            .fetch_one(fixture.owner())).await.unwrap().get("digest")
    }
    let mut markers = Vec::new();
    for (schema, table) in [
        (&fixture.source, MARKERS[0]),
        (&fixture.source, MARKERS[1]),
        (&fixture.source, MARKERS[2]),
        (&fixture.task, MARKERS[3]),
        (&fixture.artifacts, MARKERS[4]),
    ] {
        markers.push(digest(fixture, &format!("{schema}.{table}")).await);
    }
    Stable {
        catalog: fixture.snapshot().await,
        users: digest(fixture, &format!("{}.users", fixture.source)).await,
        markers,
    }
}

pub(super) async fn session_count(fixture: &Fixture) -> i64 {
    within(
        query(&format!(
            "SELECT count(*) AS count FROM {}.sessions",
            fixture.source
        ))
        .fetch_one(fixture.owner()),
    )
    .await
    .unwrap()
    .get("count")
}
