use axum::{
    http::header,
    response::{IntoResponse, Response},
};

const OPENAPI_DOCUMENT: &str = include_str!("../../openapi/openapi.json");

pub async fn document() -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        OPENAPI_DOCUMENT,
    )
        .into_response()
}
