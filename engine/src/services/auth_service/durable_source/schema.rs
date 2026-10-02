use super::*;

impl DurableAuthSource {
    /// Explicit empty-source installation, not account adoption or session backfill. Existing
    /// nonempty auth tables are left untouched, even when their usernames look familiar.
    pub async fn install_empty_schema(&self) -> Result<()> {
        bounded(async {
            let mut tx = self.transaction().await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.auth-source.' || current_schema()))").execute(&mut *tx).await?;
            let installed: bool = sqlx::query("SELECT to_regclass('collaboration_auth_source_schema') IS NOT NULL AS installed")
                .fetch_one(&mut *tx).await?.try_get("installed")?;
            if installed {
                self.lock_source(&mut tx, false).await?;
                Self::verify_schema(&mut tx).await?;
                tx.commit().await?;
                return Ok(());
            }
            for statement in include_str!("../../../../migrations/0001_auth_users_sessions.sql")
                .split(';').map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            // Fence legacy writers before checking emptiness and adding incarnation constraints.
            sqlx::query("LOCK TABLE users, sessions IN ACCESS EXCLUSIVE MODE").execute(&mut *tx).await?;
            let nonempty: bool = sqlx::query("SELECT EXISTS (SELECT 1 FROM users) OR EXISTS (SELECT 1 FROM sessions) AS nonempty")
                .fetch_one(&mut *tx).await?.try_get("nonempty")?;
            if nonempty { return Err(DurableAuthError::LegacyDataPresent); }
            Self::verify_legacy_shape(&mut tx).await?;
            for statement in include_str!("../../../../migrations/0010_durable_auth_source.sql")
                .split("-- cyanrex-statement").map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
            confirmed_one(sqlx::query("INSERT INTO collaboration_auth_source_schema (singleton, version, authority_id) VALUES (TRUE, 1, $1)")
                .bind(self.authority_id.as_uuid()).execute(&mut *tx).await?.rows_affected())?;
            self.lock_source(&mut tx, false).await?;
            Self::verify_schema(&mut tx).await?;
            tx.commit().await?;
            Ok(())
        }).await
    }

    async fn verify_legacy_shape(connection: &mut PgConnection) -> Result<()> {
        // Empty tables and a successful projection do not establish uniqueness, nullability,
        // cascading deletion or timestamp semantics. Check the catalog before any adoption.
        let columns: i64 = sqlx::query(
            "WITH expected(relation, column_name, type_name) AS (VALUES
            ('users', 'username', 'text'), ('users', 'password_salt', 'text'),
            ('users', 'password_hash', 'text'), ('users', 'totp_secret', 'text'),
            ('users', 'created_at', 'timestamptz'), ('users', 'updated_at', 'timestamptz'),
            ('sessions', 'token', 'text'), ('sessions', 'username', 'text'),
            ('sessions', 'expires_at', 'timestamptz'), ('sessions', 'created_at', 'timestamptz'))
            SELECT count(*) AS count FROM expected e JOIN pg_attribute a
            ON a.attrelid = e.relation::REGCLASS AND a.attname = e.column_name
            WHERE a.attnotnull AND NOT a.attisdropped AND a.atttypid = e.type_name::REGTYPE",
        )
        .fetch_one(&mut *connection)
        .await?
        .try_get("count")?;
        let constraints: i64 = sqlx::query("WITH shape AS (SELECT c.*,
            ARRAY(SELECT a.attname::TEXT FROM unnest(c.conkey) WITH ORDINALITY k(attnum, position)
                JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum ORDER BY k.position) AS columns,
            ARRAY(SELECT a.attname::TEXT FROM unnest(c.confkey) WITH ORDINALITY k(attnum, position)
                JOIN pg_attribute a ON a.attrelid = c.confrelid AND a.attnum = k.attnum ORDER BY k.position) AS referenced
            FROM pg_constraint c WHERE c.conrelid IN ('users'::REGCLASS, 'sessions'::REGCLASS))
            SELECT count(*) AS count FROM shape WHERE convalidated AND NOT condeferrable AND (
                (conrelid = 'users'::REGCLASS AND contype = 'p' AND columns = ARRAY['username']) OR
                (conrelid = 'sessions'::REGCLASS AND contype = 'p' AND columns = ARRAY['token']) OR
                (conrelid = 'sessions'::REGCLASS AND contype = 'f' AND columns = ARRAY['username']
                    AND confrelid = 'users'::REGCLASS AND referenced = ARRAY['username'] AND confdeltype = 'c'))")
            .fetch_one(&mut *connection).await?.try_get("count")?;
        if columns != 10 || constraints != 3 {
            return Err(DurableAuthError::InvalidRecord);
        }
        Ok(())
    }

    pub(super) async fn verify_schema(connection: &mut PgConnection) -> Result<()> {
        Self::verify_legacy_shape(connection).await?;
        let columns: i64 = sqlx::query(
            "SELECT count(*) AS count FROM pg_attribute WHERE
            attrelid IN ('users'::REGCLASS, 'sessions'::REGCLASS) AND attname = 'account_id'
            AND atttypid = 'uuid'::REGTYPE AND attnotnull AND NOT attisdropped",
        )
        .fetch_one(&mut *connection)
        .await?
        .try_get("count")?;
        let triggers: i64 = sqlx::query("SELECT count(*) AS count FROM pg_trigger WHERE NOT tgisinternal
            AND tgenabled IN ('O', 'A') AND tgtype = 19
            AND tgfoid = 'collaboration_auth_reject_reassignment()'::REGPROCEDURE
            AND ((tgrelid = 'users'::REGCLASS AND tgname = 'collaboration_auth_account_immutable')
                OR (tgrelid = 'sessions'::REGCLASS AND tgname = 'collaboration_auth_session_immutable'))")
            .fetch_one(&mut *connection).await?.try_get("count")?;
        let constraints: i64 = sqlx::query("WITH shape AS (SELECT c.*,
            ARRAY(SELECT a.attname::TEXT FROM unnest(c.conkey) WITH ORDINALITY k(attnum, position)
                JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum ORDER BY k.position) AS columns,
            ARRAY(SELECT a.attname::TEXT FROM unnest(c.confkey) WITH ORDINALITY k(attnum, position)
                JOIN pg_attribute a ON a.attrelid = c.confrelid AND a.attnum = k.attnum ORDER BY k.position) AS referenced
            FROM pg_constraint c WHERE c.conrelid IN ('users'::REGCLASS, 'sessions'::REGCLASS))
            SELECT count(*) AS count FROM shape WHERE convalidated AND NOT condeferrable
            AND ((conrelid = 'users'::REGCLASS AND contype = 'u' AND conname = 'collaboration_auth_account_id_unique' AND columns = ARRAY['account_id'])
                OR (conrelid = 'users'::REGCLASS AND contype = 'u' AND conname = 'collaboration_auth_account_coordinate' AND columns = ARRAY['username', 'account_id'])
                OR (conrelid = 'sessions'::REGCLASS AND conname = 'collaboration_auth_session_account' AND contype = 'f'
                    AND confrelid = 'users'::REGCLASS AND confdeltype = 'c' AND columns = ARRAY['username', 'account_id'] AND referenced = columns)
                OR (conrelid = 'sessions'::REGCLASS AND conname = 'collaboration_auth_session_digest' AND contype = 'c'))")
            .fetch_one(&mut *connection).await?.try_get("count")?;
        if columns != 2 || triggers != 2 || constraints != 4 {
            return Err(DurableAuthError::InvalidRecord);
        }
        Ok(())
    }
}
