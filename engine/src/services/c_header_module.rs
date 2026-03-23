use std::{collections::HashSet, path::PathBuf};

use tokio::fs;
use tokio::process::Command;

use crate::models::c_headers::{
    HeaderModuleItem, HeaderModuleState, HeaderSelectionMetadata, SelectedHeaderMetadata,
};

#[derive(Debug, Clone)]
struct HeaderSource {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    source_url: &'static str,
    include_hint: &'static str,
    file_name: &'static str,
}

const HEADER_SOURCES: [HeaderSource; 5] = [
    HeaderSource {
        id: "bpf_helpers",
        name: "bpf_helpers.h",
        description: "Core helper macros (SEC, map definitions, helper wrappers).",
        source_url: "https://raw.githubusercontent.com/libbpf/libbpf/master/src/bpf_helpers.h",
        include_hint: "#include <bpf/bpf_helpers.h>",
        file_name: "bpf_helpers.h",
    },
    HeaderSource {
        id: "bpf_helper_defs",
        name: "bpf_helper_defs.h",
        description: "Helper function definitions for verifier-known helpers.",
        source_url: "https://raw.githubusercontent.com/libbpf/libbpf/master/src/bpf_helper_defs.h",
        include_hint: "#include <bpf/bpf_helper_defs.h>",
        file_name: "bpf_helper_defs.h",
    },
    HeaderSource {
        id: "bpf_endian",
        name: "bpf_endian.h",
        description: "Endian conversion helpers for eBPF programs.",
        source_url: "https://raw.githubusercontent.com/libbpf/libbpf/master/src/bpf_endian.h",
        include_hint: "#include <bpf/bpf_endian.h>",
        file_name: "bpf_endian.h",
    },
    HeaderSource {
        id: "bpf_tracing",
        name: "bpf_tracing.h",
        description: "Tracing utility macros for tracepoint/kprobe programs.",
        source_url: "https://raw.githubusercontent.com/libbpf/libbpf/master/src/bpf_tracing.h",
        include_hint: "#include <bpf/bpf_tracing.h>",
        file_name: "bpf_tracing.h",
    },
    HeaderSource {
        id: "linux_bpf_uapi",
        name: "linux/bpf.h",
        description: "Linux UAPI BPF definitions used by many eBPF program types.",
        source_url:
            "https://raw.githubusercontent.com/torvalds/linux/master/include/uapi/linux/bpf.h",
        include_hint: "#include <linux/bpf.h>",
        file_name: "linux_bpf.h",
    },
];

#[derive(Clone)]
pub struct CHeaderModule {
    data_dir: PathBuf,
}

impl Default for CHeaderModule {
    fn default() -> Self {
        let root = std::env::var("CYANREX_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"));

        Self {
            data_dir: root.join("c_headers"),
        }
    }
}

impl CHeaderModule {
    pub async fn list(&self) -> HeaderModuleState {
        let _ = self.ensure_dirs().await;
        let selected = self.read_selected().await.unwrap_or_default();

        let headers = HEADER_SOURCES
            .iter()
            .map(|source| {
                let local_path = self.header_path(source.file_name);
                HeaderModuleItem {
                    id: source.id.to_string(),
                    name: source.name.to_string(),
                    description: source.description.to_string(),
                    source_url: source.source_url.to_string(),
                    downloaded: local_path.exists(),
                    selected: selected.contains(source.id),
                    local_path: local_path.display().to_string(),
                }
            })
            .collect();

        HeaderModuleState { headers }
    }

    pub async fn download(&self, id: &str) -> Result<String, String> {
        self.ensure_dirs()
            .await
            .map_err(|err| format!("failed to prepare module dir: {err}"))?;

        let source = HEADER_SOURCES
            .iter()
            .find(|source| source.id == id)
            .ok_or_else(|| format!("unknown header id: {id}"))?;

        let output = Command::new("wget")
            .arg("-qO-")
            .arg(source.source_url)
            .output()
            .await
            .map_err(|err| format!("failed to execute wget: {err}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(format!("wget failed: {stderr}"));
        }

        let bytes = output.stdout;

        let path = self.header_path(source.file_name);
        fs::write(&path, bytes)
            .await
            .map_err(|err| format!("failed to write header file: {err}"))?;

        Ok(format!("downloaded {} to {}", source.name, path.display()))
    }

    pub async fn set_selected(&self, id: &str, selected: bool) -> Result<String, String> {
        self.ensure_dirs()
            .await
            .map_err(|err| format!("failed to prepare module dir: {err}"))?;

        if !HEADER_SOURCES.iter().any(|source| source.id == id) {
            return Err(format!("unknown header id: {id}"));
        }

        let mut current = self.read_selected().await.unwrap_or_default();
        if selected {
            current.insert(id.to_string());
        } else {
            current.remove(id);
        }

        self.write_selected(&current)
            .await
            .map_err(|err| format!("failed to persist selection: {err}"))?;

        Ok(format!("selection updated for {id}"))
    }

    pub async fn selected_metadata(&self) -> HeaderSelectionMetadata {
        let selected = self.read_selected().await.unwrap_or_default();

        let selected_headers = HEADER_SOURCES
            .iter()
            .filter(|source| selected.contains(source.id))
            .map(|source| SelectedHeaderMetadata {
                id: source.id.to_string(),
                include_hint: source.include_hint.to_string(),
                local_path: self.header_path(source.file_name).display().to_string(),
            })
            .collect();

        HeaderSelectionMetadata { selected_headers }
    }

    pub async fn delete(&self, id: &str) -> Result<String, String> {
        self.ensure_dirs()
            .await
            .map_err(|err| format!("failed to prepare module dir: {err}"))?;

        let source = HEADER_SOURCES
            .iter()
            .find(|source| source.id == id)
            .ok_or_else(|| format!("unknown header id: {id}"))?;

        let path = self.header_path(source.file_name);
        if path.exists() {
            fs::remove_file(&path)
                .await
                .map_err(|err| format!("failed to delete header file: {err}"))?;
        }

        let mut current = self.read_selected().await.unwrap_or_default();
        current.remove(id);
        self.write_selected(&current)
            .await
            .map_err(|err| format!("failed to persist selection: {err}"))?;

        Ok(format!("deleted local header {}", source.name))
    }

    async fn ensure_dirs(&self) -> Result<(), std::io::Error> {
        fs::create_dir_all(&self.data_dir).await
    }

    fn header_path(&self, file_name: &str) -> PathBuf {
        self.data_dir.join(file_name)
    }

    fn selection_path(&self) -> PathBuf {
        self.data_dir.join("selected.json")
    }

    async fn read_selected(&self) -> Result<HashSet<String>, String> {
        let path = self.selection_path();
        if !path.exists() {
            return Ok(HashSet::new());
        }

        let content = fs::read_to_string(&path)
            .await
            .map_err(|err| format!("failed to read selection file: {err}"))?;

        let ids = serde_json::from_str::<Vec<String>>(&content)
            .map_err(|err| format!("failed to parse selection file: {err}"))?;

        Ok(ids.into_iter().collect())
    }

    async fn write_selected(&self, ids: &HashSet<String>) -> Result<(), String> {
        let path = self.selection_path();
        let mut list: Vec<String> = ids.iter().cloned().collect();
        list.sort();

        let content = serde_json::to_string_pretty(&list)
            .map_err(|err| format!("failed to encode selection: {err}"))?;

        fs::write(path, content)
            .await
            .map_err(|err| format!("failed to write selection file: {err}"))
    }
}
