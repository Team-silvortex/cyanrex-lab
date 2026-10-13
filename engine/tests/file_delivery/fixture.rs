// Reuse the real controlled endpoint, task ownership and cleanup without changing its scope.
#[allow(dead_code, unused_imports)]
#[path = "../file_manifest_client/fixture.rs"]
mod transport;
pub use crate::legacy_fixture::scope;
pub use crate::reply::{Kind, Piece, Reply};
pub use transport::{
    assert_request, checked, chunk_name, grant, observe_output, output_name, upload, Fixture,
    ObservedRequest, Selected, TOKEN,
};

use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
};

pub const CHOSEN_LEAF: &str = "chosen-by-host.bin";
pub fn selected_parent(f: &Fixture) -> PathBuf {
    let parent = f.root.parent().unwrap().join("explicit-selected-parent");
    fs::DirBuilder::new().mode(0o700).create(&parent).unwrap();
    parent
}
pub fn root_identity(path: &Path) -> (u64, u64, u32, u32) {
    let metadata = fs::metadata(path).unwrap();
    (
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.mode(),
    )
}
