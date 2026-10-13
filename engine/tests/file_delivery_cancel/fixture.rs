// Read-only real endpoint/task-cleanup reuse; the existing legacy HTTP scope remains unchanged.
#[allow(dead_code, unused_imports)]
#[path = "../file_manifest_client/fixture.rs"]
mod transport;
pub use crate::legacy_fixture::scope;
pub use crate::reply::{Kind, Piece, Reply};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};
pub use transport::{
    assert_request, checked, chunk_name, grant, observe_output, output_name, upload, Fixture,
    ObservedRequest, Selected, TOKEN,
};

pub const CHOSEN_LEAF: &str = "host-controlled-delivery.bin";
pub fn selected_directory(f: &Fixture) -> PathBuf {
    let path = f
        .root
        .parent()
        .unwrap()
        .join("controlled-ordinary-selected-parent");
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
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
