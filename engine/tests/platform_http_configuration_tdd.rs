#![cfg(unix)]
//! Public-constructor compatibility only: no requests, storage installation or open connection.

use cyanrex_engine::{
    models::collaboration::{CoreSchemaVersion, TaskDefinition, TaskDefinitionRef, WorkspaceRef},
    platform_http::{
        CatalogTaskContentHttpState, FileChunkReadHttpState, FileDistributionHttpConfigError,
        FileDistributionHttpState, FileDownloadManifestHttpState, SessionLifecycleHttpState,
        TaskContentHttpConfigError, TaskContentHttpState, TaskContentIndexHttpState,
        TextArtifactHttpState,
    },
    services::{
        auth_service::durable_source::{
            DurableAuthSource, SessionArtifactWorkspace, SessionCatalogTaskContentWorkspace,
            SessionFileChunkReadWorkspace, SessionFileDownloadWorkspace,
            SessionTaskContentIndexWorkspace, SessionTaskContentWorkspace,
        },
        file_transfer::staging::ChunkStaging,
        file_upload_publication::FileUploadPublicationStore,
        task_catalog::TaskDefinitionCatalog,
    },
};
use sqlx_core::connection::ConnectOptions;
use sqlx_postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgSslMode};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::PathBuf,
};
use uuid::Uuid;

const ORIGIN: &str = "https://platform.example";
const CAPACITIES: [usize; 6] = [0, 1, 4, 16, 17, usize::MAX];

struct Directories(PathBuf);
impl Drop for Directories {
    fn drop(&mut self) {
        for path in [
            self.0.join("artifacts"),
            self.0.join("chunks"),
            self.0.clone(),
        ] {
            if let Err(error) = fs::remove_dir(&path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("exclusive HTTP configuration fixture cleanup failed: {error}");
                }
            }
        }
    }
}

struct Fixture {
    source: DurableAuthSource,
    pool: PgPool,
    artifacts: SessionArtifactWorkspace,
    task: SessionTaskContentWorkspace,
    catalog: SessionCatalogTaskContentWorkspace,
    index: SessionTaskContentIndexWorkspace,
    downloads: SessionFileDownloadWorkspace,
    chunks: SessionFileChunkReadWorkspace,
    identities: Vec<(u64, u64, u32)>,
    directories: Directories,
}

