use super::*;

impl FilePublisherGrantStore {
    /// Trusted attribution only. Every repeated ID conflicts, even for identical metadata;
    /// issuer metadata is not authentication and this does not enable file publication.
    pub async fn issue(
        &self,
        grant_id: FileGrantId,
        issuer: FileAccountRef,
        subject: FileAccountRef,
        validity: FileGrantValidity,
    ) -> Result<StoredFilePublisherGrant> {
        self.check_account(issuer)?;
        self.check_account(subject)?;
        bounded(async {
            let mut tx = self.begin(false).await?;
            let pending = self
                .issue_in_transaction(&mut tx, grant_id, issuer, subject, validity)
                .await?;
            self.flush_and_verify_mutation(&mut tx, &pending).await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
    }

    /// Exactly one 1/Active -> 2/Revoked transition; no reactivation, changed validity or replay.
    /// Actor metadata is trusted caller attribution, not an audited registry permission check.
    pub async fn revoke(
        &self,
        grant_id: FileGrantId,
        actor: FileAccountRef,
        expected_revision: RevisionNumber,
    ) -> Result<StoredFilePublisherGrant> {
        self.check_account(actor)?;
        bounded(async {
            let mut tx = self.begin(false).await?;
            let pending = self
                .revoke_in_transaction(&mut tx, grant_id, actor, expected_revision)
                .await?;
            self.flush_and_verify_mutation(&mut tx, &pending).await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
    }

    /// Exact subject/account metadata snapshot only. Missing and foreign rows are hidden; this
    /// validates the selected complete history, not all foreign ledger health or live authority.
    pub async fn get(
        &self,
        grant_id: FileGrantId,
        subject: FileAccountRef,
    ) -> Result<Option<StoredFilePublisherGrant>> {
        self.check_account(subject)?;
        bounded(async {
            let mut tx = self.begin(true).await?;
            let pending = self.get_pending(&mut tx, grant_id, subject, false).await?;
            self.verify_pending_in_transaction(&mut tx, &pending)
                .await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
    }

    async fn flush_and_verify_mutation(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFilePublisherGrantMutation,
    ) -> Result<()> {
        // Immediate triggers can switch this setting off. Restore it both before the deferred
        // flush and after all original-pin/complete expected-ledger postchecks, before COMMIT.
        sqlx::query("SET LOCAL synchronous_commit = 'on'")
            .execute(&mut **tx)
            .await?;
        sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
            .execute(&mut **tx)
            .await?;
        self.verify_mutation_in_transaction(tx, pending).await?;
        sqlx::query("SET LOCAL synchronous_commit = 'on'")
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}
