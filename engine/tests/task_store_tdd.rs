use cyanrex_engine::{
    domain_packs::ebpf_teaching::EbpfTeachingPack,
    models::collaboration::*,
    services::{
        task_catalog::TaskCatalog,
        task_store::{TaskDraft, TaskStore, TaskStoreError},
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};

#[path = "task_store/faults.rs"]
mod faults;
#[path = "task_store/fixture.rs"]
mod fixture;
use fixture::*;

#[test]
fn task_draft_supports_manual_work_and_exact_catalog_snapshots() {
    assert!(TaskDraft::manual("Write a document", vec![]).is_ok());
    let catalog = TaskCatalog::new(EbpfTeachingPack).unwrap();
    let reference = EbpfTeachingPack::definition_ref("01-first-program").unwrap();
    assert!(TaskDraft::from_catalog(&catalog, &reference, "Experiment", vec![]).is_ok());
    let mut unknown = reference;
    unknown.version = 2.try_into().unwrap();
    assert!(matches!(
        TaskDraft::from_catalog(&catalog, &unknown, "Unknown", vec![]),
        Err(TaskStoreError::UnknownDefinition)
    ));
    for title in ["".into(), " \t".into(), "x".repeat(257), "a\nb".into()] {
        assert!(TaskDraft::manual(title, vec![]).is_err());
    }
    assert!(TaskDraft::manual("Too many inputs", vec![artifact(); 33]).is_err());
    assert!(TaskDraft::manual("Duplicate input", vec![artifact(), artifact()]).is_err());
}

#[test]
fn task_status_graph_has_no_automatic_or_self_approved_acceptance() {
    use TaskStatus::*;
    let allowed = [
        (Draft, Ready),
        (Draft, Cancelled),
        (Ready, InProgress),
        (Ready, Blocked),
        (Ready, Cancelled),
        (InProgress, Blocked),
        (InProgress, InReview),
        (InProgress, Cancelled),
        (Blocked, Ready),
        (Blocked, InProgress),
        (Blocked, Cancelled),
        (InReview, InProgress),
        (InReview, Cancelled),
    ];
    for from in [Draft, Ready, InProgress, Blocked, InReview, Cancelled] {
        for to in [Draft, Ready, InProgress, Blocked, InReview, Cancelled] {
            assert_eq!(
                from.can_transition_to(to),
                allowed.contains(&(from, to)),
                "{from:?} -> {to:?}"
            );
        }
    }
    for unsupported in ["accepted", "completed", "run_succeeded", "teacher_approved"] {
        assert!(serde_json::from_value::<TaskStatus>(serde_json::json!(unsupported)).is_err());
    }
}

#[tokio::test]
async fn closed_task_store_never_falls_back_or_installs_on_read() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let store = TaskStore::new(pool, scope());
    assert_eq!(
        store.install_empty_namespace().await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.get(reference(), owner()).await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.create(reference().task_id, owner(), manual()).await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::StorageUnavailable)
    );
}

