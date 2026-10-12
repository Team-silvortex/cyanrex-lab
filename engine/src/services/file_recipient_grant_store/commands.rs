use super::*;

impl FileRecipientGrantStore {
    /// Record supplied attribution for this exact file only. Issuer and file publisher may differ;
    /// no Published lookup, publisher permission, recipient authentication or download is performed.
    /// Every repeated ID conflicts, even when all supplied metadata matches an existing record.
    pub async fn issue(
        &self,
        grant_id: FileGrantId,
        issuer: FileAccountRef,
        file: FileVersionRef,
        publisher: FileAccountRef,
        recipient: FileAccountRef,
        validity: FileGrantValidity,
    ) -> Result<StoredFileRecipientGrant> {
        // Keep cheap scope rejection before acquiring an independent transaction.
        self.check_account(issuer)?;
        self.check_account(publisher)?;
        self.check_account(recipient)?;
        if file.workspace() != self.scope {
            return Err(FileRecipientGrantStoreError::ScopeMismatch);
        }
        bounded(async {
            let mut tx = self.begin(false).await?;
            let pending = self
                .issue_in_transaction(
                    &mut tx, grant_id, issuer, file, publisher, recipient, validity,
                )
                .await?;
            self.flush_and_verify_mutation(&mut tx, &pending).await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
    }

    /// Exactly one 1/Active -> 2/Revoked transition. The original issuer, exact file, publisher,
    /// recipient and validity remain unchanged. Actor is trusted attribution, not live authority.
    /// This legacy trusted API deliberately does not require actor == recorded publisher.
    pub async fn revoke(
        &self,
        grant_id: FileGrantId,
        actor: FileAccountRef,
        expected_revision: RevisionNumber,
    ) -> Result<StoredFileRecipientGrant> {
        self.check_account(actor)?;
        bounded(async {
            let mut tx = self.begin(false).await?;
            let pending = self
                .revoke_trusted_in_transaction(&mut tx, grant_id, actor, expected_revision)
                .await?;
            self.flush_and_verify_mutation(&mut tx, &pending).await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
    }

    /// Exact recipient/account metadata snapshot. Missing and foreign rows are hidden before
    /// decoding; this validates selected history, not foreign ledger health or current byte access.
    pub async fn get(
        &self,
        grant_id: FileGrantId,
        recipient: FileAccountRef,
    ) -> Result<Option<StoredFileRecipientGrant>> {
        self.check_account(recipient)?;
        bounded(async {
            let mut tx = self.begin(true).await?;
            let pending = self
                .get_pending(&mut tx, grant_id, recipient, false)
                .await?;
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
        pending: &PendingFileRecipientGrantMutation,
    ) -> Result<()> {
        // Trigger work can change sync policy. Restore before flushing and after every final
        // original-pin/record/complete-ledger check, so no receipt escapes before COMMIT ACK.
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
