use super::support::*;
use cyanrex_engine::services::{c_header_module::CHeaderModule, script_store::ScriptStore};

#[tokio::test]
async fn scripts_bind_http_identity_csrf_and_file_reload_without_cross_owner_deletion() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let alice = f.cookie("network-alice").await;
    let bob = f.cookie("network-bob").await;
    assert_eq!(
        f.request("GET", "/scripts", None, ORIGIN, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let payload = json!({"title":"saved source", "script":SOURCE, "username":"network-bob"});
    assert_eq!(
        f.request(
            "POST",
            "/scripts/save",
            Some(&alice),
            "https://untrusted.example",
            payload.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, saved) = f
        .request("POST", "/scripts/save", Some(&alice), ORIGIN, payload)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["record"]["username"], "network-alice");
    let record = saved["record"].clone();
    assert!(f.get("/scripts", &bob).await.as_array().unwrap().is_empty());
    assert_eq!(
        f.request(
            "POST",
            "/scripts/delete",
            Some(&bob),
            ORIGIN,
            json!({"id":record["id"]})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(f.get("/scripts", &alice).await[0], record);
    let reloaded = ScriptStore::default().list_for_user("network-alice").await;
    assert_eq!(reloaded.len(), 1);
    assert_eq!(reloaded[0].script, SOURCE);
    assert_eq!(
        f.request(
            "POST",
            "/scripts/delete",
            Some(&alice),
            ORIGIN,
            json!({"id":record["id"]})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(ScriptStore::default()
        .list_for_user("network-alice")
        .await
        .is_empty());
}

#[tokio::test]
async fn script_failed_file_save_must_not_publish_an_unsaved_record() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let cookie = f.cookie("network-alice").await;
    std::fs::create_dir_all(f.root.join("scripts/network-alice.json")).unwrap();
    let (status, _) = f
        .request(
            "POST",
            "/scripts/save",
            Some(&cookie),
            ORIGIN,
            json!({"title":"must fail", "script":SOURCE}),
        )
        .await;
    assert!(!status.is_success());
    assert!(
        f.get("/scripts", &cookie)
            .await
            .as_array()
            .unwrap()
            .is_empty(),
        "failed file write published an unsaved script in the API cache"
    );
}

#[tokio::test]
async fn script_failed_file_delete_must_retain_the_previously_saved_record() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let cookie = f.cookie("network-alice").await;
    let (_, saved) = f
        .request(
            "POST",
            "/scripts/save",
            Some(&cookie),
            ORIGIN,
            json!({"title":"keep on failure", "script":SOURCE}),
        )
        .await;
    let path = f.root.join("scripts/network-alice.json");
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let (status, _) = f
        .request(
            "POST",
            "/scripts/delete",
            Some(&cookie),
            ORIGIN,
            json!({"id":saved["record"]["id"]}),
        )
        .await;
    assert!(!status.is_success());
    assert_eq!(
        f.get("/scripts", &cookie).await.as_array().unwrap().len(),
        1,
        "failed deletion hid the saved script from the API cache"
    );
}

#[tokio::test]
async fn script_corrupt_snapshot_must_not_be_overwritten_by_a_successful_save() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let cookie = f.cookie("network-alice").await;
    std::fs::create_dir_all(f.root.join("scripts")).unwrap();
    let path = f.root.join("scripts/network-alice.json");
    let corrupt = b"{ incomplete previous snapshot";
    std::fs::write(&path, corrupt).unwrap();
    let (status, _) = f
        .request(
            "POST",
            "/scripts/save",
            Some(&cookie),
            ORIGIN,
            json!({"title":"new source", "script":SOURCE}),
        )
        .await;
    let original_preserved = std::fs::read(path).unwrap() == corrupt;
    assert!(!status.is_success() && original_preserved,
        "unreadable snapshot was overwritten: status={status}, original_preserved={original_preserved}");
}

#[tokio::test]
async fn teacher_header_selection_reloads_and_is_readable_but_not_mutable_by_students() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let teacher = f.cookie("network-teacher").await;
    let student = f.cookie("network-student").await;
    let selection = json!({"id":"bpf_helpers", "selected":true});
    assert_eq!(
        f.request(
            "POST",
            "/modules/c-headers/select",
            Some(&student),
            ORIGIN,
            selection.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request(
            "POST",
            "/modules/c-headers/select",
            Some(&teacher),
            ORIGIN,
            selection
        )
        .await
        .0,
        StatusCode::OK
    );
    let metadata = f
        .get("/modules/c-headers/selected-metadata", &student)
        .await;
    assert_eq!(metadata["selected_headers"][0]["id"], "bpf_helpers");
    assert_eq!(metadata["selected_headers"][0]["downloaded"], false);
    assert_eq!(
        CHeaderModule::default()
            .selected_metadata()
            .await
            .selected_headers
            .len(),
        1
    );
    assert_eq!(
        f.request(
            "POST",
            "/modules/c-headers/delete",
            Some(&teacher),
            ORIGIN,
            json!({"id":"bpf_helpers"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(f
        .get("/modules/c-headers/selected-metadata", &student)
        .await["selected_headers"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
#[cfg(unix)]
async fn header_download_checksum_failure_cannot_publish_a_downloaded_resource() {
    let _guard = ENV_LOCK.lock().await;
    let mut f = Fixture::new();
    let teacher = f.cookie("network-teacher").await;
    f.use_forged_downloader();
    let (status, result) = f
        .request(
            "POST",
            "/modules/c-headers/download",
            Some(&teacher),
            ORIGIN,
            json!({"id":"bpf_helpers"}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(result["message"]
        .as_str()
        .unwrap()
        .contains("signature mismatch"));
    assert!(!f.root.join("c_headers/bpf_helpers.h").exists());
    let catalog = f.get("/modules/c-headers/catalog", &teacher).await;
    assert!(catalog["headers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["downloaded"] == false));
}
