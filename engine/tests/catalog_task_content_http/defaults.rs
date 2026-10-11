use super::*;

#[tokio::test]
async fn catalog_http_missing_definition_ref_never_falls_back_to_manual_creation() {
    let f = Offline::new().await;
    let manual =
        manual_http_fixture::create_body(TASK.parse().unwrap(), &manifest("Not manual", vec![]));
    offline_error(
        &f,
        request("POST", COLLECTION, TOKEN, manual),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
}
