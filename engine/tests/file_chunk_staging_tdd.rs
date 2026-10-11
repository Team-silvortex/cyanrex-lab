#[cfg(unix)]
#[path = "file_chunk_staging/mod.rs"]
mod cases;

#[cfg(unix)]
mod baseline {
    use cyanrex_engine::{
        models::collaboration::*,
        services::file_transfer::{inspect_stream, staging::*},
    };
    use std::{fs, io::Cursor, os::unix::fs::DirBuilderExt, path::PathBuf};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("cyanrex-staging-{}", uuid::Uuid::new_v4()));
            fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: uuid::Uuid::from_u128(1).try_into().unwrap(),
            workspace_id: uuid::Uuid::from_u128(2).try_into().unwrap(),
        }
    }
    fn manifest() -> FileTransferManifest {
        inspect_stream(
            scope(),
            uuid::Uuid::from_u128(3).try_into().unwrap(),
            uuid::Uuid::from_u128(4).try_into().unwrap(),
            "inert.bin",
            &mut Cursor::new(b"inert bytes"),
        )
        .unwrap()
    }
    #[tokio::test]
    async fn stages_and_verifies_exact_chunk_in_private_root() {
        let fixture = Fixture::new();
        let store = ChunkStaging::open(scope(), &fixture.0, 1).unwrap();
        let manifest = manifest();
        store
            .stage_chunk(&manifest, 0, Cursor::new(b"inert bytes"))
            .await
            .unwrap();
        store.verify_staged_chunk(&manifest, 0).await.unwrap();
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
    #[tokio::test]
    async fn duplicate_is_conflict_without_overwriting_confirmed_bytes() {
        let fixture = Fixture::new();
        let store = ChunkStaging::open(scope(), &fixture.0, 1).unwrap();
        let manifest = manifest();
        store
            .stage_chunk(&manifest, 0, Cursor::new(b"inert bytes"))
            .await
            .unwrap();
        assert_eq!(
            store
                .stage_chunk(&manifest, 0, Cursor::new(b"other bytes"))
                .await,
            Err(ChunkStagingError::Conflict)
        );
        store.verify_staged_chunk(&manifest, 0).await.unwrap();
    }
}
