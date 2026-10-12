//! Caller-owned phases keep control admission across authority waits without SQL over file I/O.
use super::*;
use crate::{
    services::{
        file_transfer::staging::BoundFileObservation,
        file_upload_publication::{
            PendingFileUploadPublicationMutation, PublicationAcknowledgement,
        },
    },
    sqlx_compat::Postgres,
};
use sqlx_core::transaction::Transaction;

pub(crate) struct PublicationRun<'a> {
    attempt: &'a mut FileUploadPublicationAttempt,
    _permit: OwnedSemaphorePermit,
}

impl PublicationRun<'_> {
    pub(crate) fn owner(&self) -> PrincipalRef {
        self.attempt.owner()
    }
    pub(crate) fn account_id(&self) -> LegacyAccountId {
        self.attempt.account_id()
    }
}

pub(crate) struct ObservedPublication<'a> {
    run: PublicationRun<'a>,
    proof: BoundFileObservation,
}

impl ObservedPublication<'_> {
    pub(crate) fn owner(&self) -> PrincipalRef {
        self.run.owner()
    }
    pub(crate) fn account_id(&self) -> LegacyAccountId {
        self.run.account_id()
    }
    pub(crate) fn scope(&self) -> WorkspaceRef {
        self.run.attempt.manifest().workspace()
    }
    pub(crate) fn upload_namespace(&self) -> &str {
        self.run.attempt.issuer.local.inner.store.namespace()
    }
    pub(crate) fn journal_namespace(&self) -> &str {
        self.run.attempt.issuer.journal.namespace()
    }
    pub(crate) fn publication_namespace(&self) -> &str {
        self.run.attempt.issuer.publications.namespace()
    }
}

pub(crate) struct PendingPublication {
    issuer: Arc<PublisherInner>,
    pending: PendingFileUploadPublicationMutation,
}

impl FileUploadPublisher {
    pub(crate) fn scope(&self) -> WorkspaceRef {
        self.inner.local.inner.store.scope()
    }
    pub(crate) fn upload_namespace(&self) -> &str {
        self.inner.local.inner.store.namespace()
    }
    pub(crate) fn journal_namespace(&self) -> &str {
        self.inner.journal.namespace()
    }
    pub(crate) fn publication_namespace(&self) -> &str {
        self.inner.publications.namespace()
    }

