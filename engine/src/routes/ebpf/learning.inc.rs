async fn record_learning_run(
    state: &AppState,
    username: &str,
    lab_id: Option<&str>,
    template_id: Option<&str>,
    source: &str,
    result: &EbpfRunResponse,
    attach_expected: bool,
    attach_verified: bool,
) {
    let Some(lab_id) = lab_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    let outcome = crate::services::learning_store::LearningRunOutcome {
        lab_id,
        template_id,
        source,
        run_success: result.success,
        stage: &result.stage,
        attach_expected,
        attach_verified,
    };
    match state.learning_store.record_run(username, outcome).await {
        Ok(attempt) => {
            state
                .event_bus
                .publish(Event {
                    username: username.to_string(),
                    timestamp: Utc::now(),
                    source: "learning".to_string(),
                    event_type: if attempt.completed {
                        "learning.lab_completed".to_string()
                    } else {
                        "learning.lab_attempted".to_string()
                    },
                    category: EventCategory::Platform,
                    severity: if attempt.completed {
                        EventSeverity::Success
                    } else {
                        EventSeverity::Warning
                    },
                    color: if attempt.completed {
                        EventSeverity::Success.color()
                    } else {
                        EventSeverity::Warning.color()
                    },
                    payload: json!({
                        "lab_id": attempt.lab_id,
                        "attempt_id": attempt.id,
                        "completed": attempt.completed,
                        "feedback": attempt.feedback,
                    }),
                })
                .await;
        }
        Err(error) => tracing::warn!("learning attempt was not recorded: {error}"),
    }
}
