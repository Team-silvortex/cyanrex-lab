// Only the pure fixture is nested; no previous tests or scope are changed or counted here.
#[allow(dead_code)]
#[path = "../file_save/fixture.rs"]
mod private;
pub use private::{checked, observe, scope, within, Fixture};

use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub const CHOSEN_LEAF: &str = "ordinary-host-chosen.bin";

pub fn chunk_name(manifest: &crate::FileTransferManifest, index: u32) -> String {
    format!(
        "v1-{:x}-{index}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    )
}

pub fn selected_directory(f: &Fixture) -> PathBuf {
    let selected = f.directory().join("ordinary-selected-parent");
    fs::DirBuilder::new().mode(0o700).create(&selected).unwrap();
    // Observe the exact selected policy rather than relying on the process's umask.
    fs::set_permissions(&selected, fs::Permissions::from_mode(0o755)).unwrap();
    selected
}

pub fn directory_policy(path: &Path) -> (u64, u64, u32, u32, u32) {
    let metadata = fs::metadata(path).unwrap();
    (
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.gid(),
        metadata.mode() & 0o7777,
    )
}
