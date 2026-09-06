type AttachmentHttpError = (StatusCode, Json<Value>);

async fn attachment_owner(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, AttachmentHttpError> {
    crate::routes::auth::current_session_from_headers(state, headers)
        .await
        .map(|session| session.username)
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"message": "authentication required"})),
            )
        })
}

pub async fn list_attachments(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<EbpfAttachmentListResponse>, AttachmentHttpError> {
    let username = attachment_owner(state.as_ref(), &headers).await?;
    let attachments = state
        .runner_manager
        .list_attachments(&username)
        .await
        .map_err(attachment_http_error)?;
    Ok(Json(EbpfAttachmentListResponse {
        pin_paths: attachments
            .into_iter()
            .map(|attachment| attachment.pin_path)
            .collect(),
    }))
}

pub async fn list_attachment_details(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<EbpfAttachmentDetailListResponse>, AttachmentHttpError> {
    let username = attachment_owner(state.as_ref(), &headers).await?;
    let attachments = state
        .runner_manager
        .list_attachments(&username)
        .await
        .map_err(attachment_http_error)?;
    Ok(Json(EbpfAttachmentDetailListResponse { attachments }))
}

pub async fn detach_ebpf(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<EbpfDetachRequest>,
) -> (StatusCode, Json<EbpfDetachResponse>) {
    let username = match attachment_owner(state.as_ref(), &headers).await {
        Ok(username) => username,
        Err(_) => {
            return detach_failure(StatusCode::UNAUTHORIZED, "authentication required".into())
        }
    };
    match state
        .runner_manager
        .detach(RunnerDetachRequest {
            owner_username: &username,
            pin_path: payload.pin_path.as_deref(),
        })
        .await
    {
        Ok(outcome) => {
            let severity = if outcome.clean {
                EventSeverity::Success
            } else {
                EventSeverity::Warning
            };
            state
                .event_bus
                .publish(Event {
                    username,
                    timestamp: Utc::now(),
                    source: "module-ebpf".to_string(),
                    event_type: "ebpf.detached".to_string(),
                    category: EventCategory::Platform,
                    severity,
                    color: severity.color(),
                    payload: json!({
                        "detached": outcome.detached,
                        "clean": outcome.clean,
                        "safety_notes": outcome.safety_notes,
                    }),
                })
                .await;
            (
                StatusCode::OK,
                Json(EbpfDetachResponse {
                    ok: true,
                    message: if outcome.clean {
                        "detached cleanly"
                    } else {
                        "detached with safety warnings"
                    }
                    .into(),
                    detached: outcome.detached,
                    clean: outcome.clean,
                    safety_notes: outcome.safety_notes,
                }),
            )
        }
        Err(error) => detach_failure(runner_operation_status(&error), error.to_string()),
    }
}

fn attachment_http_error(error: RunnerOperationError) -> AttachmentHttpError {
    (
        runner_operation_status(&error),
        Json(json!({"message": error.to_string()})),
    )
}

fn detach_failure(status: StatusCode, message: String) -> (StatusCode, Json<EbpfDetachResponse>) {
    (
        status,
        Json(EbpfDetachResponse {
            ok: false,
            message,
            detached: Vec::new(),
            clean: false,
            safety_notes: vec!["detach failed; cleanup is not confirmed".to_string()],
        }),
    )
}
