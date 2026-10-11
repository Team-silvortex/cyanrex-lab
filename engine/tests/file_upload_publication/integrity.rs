use super::{bytes::*, fixture::*, support::*};
use cyanrex_engine::{
    models::collaboration::*,
    services::file_upload_staging::{FileUploadPublicationError, FileUploadPublicationState},
};
use sqlx_core::query::query;
use std::{
    fs,
    fs::Permissions,
    os::unix::fs::{symlink, PermissionsExt},
};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_confirmed_chunks_still_require_current_bytes_and_whole_digest() {
    for drift in [
        "whole",
        "missing",
        "corrupt",
        "short",
        "long",
        "symlink",
        "hardlink",
        "root_mode",
        "empty_root",
    ] {
        let length = if drift == "empty_root" {
            0
        } else if drift == "whole" {
            FILE_TRANSFER_CHUNK_BYTES + 17
        } else {
            17
        };
        let mut f = Fixture::ready(length).await;
        if drift == "whole" {
            let mut wire = serde_json::to_value(&f.manifest).unwrap();
            wire["sha256"] = serde_json::json!("a".repeat(64));
            f.manifest =
                FileTransferManifest::parse_json(&serde_json::to_vec(&wire).unwrap()).unwrap();
            query(&format!("UPDATE {SOURCE_ROWS} SET manifest=$1"))
                .bind(serde_json::to_vec(&f.manifest).unwrap())
                .execute(&f.source_pool)
                .await
                .unwrap();
        }
        stage_all(&f).await;
        let mut attempt = f
            .publisher
            .prepare(upload(1), owner(1), account(1))
            .await
            .unwrap();
        let original = tree(&f.root);
        let saved = f.directory.join("original-outside-root");
        let path = original.first().map(|e| f.root.join(&e.name));
        match drift {
            "whole" => {}
            "root_mode" | "empty_root" => {
                fs::set_permissions(&f.root, Permissions::from_mode(0o770)).unwrap()
            }
            "hardlink" => fs::hard_link(path.as_ref().unwrap(), &saved).unwrap(),
            "missing" | "symlink" => {
                fs::rename(path.as_ref().unwrap(), &saved).unwrap();
                if drift == "symlink" {
                    symlink(&saved, path.as_ref().unwrap()).unwrap();
                }
            }
            "corrupt" | "short" | "long" => {
                let mut content = fs::read(path.as_ref().unwrap()).unwrap();
                if drift == "corrupt" {
                    content[0] ^= 1;
                } else if drift == "short" {
                    content.pop();
                } else {
                    content.push(0);
                }
                fs::set_permissions(path.as_ref().unwrap(), Permissions::from_mode(0o600)).unwrap();
                fs::write(path.as_ref().unwrap(), content).unwrap();
                fs::set_permissions(path.as_ref().unwrap(), Permissions::from_mode(0o400)).unwrap();
            }
            _ => unreachable!(),
        }
        let saved_before = saved.exists().then(|| snapshot(&saved));
        let before = snapshot_all(&f).await;
        assert_eq!(
            f.publisher.publish(&mut attempt).await.unwrap_err(),
            FileUploadPublicationError::Unavailable,
            "{drift}"
        );
        assert_eq!(attempt.state(), FileUploadPublicationState::Unknown);
        assert_eq!(snapshot_all(&f).await, before);
        if let Some(old) = saved_before {
            assert_eq!(snapshot(&saved), old);
        }
        assert!(f
            .publication
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap()
            .is_none());
        assert_eq!(snapshot_all(&f).await, before);
        if drift == "empty_root" {
            fs::set_permissions(&f.root, Permissions::from_mode(0o700)).unwrap();
            publish(&f, &f.publisher, 1, &f.manifest).await;
        }
        f.cleanup().await;
    }
    let f = Fixture::ready(17).await;
    stage_all(&f).await;
    publish(&f, &f.publisher, 1, &f.manifest).await;
    f.cleanup().await;
}
