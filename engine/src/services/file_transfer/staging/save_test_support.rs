pub(super) use super::super::assembly_test_support::{
    output, restored, runtime, staged, waker, Barrier, ReleaseOnDrop, Root, Stamp,
};
use super::*;
use crate::services::file_transfer::verify_file;
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) struct Fixture<'a> {
    pub(super) source: &'a Root,
    pub(super) destination: &'a Root,
    pub(super) staging: super::super::ChunkStaging,
    pub(super) manifest: FileTransferManifest,
    original_source: (Vec<(String, Stamp, Vec<u8>)>, (u64, u64, u32)),
    original_destination: (u64, u64, u32),
}
impl<'a> Fixture<'a> {
    pub(super) fn ready(
        runtime: &tokio::runtime::Runtime,
        source: &'a Root,
        destination: &'a Root,
        bytes: &[u8],
        workers: usize,
    ) -> (Self, AssembledFile) {
        let (staging, manifest) = staged(runtime, source, bytes);
        let staging = if workers == 1 {
            staging
        } else {
            super::super::ChunkStaging::open(manifest.workspace(), &source.0, workers).unwrap()
        };
        let assembled = runtime
            .block_on(staging.assemble_staged_file(&manifest))
            .unwrap();
        let original_source = (tree(&source.0), source.identity());
        let original_destination = destination.identity();
        (
            Self {
                source,
                destination,
                staging,
                manifest,
                original_source,
                original_destination,
            },
            assembled,
        )
    }
    pub(super) fn target(&self, leaf: &str) -> FileSaveTarget {
        FileSaveTarget::open(&self.destination.0, leaf).unwrap()
    }
    pub(super) fn path(&self, leaf: &str) -> PathBuf {
        self.destination.0.join(leaf)
    }
    pub(super) fn unchanged_source(&self) {
        assert_eq!(
            (tree(&self.source.0), self.source.identity()),
            self.original_source
        );
        assert_eq!(self.destination.identity(), self.original_destination);
    }
    pub(super) fn saved(&self, leaf: &str, bytes: &[u8], inode: Option<u64>) {
        let path = self.path(leaf);
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file());
        if let Some(inode) = inode {
            assert_eq!(metadata.ino(), inode);
        }
        assert_eq!(metadata.mode() & 0o7777, 0o400);
        assert_eq!(
            metadata.uid(),
            fs::metadata(&self.destination.0).unwrap().uid()
        );
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.len(), bytes.len() as u64);
        let original = fs::metadata(output(&self.source, &self.manifest)).unwrap();
        assert_ne!(
            (metadata.dev(), metadata.ino()),
            (original.dev(), original.ino())
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        verify_file(&self.manifest, &mut fs::File::open(&path).unwrap()).unwrap();
        self.unchanged_source();
    }
}

pub(super) fn tree(path: &Path) -> Vec<(String, Stamp, Vec<u8>)> {
    let mut entries = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                Stamp::of(&entry.metadata().unwrap()),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

pub(super) fn proof_gone(
    runtime: &tokio::runtime::Runtime,
    proof: &std::sync::Weak<super::super::assembled_proof::AssembledProof>,
) {
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            while proof.strong_count() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the actual finished job must release its final original proof reference");
    });
}
