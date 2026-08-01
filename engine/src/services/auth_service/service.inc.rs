impl AuthService {
    pub fn new_with_default_admin() -> Self {
        let username = std::env::var("CYANREX_ADMIN_USERNAME")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .and_then(|value| sanitize_username(&value).ok())
            .unwrap_or_else(|| DEFAULT_ADMIN_USERNAME.to_string());

        let password = std::env::var("CYANREX_ADMIN_PASSWORD")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ADMIN_PASSWORD.to_string());

        let totp_secret = std::env::var("CYANREX_ADMIN_TOTP_SECRET")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_ADMIN_TOTP_SECRET.to_string());

        let password_salt = generate_password_salt();
        let default_admin = UserRecord {
            username: username.clone(),
            password_salt: password_salt.clone(),
            password_hash: derive_password_hash(&password, &password_salt),
            totp_secret,
        };

        let mut users = HashMap::new();
        users.insert(username, default_admin.clone());

        let db_pool = std::env::var("DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .and_then(|url| {
                PgPoolOptions::new()
                    .max_connections(5)
                    .connect_lazy(&url)
                    .ok()
            });

        Self {
            users: Arc::new(RwLock::new(users)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            login_attempts: Arc::new(RwLock::new(HashMap::new())),
            db_pool,
            schema_ready: Arc::new(OnceCell::new()),
            db_disabled: Arc::new(AtomicBool::new(false)),
            default_admin,
        }
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        otp: &str,
    ) -> Result<LoginOk, AuthError> {
        let normalized_username = sanitize_username(username).map_err(|_| AuthError::InvalidCredentials)?;
        let attempt_key = normalized_username.clone();
        {
            let attempts = self
                .login_attempts
                .read()
                .expect("login attempts lock poisoned");
            if attempts
                .get(&attempt_key)
                .and_then(|attempt| attempt.blocked_until)
                .is_some_and(|until| until > Utc::now())
            {
                return Err(AuthError::RateLimited);
            }
        }

        let user = self
            .get_user(&normalized_username)
            .await
            .ok_or_else(|| {
                self.record_login_failure(&attempt_key);
                AuthError::InvalidCredentials
            })?;

        if !verify_password_async(password, &user.password_salt, &user.password_hash).await {
            self.record_login_failure(&attempt_key);
            return Err(AuthError::InvalidCredentials);
        }

        if !verify_totp(&user.totp_secret, otp) {
            self.record_login_failure(&attempt_key);
            return Err(AuthError::InvalidOtp);
        }

        self.login_attempts
            .write()
            .expect("login attempts lock poisoned")
            .remove(&attempt_key);

        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::hours(SESSION_HOURS);

        let session = SessionRecord {
            token: token.clone(),
            username: user.username.clone(),
            expires_at,
        };

        {
            let mut sessions = self.sessions.write().expect("auth sessions lock poisoned");
            sessions.insert(token.clone(), session);
        }

        if let Some(pool) = self.active_pool() {
            if let Err(error) = self.ensure_schema_and_seed().await {
                tracing::warn!("auth db unavailable, fallback to memory: {error}");
            } else if let Err(error) = sqlx::query(
                "INSERT INTO sessions (token, username, expires_at) VALUES ($1, $2, $3)",
            )
            .bind(hash_session_token(&token))
            .bind(&user.username)
            .bind(expires_at)
            .execute(pool)
            .await
            {
                self.disable_db(&format!("insert session failed: {error}"));
            }
        }

        Ok(LoginOk {
            token,
            username: user.username,
            expires_at,
        })
    }

    fn record_login_failure(&self, key: &str) {
        const MAX_FAILURES: u32 = 5;
        const LOCK_MINUTES: i64 = 5;
        let mut attempts = self
            .login_attempts
            .write()
            .expect("login attempts lock poisoned");
        let attempt = attempts.entry(key.to_string()).or_insert(LoginAttempt {
            failures: 0,
            blocked_until: None,
        });
        attempt.failures = attempt.failures.saturating_add(1);
        if attempt.failures >= MAX_FAILURES {
            attempt.blocked_until = Some(Utc::now() + Duration::minutes(LOCK_MINUTES));
            attempt.failures = 0;
        }
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
                    Ok(None) => return None,
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

    pub async fn logout(&self, token: &str) {
        let token_hash = hash_session_token(token);
        {
            let mut sessions = self.sessions.write().expect("auth sessions lock poisoned");
            sessions.remove(token);
        }

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                if let Err(error) = sqlx::query("DELETE FROM sessions WHERE token = $1")
                    .bind(&token_hash)
                    .execute(pool)
                    .await
                {
                    self.disable_db(&format!("logout delete session failed: {error}"));
                }
            }
        }
    }

    pub fn generate_current_totp_for_user(&self, username: &str) -> Option<String> {
        let users = self.users.read().expect("auth users lock poisoned");
        let user = users.get(username)?;
        Some(compute_current_totp_code(&user.totp_secret))
    }

    pub async fn bootstrap_totp(
        &self,
        username: &str,
        password: &str,
    ) -> Result<TotpBootstrap, AuthError> {
        let username = sanitize_username(username).map_err(|_| AuthError::InvalidCredentials)?;
        let user = self
            .get_user(&username)
            .await
            .ok_or(AuthError::InvalidCredentials)?;

        if !verify_password_async(password, &user.password_salt, &user.password_hash).await {
            return Err(AuthError::InvalidCredentials);
        }

        let issuer = std::env::var("CYANREX_TOTP_ISSUER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cyanrex-lab".to_string());
        let account_name = user.username.clone();
        let otpauth_uri = build_otpauth_uri(&issuer, &account_name, &user.totp_secret);

        Ok(TotpBootstrap {
            issuer,
            account_name,
            secret: user.totp_secret,
            otpauth_uri,
        })
    }

    pub async fn register(&self, username: &str, password: &str) -> Result<RegisterOk, AuthError> {
        let normalized_username = match sanitize_username(username) {
            Ok(name) => name,
            Err(_) => return Err(AuthError::InvalidInput),
        };
        if password.len() < 8 {
            return Err(AuthError::InvalidInput);
        }

        let totp_secret = generate_totp_secret();
        let password_salt = generate_password_salt();
        let password_hash = derive_password_hash_async(password, &password_salt)
            .await
            .ok_or(AuthError::InvalidInput)?;
        let issuer = std::env::var("CYANREX_TOTP_ISSUER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cyanrex-lab".to_string());
        let account_name = normalized_username.to_string();
        let otpauth_uri = build_otpauth_uri(&issuer, &account_name, &totp_secret);

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                let inserted = sqlx::query(
                    "INSERT INTO users (username, password_salt, password_hash, totp_secret, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, NOW(), NOW())
                     ON CONFLICT (username) DO NOTHING
                     RETURNING username",
                )
                .bind(&account_name)
                .bind(&password_salt)
                .bind(&password_hash)
                .bind(&totp_secret)
                .fetch_optional(pool)
                .await;

                match inserted {
                    Ok(None) => return Err(AuthError::UserAlreadyExists),
                    Ok(Some(_)) => {
                        let mut users = self.users.write().expect("auth users lock poisoned");
                        users.insert(
                            account_name.clone(),
                            UserRecord {
                                username: account_name.clone(),
                                password_salt: password_salt.clone(),
                                password_hash: password_hash.clone(),
                                totp_secret: totp_secret.clone(),
                            },
                        );
                    }
                    Err(error) => {
                        self.disable_db(&format!("register insert failed: {error}"));
                    }
                }
            }
        }

        if self.active_pool().is_none() {
            let mut users = self.users.write().expect("auth users lock poisoned");
            if users.contains_key(&account_name) {
                return Err(AuthError::UserAlreadyExists);
            }
            users.insert(
                account_name.clone(),
                UserRecord {
                    username: account_name.clone(),
                    password_salt,
                    password_hash,
                    totp_secret: totp_secret.clone(),
                },
            );
        }

        Ok(RegisterOk {
            issuer,
            account_name,
            secret: totp_secret,
            otpauth_uri,
        })
    }

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

        let user = self
            .get_user(username)
            .await
            .ok_or(AuthError::InvalidCredentials)?;

        if !verify_password_async(current_password, &user.password_salt, &user.password_hash).await {
            return Err(AuthError::InvalidCredentials);
        }
        if !verify_totp(&user.totp_secret, otp) {
            return Err(AuthError::InvalidOtp);
        }

        let new_salt = generate_password_salt();
        let new_hash = derive_password_hash_async(new_password, &new_salt)
            .await
            .ok_or(AuthError::InvalidInput)?;

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                if let Err(error) = sqlx::query(
                    "UPDATE users SET password_salt = $1, password_hash = $2, updated_at = NOW() WHERE username = $3",
                )
                .bind(&new_salt)
                .bind(&new_hash)
            .bind(&username)
                .execute(pool)
                .await
                {
                    self.disable_db(&format!("change password update failed: {error}"));
                }
            }
        }

        let mut users = self.users.write().expect("auth users lock poisoned");
        if let Some(record) = users.get_mut(username) {
            record.password_salt = new_salt;
            record.password_hash = new_hash;
        }

        Ok(())
    }

    pub async fn delete_account(
        &self,
        username: &str,
        password: &str,
        otp: &str,
    ) -> Result<(), AuthError> {
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

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                let count = sqlx::query("SELECT COUNT(*) AS count FROM users")
                    .fetch_one(pool)
                    .await
                    .map(|row| row.get::<i64, _>("count"));

                match count {
                    Ok(total) if total <= 1 => return Err(AuthError::Forbidden),
                    Ok(_) => {
                        if let Err(error) = sqlx::query("DELETE FROM sessions WHERE username = $1")
                            .bind(username)
                            .execute(pool)
                            .await
                        {
                            self.disable_db(&format!(
                                "delete account clear sessions failed: {error}"
                            ));
                        }
                        if let Err(error) = sqlx::query("DELETE FROM users WHERE username = $1")
                            .bind(username)
                            .execute(pool)
                            .await
                        {
                            self.disable_db(&format!("delete account delete user failed: {error}"));
                        }
                    }
                    Err(error) => {
                        self.disable_db(&format!("delete account count failed: {error}"));
                    }
                }
            }
        } else {
            let users = self.users.read().expect("auth users lock poisoned");
            if users.len() <= 1 {
                return Err(AuthError::Forbidden);
            }
            drop(users);
        }

        {
            let mut users = self.users.write().expect("auth users lock poisoned");
            users.remove(username);
        }
        {
            let mut sessions = self.sessions.write().expect("auth sessions lock poisoned");
            sessions.retain(|_, session| session.username != username);
        }

        Ok(())
    }

    fn active_pool(&self) -> Option<&PgPool> {
        if !crate::config::db_fallback_enabled() {
            return None;
        }
        if self.db_disabled.load(Ordering::Relaxed) {
            return None;
        }
        self.db_pool.as_ref()
    }

    fn disable_db(&self, reason: &str) {
        tracing::warn!("disabling auth db persistence: {reason}");
        self.db_disabled.store(true, Ordering::Relaxed);
    }

    async fn ensure_schema_and_seed(&self) -> Result<(), sqlx::Error> {
        let Some(pool) = self.active_pool() else {
            return Ok(());
        };

        self.schema_ready
            .get_or_try_init(|| async {
                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS users (
                        username TEXT PRIMARY KEY,
                        password_salt TEXT NOT NULL,
                        password_hash TEXT NOT NULL,
                        totp_secret TEXT NOT NULL,
                        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                    )",
                )
                .execute(pool)
                .await?;

                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS sessions (
                        token TEXT PRIMARY KEY,
                        username TEXT NOT NULL REFERENCES users(username) ON DELETE CASCADE,
                        expires_at TIMESTAMPTZ NOT NULL,
                        created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                    )",
                )
                .execute(pool)
                .await?;

                sqlx::query(
                    "INSERT INTO users (username, password_salt, password_hash, totp_secret, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, NOW(), NOW())
                     ON CONFLICT (username) DO NOTHING",
                )
                .bind(&self.default_admin.username)
                .bind(&self.default_admin.password_salt)
                .bind(&self.default_admin.password_hash)
                .bind(&self.default_admin.totp_secret)
                .execute(pool)
                .await?;

                if std::env::var("CYANREX_ROTATE_ADMIN_CREDENTIALS")
                    .ok()
                    .is_some_and(|value| value.eq_ignore_ascii_case("true") || value == "1")
                {
                    sqlx::query(
                        "UPDATE users
                         SET password_salt = $1, password_hash = $2, totp_secret = $3, updated_at = NOW()
                         WHERE username = $4",
                    )
                    .bind(&self.default_admin.password_salt)
                    .bind(&self.default_admin.password_hash)
                    .bind(&self.default_admin.totp_secret)
                    .bind(&self.default_admin.username)
                    .execute(pool)
                    .await?;
                    sqlx::query("DELETE FROM sessions WHERE username = $1")
                        .bind(&self.default_admin.username)
                        .execute(pool)
                        .await?;
                }

                let rows = sqlx::query(
                    "SELECT username, password_salt, password_hash, totp_secret FROM users",
                )
                .fetch_all(pool)
                .await?;

                let mut users = self.users.write().expect("auth users lock poisoned");
                users.clear();
                for row in rows {
                    users.insert(
                        row.get::<String, _>("username"),
                        UserRecord {
                            username: row.get("username"),
                            password_salt: row.get("password_salt"),
                            password_hash: row.get("password_hash"),
                            totp_secret: row.get("totp_secret"),
                        },
                    );
                }

                Ok(())
            })
            .await
            .map(|_| ())
    }

    async fn get_user(&self, username: &str) -> Option<UserRecord> {
        let username = sanitize_username(username).ok()?;
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema_and_seed().await.is_ok() {
                match sqlx::query(
                    "SELECT username, password_salt, password_hash, totp_secret FROM users WHERE username = $1",
                )
                .bind(&username)
                .fetch_optional(pool)
                .await
                {
                    Ok(Some(row)) => {
                        let record = UserRecord {
                            username: row.get("username"),
                            password_salt: row.get("password_salt"),
                            password_hash: row.get("password_hash"),
                            totp_secret: row.get("totp_secret"),
                        };
                        let mut users = self.users.write().expect("auth users lock poisoned");
                        users.insert(record.username.clone(), record.clone());
                        return Some(record);
                    }
                    Ok(None) => return None,
                    Err(error) => self.disable_db(&format!("get user failed: {error}")),
                }
            }
        }

        let users = self.users.read().expect("auth users lock poisoned");
        users.get(&username).cloned()
    }
}
