//! No database access: definitions are selected explicitly before storage dispatch.
use super::*;
use cyanrex_engine::services::task_catalog::TaskDefinitionCatalog;

fn versioned(name: &str) -> VersionedName {
    VersionedName {
        name: name.parse().unwrap(),
        version: rev(1),
    }
}

fn definition() -> TaskDefinition {
    TaskDefinition {
        schema_version: CoreSchemaVersion,
        reference: TaskDefinitionRef {
            package: versioned("example.documents"),
            name: "example.documents.summary".parse().unwrap(),
            version: rev(1),
        },
        title: "A document task".into(),
        summary: "Frozen metadata, no evaluator".into(),
        evidence_schema: Some(versioned("example.documents.text")),
        assessment_policy: Some(versioned("example.documents.words")),
    }
}

fn catalog() -> TaskDefinitionCatalog {
    let definition = definition();
    TaskDefinitionCatalog::new(definition.reference.package.clone(), vec![definition]).unwrap()
}

async fn closed_pool() -> PgPool {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    pool
}

#[tokio::test]
async fn catalog_content_unknown_exact_definitions_are_rejected_before_storage() {
    let store = TaskContentStore::from_definitions(closed_pool().await, scope(), &catalog());
    let original = definition().reference;
    let mut unknown = vec![original.clone(); 4];
    unknown[0].version = rev(2);
    unknown[1].package.version = rev(2);
    unknown[2].name = "example.documents.other".parse().unwrap();
    unknown[3].package.name = "another.documents".parse().unwrap();
    for reference in unknown {
        assert_eq!(
            store
                .create_catalog(id(), owner(), &reference, manifest())
                .await,
            Err(TaskStoreError::UnknownDefinition)
        );
    }
}

#[tokio::test]
async fn catalog_content_and_manual_creation_do_not_cross_the_explicit_handle_mode() {
    let pool = closed_pool().await;
    let manual = TaskContentStore::new(pool.clone(), scope());
    let catalog = TaskContentStore::from_definitions(pool, scope(), &catalog());
    assert_eq!(
        manual
            .create_catalog(id(), owner(), &definition().reference, manifest())
            .await,
        Err(TaskStoreError::InvalidInput)
    );
    assert_eq!(
        catalog.create(id(), owner(), manifest()).await,
        Err(TaskStoreError::InvalidInput)
    );
}

#[tokio::test]
async fn admitted_catalog_content_never_falls_back_when_storage_is_closed() {
    let metadata = catalog();
    let store = TaskContentStore::from_definitions(closed_pool().await, scope(), &metadata);
    let store = store.clone();
    drop(metadata);
    fn send_sync<T: Send + Sync>(_: &T) {}
    send_sync(&store);
    assert_eq!(
        store
            .create_catalog(id(), owner(), &definition().reference, manifest())
            .await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.get(reference(), owner()).await,
        Err(TaskStoreError::StorageUnavailable)
    );
}

#[tokio::test]
async fn catalog_content_scope_is_checked_without_a_database_or_owner_override() {
    let store = TaskContentStore::from_definitions(closed_pool().await, scope(), &catalog());
    let foreign = PrincipalRef {
        authority_id: id(),
        ..owner()
    };
    assert_eq!(
        store
            .create_catalog(id(), foreign, &definition().reference, manifest())
            .await,
        Err(TaskStoreError::ScopeMismatch)
    );
    let foreign_ref = ArtifactRef {
        workspace: WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        ..manifest().input_refs()[0].clone()
    };
    let foreign_manifest = TaskContentManifest::new(
        "Foreign",
        vec![TextPayloadBinding::new(foreign_ref, "notes", "future.language").unwrap()],
    )
    .unwrap();
    assert_eq!(
        store
            .create_catalog(id(), owner(), &definition().reference, foreign_manifest)
            .await,
        Err(TaskStoreError::ScopeMismatch)
    );
}
