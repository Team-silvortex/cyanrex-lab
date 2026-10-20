use super::{
    catalog_fixture::{within, Fixture},
    marker_fixture,
    storage_helpers::{self, Mode, Root},
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::LegacyUsername,
    platform_host::{PlatformLoginConnector, PlatformSessionConnector, PlatformStorageConnector},
};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};
use std::sync::OnceLock;
use uuid::Uuid;

const SALT: &str = "synthetic-platform-login-salt";
const SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
const MARKERS: [&str; 5] = [
    "collaboration_auth_source_schema",
    "collaboration_identity_schema",
    "collaboration_access_schema",
    "collaboration_task_schema",
    "collaboration_artifact_schema",
];

pub(super) fn username() -> LegacyUsername {
    "marker-fixture".parse().unwrap()
}
pub(super) fn password() -> &'static str {
    "public-synthetic-login-password-only"
}

fn password_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| {
        let salt = SaltString::encode_b64(SALT.as_bytes()).unwrap();
        // Argon2's default is the supported Argon2id v19 m19456/t2/p1/32-byte profile.
        Argon2::default()
            .hash_password(password().as_bytes(), &salt)
            .unwrap()
            .to_string()
    })
}

pub(super) fn otp() -> String {
    // A real RFC 6238 secret and next permitted drift counter keep a full 30s margin;
    // no KDF-duration sleep or production clock override is involved.
    let counter = Utc::now().timestamp().div_euclid(30) as u64 + 1;
    let mut mac = Hmac::<Sha1>::new_from_slice(b"12345678901234567890").unwrap();
    mac.update(&counter.to_be_bytes());
    let bytes = mac.finalize().into_bytes();
    let offset = (bytes[19] & 15) as usize;
    let value = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;
    format!("{:06}", value % 1_000_000)
}

pub(super) struct Prepared<'a> {
    pub(super) owner: &'a Fixture,
    root: Option<Root>,
}

impl<'a> Prepared<'a> {
    pub(super) async fn new(owner: &'a Fixture) -> Self {
        marker_fixture::prepare(owner).await;
        // Local synthetic credential replacement only; unrelated marker/Task fake values stay.
        within(query(&format!("UPDATE {}.users SET password_salt=$1,password_hash=$2,totp_secret=$3 WHERE username='marker-fixture'",owner.source))
            .bind(SALT).bind(password_hash()).bind(SECRET).execute(owner.owner())).await.unwrap();
        read_only(owner, false).await;
        for (table, grants) in [
            ("users", "SELECT (username,account_id,password_salt,password_hash,totp_secret,otp_last_counter), UPDATE (username,otp_last_counter)"),
            ("sessions", "SELECT (token,username,account_id,expires_at), UPDATE (token), INSERT (token,username,account_id,expires_at)"),
            ("collaboration_auth_source_schema", "UPDATE (singleton)"),
        ] {
            owner.sql(&format!("GRANT {grants} ON {}.{table} TO {}", owner.source,owner.role)).await;
        }
        Self {
            owner,
            root: Some(Root::new()),
        }
    }

    pub(super) fn root(&self) -> &Root {
        self.root.as_ref().unwrap()
    }
    pub(super) async fn storage(&self) -> PlatformStorageConnector {
        storage_helpers::binding(self.owner, self.root(), Mode::Expected)
            .await
            .unwrap()
    }
    pub(super) async fn connector(&self) -> PlatformLoginConnector {
        PlatformLoginConnector::new(self.storage().await)
    }
    pub(super) async fn session_connector(&self) -> PlatformSessionConnector {
        PlatformSessionConnector::new(self.storage().await)
    }
    pub(super) fn finish(mut self) -> bool {
        self.root.take().unwrap().finish()
    }

