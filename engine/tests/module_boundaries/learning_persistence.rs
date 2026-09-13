//! F28/F31 → D03: private snapshots, exclusive temporary files and failed-commit cleanup.
use super::support::*;
use cyanrex_engine::services::learning_store::{LearningRunOutcome, LearningStore};

fn outcome() -> LearningRunOutcome<'static> {
    LearningRunOutcome {
        lab_id: "01-first-program",
        template_id: Some("xdp-pass"),
        source: SOURCE,
        run_success: false,
        stage: "compile",
        attach_expected: false,
        attach_verified: false,
    }
}

#[cfg(unix)]
#[tokio::test]
async fn new_learning_files_and_directories_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let root_mode = std::fs::metadata(&f.root).unwrap().permissions().mode();
    let private = f.root.join("private");
    let classroom = private.join("classroom");
    let path = classroom.join("attempts.json");
    LearningStore::with_local_data_path(path.clone())
        .record_run("network-student", outcome())
        .await
        .unwrap();
    for entry in [&private, &classroom, &path] {
        let mode = std::fs::metadata(entry).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o077,
            0,
            "learning source/feedback must not be group/world readable: {entry:?}"
        );
    }
    assert_eq!(
        std::fs::metadata(&f.root).unwrap().permissions().mode(),
        root_mode,
        "do not chmod an existing parent"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn learning_commit_does_not_follow_a_preexisting_temporary_symlink() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let victim = f.root.join("outside-snapshot.txt");
    let original = b"do not overwrite this unrelated fixture file";
    std::fs::write(&victim, original).unwrap();
    let snapshot = f.root.join("learning.json");
    let legacy_temp = snapshot.with_extension("json.tmp");
    std::os::unix::fs::symlink(&victim, &legacy_temp).unwrap();
    f.state
        .learning_store
        .record_run("network-student", outcome())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(&victim).unwrap(),
        original,
        "snapshot temp creation must not truncate a symlink target"
    );
    assert!(
        std::fs::symlink_metadata(&legacy_temp)
            .unwrap()
            .file_type()
            .is_symlink(),
        "do not delete a preexisting path"
    );
    assert!(!std::fs::symlink_metadata(&snapshot)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[tokio::test]
async fn failed_learning_rename_removes_only_its_temporary_snapshot() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    f.state
        .learning_store
        .record_run("network-student", outcome())
        .await
        .unwrap();
    let snapshot = f.root.join("learning.json");
    let backup = f.root.join("preserved.json");
    std::fs::rename(&snapshot, &backup).unwrap();
    std::fs::create_dir(&snapshot).unwrap();
    let before: std::collections::BTreeSet<_> = std::fs::read_dir(&f.root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(f
        .state
        .learning_store
        .record_run("network-student", outcome())
        .await
        .is_err());
    let after = std::fs::read_dir(&f.root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        before, after,
        "failed rename must not leave private source in a temporary file"
    );
    assert_eq!(
        f.state
            .learning_store
            .attempts_for_user("network-student")
            .await
            .unwrap()
            .len(),
        1
    );
    std::fs::remove_dir(&snapshot).unwrap();
    std::fs::rename(&backup, &snapshot).unwrap();
    f.state
        .learning_store
        .record_run("network-student", outcome())
        .await
        .unwrap();
    assert_eq!(
        f.state
            .learning_store
            .attempts_for_user("network-student")
            .await
            .unwrap()
            .len(),
        2
    );
}
