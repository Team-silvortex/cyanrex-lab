//! Selected canonical head and complete two-entry history, not a provenance assertion.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Change {
    pub(super) grant: FilePublisherGrant,
    pub(super) actor: FileAccountRef,
    pub(super) recorded_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Selected {
    pub(super) record: StoredFilePublisherGrant,
    pub(super) changes: Vec<Change>,
}

pub(super) fn canonical(grant: &FilePublisherGrant) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(grant).map_err(|_| FilePublisherGrantStoreError::InvalidRecord)?;
    if raw.len() > MAX_FILE_GRANT_JSON_BYTES {
        return Err(FilePublisherGrantStoreError::InvalidRecord);
    }
    Ok(raw)
}

impl Selected {
    pub(super) fn fingerprint(&self) -> Result<[u8; 32]> {
        let mut hash = Sha256::new();
        hash.update(b"cyanrex.file-publisher-grant.record.v1\0");
        account_fingerprint(&mut hash, self.record.issuer);
        let raw = canonical(&self.record.grant)?;
        hash.update((raw.len() as u32).to_be_bytes());
        hash.update(raw);
        hash.update((self.changes.len() as u32).to_be_bytes());
        for change in &self.changes {
            account_fingerprint(&mut hash, change.actor);
            hash.update(change.recorded_at.to_be_bytes());
            let raw = canonical(&change.grant)?;
            hash.update((raw.len() as u32).to_be_bytes());
            hash.update(raw);
        }
        Ok(hash.finalize().into())
    }
}

fn account_fingerprint(hash: &mut Sha256, account: FileAccountRef) {
    hash.update(account.principal().authority_id.as_uuid().as_bytes());
    hash.update(account.principal().principal_id.as_uuid().as_bytes());
    hash.update(account.account_id().as_uuid().as_bytes());
}

fn invalid() -> FilePublisherGrantStoreError {
    FilePublisherGrantStoreError::InvalidRecord
}

pub(super) fn revision(value: i64) -> Result<u64> {
    let value = u64::try_from(value).map_err(|_| invalid())?;
    if !(1..=2).contains(&value) {
        return Err(invalid());
    }
    Ok(value)
}

fn validity(not_before: i64, expires_at: i64) -> Result<FileGrantValidity> {
    FileGrantValidity::new(
        u64::try_from(not_before).map_err(|_| invalid())?,
        u64::try_from(expires_at).map_err(|_| invalid())?,
    )
    .map_err(|_| invalid())
}

fn grant(raw: Option<Vec<u8>>) -> Result<FilePublisherGrant> {
    let raw = raw.ok_or_else(invalid)?;
    let grant = FilePublisherGrant::parse_json(&raw).map_err(|_| invalid())?;
    if canonical(&grant)? != raw {
        return Err(invalid());
    }
    Ok(grant)
}

impl FilePublisherGrantStore {
    fn decode_account(&self, principal: Uuid, account: Uuid) -> Result<FileAccountRef> {
        Ok(FileAccountRef::new(
            PrincipalRef {
                authority_id: self.scope.authority_id,
                principal_id: PrincipalId::try_from(principal).map_err(|_| invalid())?,
            },
            LegacyAccountId::try_from(account).map_err(|_| invalid())?,
        ))
    }

