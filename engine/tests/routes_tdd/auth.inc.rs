#[tokio::test]
async fn post_auth_register_should_create_user_with_totp_bootstrap_payload() {
    let app = build_router(build_state());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"alice","password":"alice-pass-123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["account_name"], "alice");
    assert!(json["secret"].is_string());
}

#[tokio::test]
async fn post_auth_change_password_should_require_valid_otp_and_update_login_password() {
    let state = build_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;

    let alice_otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let alice_cookie = login_for_user(&app, "alice", "alice-pass-123", &alice_otp).await;

    let change_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/password/change")
                .header("content-type", "application/json")
                .header(header::COOKIE, alice_cookie)
                .body(Body::from(
                    serde_json::json!({
                        "current_password": "alice-pass-123",
                        "new_password": "alice-pass-456",
                        "otp": alice_otp,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(change_response.status(), StatusCode::OK);

    // old password should fail
    let old_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "alice",
                        "password": "alice-pass-123",
                        "otp": state.auth_service.generate_current_totp_for_user("alice").unwrap(),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);

    let new_login = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "alice",
                        "password": "alice-pass-456",
                        "otp": state.auth_service.generate_current_totp_for_user("alice").unwrap(),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(new_login.status(), StatusCode::OK);
}

#[tokio::test]
async fn post_auth_delete_should_remove_user_and_invalidate_login() {
    let state = build_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;

    let otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let session_cookie = login_for_user(&app, "alice", "alice-pass-123", &otp).await;

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/delete")
                .header("content-type", "application/json")
                .header(header::COOKIE, session_cookie)
                .body(Body::from(
                    serde_json::json!({
                        "password": "alice-pass-123",
                        "otp": otp,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_response.status(), StatusCode::OK);

    let login_after_delete = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "alice",
                        "password": "alice-pass-123",
                        "otp": state.auth_service.generate_current_totp_for_user("alice").unwrap_or_else(|| "000000".to_string()),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(login_after_delete.status(), StatusCode::UNAUTHORIZED);
}

async fn login_and_get_session_cookie(app: &Router, otp: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "admin",
                        "password": "cyanrex-admin",
                        "otp": otp,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("set-cookie should exist")
        .to_string();

    let session_pair = set_cookie
        .split(';')
        .next()
        .expect("cookie pair should exist")
        .to_string();

    assert!(session_pair.starts_with("cyanrex_session="));
    session_pair
}

async fn register_user(app: &Router, username: &str, password: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": username,
                        "password": password,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn login_for_user(app: &Router, username: &str, password: &str, otp: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": username,
                        "password": password,
                        "otp": otp,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookie| cookie.split(';').next())
        .unwrap_or_default()
        .to_string()
}
