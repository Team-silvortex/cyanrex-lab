impl AuthService {
    pub async fn change_password(
        &self,
        username: &str,
        current_password: &str,
        new_password: &str,
        otp: &str,
    ) -> Result<(), AuthError> {
        if new_password.len() < 8 {
            return Err(AuthError::WeakPassword);
        }
        self.mutation_pool()?;
        let user = self
            .get_user(username)
            .await
            .ok_or(AuthError::InvalidCredentials)?;
        if !verify_password_async(current_password, &user.password_salt, &user.password_hash).await
        {
            return Err(AuthError::InvalidCredentials);
        }
        if !verify_totp(&user.totp_secret, otp) {
            return Err(AuthError::InvalidOtp);
        }
        let new_salt = generate_password_salt();
        let new_hash = derive_password_hash_async(new_password, &new_salt)
            .await
            .ok_or(AuthError::InvalidInput)?;

        let persisted = if let Some(pool) = self.mutation_pool()? {
            self.ensure_schema_and_seed()
                .await
                .map_err(Self::mutation_storage_error)?;
            let updated = sqlx::query(
                "UPDATE users SET password_salt = $1, password_hash = $2, updated_at = NOW()
                 WHERE username = $3 AND password_salt = $4 AND password_hash = $5 AND totp_secret = $6",
            )
            .bind(&new_salt).bind(&new_hash).bind(&user.username)
            .bind(&user.password_salt).bind(&user.password_hash).bind(&user.totp_secret)
            .execute(pool).await.map_err(Self::mutation_storage_error)?;
            if updated.rows_affected() != 1 {
                return Err(AuthError::InvalidCredentials);
            }
            true
        } else {
            false
        };

        let mut users = self.users.write().expect("auth users lock poisoned");
        match users.get_mut(&user.username) {
            Some(record) if same_credentials(record, &user) => {
                record.password_salt = new_salt;
                record.password_hash = new_hash;
            }
            // A later authoritative read/change may already have replaced or evicted this snapshot.
            _ if persisted => {}
            _ => return Err(AuthError::InvalidCredentials),
        }
        Ok(())
    }

    pub async fn delete_account(
        &self,
        username: &str,
        password: &str,
        otp: &str,
    ) -> Result<(), AuthError> {
        self.mutation_pool()?;
        let user = self
            .get_user(username)
            .await
            .ok_or(AuthError::InvalidCredentials)?;
        if !verify_password_async(password, &user.password_salt, &user.password_hash).await {
            return Err(AuthError::InvalidCredentials);
        }
        if !verify_totp(&user.totp_secret, otp) {
            return Err(AuthError::InvalidOtp);
        }
        if user.username == self.default_admin.username {
            return Err(AuthError::Forbidden);
        }

        let persisted = if let Some(pool) = self.mutation_pool()? {
            self.ensure_schema_and_seed()
                .await
                .map_err(Self::mutation_storage_error)?;
            // Keep session removal and account deletion in one commit, including zero-row failures.
            let mut transaction = pool.begin().await.map_err(Self::mutation_storage_error)?;
            let total: i64 = sqlx::query("SELECT COUNT(*) AS count FROM users")
                .fetch_one(&mut *transaction)
                .await
                .map_err(Self::mutation_storage_error)?
                .get("count");
            if total <= 1 {
                return Err(AuthError::Forbidden);
            }
            sqlx::query("DELETE FROM sessions WHERE username = $1")
                .bind(&user.username)
                .execute(&mut *transaction)
                .await
                .map_err(Self::mutation_storage_error)?;
            let deleted = sqlx::query(
                "DELETE FROM users WHERE username = $1 AND password_salt = $2 AND password_hash = $3 AND totp_secret = $4",
            )
            .bind(&user.username).bind(&user.password_salt).bind(&user.password_hash).bind(&user.totp_secret)
            .execute(&mut *transaction).await.map_err(Self::mutation_storage_error)?;
            if deleted.rows_affected() != 1 {
                return Err(AuthError::InvalidCredentials);
            }
            // A BEFORE DELETE trigger may suppress session deletion (including the FK cascade).
            // Verify the postcondition in this transaction before acknowledging account removal.
            let remaining = sqlx::query("SELECT token FROM sessions WHERE username = $1 LIMIT 1")
                .bind(&user.username)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(Self::mutation_storage_error)?;
            if remaining.is_some() {
                return Err(AuthError::StorageUnavailable);
            }
            transaction
                .commit()
                .await
                .map_err(Self::mutation_storage_error)?;
            true
        } else {
            false
        };

        {
            let mut users = self.users.write().expect("auth users lock poisoned");
            if !persisted {
                if users.len() <= 1 {
                    return Err(AuthError::Forbidden);
                }
                if !users
                    .get(&user.username)
                    .is_some_and(|record| same_credentials(record, &user))
                {
                    return Err(AuthError::InvalidCredentials);
                }
            }
            users.remove(&user.username);
        }
        self.sessions
            .write()
            .expect("auth sessions lock poisoned")
            .retain(|_, session| session.username != user.username);
        Ok(())
    }

    fn mutation_pool(&self) -> Result<Option<&PgPool>, AuthError> {
        if !crate::config::db_fallback_enabled() || self.db_pool.is_none() {
            return Ok(None);
        }
        // Configured durable auth must not acknowledge a memory-only credential change/revocation.
        if self.db_disabled.load(Ordering::Relaxed) {
            return Err(AuthError::StorageUnavailable);
        }
        Ok(self.db_pool.as_ref())
    }

    fn mutation_storage_error(error: sqlx::Error) -> AuthError {
        tracing::warn!(%error, "auth mutation persistence was not confirmed");
        AuthError::StorageUnavailable
    }
}

fn same_credentials(current: &UserRecord, expected: &UserRecord) -> bool {
    current.password_salt == expected.password_salt
        && current.password_hash == expected.password_hash
        && current.totp_secret == expected.totp_secret
}
