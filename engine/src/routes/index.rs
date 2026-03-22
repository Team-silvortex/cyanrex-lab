use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct IndexResponse {
    pub name: &'static str,
    pub status: &'static str,
}

pub async fn index() -> Json<IndexResponse> {
    Json(IndexResponse {
        name: "cyanrex-engine",
        status: "running",
    })
}
