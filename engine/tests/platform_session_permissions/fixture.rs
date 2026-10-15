use super::http_fixture::{self, within, Fixture};
use axum::Router;
use cyanrex_engine::{
    platform_http::{build_session_lifecycle_router, SessionLifecycleHttpState},
    services::auth_service::durable_source::DurableAuthSource,
};
use futures_util::FutureExt;
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin, str::FromStr};
use uuid::Uuid;

type Case<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub struct Restricted<'f> {
    pub owner: &'f Fixture,
    pub role: String,
    pub application: String,
    created: bool,
    pool: Option<PgPool>,
    source: Option<DurableAuthSource>,
}

impl<'f> Restricted<'f> {
    fn empty(owner: &'f Fixture) -> Self {
        Self {
            owner,
            role: format!("session_min_role_{}", Uuid::new_v4().simple()),
            application: format!("session_min_client_{}", Uuid::new_v4().simple()),
            created: false,
            pool: None,
            source: None,
        }
    }

    async fn prepare(&mut self) {
        // These generated values never come from an application account or configured password.
        let password = Uuid::new_v4().simple().to_string();
        self.admin_sql(&format!("CREATE ROLE {} LOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD '{password}'", self.role)).await;
        self.created = true;
        self.admin_sql(&format!(
            "GRANT USAGE ON SCHEMA {} TO {}",
            self.owner.schema, self.role
        ))
        .await;
        for grant in [
            "SELECT (username, account_id), UPDATE (username) ON users",
            "SELECT (token, username, account_id, expires_at), UPDATE (token), DELETE ON sessions",
            "SELECT (singleton, version, authority_id), UPDATE (singleton) ON collaboration_auth_source_schema",
        ] {
            self.admin_sql(&format!("GRANT {grant} TO {}", self.role)).await;
        }
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("set an explicit disposable CYANREX_TEST_DATABASE_URL");
        let options = PgConnectOptions::from_str(&url)
            .unwrap_or_else(|_| panic!("disposable restricted fixture URL rejected"))
            .username(&self.role)
            .password(&password);
        let schema = self.owner.schema.clone();
        let application = self.application.clone();
        let pool = within(PgPoolOptions::new().max_connections(2).after_connect(move |connection, _| {
            let schema = schema.clone();
            let application = application.clone();
            Box::pin(async move {
                query("SELECT set_config('search_path', $1, false), set_config('application_name', $2, false)")
                    .bind(schema).bind(application).execute(connection).await?;
                Ok(())
            })
        }).connect_with(options)).await
            .unwrap_or_else(|_| panic!("restricted fixture login unavailable"));
        self.source = Some(DurableAuthSource::new(
            pool.clone(),
            http_fixture::authority(),
        ));
        self.pool = Some(pool);
    }

    pub fn pool(&self) -> &PgPool {
        self.pool.as_ref().unwrap()
    }
    pub fn source(&self) -> &DurableAuthSource {
        self.source.as_ref().unwrap()
    }
    pub fn app(&self) -> Router {
        build_session_lifecycle_router(
            SessionLifecycleHttpState::new(self.source().clone(), http_fixture::ORIGIN)
                .unwrap_or_else(|_| panic!("trusted restricted HTTP state rejected")),
        )
    }
    pub async fn admin_sql(&self, statement: &str) {
        within(query(statement).execute(self.owner.pool()))
            .await
            .unwrap_or_else(|_| panic!("owned restricted fixture setup failed"));
    }

    pub async fn exact_role(&self) -> bool {
        within(
            query("SELECT current_user::text=$1 AND session_user::text=$1 AS exact")
                .bind(&self.role)
                .fetch_one(self.pool()),
        )
        .await
        .unwrap_or_else(|_| panic!("restricted login role observation failed"))
        .get("exact")
    }

