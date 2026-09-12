use super::*;
use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
};

pub(super) enum Mutation {
    Read,
    Save(UserScript),
    Delete(String),
}

impl ScriptStore {
    pub(super) async fn read_local_scripts(
        &self,
        username: &str,
    ) -> Result<Vec<UserScript>, String> {
        if let Some(records) = self.in_memory.read().await.get(username) {
            return Ok(records.clone());
        }
        self.commit_local_scripts(username, Mutation::Read).await
    }

    pub(super) async fn commit_local_scripts(
        &self,
        username: &str,
        mutation: Mutation,
    ) -> Result<Vec<UserScript>, String> {
        let guard = self.local_writer.clone().lock_owned().await;
        let store = self.clone();
        let username = username.to_string();
        // Admission, load, write, rename and publication remain one operation even if the caller
        // is cancelled. A cancelled queued caller never reaches this worker.
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            let result = store.apply_local(&username, mutation);
            if let Err(error) = &result {
                tracing::warn!("local script operation failed: {error}");
            }
            result
        })
        .await
        .map_err(|error| format!("script storage worker failed: {error}"))?
    }

    fn apply_local(&self, username: &str, mutation: Mutation) -> Result<Vec<UserScript>, String> {
        let path = self.data_dir.join(format!("{username}.json"));
        let cached = self.in_memory.blocking_read().get(username).cloned();
        let mut records = match cached {
            Some(records) => records,
            None => match fs::read(&path) {
                Ok(bytes) => serde_json::from_slice::<Vec<UserScript>>(&bytes)
                    .map_err(|error| format!("failed to parse persisted scripts: {error}"))?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(error) => return Err(format!("failed to read persisted scripts: {error}")),
            },
        };
        if records.iter().any(|record| record.username != username) {
            return Err("persisted script owner does not match its file".to_string());
        }
        let write = match mutation {
            Mutation::Read => false,
            Mutation::Save(record) => {
                records.insert(0, record);
                true
            }
            Mutation::Delete(id) => {
                records.retain(|record| record.id != id);
                true
            }
        };
        if write {
            persist_snapshot(&path, &records)?;
        }
        self.in_memory
            .blocking_write()
            .insert(username.to_string(), records.clone());
        Ok(records)
    }
}

fn persist_snapshot(path: &Path, records: &[UserScript]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "invalid script path".to_string())?;
    let mut directory = fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory
        .create(parent)
        .map_err(|error| format!("failed to prepare script data dir: {error}"))?;
    let temp_path = parent.join(format!(".script-{}.tmp", Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(&temp_path)
        .map_err(|error| format!("failed to create script snapshot: {error}"))?;
    let _cleanup = TemporarySnapshot(temp_path.clone());
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, records)
        .map_err(|error| format!("failed to encode/write scripts: {error}"))?;
    writer
        .flush()
        .map_err(|error| format!("failed to flush scripts: {error}"))?;
    drop(writer);
    fs::rename(&temp_path, path).map_err(|error| format!("failed to finalize scripts: {error}"))
}

struct TemporarySnapshot(PathBuf);
impl Drop for TemporarySnapshot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.0) {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("script temporary snapshot cleanup failed: {error}");
            }
        }
    }
}
