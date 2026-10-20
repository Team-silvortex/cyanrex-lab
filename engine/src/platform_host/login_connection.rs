//! Explicit existing-account login over one original binding, not a public issuer or lifecycle API.
use super::PlatformStorageConnector;
use crate::{
    models::collaboration::LegacyUsername,
    services::auth_service::durable_source::{
        login_transaction::{LoginContext, PreparedLoginRead},
        DurableAuthError, DurableLogin,
    },
};
use sqlx_core::connection::Connection;
use sqlx_postgres::PgConnection;
use std::{sync::Arc, time::Duration};

/// No Clone/Debug, pool, registration, policy override, secret accessor or replacement baseline.
pub struct PlatformLoginConnector {
    storage: Arc<PlatformStorageConnector>,
    context: LoginContext,
}

impl PlatformLoginConnector {
    /// Pure composition; authority, Origin and mandatory stock limits come from this same binding.
    pub fn new(storage: PlatformStorageConnector) -> Self {
        Self::from_shared(Arc::new(storage))
    }

    pub(super) fn from_shared(storage: Arc<PlatformStorageConnector>) -> Self {
        let context = LoginContext::new(storage.config().scope().authority_id);
        Self { storage, context }
    }

    pub fn origin(&self) -> &str {
        self.storage.config().origin()
    }

    pub async fn login(
        &self,
        username: &LegacyUsername,
        password: &str,
        otp: &str,
    ) -> Result<DurableLogin, DurableAuthError> {
        LoginContext::validate_input(password, otp)?;
        self.context.admit(username)?;
        let work = tokio::time::timeout(
            Duration::from_secs(10),
            self.login_work(username, password, otp),
        )
        .await
        .unwrap_or(Err(DurableAuthError::StorageUnavailable));
        let result = match work {
            Ok((connection, result)) => {
                // A confirmed commit stays confirmed even if its later graceful close fails.
                let _ = tokio::time::timeout(Duration::from_secs(10), connection.close()).await;
                result
            }
            Err(error) => Err(error),
        };
        if result.is_ok() {
            self.context.clear(username);
        }
        result
    }

    async fn login_work(
        &self,
        username: &LegacyUsername,
        password: &str,
        otp: &str,
    ) -> Result<(PgConnection, Result<DurableLogin, DurableAuthError>), DurableAuthError> {
        let mut connection = self
            .storage
            .connect()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let read = async {
            let mut tx = connection
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await?;
            self.verify(&mut tx).await?;
            let prepared = PreparedLoginRead::prepare(&self.context, &mut tx, username).await?;
            self.verify(&mut tx).await?;
            tx.commit().await?;
            Ok::<_, DurableAuthError>(prepared)
        }
        .await;
        // Even a failed read is closed before returning. Close and KDF share the total login
        // budget; no physical connection or database lock is retained during password work.
        let closed = connection
            .close()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable);
        let prepared = read?;
        closed?;
        let verified = prepared
            .verify_password(&self.context, password, otp)
            .await?;
        let mut connection = self
            .storage
            .connect()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let result = async {
            let mut tx = connection
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await?;
            self.verify(&mut tx).await?;
            let prepared = verified
                .prepare(
                    &self.context,
                    &mut tx,
                    otp,
                    Some(self.storage.config().limits()),
                )
                .await?;
            // Includes original catalog/marker identities after all deferred INSERT work. Do
            // not place this guard after the final fresh credential, stock and OTP proof.
            self.verify(&mut tx).await?;
            prepared.commit(&self.context, tx, otp).await
        }
        .await;
        Ok((connection, result))
    }

    async fn verify(
        &self,
        tx: &mut sqlx_core::transaction::Transaction<'_, crate::sqlx_compat::Postgres>,
    ) -> Result<(), DurableAuthError> {
        self.storage
            .verify_transaction(tx)
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)
    }
}