    pub async fn minimal_acl(&self) -> bool {
        within(query("SELECT NOT r.rolsuper AND NOT r.rolinherit AND NOT r.rolcreatedb
            AND NOT r.rolcreaterole AND NOT r.rolreplication AND NOT r.rolbypassrls
            AND NOT pg_catalog.has_schema_privilege(r.oid, current_schema(), 'CREATE')
            AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_auth_members WHERE member=r.oid)
            AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_class c WHERE c.relnamespace=current_schema()::regnamespace
                AND (c.relowner=r.oid OR pg_catalog.has_table_privilege(r.oid,c.oid,'SELECT')
                    OR pg_catalog.has_table_privilege(r.oid,c.oid,'UPDATE')
                    OR pg_catalog.has_table_privilege(r.oid,c.oid,'INSERT')
                    OR pg_catalog.has_table_privilege(r.oid,c.oid,'TRIGGER')))
            AND (SELECT count(*) FROM pg_catalog.pg_attribute a JOIN pg_catalog.pg_class c ON c.oid=a.attrelid
                WHERE c.relnamespace=current_schema()::regnamespace AND a.attnum>0 AND NOT a.attisdropped
                AND pg_catalog.has_column_privilege(r.oid,c.oid,a.attnum,'SELECT'))=9
            AND (SELECT count(*) FROM pg_catalog.pg_attribute a JOIN pg_catalog.pg_class c ON c.oid=a.attrelid
                WHERE c.relnamespace=current_schema()::regnamespace AND a.attnum>0 AND NOT a.attisdropped
                AND pg_catalog.has_column_privilege(r.oid,c.oid,a.attnum,'UPDATE'))=3
            AND pg_catalog.has_table_privilege(r.oid,'sessions','DELETE')
            AND NOT pg_catalog.has_table_privilege(r.oid,'users','DELETE')
            AND NOT pg_catalog.has_table_privilege(r.oid,'collaboration_auth_source_schema','DELETE')
            AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_attribute a
                WHERE a.attrelid='users'::regclass AND a.attname IN
                    ('password_salt','password_hash','totp_secret','otp_last_counter')
                AND (pg_catalog.has_column_privilege(r.oid,a.attrelid,a.attnum,'SELECT')
                    OR pg_catalog.has_column_privilege(r.oid,a.attrelid,a.attnum,'UPDATE'))) AS minimal
            FROM pg_catalog.pg_roles r WHERE r.rolname=$1")
            .bind(&self.role).fetch_one(self.owner.pool())).await
            .unwrap_or_else(|_| panic!("independent restricted ACL observation failed")).get("minimal")
    }

    pub async fn credential_projection_denied(&self) -> bool {
        let mut denied = true;
        for column in [
            "password_salt",
            "password_hash",
            "totp_secret",
            "otp_last_counter",
        ] {
            let result = within(
                query(&format!("SELECT {column} FROM ONLY users LIMIT 0")).fetch_all(self.pool()),
            )
            .await;
            denied &= result
                .err()
                .and_then(|error| {
                    error
                        .as_database_error()
                        .and_then(|database| database.code().map(|code| code == "42501"))
                })
                .unwrap_or(false);
        }
        denied
    }

    pub async fn blocked_by(&self, blocker: i32) -> i32 {
        within(async {
            loop {
                let row = query(
                    "SELECT pid FROM pg_catalog.pg_stat_activity
                    WHERE datname=current_database() AND application_name=$1
                    AND $2=ANY(pg_catalog.pg_blocking_pids(pid)) ORDER BY pid LIMIT 1",
                )
                .bind(&self.application)
                .bind(blocker)
                .fetch_optional(self.owner.admin())
                .await
                .unwrap_or_else(|_| panic!("restricted actual blocker observation failed"));
                if let Some(row) = row {
                    return row.get("pid");
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
    }

    pub async fn no_owned_prepared_slots(&self) -> bool {
        let mut held = Vec::new();
        let mut empty = true;
        for _ in 0..2 {
            let mut connection = within(self.pool().acquire())
                .await
                .unwrap_or_else(|_| panic!("restricted pool drain failed"));
            empty &= within(
                query(
                    "SELECT NOT EXISTS (SELECT 1 FROM pg_catalog.pg_prepared_statements
                WHERE name='cyanrex_session_source_relations_v1') AS empty",
                )
                .fetch_one(&mut *connection),
            )
            .await
            .unwrap_or_else(|_| panic!("owned prepared slot observation failed"))
            .get::<bool, _>("empty");
            held.push(connection);
        }
        empty
    }

    pub async fn cleanup_after_partial_view(owner: &'f Fixture) -> bool {
        let role = Self::checked(owner, |r| {
            Box::pin(async move {
                r.admin_sql("ALTER TABLE users RENAME TO session_permissions_saved_users")
                    .await;
                r.role.clone()
            })
        })
        .await;
        within(
            query(
                "SELECT NOT EXISTS (SELECT 1 FROM pg_catalog.pg_roles WHERE rolname=$1) AS removed",
            )
            .bind(role)
            .fetch_one(owner.admin()),
        )
        .await
        .unwrap_or_else(|_| panic!("partial-view role cleanup observation failed"))
        .get("removed")
    }

    async fn cleanup(&mut self) {
        self.source.take();
        if let Some(pool) = self.pool.take() {
            within(pool.close()).await;
        }
        if self.created {
            // Partial view setup may have renamed users without creating its replacement.
            // Check each exact owned name independently; missing users must not retain the role.
            for (relation, privileges) in [
                ("users", "SELECT (username, account_id), UPDATE (username)"),
                (
                    "session_permissions_saved_users",
                    "SELECT (username, account_id), UPDATE (username)",
                ),
                (
                    "sessions",
                    "SELECT (token, username, account_id, expires_at), UPDATE (token), DELETE",
                ),
                (
                    "collaboration_auth_source_schema",
                    "SELECT (singleton, version, authority_id), UPDATE (singleton)",
                ),
            ] {
                let qualified = format!("{}.{relation}", self.owner.schema);
                let present: bool = within(
                    query("SELECT pg_catalog.to_regclass($1) IS NOT NULL AS present")
                        .bind(&qualified)
                        .fetch_one(self.owner.admin()),
                )
                .await
                .unwrap_or_else(|_| panic!("owned relation cleanup observation failed"))
                .get("present");
                if !present {
                    continue;
                }
                self.admin_sql(&format!(
                    "REVOKE {privileges} ON {qualified} FROM {}",
                    self.role
                ))
                .await;
                self.admin_sql(&format!(
                    "REVOKE ALL PRIVILEGES ON TABLE {qualified} FROM {}",
                    self.role
                ))
                .await;
            }
            self.admin_sql(&format!(
                "REVOKE ALL PRIVILEGES ON SCHEMA {} FROM {}",
                self.owner.schema, self.role
            ))
            .await;
            self.admin_sql(&format!("DROP ROLE {}", self.role)).await;
            self.created = false;
        }
    }

    pub async fn checked<T>(
        owner: &'f Fixture,
        case: impl for<'a> FnOnce(&'a Self) -> Case<'a, T>,
    ) -> T {
        let mut restricted = Self::empty(owner);
        let result = AssertUnwindSafe(async {
            restricted.prepare().await;
            case(&restricted).await
        })
        .catch_unwind()
        .await;
        let cleanup = AssertUnwindSafe(restricted.cleanup()).catch_unwind().await;
        if let Err(panic) = cleanup {
            std::panic::resume_unwind(panic);
        }
        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
}
