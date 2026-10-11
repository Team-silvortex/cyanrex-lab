use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_http_invalid_new_text_and_corrupt_saved_bytes_cannot_be_hidden() {
    for missing in [false, true] {
        let (f, workspace, route, task, old, new) = pair().await;
        let path = f.blob(&old.reference);
        if missing {
            fs::remove_file(&path).unwrap();
        } else {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::write(&path, b"Corrupted text").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        }
        for req in saved_requests(&task, &f.auth.learner_token) {
            denied(
                &f,
                &route,
                req,
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
            )
            .await;
        }
        for next in [
            manifest("Labels only", vec![old.reference.clone()]),
            manifest("Remove last corrupt pin", vec![]),
            manifest("Replace corrupt pin", vec![new.reference.clone()]),
        ] {
            denied(
                &f,
                &route,
                request(
                    "PUT",
                    &format!("{}/content", task_path(task.task.reference.task_id)),
                    &f.auth.learner_token,
                    replace_body(&task, &next),
                ),
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
            )
            .await;
        }
        assert_eq!(
            fs::read(f.blob(&new.reference)).unwrap(),
            b"Replacement input"
        );
        f.assert_pool_restored().await;
        drop((route, workspace));
        f.cleanup().await;
    }
    let (f, workspace, route, task, _, _) = pair().await;
    for bytes in [
        vec![0xff],
        b"bad\0text".to_vec(),
        "bad\u{85}text".as_bytes().to_vec(),
        vec![b'x'; 256 * 1024 + 1],
    ] {
        let invalid = f.create_artifact(&f.auth.learner_token, &bytes).await;
        let value = manifest("Invalid text", vec![invalid.reference]);
        for req in mutations(&task, &f.auth.learner_token, &value, &values()[0])
            .into_iter()
            .take(2)
        {
            denied(&f, &route, req, StatusCode::BAD_REQUEST, "invalid_request").await;
        }
    }
    // A trusted raw store may pin bytes that the Session text boundary must still reject.
    let binary = f.create_artifact(&f.auth.learner_token, &[0xff]).await;
    let store =
        TaskContentStore::from_definitions(f.task_pool.clone(), scope(), &definitions(&values()));
    let binary_task = store
        .create_catalog(
            id(),
            f.auth.learner.principal.reference,
            &values()[1].reference,
            manifest("Unbound trusted binary", vec![binary.reference]),
        )
        .await
        .unwrap();
    for req in saved_requests(&binary_task, &f.auth.learner_token) {
        denied(
            &f,
            &route,
            req,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
    assert_eq!(
        store
            .get(binary_task.task.reference, binary_task.task.owner)
            .await
            .unwrap(),
        Some(binary_task)
    );
    read(&f, &route, &f.auth.learner_token, &task).await;
    f.assert_pool_restored().await;
    drop((store, route, workspace));
    f.cleanup().await;
}
