use super::{
    catalog_fixture::Fixture,
    marker_fixture::{prepare, snapshot},
    process::{rejected, Input},
    support::{lock_marker, no_connections, pending_marker_lock, release},
};
use futures_util::FutureExt;
use std::{
    fs::{self, Permissions},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    panic::AssertUnwindSafe,
    path::Path,
};

fn replace(root: &Path, saved: &Path) -> bool {
    if fs::rename(root, saved).is_err() {
        return false;
    }
    if fs::DirBuilder::new().mode(0o700).create(root).is_err() {
        return false;
    }
    if fs::set_permissions(root, Permissions::from_mode(0o700)).is_err() {
        return false;
    }
    fs::write(
        root.join("different-synthetic-root.bin"),
        b"replacement, not original root\n",
    )
    .is_ok()
}

fn restore(root: &Path, saved: &Path) -> bool {
    if !saved.exists() {
        return true;
    }
    // Both literal descendants belong exclusively to this random fixture. Never follow
    // a symlink or remove any external Artifact root or application directory.
    if root.exists() && fs::remove_dir_all(root).is_err() {
        return false;
    }
    fs::rename(saved, root).is_ok()
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_check_rechecks_original_artifact_root_after_real_marker_lock_wait(
) {
    let (run, pending, replaced, released, restored, unchanged, no_backend, input_unchanged) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                prepare(fixture).await;
                let input = Input::new(fixture);
                let before = snapshot(fixture).await;
                let saved = input.root.with_file_name("artifacts-original");
                let (holder, oid, holder_pid) = lock_marker(fixture).await;
                let running = input.spawn(None);
                let phase = AssertUnwindSafe(async {
                    let pending = pending_marker_lock(fixture, oid, holder_pid).await;
                    let replaced = pending && replace(&input.root, &saved);
                    (pending, replaced)
                })
                .catch_unwind()
                .await;
                // Always release/close the independent holder and reap the direct child
                // before a panic or final behavior assertion can escape this case.
                let released = release(holder).await;
                let run = running.finish(false).await;
                let restored = restore(&input.root, &saved);
                let no_backend = no_connections(fixture).await;
                let unchanged = before == snapshot(fixture).await;
                let input_unchanged = input.finish();
                match phase {
                    Ok((pending, replaced)) => (
                        run,
                        pending,
                        replaced,
                        released,
                        restored,
                        unchanged,
                        no_backend,
                        input_unchanged,
                    ),
                    Err(panic) => std::panic::resume_unwind(panic),
                }
            })
        })
        .await;
    rejected(&run);
    assert!(
        pending,
        "a real ungranted AccessShareLock must identify this child and holder"
    );
    assert!(replaced && released && restored && unchanged && no_backend && input_unchanged);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_check_killed_during_real_marker_wait_leaves_no_mutation_or_backend(
) {
    let (run, pending, released, unchanged, no_backend, input_unchanged) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                prepare(fixture).await;
                let input = Input::new(fixture);
                let before = snapshot(fixture).await;
                let (holder, oid, holder_pid) = lock_marker(fixture).await;
                let running = input.spawn(None);
                let phase = AssertUnwindSafe(pending_marker_lock(fixture, oid, holder_pid))
                    .catch_unwind()
                    .await;
                // Kill and wait for this owned direct child, then release the database holder.
                // Closing a client alone is not claimed to prove immediate remote cancellation.
                let run = running.finish(true).await;
                let released = release(holder).await;
                let no_backend = no_connections(fixture).await;
                let unchanged = before == snapshot(fixture).await;
                let input_unchanged = input.finish();
                match phase {
                    Ok(pending) => (
                        run,
                        pending,
                        released,
                        unchanged,
                        no_backend,
                        input_unchanged,
                    ),
                    Err(panic) => std::panic::resume_unwind(panic),
                }
            })
        })
        .await;
    assert!(
        pending,
        "cancellation must occur during an independently observed real lock wait"
    );
    assert!(!run.timed_out && !run.output.status.success());
    assert!(run.output.stdout.is_empty() && run.output.stderr.is_empty());
    assert!(released && unchanged && no_backend && input_unchanged);
}
