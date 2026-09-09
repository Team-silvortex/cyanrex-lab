use super::*;
use crate::services::learning_store::LearningRunOutcome;
use crate::sqlx_compat::PgPoolOptions;

#[tokio::test]
#[ignore = "requires CYANREX_TEST_DATABASE_URL pointing to a disposable PostgreSQL database"]
async fn postgres_feedback_migrates_legacy_rows_and_serializes_updates() {
    let url =
        std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable test database URL required");
    let admin = PgPoolOptions::new().connect(&url).await.unwrap();
    let schema = format!("learning_feedback_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let connection_schema = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |connection, _| {
            let schema = connection_schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path', $1, false)")
                    .bind(schema)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    for statement in include_str!("../../../migrations/0004_learning_attempts.sql")
        .split(';')
        .filter(|statement| !statement.trim().is_empty())
    {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO learning_attempts
        (id, username, lab_id, source, source_sha256, run_success, stage,
         attach_expected, attach_verified, completed, feedback, created_at)
        VALUES ($1, 'student', '01-first-program', 'original source', 'hash', true,
         'run', false, false, true, 'accepted', now())",
    )
    .bind(&id)
    .execute(&pool)
    .await
    .unwrap();
    let path = std::env::temp_dir().join(format!("{schema}.json"));
    let mut store = LearningStore::with_local_data_path(path.clone());
    store.db_pool = Some(pool.clone());
    assert!(
        store.active_pool().is_some(),
        "CYANREX_DB_FALLBACK must be enabled"
    );
    let legacy = store.attempts_for_user("student").await;
    assert_eq!(legacy.len(), 1);
    assert!(legacy[0].teacher_feedback.is_none());
    let request = SaveTeacherFeedbackRequest {
        username: "student".into(),
        attempt_id: id,
        comment: "检查边界".into(),
        expected_revision: 0,
    };
    let (first, second) = tokio::join!(
        store.save_teacher_feedback("teacher", &request),
        store.save_teacher_feedback("admin", &request),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(
        matches!(first, Err(LearningFeedbackError::Conflict))
            || matches!(second, Err(LearningFeedbackError::Conflict))
    );
    let mut request = request;
    request.expected_revision = 1;
    request.comment = "Updated".into();
    let saved = store
        .save_teacher_feedback("teacher", &request)
        .await
        .unwrap();
    let mut reloaded = LearningStore::with_local_data_path(path.clone());
    reloaded.db_pool = Some(pool.clone());
    let attempts = reloaded.attempts_for_user("student").await;
    let feedback = attempts[0].teacher_feedback.as_ref().unwrap();
    assert_eq!(feedback.comment, "Updated");
    assert_eq!(feedback.reviewer, "teacher");
    assert_eq!(feedback.revision, 2);
    // PostgreSQL timestamps retain microseconds while Rust's clock can include nanoseconds.
    assert_eq!(
        feedback.updated_at.timestamp_micros(),
        saved.updated_at.timestamp_micros()
    );
    assert_eq!(attempts[0].source, "original source");
    assert!(attempts[0].completed);
    assert_eq!(attempts[0].feedback, vec!["accepted"]);
    let resumed = reloaded
        .attempt_for_user("student", &request.attempt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed.source, "original source");
    assert_eq!(resumed.teacher_feedback.as_ref().unwrap().revision, 2);
    assert!(reloaded
        .attempt_for_user("other-student", &request.attempt_id)
        .await
        .unwrap()
        .is_none());
    assert!(reloaded
        .attempt_for_user("student", &Uuid::new_v4().to_string())
        .await
        .unwrap()
        .is_none());
    request.username = "other-student".into();
    assert!(matches!(
        store.save_teacher_feedback("teacher", &request).await,
        Err(LearningFeedbackError::NotFound)
    ));
    request.username = "student".into();
    request.attempt_id = Uuid::new_v4().to_string();
    assert!(matches!(
        store.save_teacher_feedback("teacher", &request).await,
        Err(LearningFeedbackError::NotFound)
    ));
    let new_attempt = store
        .record_run(
            "student",
            LearningRunOutcome {
                lab_id: "01-first-program",
                template_id: Some("xdp-pass"),
                source: "source",
                run_success: false,
                stage: "compile",
                attach_expected: false,
                attach_verified: false,
            },
        )
        .await
        .unwrap();
    assert!(new_attempt.teacher_feedback.is_none());
    assert_eq!(reloaded.attempts_for_user("student").await.len(), 2);
    let resumed_new = reloaded
        .attempt_for_user("student", &new_attempt.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed_new.source, "source");
    assert_eq!(resumed_new.source_sha256, new_attempt.source_sha256);
    pool.close().await;
    assert!(reloaded
        .attempt_for_user("student", &new_attempt.id)
        .await
        .is_err());
    assert!(matches!(
        store.save_teacher_feedback("teacher", &request).await,
        Err(LearningFeedbackError::Storage(_))
    ));
    assert!(
        !path.exists(),
        "failed database writes must not create a local snapshot"
    );
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
