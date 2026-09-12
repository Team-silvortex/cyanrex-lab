impl AuthService {
    async fn issue_verified_session(
        &self,
        user: &UserRecord,
        session: SessionRecord,
        pool: Option<&PgPool>,
    ) -> Result<(), AuthError> {
        if let Some(pool) = pool {
            self.ensure_schema_and_seed().await.map_err(Self::mutation_storage_error)?;
            let mut transaction = pool.begin().await.map_err(Self::mutation_storage_error)?;
            let inserted = sqlx::query(
                "INSERT INTO sessions (token, username, expires_at) VALUES ($1, $2, $3)",
            )
            .bind(hash_session_token(&session.token))
            .bind(&user.username)
            .bind(session.expires_at)
            .execute(&mut *transaction)
            .await
            .map_err(Self::mutation_storage_error)?;
            if inserted.rows_affected() != 1 {
                return Err(AuthError::StorageUnavailable);
            }
            // A fresh statement snapshot after INSERT catches deletion/recreation or a credential
            // change while its trigger/FK checks were waiting. SHARE prevents changes until COMMIT.
            let current = sqlx::query(
                "SELECT username FROM users WHERE username = $1 AND password_salt = $2
                 AND password_hash = $3 AND totp_secret = $4 FOR SHARE",
            )
            .bind(&user.username).bind(&user.password_salt)
            .bind(&user.password_hash).bind(&user.totp_secret)
            .fetch_optional(&mut *transaction).await.map_err(Self::mutation_storage_error)?;
            if current.is_none() {
                return Err(AuthError::InvalidCredentials);
            }
            transaction.commit().await.map_err(Self::mutation_storage_error)?;
        }

        // Hold the identity lock through publication, including in intentionally volatile mode.
        // A later durable mutation may already have evicted/replaced this cache after our COMMIT;
        // do not repopulate it with that stale session.
        let users = self.users.read().expect("auth users lock poisoned");
        if users.get(&user.username).is_some_and(|current| same_credentials(current, user)) {
            self.sessions.write().expect("auth sessions lock poisoned")
                .insert(session.token.clone(), session);
        } else if pool.is_none() {
            return Err(AuthError::InvalidCredentials);
        }
        Ok(())
    }

    pub async fn validate_session(&self, token: &str) -> Option<SessionRecord> {
        let token_hash = hash_session_token(token);
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                match sqlx::query(
                    "SELECT token, username, expires_at FROM sessions WHERE token = $1",
                )
                .bind(&token_hash)
                .fetch_optional(pool)
                .await
                {
                    Ok(Some(row)) => {
                        let expires_at: DateTime<Utc> = row.get("expires_at");
                        if expires_at <= Utc::now() {
                            self.forget_cached_session(token);
                            let _ = sqlx::query("DELETE FROM sessions WHERE token = $1")
                                .bind(&token_hash)
                                .execute(pool)
                                .await;
                            return None;
                        }

                        return Some(SessionRecord {
                            token: token.to_string(),
                            username: row.get("username"),
                            expires_at,
                        });
                    }
                    Ok(None) => {
                        self.forget_cached_session(token);
                        return None;
                    }
                    Err(error) => self.disable_db(&format!("validate session failed: {error}")),
                }
            }
        }

        let mut sessions = self.sessions.write().expect("auth sessions lock poisoned");
        if let Some(session) = sessions.get(token).cloned() {
            if session.expires_at > Utc::now() {
                return Some(session);
            }
            sessions.remove(token);
        }

        None
    }

    pub async fn logout(&self, token: &str) -> Result<(), AuthError> {
        let token_hash = hash_session_token(token);
        if let Some(pool) = self.mutation_pool()? {
            self.ensure_schema_and_seed()
                .await
                .map_err(Self::mutation_storage_error)?;
            let deleted = sqlx::query("DELETE FROM sessions WHERE token = $1")
                .bind(&token_hash)
                .execute(pool)
                .await
                .map_err(Self::mutation_storage_error)?;
            if deleted.rows_affected() == 0 {
                // Already absent is an idempotent success; a suppressed deletion is not.
                let remaining = sqlx::query("SELECT token FROM sessions WHERE token = $1")
                    .bind(&token_hash)
                    .fetch_optional(pool)
                    .await
                    .map_err(Self::mutation_storage_error)?;
                if remaining.is_some() {
                    return Err(AuthError::StorageUnavailable);
                }
            }
        }
        self.forget_cached_session(token);
        Ok(())
    }

    // Once the database denies a session, a later outage must not resurrect its cache entry.
    fn forget_cached_session(&self, token: &str) {
        self.sessions
            .write()
            .expect("auth sessions lock poisoned")
            .remove(token);
    }
}
