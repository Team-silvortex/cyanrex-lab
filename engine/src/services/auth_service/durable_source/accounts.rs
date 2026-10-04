use super::*;

pub(super) struct AccountRecord {
    pub account: DurableAccountRef,
    pub user: UserRecord,
    pub(super) otp_watermark: otp_consumption::Watermark,
}

impl DurableAuthSource {
    /// Caller holds the source writer fence and owns commit. Never publish this pending result.
    pub(super) async fn insert_prepared_account(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        expected: &UserRecord,
    ) -> Result<DurableRegistration> {
        let username: LegacyUsername = expected.username.parse()?;
        if self.account_record(tx, &username).await?.is_some() {
            return Err(DurableAuthError::AccountExists);
        }
        let account = DurableAccountRef {
            authority_id: self.authority_id,
            username: username.clone(),
            account_id: LegacyAccountId::try_from(Uuid::new_v4())?,
        };
        confirmed_one(sqlx::query("INSERT INTO users (username, account_id, password_salt, password_hash, totp_secret) VALUES ($1, $2, $3, $4, $5)")
            .bind(username.as_str()).bind(account.account_id.as_uuid()).bind(&expected.password_salt).bind(&expected.password_hash).bind(&expected.totp_secret)
            .execute(&mut **tx).await?.rows_affected())?;
        let actual = self
            .account_record(tx, &username)
            .await?
            .ok_or(DurableAuthError::StorageUnavailable)?;
        if actual.account != account
            || !same_credentials(&actual.user, expected)
            || actual.otp_watermark.stored() != -1
        {
            return Err(DurableAuthError::StorageUnavailable);
        }
        let issuer = "cyanrex-lab".to_string();
        let bootstrap = RegisterOk {
            issuer: issuer.clone(),
            account_name: username.to_string(),
            otpauth_uri: build_otpauth_uri(&issuer, username.as_str(), &expected.totp_secret),
            secret: expected.totp_secret.clone(),
        };
        Ok(DurableRegistration { account, bootstrap })
    }

    pub(super) async fn account_record(
        &self,
        connection: &mut PgConnection,
        username: &LegacyUsername,
    ) -> Result<Option<AccountRecord>> {
        let row = sqlx::query("SELECT username, account_id, password_salt, password_hash, totp_secret, otp_last_counter FROM users WHERE username = $1 FOR SHARE")
            .bind(username.as_str()).fetch_optional(connection).await?;
        row.map(|row| {
            let name: String = row.try_get("username")?;
            let account = DurableAccountRef {
                authority_id: self.authority_id,
                username: name.clone().parse()?,
                account_id: LegacyAccountId::try_from(row.try_get::<Uuid, _>("account_id")?)?,
            };
            let user = UserRecord {
                username: name,
                password_salt: row.try_get("password_salt")?,
                password_hash: row.try_get("password_hash")?,
                totp_secret: row.try_get("totp_secret")?,
            };
            password_profile::validate(&user.password_hash)?;
            if user.password_salt.len() > 128
                || user.totp_secret.len() > 128
                || super::super::decode_base32_secret(&user.totp_secret)
                    .is_none_or(|bytes| bytes.is_empty())
            {
                return Err(DurableAuthError::InvalidRecord);
            }
            let otp_watermark = otp_consumption::Watermark::from_stored(
                row.try_get::<i64, _>("otp_last_counter")
                    .map_err(|_| DurableAuthError::InvalidRecord)?,
            )
            .map_err(|_| DurableAuthError::InvalidRecord)?;
            Ok(AccountRecord {
                account,
                user,
                otp_watermark,
            })
        })
        .transpose()
    }

    /// Trusted registration-adapter primitive, not public enrollment or teacher bootstrap.
    /// Commits a new random incarnation; never derives it from a name, credential or import time.
    pub async fn register(
        &self,
        username: &LegacyUsername,
        password: &str,
    ) -> Result<DurableRegistration> {
        if !(8..=4096).contains(&password.len()) {
            return Err(DurableAuthError::InvalidInput);
        }
        bounded(async {
            let salt = generate_password_salt();
            let hash = derive_password_hash(password, &salt).await?;
            let secret = generate_totp_secret();
            let mut tx = self.transaction().await?;
            self.lock_source(&mut tx, true).await?;
            Self::verify_schema(&mut tx).await?;
            let expected = UserRecord {
                username: username.to_string(),
                password_salt: salt,
                password_hash: hash,
                totp_secret: secret,
            };
            let pending = self.insert_prepared_account(&mut tx, &expected).await?;
            Self::verify_schema(&mut tx).await?;
            tx.commit().await?;
            Ok(pending)
        })
        .await
    }
}
