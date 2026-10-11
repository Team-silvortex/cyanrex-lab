use super::*;
use records::ChunkRow;

impl FileUploadChunkJournal {
    /// Exact trusted owner/account filtering only; neither state is an original receipt or retry grant.
    pub async fn get(
        &self,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
        index: u32,
    ) -> Result<Option<UploadChunkRecord>> {
        self.check_input(owner, index)?;
        bounded(async {
            let mut tx = self.begin(true).await?;
            let pin = self.check_schema(&mut tx, false).await?;
            let row = self
                .read_row(&mut tx, upload_id, index, Some((owner, account_id)))
                .await?;
            pin.verify(self, &mut tx, false).await?;
            tx.commit().await?;
            Ok(row.map(|row| row.coordinate.record))
        })
        .await
    }

    /// Original journal identity only. No acknowledgement or token exists until COMMIT succeeds.
    pub(crate) async fn reserve_unknown(
        &self,
        pin: &JournalSchemaPin,
        coordinate: &ChunkCoordinate,
    ) -> Result<UnknownChunk> {
        self.check_input(coordinate.record.owner, coordinate.record.index)?;
        if coordinate.scope != self.scope {
            return Err(FileUploadChunkJournalError::ScopeMismatch);
        }
        if coordinate.record.state != PersistentChunkState::Unknown {
            return Err(FileUploadChunkJournalError::InvalidInput);
        }
        bounded(async {
            let mut tx = self.begin(false).await?;
            pin.verify(self, &mut tx, true).await?;
            let mut expected = self.ledger(&mut tx).await?;
            // Corruption fails closed before admission; every valid occupied coordinate conflicts,
            // irrespective of its old source/root binding or status, and before the global capacity.
            if expected.contains(coordinate) || !expected.compatible(coordinate) { return Err(FileUploadChunkJournalError::Conflict); }
            if expected.len() >= self.max_entries as usize { return Err(FileUploadChunkJournalError::CapacityExhausted); }
            let row = ChunkRow { coordinate: coordinate.clone(), nonce: Uuid::new_v4() };
            expected.add(&row)?;
            confirmed_one(sqlx::query("INSERT INTO collaboration_file_upload_chunks
                (upload_id,chunk_index,owner_id,account_id,source_key,root_key,total_length,chunk_length,sha256,nonce,confirmed)
                VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,FALSE)")
                .bind(row.coordinate.record.upload_id.as_uuid()).bind(row.coordinate.record.index as i32)
                .bind(row.coordinate.record.owner.principal_id.as_uuid()).bind(row.coordinate.record.account_id.as_uuid())
                .bind(row.coordinate.source_key.as_slice()).bind(row.coordinate.root_key.as_slice())
                .bind(row.coordinate.total_length as i64).bind(row.coordinate.record.byte_length as i64)
                .bind(row.coordinate.record.sha256.as_str()).bind(row.nonce).execute(&mut *tx).await?.rows_affected())?;
            self.flush_and_verify(&mut tx, pin, &row, &expected).await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            tx.commit().await?;
            Ok(UnknownChunk { schema: pin.retain(), expected: row })
        }).await
    }

    /// Private original acknowledgement only. A matching lookup or reported state cannot supply it.
    /// A repeated confirmation conflicts; errors never reset, release or repair the retained row.
    pub(crate) async fn confirm(&self, unknown: &UnknownChunk) -> Result<()> {
        let before = &unknown.expected;
        self.check_input(
            before.coordinate.record.owner,
            before.coordinate.record.index,
        )?;
        if before.coordinate.scope != self.scope {
            return Err(FileUploadChunkJournalError::ScopeMismatch);
        }
        bounded(async {
            let mut tx = self.begin(false).await?;
            unknown.schema.verify(self, &mut tx, true).await?;
            let mut expected = self.ledger(&mut tx).await?;
            let row = self
                .read_row(
                    &mut tx,
                    before.coordinate.record.upload_id,
                    before.coordinate.record.index,
                    None,
                )
                .await?
                .ok_or(FileUploadChunkJournalError::InvalidRecord)?;
            let mut after = before.clone();
            after.coordinate.record.state = PersistentChunkState::Confirmed;
            if row == after {
                return Err(FileUploadChunkJournalError::Conflict);
            }
            if row != *before {
                return Err(FileUploadChunkJournalError::InvalidRecord);
            }
            expected.confirm(before, &after)?;
            confirmed_one(
                sqlx::query(
                    "UPDATE collaboration_file_upload_chunks SET confirmed=TRUE
                WHERE upload_id=$1 AND chunk_index=$2 AND nonce=$3 AND NOT confirmed",
                )
                .bind(before.coordinate.record.upload_id.as_uuid())
                .bind(before.coordinate.record.index as i32)
                .bind(before.nonce)
                .execute(&mut *tx)
                .await?
                .rows_affected(),
            )?;
            self.flush_and_verify(&mut tx, &unknown.schema, &after, &expected)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            Ok(())
        })
        .await
    }

    async fn flush_and_verify(
        &self,
        tx: &mut Tx<'_>,
        pin: &JournalSchemaPin,
        row: &ChunkRow,
        expected: &ledger::Ledger,
    ) -> Result<()> {
        // Flush ordinary deferred events before exact readback; arbitrary trigger code remains trusted.
        sqlx::query("SET LOCAL synchronous_commit = 'on'")
            .execute(&mut **tx)
            .await?;
        sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
            .execute(&mut **tx)
            .await?;
        pin.verify(self, tx, true).await?;
        let stored = self
            .read_row(
                tx,
                row.coordinate.record.upload_id,
                row.coordinate.record.index,
                None,
            )
            .await?
            .ok_or(FileUploadChunkJournalError::InvalidRecord)?;
        if stored != *row || self.ledger(tx).await? != *expected {
            return Err(FileUploadChunkJournalError::InvalidRecord);
        }
        pin.verify(self, tx, true).await
    }
}