    pub(super) async fn minimal_acl(&self) -> bool {
        let f = self.owner;
        let rows = within(
            query(
                "SELECT n.nspname::text AS schema,c.relname::text AS table,a.attname::text AS col,
            has_column_privilege($1,c.oid,a.attnum,'SELECT') AS s,
            has_column_privilege($1,c.oid,a.attnum,'INSERT') AS i,
            has_column_privilege($1,c.oid,a.attnum,'UPDATE') AS u
            FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
            JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped
            WHERE n.nspname=ANY($2) AND c.relkind='r'",
            )
            .bind(&f.role)
            .bind(vec![f.source.clone(), f.task.clone(), f.artifacts.clone()])
            .fetch_all(f.owner()),
        )
        .await
        .unwrap();
        let mut counts = [0usize; 3];
        let columns = rows.iter().all(|row| {
            let schema: String = row.get("schema");
            let table: String = row.get("table");
            let col: String = row.get("col");
            let source = schema == f.source;
            let expected_s = MARKERS.contains(&table.as_str())
                || (source
                    && match table.as_str() {
                        "users" => [
                            "username",
                            "account_id",
                            "password_salt",
                            "password_hash",
                            "totp_secret",
                            "otp_last_counter",
                        ]
                        .contains(&col.as_str()),
                        "sessions" => ["token", "username", "account_id", "expires_at"]
                            .contains(&col.as_str()),
                        _ => false,
                    });
            let expected_i = source
                && table == "sessions"
                && ["token", "username", "account_id", "expires_at"].contains(&col.as_str());
            let expected_u = source
                && match table.as_str() {
                    "users" => ["username", "otp_last_counter"].contains(&col.as_str()),
                    "sessions" => col == "token",
                    "collaboration_auth_source_schema" => col == "singleton",
                    _ => false,
                };
            let actual = [
                row.get::<bool, _>("s"),
                row.get::<bool, _>("i"),
                row.get::<bool, _>("u"),
            ];
            for (index, value) in actual.iter().enumerate() {
                counts[index] += usize::from(*value);
            }
            actual == [expected_s, expected_i, expected_u]
        });
        let safe = within(query("SELECT NOT r.rolsuper AND NOT r.rolinherit AND NOT r.rolcreatedb
            AND NOT r.rolcreaterole AND NOT r.rolreplication AND NOT r.rolbypassrls
            AND NOT EXISTS(SELECT 1 FROM pg_catalog.pg_auth_members WHERE member=r.oid)
            AND NOT has_database_privilege(r.oid,current_database(),'CREATE')
            AND NOT has_database_privilege(r.oid,current_database(),'TEMP')
            AND NOT EXISTS(SELECT 1 FROM pg_catalog.pg_namespace n WHERE n.nspname=ANY($2)
                AND (n.nspowner=r.oid OR has_schema_privilege(r.oid,n.oid,'CREATE')))
            AND NOT EXISTS(SELECT 1 FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                WHERE n.nspname=ANY($2) AND c.relkind='r' AND (c.relowner=r.oid
                OR has_table_privilege(r.oid,c.oid,'INSERT') OR has_table_privilege(r.oid,c.oid,'UPDATE')
                OR has_table_privilege(r.oid,c.oid,'DELETE') OR has_table_privilege(r.oid,c.oid,'TRIGGER')
                OR (NOT c.relname=ANY($3) AND has_table_privilege(r.oid,c.oid,'SELECT')))) AS safe
            FROM pg_catalog.pg_roles r WHERE rolname=$1")
            .bind(&f.role).bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()])
            .bind(MARKERS.map(str::to_owned).to_vec()).fetch_one(f.owner())).await.unwrap().get::<bool,_>("safe");
        columns && counts == [25, 4, 4] && safe
    }
}

pub(super) async fn read_only(f: &Fixture, enabled: bool) {
    f.sql(&format!(
        "ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only={}",
        f.role,
        f.database,
        if enabled { "on" } else { "off" }
    ))
    .await;
    let connection = within(f.runtime().acquire()).await.unwrap();
    within(connection.close()).await.unwrap();
}

pub(super) async fn snapshot(f: &Fixture) -> marker_fixture::Snapshot {
    marker_fixture::snapshot(f).await
}
pub(super) async fn session_count(f: &Fixture) -> i64 {
    within(
        query(&format!(
            "SELECT COUNT(*) AS count FROM {}.sessions",
            f.source
        ))
        .fetch_one(f.owner()),
    )
    .await
    .unwrap()
    .get("count")
}
pub(super) async fn watermark(f: &Fixture) -> i64 {
    within(
        query(&format!(
            "SELECT otp_last_counter AS counter FROM {}.users WHERE username='marker-fixture'",
            f.source
        ))
        .fetch_one(f.owner()),
    )
    .await
    .unwrap()
    .get("counter")
}
pub(super) async fn seed_session(f: &Fixture) {
    let digest = format!(
        "{:x}",
        Sha256::digest(Uuid::new_v4().to_string().as_bytes())
    );
    within(query(&format!("INSERT INTO {}.sessions (token,username,account_id,expires_at) VALUES ($1,'marker-fixture','00000000-0000-0000-0000-000000008211',clock_timestamp()+interval '12 hours')",f.source))
        .bind(digest).execute(f.owner())).await.unwrap();
}