    pub(crate) fn start<'a>(
        &self,
        attempt: &'a mut FileUploadPublicationAttempt,
    ) -> Result<PublicationRun<'a>, FileUploadPublicationError> {
        if !Arc::ptr_eq(&self.inner, &attempt.issuer)
            || attempt.state != FileUploadPublicationState::Ready
        {
            return Err(FileUploadPublicationError::InvalidInput);
        }
        let permit = self
            .inner
            .local
            .admit()
            .map_err(|_| FileUploadPublicationError::CapacityExhausted)?;
        attempt.state = FileUploadPublicationState::Unknown;
        Ok(PublicationRun {
            attempt,
            _permit: permit,
        })
    }

    pub(crate) async fn observe<'a>(
        &self,
        run: PublicationRun<'a>,
    ) -> Result<ObservedPublication<'a>, FileUploadPublicationError> {
        self.check_run(&run)?;
        let attempt = &run.attempt;
        self.inner
            .local
            .inner
            .store
            .verify_pin(&attempt.source)
            .await
            .map_err(FileUploadPublicationError::Store)?;
        self.inner
            .journal
            .verify_confirmed_file(&attempt.source, attempt.key, &attempt.journal)
            .await
            .map_err(FileUploadPublicationError::Journal)?;
        // The source caller releases all SQL before this fresh ordered chunk/whole hash scan.
        // A public metadata-only observation cannot replace this actual byte verification.
        let proof = self
            .inner
            .local
            .inner
            .staging
            .observe_bound_file(attempt.key, attempt.manifest())
            .await
            .map_err(initial_filesystem_error)?;
        self.inner
            .local
            .inner
            .store
            .verify_pin(&attempt.source)
            .await
            .map_err(|_| FileUploadPublicationError::Unconfirmed)?;
        self.inner
            .journal
            .verify_confirmed_file(&attempt.source, attempt.key, &attempt.journal)
            .await
            .map_err(|_| FileUploadPublicationError::Unconfirmed)?;
        let proof = self
            .inner
            .local
            .inner
            .staging
            .recheck_bound_file_retained(attempt.key, attempt.manifest(), proof)
            .await
            .map_err(|_| FileUploadPublicationError::Unconfirmed)?;
        Ok(ObservedPublication { run, proof })
    }

    pub(crate) async fn verify_source_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        observed: &ObservedPublication<'_>,
    ) -> Result<(), FileUploadPublicationError> {
        self.check_run(&observed.run)?;
        self.inner
            .local
            .inner
            .store
            .verify_pin_in_transaction(tx, &observed.run.attempt.source)
            .await
            .map_err(|_| FileUploadPublicationError::Unconfirmed)
    }

    pub(crate) async fn verify_journal_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        observed: &ObservedPublication<'_>,
    ) -> Result<(), FileUploadPublicationError> {
        self.check_run(&observed.run)?;
        let attempt = &observed.run.attempt;
        self.inner
            .journal
            .verify_confirmed_file_in_transaction(
                tx,
                &attempt.source,
                attempt.key,
                &attempt.journal,
            )
            .await
            .map_err(|_| FileUploadPublicationError::Unconfirmed)
    }

    pub(crate) async fn publish_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        observed: &ObservedPublication<'_>,
    ) -> Result<PendingPublication, FileUploadPublicationError> {
        self.check_run(&observed.run)?;
        let pending = self
            .inner
            .publications
            .publish_in_transaction(
                tx,
                &observed.run.attempt.publication,
                &observed.run.attempt.coordinate,
            )
            .await
            .map_err(publication_error)?;
        Ok(PendingPublication {
            issuer: Arc::clone(&self.inner),
            pending,
        })
    }

    pub(crate) async fn verify_publication_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        pending: &PendingPublication,
    ) -> Result<(), FileUploadPublicationError> {
        if !Arc::ptr_eq(&self.inner, &pending.issuer) {
            return Err(FileUploadPublicationError::InvalidInput);
        }
        self.inner
            .publications
            .verify_mutation_in_transaction(tx, &pending.pending)
            .await
            .map_err(publication_error)
    }

    pub(crate) async fn finish(
        &self,
        observed: ObservedPublication<'_>,
        pending: PendingPublication,
    ) -> Result<FileUploadPublicationReceipt, FileUploadPublicationError> {
        self.check_run(&observed.run)?;
        if !Arc::ptr_eq(&self.inner, &pending.issuer)
            || !pending.pending.is_for(&observed.run.attempt.coordinate)
        {
            return Err(FileUploadPublicationError::Unconfirmed);
        }
        // Call ONLY after the source COMMIT ACK. A pending insert is never that acknowledgement.
        self.finish_ack(observed, pending.pending.into_acknowledgement())
            .await
    }

    /// The pre-existing trusted API retains its own publication transaction and no Session gate.
    /// Reuse identical observation/finalization phases rather than introducing an alternate proof.
    pub(super) async fn publish_trusted(
        &self,
        run: PublicationRun<'_>,
    ) -> Result<FileUploadPublicationReceipt, FileUploadPublicationError> {
        let observed = self.observe(run).await?;
        let attempt = &observed.run.attempt;
        let acknowledgement = self
            .inner
            .publications
            .publish(&attempt.publication, &attempt.coordinate)
            .await
            .map_err(publication_error)?;
        self.finish_ack(observed, acknowledgement).await
    }

    pub(super) async fn finish_ack(
        &self,
        observed: ObservedPublication<'_>,
        acknowledgement: PublicationAcknowledgement,
    ) -> Result<FileUploadPublicationReceipt, FileUploadPublicationError> {
        self.check_run(&observed.run)?;
        let ObservedPublication { run, proof } = observed;
        let PublicationRun { attempt, _permit } = run;
        self.inner
            .local
            .inner
            .store
            .verify_pin(&attempt.source)
            .await
            .map_err(|_| FileUploadPublicationError::Unconfirmed)?;
        self.inner
            .journal
            .verify_confirmed_file(&attempt.source, attempt.key, &attempt.journal)
            .await
            .map_err(|_| FileUploadPublicationError::Unconfirmed)?;
        self.inner
            .publications
            .verify_ack(&acknowledgement)
            .await
            .map_err(|_| FileUploadPublicationError::Unconfirmed)?;
        self.inner
            .local
            .inner
            .staging
            .recheck_bound_file(attempt.key, attempt.manifest(), proof)
            .await
            .map_err(|_| FileUploadPublicationError::Unconfirmed)?;
        // No awaited operation follows this last check of the SAME original whole-file proof.
        let receipt = FileUploadPublicationReceipt {
            record: acknowledgement.record().clone(),
        };
        attempt.state = FileUploadPublicationState::Confirmed;
        Ok(receipt)
    }

    fn check_run(&self, run: &PublicationRun<'_>) -> Result<(), FileUploadPublicationError> {
        if !Arc::ptr_eq(&self.inner, &run.attempt.issuer)
            || run.attempt.state != FileUploadPublicationState::Unknown
        {
            return Err(FileUploadPublicationError::InvalidInput);
        }
        Ok(())
    }
}
