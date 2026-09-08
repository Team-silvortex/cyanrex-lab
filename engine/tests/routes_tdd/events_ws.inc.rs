use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::{client::IntoClientRequest, Message as WsMessage}};

struct EventWsServer(tokio::task::JoinHandle<()>);

impl Drop for EventWsServer {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn event_ws_server(app: Router) -> (EventWsServer, String) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (EventWsServer(task), format!("ws://{address}/ws/events"))
}

fn ws_request(url: &str, cookie: Option<&str>) -> tokio_tungstenite::tungstenite::http::Request<()> {
    let mut request = url.into_client_request().unwrap();
    request.headers_mut().insert(header::ORIGIN, "http://localhost:3000".parse().unwrap());
    if let Some(cookie) = cookie {
        request.headers_mut().insert(header::COOKIE, cookie.parse().unwrap());
    }
    request
}

fn ws_test_event(username: &str, index: usize) -> cyanrex_engine::models::event::Event {
    use cyanrex_engine::models::event::{Event, EventCategory, EventColor, EventSeverity};
    Event {
        username: username.to_owned(), timestamp: chrono::Utc::now(), source: "ws-test".into(),
        event_type: "test.event".into(), category: EventCategory::Kernel,
        severity: EventSeverity::Success, color: EventColor::Green,
        payload: serde_json::json!({"index": index}),
    }
}

#[tokio::test]
async fn event_websocket_requires_a_session_and_allowed_origin() {
    let _guard = CSRF_ENV_LOCK.lock().await;
    let state = test_state();
    let app = build_router(state.clone());
    let otp = state.auth_service.generate_current_totp_for_user("admin").unwrap();
    let cookie = login_and_get_session_cookie(&app, &otp).await;
    let (_server, url) = event_ws_server(app).await;
    let error = connect_async(ws_request(&url, None)).await.unwrap_err();
    assert!(matches!(error, tokio_tungstenite::tungstenite::Error::Http(response)
        if response.status() == StatusCode::UNAUTHORIZED));
    let mut request = ws_request(&url, Some(&cookie));
    request.headers_mut().insert(header::ORIGIN, "http://untrusted.invalid".parse().unwrap());
    let error = connect_async(request).await.unwrap_err();
    assert!(matches!(error, tokio_tungstenite::tungstenite::Error::Http(response)
        if response.status() == StatusCode::FORBIDDEN));
    let mut request = ws_request(&url, Some(&cookie));
    request.headers_mut().remove(header::ORIGIN);
    let error = connect_async(request).await.unwrap_err();
    assert!(matches!(error, tokio_tungstenite::tungstenite::Error::Http(response)
        if response.status() == StatusCode::FORBIDDEN));
}

#[tokio::test]
async fn event_websocket_keeps_raw_event_frames_and_owner_isolation() {
    let _guard = CSRF_ENV_LOCK.lock().await;
    let state = test_state();
    let app = build_router(state.clone());
    let otp = state.auth_service.generate_current_totp_for_user("admin").unwrap();
    let cookie = login_and_get_session_cookie(&app, &otp).await;
    let (_server, url) = event_ws_server(app).await;
    let (mut socket, _) = connect_async(ws_request(&url, Some(&cookie))).await.unwrap();
    state.event_bus.publish(ws_test_event("another-user", 1)).await;
    let event = ws_test_event("admin", 2);
    state.event_bus.publish(event.clone()).await;
    let message = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await.unwrap().unwrap().unwrap();
    let WsMessage::Text(text) = message else { panic!("expected a raw Event frame") };
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), serde_json::to_value(event).unwrap());
    socket.close(None).await.unwrap();
}

#[tokio::test]
async fn event_websocket_lag_closes_with_resync_code_instead_of_silent_loss() {
    let _guard = CSRF_ENV_LOCK.lock().await;
    let mut state = test_state();
    std::sync::Arc::get_mut(&mut state).unwrap().event_bus =
        cyanrex_engine::services::event_bus::EventBus::new(1);
    let app = build_router(state.clone());
    let otp = state.auth_service.generate_current_totp_for_user("admin").unwrap();
    let cookie = login_and_get_session_cookie(&app, &otp).await;
    let (_server, url) = event_ws_server(app).await;
    let (mut socket, _) = connect_async(ws_request(&url, Some(&cookie))).await.unwrap();
    // A burst exceeds the tiny test channel before the current-thread socket task can drain it.
    // Even lag caused by other users requires conservative resync, without leaking their counts.
    for index in 0..64 {
        state.event_bus.publish(ws_test_event("another-user", index)).await;
    }
    let message = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await.expect("lag must not be silently ignored").unwrap().unwrap();
    let WsMessage::Close(Some(frame)) = message else { panic!("expected resync close") };
    assert_eq!(u16::from(frame.code), 1013);
    assert_eq!(frame.reason, "event stream lagged; reload /events");
}