#[test]
fn task_store_is_not_wired_to_live_state_or_domain_services() {
    let state = include_str!("../src/state.rs");
    assert!(!state.contains("TaskStore"));
    for source in [
        include_str!("../src/services/task_store/mod.rs"),
        include_str!("../src/services/task_store/commands.rs"),
        include_str!("../src/services/task_store/draft.rs"),
        include_str!("../src/services/task_store/records.rs"),
        include_str!("../src/services/task_store/schema.rs"),
        include_str!("../src/models/collaboration/work.rs"),
    ] {
        for forbidden in [
            "::domain_packs",
            "::learning",
            "ebpf",
            "AuthRole",
            "AuthService",
            "std::env",
        ] {
            assert!(!source.contains(forbidden));
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_keep_manual_and_catalog_work_across_reopen() {
    let f = Fixture::ready().await;
    let catalog = TaskCatalog::new(EbpfTeachingPack).unwrap();
    let pinned = catalog.definitions()[0].clone();
    let typed = TaskDraft::from_catalog(
        &catalog,
        &pinned.reference,
        "Pinned experiment",
        vec![artifact()],
    )
    .unwrap();
    let first = f
        .store
        .create(reference().task_id, owner(), typed)
        .await
        .unwrap();
    assert_eq!(first.definition, Some(pinned));
    assert_eq!(first.status, TaskStatus::Draft);
    assert_eq!(first.revision, rev(1));
    assert_eq!(first.input_refs, vec![artifact()]);
    let manual = f.store.create(id(), owner(), manual()).await.unwrap();
    assert!(manual.definition.is_none());
    let reopened = TaskStore::new(f.pool.clone(), scope());
    assert_eq!(
        reopened.get(reference(), owner()).await.unwrap(),
        Some(first)
    );
    assert_eq!(
        reopened.get(manual.reference, owner()).await.unwrap(),
        Some(manual)
    );
    assert_eq!(f.count("collaboration_task_outbox").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_scope_owner_and_input_boundaries_are_explicit() {
    let f = Fixture::ready().await;
    f.store
        .create(reference().task_id, owner(), manual())
        .await
        .unwrap();
    let other = PrincipalRef {
        principal_id: id(),
        ..owner()
    };
    assert_eq!(f.store.get(reference(), other).await.unwrap(), None);
    assert_eq!(
        f.store
            .transition(reference(), other, rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::NotFound)
    );
    let foreign = PrincipalRef {
        authority_id: id(),
        ..owner()
    };
    assert_eq!(
        f.store.create(id(), foreign, manual()).await,
        Err(TaskStoreError::ScopeMismatch)
    );
    let wrong_scope = WorkspaceRef {
        workspace_id: id(),
        ..scope()
    };
    let wrong = TaskStore::new(f.pool.clone(), wrong_scope);
    assert_eq!(
        wrong
            .get(
                TaskRef {
                    workspace: wrong_scope,
                    ..reference()
                },
                owner()
            )
            .await,
        Err(TaskStoreError::ScopeMismatch)
    );
    let mut foreign_input = artifact();
    foreign_input.workspace = wrong_scope;
    let input = TaskDraft::manual("Foreign input", vec![foreign_input]).unwrap();
    assert_eq!(
        f.store.create(id(), owner(), input).await,
        Err(TaskStoreError::ScopeMismatch)
    );
    assert_eq!(f.count("collaboration_tasks").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_transitions_pin_content_and_reject_stale_or_terminal_writes() {
    let f = Fixture::ready().await;
    let initial = f
        .store
        .create(reference().task_id, owner(), manual())
        .await
        .unwrap();
    let mut current = initial.clone();
    for status in [
        TaskStatus::Ready,
        TaskStatus::InProgress,
        TaskStatus::InReview,
        TaskStatus::InProgress,
        TaskStatus::Cancelled,
    ] {
        let next = f
            .store
            .transition(reference(), owner(), current.revision, status)
            .await
            .unwrap();
        assert_eq!(u64::from(next.revision), u64::from(current.revision) + 1);
        assert_eq!(next.definition, initial.definition);
        assert_eq!(next.input_refs, initial.input_refs);
        assert_eq!(next.created_at, initial.created_at);
        current = next;
    }
    assert_eq!(
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::StaleRevision)
    );
    assert_eq!(
        f.store
            .transition(reference(), owner(), current.revision, TaskStatus::Ready)
            .await,
        Err(TaskStoreError::InvalidTransition)
    );
    assert_eq!(f.count("collaboration_task_outbox").await, 6);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_concurrent_creation_and_revision_change_have_one_winner() {
    let f = Fixture::ready().await;
    let (one, two) = tokio::join!(
        f.store.create(reference().task_id, owner(), manual()),
        f.store.create(reference().task_id, owner(), manual())
    );
    assert!(matches!(
        (one, two),
        (Ok(_), Err(TaskStoreError::Conflict)) | (Err(TaskStoreError::Conflict), Ok(_))
    ));
    let (one, two) = tokio::join!(
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready),
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Cancelled)
    );
    assert!(matches!(
        (one, two),
        (Ok(_), Err(TaskStoreError::StaleRevision)) | (Err(TaskStoreError::StaleRevision), Ok(_))
    ));
    assert_eq!(f.count("collaboration_tasks").await, 1);
    assert_eq!(f.count("collaboration_task_outbox").await, 2);
    f.cleanup().await;
}
