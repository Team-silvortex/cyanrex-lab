use super::*;
use axum::{
    body::{Body, Bytes},
    http::Response,
    routing::get,
};
use futures_util::{stream, StreamExt};
use std::convert::Infallible;

fn chunked(payload: Vec<u8>, never_finish: bool) -> Body {
    let chunks: Vec<Result<Bytes, Infallible>> = payload
        .chunks(8192)
        .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
        .collect();
    if never_finish {
        Body::from_stream(stream::iter(chunks).chain(stream::pending()))
    } else {
        Body::from_stream(stream::iter(chunks))
    }
}

async fn response_for(
    body: Vec<u8>,
    never_finish: bool,
    status: u16,
) -> (TestServer, reqwest::Response) {
    let app = Router::new().route(
        "/",
        get(move || {
            let body = body.clone();
            async move {
                Response::builder()
                    .status(status)
                    .body(chunked(body, never_finish))
                    .unwrap()
            }
        }),
    );
    let server = TestServer::start(app).await;
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let response = client.get(&server.url).send().await.unwrap();
    assert!(
        response.content_length().is_none(),
        "fixture must exercise streaming, not the size hint"
    );
    (server, response)
}

#[tokio::test]
async fn oversized_chunked_response_is_rejected_before_waiting_for_eof() {
    for status in [200, 500] {
        let (_server, response) =
            response_for(vec![b'x'; MAX_RESPONSE_BYTES as usize + 1], true, status).await;
        let outcome = tokio::time::timeout(
            Duration::from_secs(2),
            decode_response::<serde_json::Value>(response),
        )
        .await;
        assert!(
            outcome.is_ok(),
            "must stop as soon as 640 KiB is exceeded, without waiting for EOF"
        );
        assert!(
            matches!(outcome.unwrap(), Err(RunnerAgentClientError::Protocol(message)) if message.contains("640 KiB"))
        );
    }
}

#[tokio::test]
async fn exact_limit_chunked_json_is_accepted() {
    let payload = serde_json::to_vec(&"x".repeat(MAX_RESPONSE_BYTES as usize - 2)).unwrap();
    assert_eq!(payload.len(), MAX_RESPONSE_BYTES as usize);
    let (_server, response) = response_for(payload, false, 200).await;
    assert_eq!(
        decode_response::<String>(response).await.unwrap().len(),
        MAX_RESPONSE_BYTES as usize - 2
    );
}

#[tokio::test]
async fn small_chunked_server_error_keeps_its_status_and_retry_policy() {
    let (_server, response) = response_for(
        br#"{"message":"synthetic unavailable"}"#.to_vec(),
        false,
        503,
    )
    .await;
    let error = decode_response::<serde_json::Value>(response)
        .await
        .unwrap_err();
    assert!(error.retryable());
    assert!(matches!(
        error,
        RunnerAgentClientError::Server {
            status: StatusCode::SERVICE_UNAVAILABLE,
            ..
        }
    ));
}
