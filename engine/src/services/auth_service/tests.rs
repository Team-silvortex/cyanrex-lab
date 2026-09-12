use super::*;

#[tokio::test]
async fn verified_memory_login_cannot_publish_after_credentials_change_or_removal() {
    let mut auth = AuthService::new_with_default_teacher();
    auth.db_pool = None;
    let verified = auth.default_admin.clone();
    for replacement in [Some("new-salt"), None] {
        if let Some(salt) = replacement {
            auth.users
                .write()
                .unwrap()
                .get_mut(&verified.username)
                .unwrap()
                .password_salt = salt.into();
        } else {
            auth.users.write().unwrap().remove(&verified.username);
        }
        let session = SessionRecord {
            token: Uuid::new_v4().to_string(),
            username: verified.username.clone(),
            expires_at: Utc::now() + Duration::hours(1),
        };
        assert!(matches!(
            auth.issue_verified_session(&verified, session, None).await,
            Err(AuthError::InvalidCredentials)
        ));
        assert!(auth.sessions.read().unwrap().is_empty());
    }
}

async fn auth_with_unavailable_schema() -> AuthService {
    let mut auth = AuthService::new_with_default_teacher();
    // A closed lazy pool fails immediately without connecting to any host or real database.
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic:synthetic@127.0.0.1:1/cyanrex_unreachable")
        .unwrap();
    pool.close().await;
    auth.db_pool = Some(pool);
    assert!(
        auth.active_pool().is_some(),
        "exercise the configured DB path"
    );
    auth
}

#[tokio::test]
async fn registration_after_schema_failure_returns_a_usable_fallback_account() {
    let auth = auth_with_unavailable_schema().await;
    let registered = auth
        .register("offline-student", "synthetic-password")
        .await
        .unwrap();
    assert!(
        auth.users.read().unwrap().contains_key("offline-student"),
        "registration must not report success without creating the account"
    );
    assert!(auth.db_disabled.load(Ordering::Relaxed));
    let uri = reqwest::Url::parse(&registered.otpauth_uri).unwrap();
    let uri_secret = uri
        .query_pairs()
        .find(|(name, _)| name == "secret")
        .unwrap()
        .1;
    assert_eq!(uri_secret, registered.secret);
    let otp = compute_current_totp_code(&uri_secret);
    let login = auth
        .login("offline-student", "synthetic-password", &otp)
        .await
        .unwrap();
    assert_eq!(
        auth.validate_session(&login.token).await.unwrap().username,
        "offline-student"
    );
    assert!(matches!(
        auth.login("offline-student", "wrong-password", &otp).await,
        Err(AuthError::InvalidCredentials)
    ));
}

#[tokio::test]
async fn schema_failure_cannot_reissue_secrets_for_an_existing_memory_account() {
    let auth = auth_with_unavailable_schema().await;
    let username = auth.default_admin.username.clone();
    let original = auth
        .users
        .read()
        .unwrap()
        .get(&username)
        .unwrap()
        .totp_secret
        .clone();
    assert!(matches!(
        auth.register(&username, "different-password").await,
        Err(AuthError::UserAlreadyExists)
    ));
    assert_eq!(
        auth.users
            .read()
            .unwrap()
            .get(&username)
            .unwrap()
            .totp_secret,
        original
    );
}

#[tokio::test]
async fn concurrent_registration_during_schema_failure_has_exactly_one_winner() {
    let auth = auth_with_unavailable_schema().await;
    let (first, second) = tokio::join!(
        auth.register("same-student", "first-synthetic-password"),
        auth.register(" SAME-STUDENT ", "second-synthetic-password")
    );
    let (created, duplicate, password) = if first.is_ok() {
        (first, second, "first-synthetic-password")
    } else {
        (second, first, "second-synthetic-password")
    };
    let created = created.unwrap();
    assert!(matches!(duplicate, Err(AuthError::UserAlreadyExists)));
    let otp = compute_current_totp_code(&created.secret);
    assert!(auth.login("same-student", password, &otp).await.is_ok());
}

#[test]
fn totp_uri_encodes_issuer_and_label_without_altering_credentials_or_parameters() {
    let secret = "JBSWY3DPEHPK3PXP======"; // Synthetic URI fixture only.
    for issuer in [
        "R&D / 教室 #2 + 100%",
        "Lab?secret=wrong&digits=8",
        "cyanrex-lab",
    ] {
        let uri = build_otpauth_uri(issuer, "student-one", secret);
        let parsed = reqwest::Url::parse(&uri).unwrap();
        assert_eq!(parsed.scheme(), "otpauth");
        assert_eq!(parsed.host_str(), Some("totp"));
        assert!(
            parsed.fragment().is_none(),
            "issuer text must never become a URI fragment"
        );
        assert_eq!(parsed.path_segments().unwrap().count(), 1);
        let parameters = parsed.query_pairs().into_owned().collect::<Vec<_>>();
        assert_eq!(
            parameters,
            vec![
                ("secret".into(), secret.into()),
                ("issuer".into(), issuer.into()),
                ("algorithm".into(), "SHA1".into()),
                ("digits".into(), "6".into()),
                ("period".into(), "30".into()),
            ]
        );
        if issuer.starts_with("R&D") {
            assert_eq!(
                parsed.path(),
                "/R%26D%20%2F%20%E6%95%99%E5%AE%A4%20%232%20%2B%20100%25:student-one"
            );
        }
    }
}

#[tokio::test]
async fn account_mutations_use_the_same_normalized_identity_as_login() {
    let mut auth = AuthService::new_with_default_teacher();
    auth.db_pool = None;
    let registration = auth
        .register("case-student", "synthetic-password")
        .await
        .unwrap();
    let otp = compute_current_totp_code(&registration.secret);
    let session = auth
        .login("case-student", "synthetic-password", &otp)
        .await
        .unwrap();
    auth.change_password(
        " CASE-STUDENT ",
        "synthetic-password",
        "changed-synthetic-password",
        &otp,
    )
    .await
    .unwrap();
    assert!(auth
        .login("case-student", "changed-synthetic-password", &otp)
        .await
        .is_ok());
    auth.delete_account(" CASE-STUDENT ", "changed-synthetic-password", &otp)
        .await
        .unwrap();
    assert!(!auth.users.read().unwrap().contains_key("case-student"));
    assert!(auth.validate_session(&session.token).await.is_none());
    assert!(auth
        .login("case-student", "changed-synthetic-password", &otp)
        .await
        .is_err());
}

#[tokio::test]
async fn concurrent_memory_password_changes_have_one_winner() {
    let mut auth = AuthService::new_with_default_teacher();
    auth.db_pool = None;
    let registration = auth
        .register("change-student", "synthetic-password")
        .await
        .unwrap();
    let otp = compute_current_totp_code(&registration.secret);
    let (first, second) = tokio::join!(
        auth.change_password(
            "change-student",
            "synthetic-password",
            "first-synthetic-change",
            &otp
        ),
        auth.change_password(
            " CHANGE-STUDENT ",
            "synthetic-password",
            "second-synthetic-change",
            &otp
        )
    );
    let password = if first.is_ok() {
        "first-synthetic-change"
    } else {
        "second-synthetic-change"
    };
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(auth.login("change-student", password, &otp).await.is_ok());
}
