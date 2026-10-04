//! Explicit trusted-operator initialization of a fresh namespace, never a first-login claim.
use super::*;
use crate::{
    models::collaboration::{Workspace, WorkspaceRef},
    services::collaboration_identity_store::{
        CollaborationIdentityStore as Registry, IdentityStoreError, StoredLegacyAccessPolicy,
        StoredLegacyIdentity,
    },
};

// Contains the one-time TOTP enrollment secret: deliberately no Debug or Serialize.
pub struct AuthorityBootstrap {
    pub registration: DurableRegistration,
    pub workspace: Workspace,
    pub identity: StoredLegacyIdentity,
    pub policy: StoredLegacyAccessPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BootstrapError {
    #[error(transparent)]
    Authentication(#[from] DurableAuthError),
    #[error(transparent)]
    Identity(#[from] IdentityStoreError),
    #[error("bootstrap requires an empty, uninitialized namespace")]
    NotEmpty,
    #[error("bootstrap requires one explicit non-system namespace without temporary objects")]
    InvalidNamespace,
}
impl From<sqlx::Error> for BootstrapError {
    fn from(_: sqlx::Error) -> Self {
        Self::Authentication(DurableAuthError::StorageUnavailable)
    }
}

impl DurableAuthSource {
    /// Trusted local provisioning only: creates the first account, owner/teacher and explicit
    /// deployment grant, with both audit baselines, on one transaction in a fresh namespace.
    /// No env defaults, existing-account adoption, Session issuance, success replay or recovery.
    /// A lost commit acknowledgement requires reconciliation, never destructive reinitialization.
    pub async fn bootstrap_empty_authority(
        &self,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        password: &str,
    ) -> std::result::Result<AuthorityBootstrap, BootstrapError> {
        if scope.authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        if !(8..=4096).contains(&password.len()) {
            return Err(DurableAuthError::InvalidInput.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let salt = generate_password_salt();
            let hash = derive_password_hash(password, &salt).await?;
            let expected = UserRecord { username: username.to_string(), password_salt: salt, password_hash: hash, totp_secret: generate_totp_secret() };
            let mut tx = self.transaction().await?;
            let namespace: bool = sqlx::query("SELECT current_schema() IS NOT NULL
                AND current_schema() NOT LIKE 'pg_%' AND current_schema() <> 'information_schema'
                AND current_schemas(false) = ARRAY[current_schema()]::name[]
                AND pg_my_temp_schema() = 0 AS valid")
                .fetch_one(&mut *tx).await?.try_get("valid")?;
            if !namespace { return Err(BootstrapError::InvalidNamespace); }
            // Fence absent metadata using the SAME advisory keys as all three explicit installers.
            // Take every installer fence before DDL; then source -> registry metadata -> authority.
            for prefix in ["cyanrex.auth-source.", "cyanrex.identity.", "cyanrex.access."] {
                sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext($1 || current_schema()))")
                    .bind(prefix).execute(&mut *tx).await?;
            }
            let populated: bool = sqlx::query("SELECT
                EXISTS (SELECT 1 FROM pg_class WHERE relnamespace = current_schema()::regnamespace)
                OR EXISTS (SELECT 1 FROM pg_proc WHERE pronamespace = current_schema()::regnamespace)
                OR EXISTS (SELECT 1 FROM pg_type WHERE typnamespace = current_schema()::regnamespace) AS populated")
                .fetch_one(&mut *tx).await?.try_get("populated")?;
            if populated { return Err(BootstrapError::NotEmpty); }
            for (template, delimiter) in [
                (include_str!("../../../../migrations/0001_auth_users_sessions.sql"), ";"),
                (include_str!("../../../../migrations/0010_durable_auth_source.sql"), "-- cyanrex-statement"),
                (include_str!("../../../../migrations/0015_durable_otp_consumption.sql"), "-- cyanrex-statement"),
            ] {
                for statement in template.split(delimiter).map(str::trim).filter(|s| !s.is_empty()) {
                    sqlx::query(statement).execute(&mut *tx).await?;
                }
            }
            confirmed_one(sqlx::query("INSERT INTO collaboration_auth_source_schema (singleton, version, authority_id) VALUES (TRUE, $1, $2)")
                .bind(SOURCE_SCHEMA_VERSION).bind(self.authority_id.as_uuid()).execute(&mut *tx).await?.rows_affected())?;
            self.lock_source(&mut tx, true).await?;
            Self::verify_schema(&mut tx).await?;
            let workspace = Registry::begin_empty_bootstrap(&mut tx, scope).await?;
            let registration = self.insert_prepared_account(&mut tx, &expected).await?;
            let (identity, policy) = Registry::finish_empty_bootstrap(&mut tx, &workspace, username, registration.account.account_id).await?;
            // Audit/metadata triggers may have changed source rows after account insertion.
            self.lock_source(&mut tx, true).await?;
            Self::verify_schema(&mut tx).await?;
            let account = self.account_record(&mut tx, username).await?.ok_or(DurableAuthError::StorageUnavailable)?;
            let counts = sqlx::query("SELECT (SELECT count(*) FROM users) AS users, (SELECT count(*) FROM sessions) AS sessions")
                .fetch_one(&mut *tx).await?;
            if account.account != registration.account || !same_credentials(&account.user, &expected)
                || account.otp_watermark.stored() != -1
                || counts.try_get::<i64, _>("users")? != 1 || counts.try_get::<i64, _>("sessions")? != 0 {
                return Err(DurableAuthError::StorageUnavailable.into());
            }
            tx.commit().await?;
            Ok(AuthorityBootstrap { registration, workspace, identity, policy })
        }).await.map_err(|_| BootstrapError::Authentication(DurableAuthError::StorageUnavailable))?
    }
}
