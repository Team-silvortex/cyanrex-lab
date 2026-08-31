#[tokio::test]
async fn post_command_should_return_structured_module_results_for_admin() {
    let state = test_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let session_cookie = login_and_get_session_cookie(&app, &otp).await;

    let start_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/command")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, &session_cookie)
                .body(Body::from(
                    r#"{"commandType":"StartModule","moduleName":"  module-network  "}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(start_response.status(), StatusCode::OK);
    let start_payload = start_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let start_json: Value = serde_json::from_slice(&start_payload).unwrap();
    assert_eq!(start_json["ok"], true);
    assert_eq!(start_json["commandType"], "StartModule");
    assert_eq!(start_json["module"]["name"], "module-network");
    assert_eq!(start_json["module"]["status"], "running");

    let list_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/command")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, &session_cookie)
                .body(Body::from(r#"{"commandType":"ListModules"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_payload = list_response.into_body().collect().await.unwrap().to_bytes();
    let list_json: Value = serde_json::from_slice(&list_payload).unwrap();
    assert_eq!(list_json["ok"], true);
    assert_eq!(list_json["commandType"], "ListModules");
    let modules = list_json["modules"].as_array().unwrap();
    let network = modules
        .iter()
        .find(|module| module["name"] == "module-network")
        .expect("module-network should be discovered");
    assert_eq!(network["status"], "running");
    assert_eq!(network["version"], env!("CARGO_PKG_VERSION"));
    assert!(network["capabilities"].is_array());
}

#[tokio::test]
async fn post_command_should_validate_module_name_and_describe_experiment_handoff() {
    let state = test_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let session_cookie = login_and_get_session_cookie(&app, &otp).await;

    let invalid_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/command")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, &session_cookie)
                .body(Body::from(
                    r#"{"commandType":"StopModule","moduleName":"   "}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);
    let invalid_payload = invalid_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let invalid_json: Value = serde_json::from_slice(&invalid_payload).unwrap();
    assert_eq!(invalid_json["ok"], false);
    assert!(invalid_json["message"]
        .as_str()
        .unwrap_or_default()
        .contains("module name"));

    let unknown_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/command")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, &session_cookie)
                .body(Body::from(
                    r#"{"commandType":"StartModule","moduleName":"module-unknown"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(unknown_response.status(), StatusCode::BAD_REQUEST);
    let unknown_payload = unknown_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let unknown_json: Value = serde_json::from_slice(&unknown_payload).unwrap();
    assert_eq!(unknown_json["ok"], false);
    assert!(unknown_json["message"]
        .as_str()
        .unwrap_or_default()
        .contains("unknown module"));

    let experiment_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/command")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, &session_cookie)
                .body(Body::from(r#"{"commandType":"RunExperiment"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(experiment_response.status(), StatusCode::OK);
    let experiment_payload = experiment_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let experiment_json: Value = serde_json::from_slice(&experiment_payload).unwrap();
    assert_eq!(experiment_json["ok"], true);
    assert_eq!(experiment_json["commandType"], "RunExperiment");
    assert_eq!(experiment_json["nextPath"], "/ebpf");
}

#[tokio::test]
async fn post_command_should_be_forbidden_for_teacher() {
    let state = test_state();
    let app = build_router(state.clone());
    register_user(&app, "teacher", "teacher-pass-123").await;
    let otp = state
        .auth_service
        .generate_current_totp_for_user("teacher")
        .expect("teacher otp should exist");
    let session_cookie = login_for_user(&app, "teacher", "teacher-pass-123", &otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/command")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, session_cookie)
                .body(Body::from(r#"{"commandType":"ListModules"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