    pub(super) async fn read_selected(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        subject: Option<FileAccountRef>,
        share: bool,
    ) -> Result<Option<Selected>> {
        // Foreign subject/incarnation rows are filtered in SQL before their bodies are decoded.
        let query = if share {
            "SELECT grant_id,subject_id,subject_account_id,issuer_id,issuer_account_id,
            revision,not_before,expires_at,
            CASE WHEN octet_length(grant_payload)<=$4 THEN grant_payload ELSE NULL END AS grant_payload
            FROM collaboration_file_publisher_grants WHERE grant_id=$1
            AND ($2::uuid IS NULL OR subject_id=$2) AND ($3::uuid IS NULL OR subject_account_id=$3) FOR SHARE"
        } else {
            "SELECT grant_id,subject_id,subject_account_id,issuer_id,issuer_account_id,
            revision,not_before,expires_at,
            CASE WHEN octet_length(grant_payload)<=$4 THEN grant_payload ELSE NULL END AS grant_payload
            FROM collaboration_file_publisher_grants WHERE grant_id=$1
            AND ($2::uuid IS NULL OR subject_id=$2) AND ($3::uuid IS NULL OR subject_account_id=$3)"
        };
        let row = sqlx::query(query)
            .bind(grant_id.as_uuid())
            .bind(subject.map(|subject| subject.principal().principal_id.as_uuid()))
            .bind(subject.map(|subject| subject.account_id().as_uuid()))
            .bind(MAX_FILE_GRANT_JSON_BYTES as i32)
            .fetch_optional(&mut **tx)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let id =
            FileGrantId::try_from(row.try_get::<Uuid, _>("grant_id")?).map_err(|_| invalid())?;
        let selected_subject = self.decode_account(
            row.try_get("subject_id")?,
            row.try_get("subject_account_id")?,
        )?;
        let issuer =
            self.decode_account(row.try_get("issuer_id")?, row.try_get("issuer_account_id")?)?;
        let selected_revision = revision(row.try_get("revision")?)?;
        let selected_validity = validity(row.try_get("not_before")?, row.try_get("expires_at")?)?;
        let selected_grant = grant(row.try_get("grant_payload")?)?;
        let state = if selected_revision == 1 {
            FileGrantState::Active
        } else {
            FileGrantState::Revoked
        };
        if id != grant_id
            || selected_grant.id() != id
            || selected_grant.workspace() != self.scope
            || selected_grant.subject() != selected_subject
            || selected_grant.validity() != selected_validity
            || u64::from(selected_grant.revision()) != selected_revision
            || selected_grant.state() != state
            || subject.is_some_and(|subject| subject != selected_subject)
        {
            return Err(invalid());
        }
        let history_query = if share {
            "SELECT grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,
            CASE WHEN octet_length(grant_payload)<=$2 THEN grant_payload ELSE NULL END AS grant_payload
            FROM collaboration_file_publisher_grant_changes WHERE grant_id=$1 ORDER BY revision LIMIT 3 FOR SHARE"
        } else {
            "SELECT grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,
            CASE WHEN octet_length(grant_payload)<=$2 THEN grant_payload ELSE NULL END AS grant_payload
            FROM collaboration_file_publisher_grant_changes WHERE grant_id=$1 ORDER BY revision LIMIT 3"
        };
        let rows = sqlx::query(history_query)
            .bind(id.as_uuid())
            .bind(MAX_FILE_GRANT_JSON_BYTES as i32)
            .fetch_all(&mut **tx)
            .await?;
        if rows.len() != selected_revision as usize {
            return Err(invalid());
        }
        let mut changes = Vec::with_capacity(rows.len());
        for (position, row) in rows.into_iter().enumerate() {
            let history_id = FileGrantId::try_from(row.try_get::<Uuid, _>("grant_id")?)
                .map_err(|_| invalid())?;
            let history_revision = revision(row.try_get("revision")?)?;
            let history_validity =
                validity(row.try_get("not_before")?, row.try_get("expires_at")?)?;
            let actor =
                self.decode_account(row.try_get("actor_id")?, row.try_get("actor_account_id")?)?;
            let recorded_at =
                u64::try_from(row.try_get::<i64, _>("recorded_at")?).map_err(|_| invalid())?;
            if recorded_at > MAX_FILE_GRANT_UNIX_SECONDS {
                return Err(invalid());
            }
            let history_grant = grant(row.try_get("grant_payload")?)?;
            let state = if position == 0 {
                FileGrantState::Active
            } else {
                FileGrantState::Revoked
            };
            if history_id != id
                || history_revision != position as u64 + 1
                || history_grant.id() != id
                || history_grant.workspace() != self.scope
                || history_grant.subject() != selected_subject
                || history_validity != selected_validity
                || history_grant.validity() != selected_validity
                || u64::from(history_grant.revision()) != history_revision
                || history_grant.state() != state
                || (position == 0 && actor != issuer)
            {
                return Err(invalid());
            }
            changes.push(Change {
                grant: history_grant,
                actor,
                recorded_at,
            });
        }
        if changes
            .last()
            .is_none_or(|change| change.grant != selected_grant)
        {
            return Err(invalid());
        }
        Ok(Some(Selected {
            record: StoredFilePublisherGrant {
                grant: selected_grant,
                issuer,
            },
            changes,
        }))
    }

    pub(super) async fn database_now(&self, tx: &mut Tx<'_>) -> Result<u64> {
        // Guard before casting to BIGINT, including unrepresentable or out-of-range clock values.
        let row = sqlx::query(
            "SELECT CASE WHEN now BETWEEN 0 AND 253402300799
            THEN now::bigint ELSE NULL END AS now
            FROM (SELECT floor(extract(epoch FROM pg_catalog.clock_timestamp())) AS now) observed",
        )
        .fetch_one(&mut **tx)
        .await?;
        let now: Option<i64> = row.try_get("now")?;
        let now = u64::try_from(now.ok_or_else(invalid)?).map_err(|_| invalid())?;
        if now > MAX_FILE_GRANT_UNIX_SECONDS {
            return Err(invalid());
        }
        Ok(now)
    }

    pub(super) async fn insert_change(&self, tx: &mut Tx<'_>, change: &Change) -> Result<()> {
        let grant = &change.grant;
        let raw = canonical(grant)?;
        confirmed_one(sqlx::query("INSERT INTO collaboration_file_publisher_grant_changes
            (grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(grant.id().as_uuid()).bind(u64::from(grant.revision()) as i64)
            .bind(change.actor.principal().principal_id.as_uuid()).bind(change.actor.account_id().as_uuid())
            .bind(grant.validity().not_before() as i64).bind(grant.validity().expires_at() as i64)
            .bind(change.recorded_at as i64).bind(raw).execute(&mut **tx).await?.rows_affected())
    }
}