impl Fixture {
    async fn new() -> Self {
        let pool = PgPoolOptions::new().connect_lazy_with(
            PgConnectOptions::new_without_pgpass()
                .host("127.0.0.1")
                .port(1)
                .username("synthetic_closed_http_configuration")
                .database("synthetic_unopened_http_configuration")
                .password("")
                .ssl_mode(PgSslMode::Disable)
                .disable_statement_logging(),
        );
        pool.close().await;
        assert!(pool.is_closed());
        let scope = WorkspaceRef {
            authority_id: Uuid::from_u128(4301).try_into().unwrap(),
            workspace_id: Uuid::from_u128(4302).try_into().unwrap(),
        };
        let source = DurableAuthSource::new(pool.clone(), scope.authority_id);
        let root = std::env::temp_dir().join(format!("cyanrex-http-config-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let directories = Directories(root.clone());
        for name in ["artifacts", "chunks"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(root.join(name))
                .unwrap();
        }
        let artifacts = source
            .open_artifact_workspace("http_config_artifacts", scope, &root.join("artifacts"))
            .unwrap();
        let task = SessionTaskContentWorkspace::new("http_config_tasks", scope, artifacts.clone())
            .unwrap();
        let package = "sample.document".parse().unwrap();
        let version = 1.try_into().unwrap();
        let definitions = TaskDefinitionCatalog::new(
            cyanrex_engine::models::collaboration::VersionedName {
                name: package,
                version,
            },
            vec![TaskDefinition {
                schema_version: CoreSchemaVersion,
                reference: TaskDefinitionRef {
                    package: cyanrex_engine::models::collaboration::VersionedName {
                        name: "sample.document".parse().unwrap(),
                        version: 1.try_into().unwrap(),
                    },
                    name: "sample.document.prepare".parse().unwrap(),
                    version: 1.try_into().unwrap(),
                },
                title: "Frozen configuration fixture".into(),
                summary: "Metadata only, with no provider or execution capability".into(),
                evidence_schema: None,
                assessment_policy: None,
            }],
        )
        .unwrap();
        let catalog = SessionCatalogTaskContentWorkspace::from_definitions(
            "http_config_tasks",
            scope,
            artifacts.clone(),
            &definitions,
        )
        .unwrap();
        let index = SessionTaskContentIndexWorkspace::from_definitions(
            "http_config_tasks",
            scope,
            &definitions,
        )
        .unwrap();
        let publication = FileUploadPublicationStore::new(
            pool.clone(),
            scope,
            Uuid::new_v4(),
            "http_config_publications",
            16,
        )
        .unwrap();
        let recipients = source
            .file_recipient_grant_workspace("http_config_recipients", scope, Uuid::new_v4(), 16)
            .unwrap();
        let downloads = source
            .file_download_workspace(publication, recipients)
            .unwrap();
        let chunks = source
            .file_chunk_read_workspace(
                downloads.clone(),
                ChunkStaging::open(scope, &root.join("chunks"), 1).unwrap(),
                1,
            )
            .unwrap();
        let identities = Self::identities(&directories);
        Self {
            source,
            pool,
            artifacts,
            task,
            catalog,
            index,
            downloads,
            chunks,
            identities,
            directories,
        }
    }

    fn identities(directories: &Directories) -> Vec<(u64, u64, u32)> {
        [
            directories.0.clone(),
            directories.0.join("artifacts"),
            directories.0.join("chunks"),
        ]
        .map(|path| {
            let metadata = fs::symlink_metadata(path).unwrap();
            assert!(metadata.is_dir());
            (metadata.dev(), metadata.ino(), metadata.mode())
        })
        .into()
    }

    fn assert_inert(&self) {
        assert!(self.pool.is_closed());
        assert_eq!(Self::identities(&self.directories), self.identities);
        assert_eq!(fs::read_dir(&self.directories.0).unwrap().count(), 2);
        for name in ["artifacts", "chunks"] {
            assert_eq!(
                fs::read_dir(self.directories.0.join(name)).unwrap().count(),
                0
            );
        }
    }
}

fn characterize(
    mut new: impl FnMut(&str) -> Result<(), TaskContentHttpConfigError>,
    mut bounded: impl FnMut(&str, usize) -> Result<(), TaskContentHttpConfigError>,
) {
    for origin in [
        ORIGIN,
        "https://platform.example:8443",
        "https://127.0.0.1",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
    ] {
        assert_eq!(new(origin), Ok(()), "new: {origin:?}");
        for capacity in CAPACITIES {
            let expected = if (1..=16).contains(&capacity) {
                Ok(())
            } else {
                Err(TaskContentHttpConfigError::InvalidCapacity)
            };
            assert_eq!(
                bounded(origin, capacity),
                expected,
                "{origin:?}, {capacity}"
            );
        }
    }
    for origin in [
        "",
        "null",
        "*",
        "https://*.example",
        "http://platform.example",
        "http://192.168.1.1",
        "https://PLATFORM.example",
        "https://platform.example/",
        "https://platform.example:443",
        "https://platform.example/path",
        "https://platform.example?q=a",
        "https://platform.example#x",
        "https://user@platform.example",
        " https://platform.example",
        "https://platform.example ",
    ] {
        assert_eq!(new(origin), Err(TaskContentHttpConfigError::InvalidOrigin));
        for capacity in CAPACITIES {
            assert_eq!(
                bounded(origin, capacity),
                Err(TaskContentHttpConfigError::InvalidOrigin),
                "origin must precede capacity: {origin:?}, {capacity}"
            );
        }
    }
}

macro_rules! workspace_test {
    ($test:ident, $state:ident, $workspace:ident) => {
        #[tokio::test]
        async fn $test() {
            let f = Fixture::new().await;
            characterize(
                |origin| $state::new(f.source.clone(), f.$workspace.clone(), origin).map(drop),
                |origin, capacity| {
                    $state::with_capacity(f.source.clone(), f.$workspace.clone(), origin, capacity)
                        .map(drop)
                },
            );
            f.assert_inert();
        }
    };
}

workspace_test!(task_http_configuration_matrix, TaskContentHttpState, task);
workspace_test!(
    catalog_http_configuration_matrix,
    CatalogTaskContentHttpState,
    catalog
);
workspace_test!(
    index_http_configuration_matrix,
    TaskContentIndexHttpState,
    index
);
workspace_test!(
    text_http_configuration_matrix,
    TextArtifactHttpState,
    artifacts
);
workspace_test!(
    manifest_http_configuration_matrix,
    FileDownloadManifestHttpState,
    downloads
);
workspace_test!(
    chunk_http_configuration_matrix,
    FileChunkReadHttpState,
    chunks
);

#[tokio::test]
async fn session_http_configuration_matrix() {
    let f = Fixture::new().await;
    characterize(
        |origin| SessionLifecycleHttpState::new(f.source.clone(), origin).map(drop),
        |origin, capacity| {
            SessionLifecycleHttpState::with_capacity(f.source.clone(), origin, capacity).map(drop)
        },
    );
    f.assert_inert();
}

#[tokio::test]
async fn distribution_http_origin_precedes_both_capacities_then_workspace_identity() {
    let f = Fixture::new().await;
    assert!(FileDistributionHttpState::new(f.source.clone(), f.chunks.clone(), ORIGIN).is_ok());
    let foreign = DurableAuthSource::new(f.pool.clone(), Uuid::from_u128(4399).try_into().unwrap());
    for manifests in CAPACITIES {
        for chunks in CAPACITIES {
            let valid = (1..=16).contains(&manifests) && (1..=16).contains(&chunks);
            assert_eq!(
                FileDistributionHttpState::with_capacities(
                    f.source.clone(),
                    f.chunks.clone(),
                    ORIGIN,
                    manifests,
                    chunks,
                )
                .map(drop),
                if valid {
                    Ok(())
                } else {
                    Err(FileDistributionHttpConfigError::InvalidCapacity)
                }
            );
            for source in [f.source.clone(), foreign.clone()] {
                assert_eq!(
                    FileDistributionHttpState::with_capacities(
                        source,
                        f.chunks.clone(),
                        "null",
                        manifests,
                        chunks,
                    )
                    .map(drop),
                    Err(FileDistributionHttpConfigError::InvalidOrigin)
                );
            }
            assert_eq!(
                FileDistributionHttpState::with_capacities(
                    foreign.clone(),
                    f.chunks.clone(),
                    ORIGIN,
                    manifests,
                    chunks,
                )
                .map(drop),
                Err(if valid {
                    FileDistributionHttpConfigError::InvalidWorkspace
                } else {
                    FileDistributionHttpConfigError::InvalidCapacity
                })
            );
        }
    }
    f.assert_inert();
}
