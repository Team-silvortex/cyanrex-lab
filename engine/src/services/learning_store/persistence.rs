use super::{Arc, AttemptSnapshot, LabAttempt, LearningStore};
use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
};
use tokio::sync::OwnedMutexGuard;

#[cfg(test)]
#[path = "persistence_tests.rs"]
mod tests;

impl LearningStore {
    pub(super) async fn persist_and_publish(
        &self,
        attempts: AttemptSnapshot,
        guard: OwnedMutexGuard<()>,
    ) -> Result<(), String> {
        let path = self.data_path.clone();
        let memory = self.in_memory.clone();
        // Acquire admission in the caller, before spawning: only one commit per store is queued/running.
        // Dropping the request's JoinHandle detaches this work; it must retain the lock through publication.
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            let result = persist_attempts(&path, &attempts);
            if let Err(error) = &result {
                // A cancelled caller can no longer observe the returned storage error.
                tracing::warn!("local learning commit failed: {error}");
            }
            result?;
            let previous = {
                let mut current = memory.blocking_write();
                std::mem::replace(&mut *current, attempts)
            };
            // Old readers can retain a snapshot; final O(n) pointer drops stay outside the memory lock.
            drop(previous);
            Ok(())
        })
        .await
        .map_err(|error| format!("learning persistence worker failed: {error}"))?
    }
}

fn persist_attempts(path: &Path, attempts: &[Arc<LabAttempt>]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "invalid learning data path".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to prepare learning data dir: {error}"))?;
    let temp_path = path.with_extension("json.tmp");
    let file = fs::File::create(&temp_path)
        .map_err(|error| format!("failed to persist learning attempts: {error}"))?;
    write_pretty(file, &SerializableAttempts(attempts))?;
    fs::rename(&temp_path, path)
        .map_err(|error| format!("failed to finalize learning attempts: {error}"))
}

fn write_pretty(writer: impl Write, value: &impl serde::Serialize) -> Result<(), String> {
    let mut writer = BufWriter::with_capacity(64 * 1024, writer);
    serde_json::to_writer_pretty(&mut writer, value)
        .map_err(|error| format!("failed to encode/write learning attempts: {error}"))?;
    // BufWriter's Drop ignores I/O errors. Never rename unless this explicit flush succeeded.
    writer
        .flush()
        .map_err(|error| format!("failed to flush learning attempts: {error}"))
}

struct SerializableAttempts<'a>(&'a [Arc<LabAttempt>]);

impl serde::Serialize for SerializableAttempts<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Keep the existing plain JSON array without requiring serde's Arc wire support.
        serializer.collect_seq(self.0.iter().map(Arc::as_ref))
    }
}
