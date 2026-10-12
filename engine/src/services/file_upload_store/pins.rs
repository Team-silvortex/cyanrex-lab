//! Short-lived private source observations, not authentication or persistent chunk progress.
use super::{schema::SchemaPin, *};
use sha2::{Digest, Sha256};

pub(crate) struct UploadReservationPin {
    reservation: FileUploadReservation,
    schema: SchemaPin,
    source_key: [u8; 32],
}

impl UploadReservationPin {
    pub(crate) fn reservation(&self) -> &FileUploadReservation {
        &self.reservation
    }

    pub(crate) fn source_key(&self) -> [u8; 32] {
        self.source_key
    }
}

impl FileUploadStore {
    /// Captures one exact current-account record and its original schema identity.
    /// This remains a read-only snapshot, not a promise that the record cannot later change.
    pub(crate) async fn capture_pin(
        &self,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> Result<Option<UploadReservationPin>> {
        self.check_owner(owner)?;
        bounded(async {
            let mut tx = self.begin(true).await?;
            let schema = self.check_schema(&mut tx, false).await?;
            let record = self
                .read_record(&mut tx, upload_id, Some((owner, account_id)))
                .await?;
            schema.verify(self, &mut tx, false).await?;
            let pin = record.map(|(reservation, record_digest)| UploadReservationPin {
                source_key: self.source_key(&schema, record_digest),
                reservation,
                schema,
            });
            tx.commit().await?;
            Ok(pin)
        })
        .await
    }

    /// Rechecks the original schema and complete exact record using fresh statement snapshots.
    /// No SQL lock spans filesystem work; this is not protection against later database changes.
    pub(crate) async fn verify_pin(&self, pin: &UploadReservationPin) -> Result<()> {
        self.check_owner(pin.reservation.owner())?;
        bounded(async {
            let mut tx = self.begin(true).await?;
            // The shared reader starts RR READ ONLY. Keep read-only policy, but avoid an old
            // namespace snapshot concealing record changes committed while a table lock waits.
            sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .execute(&mut *tx)
                .await?;
            self.verify_pinned_record(&mut tx, pin, false).await?;
            tx.commit().await?;
            Ok(())
        })
        .await
    }

    /// Borrow the caller's fresh source transaction while the original upload namespace is
    /// selected. The table fence survives subsequent authorization waits until caller COMMIT.
    /// This performs no pool acquisition, settings, deferred flush, COMMIT or filesystem work.
    pub(crate) async fn verify_pin_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pin: &UploadReservationPin,
    ) -> Result<()> {
        self.check_owner(pin.reservation.owner())?;
        pin.schema.verify_selected(self, tx, false).await?;
        sqlx::query(
            "LOCK TABLE collaboration_file_upload_schema, collaboration_file_uploads IN SHARE MODE",
        )
        .execute(&mut **tx)
        .await?;
        // The lock may wait; compare the ORIGINAL pin again using the caller's RC snapshot.
        self.verify_pinned_record(tx, pin, true).await
    }

    async fn verify_pinned_record(
        &self,
        tx: &mut Tx<'_>,
        pin: &UploadReservationPin,
        selected: bool,
    ) -> Result<()> {
        self.verify_pinned_schema(tx, pin, selected).await?;
        let stored = self
            .read_record(
                tx,
                pin.reservation.upload_id(),
                Some((pin.reservation.owner(), pin.reservation.account_id())),
            )
            .await?
            .ok_or(FileUploadStoreError::InvalidRecord)?
            .0;
        if stored != pin.reservation {
            return Err(FileUploadStoreError::InvalidRecord);
        }
        self.verify_pinned_schema(tx, pin, selected).await?;
        Ok(())
    }

    async fn verify_pinned_schema(
        &self,
        tx: &mut Tx<'_>,
        pin: &UploadReservationPin,
        selected: bool,
    ) -> Result<()> {
        if selected {
            pin.schema.verify_selected(self, tx, false).await
        } else {
            pin.schema.verify(self, tx, false).await
        }
    }

    fn source_key(&self, schema: &SchemaPin, record_digest: [u8; 32]) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"cyanrex.file-upload.staging-source.v1\0");
        hash.update((self.namespace.len() as u32).to_be_bytes());
        hash.update(self.namespace.as_bytes());
        hash.update(self.installation_id.as_bytes());
        let (namespace, relations) = schema.source_identity();
        hash.update(namespace.to_be_bytes());
        for oid in relations {
            hash.update(oid.to_be_bytes());
        }
        hash.update(record_digest);
        hash.finalize().into()
    }
}
