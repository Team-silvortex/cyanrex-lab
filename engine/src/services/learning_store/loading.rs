use super::{Arc, AttemptSnapshot, LabAttempt, LearningStore};
use serde::{
    de::{SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{fmt, fs, io::ErrorKind, path::Path};

impl LearningStore {
    pub(super) async fn load_memory(&self) -> Result<(), String> {
        if self.memory_loaded.initialized() {
            return Ok(());
        }
        // Reuse write admission so cancelled callers cannot start overlapping initializations or writes.
        let guard = self.persist_lock.clone().lock_owned().await;
        if self.memory_loaded.initialized() {
            return Ok(());
        }
        let path = self.data_path.clone();
        let memory = self.in_memory.clone();
        let ready = self.memory_loaded.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            let loaded = read_snapshot(&path);
            if loaded.is_err() {
                // Parser errors may contain stored values. Log no source, usernames or malformed JSON.
                tracing::warn!("local learning initialization failed; no snapshot published");
            }
            let attempts = loaded?;
            let previous = {
                let mut current = memory.blocking_write();
                std::mem::replace(&mut *current, attempts)
            };
            // Only the admitted worker sets this cell, after publishing the complete snapshot.
            ready.set(()).expect("load admission owns initialization");
            drop(previous);
            Ok(())
        })
        .await
        .map_err(|error| format!("learning initialization worker failed: {error}"))?
    }
}

fn read_snapshot(path: &Path) -> Result<AttemptSnapshot, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Arc::new(Vec::new())),
        Err(error) => return Err(format!("failed to read learning attempts: {error}")),
    };
    // Keep whole-file UTF-8 validation (including unknown fields), without a Vec<LabAttempt> copy.
    serde_json::from_str::<LoadedAttempts>(&content)
        .map(|loaded| loaded.0)
        .map_err(|error| format!("failed to parse learning attempts: {error}"))
}

struct LoadedAttempts(AttemptSnapshot);

impl<'de> Deserialize<'de> for LoadedAttempts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SnapshotVisitor;
        impl<'de> Visitor<'de> for SnapshotVisitor {
            type Value = LoadedAttempts;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an array of learning attempts")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
                let mut rows = Vec::new();
                while let Some(row) = input.next_element::<LabAttempt>()? {
                    rows.push(Arc::new(row));
                }
                Ok(LoadedAttempts(Arc::new(rows)))
            }
        }
        deserializer.deserialize_seq(SnapshotVisitor)
    }
}
