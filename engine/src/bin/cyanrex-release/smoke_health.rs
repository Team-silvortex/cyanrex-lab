use crate::smoke_http::SmokeHttpClient;
use serde::Deserialize;

#[derive(Deserialize)]
struct HealthResponse {
    status: String,
}

pub async fn run(engine_url: &str) -> Result<(), String> {
    let response: HealthResponse = SmokeHttpClient::public_get_json(engine_url, "/health").await?;
    if response.status != "ok" {
        return Err("Engine health payload is not ok".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;
    use axum::routing::get;
    use axum::{Json, Router};
    use serde_json::json;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn health_probe_requires_an_ok_payload() {
        let ok = Router::new().route("/health", get(|| async { Json(json!({"status": "ok"})) }));
        let (url, server) = serve(ok).await;
        run(&url).await.unwrap();
        server.abort();

        let failed = Router::new().route(
            "/health",
            get(|| async { Json(json!({"status": "degraded"})) }),
        );
        let (url, server) = serve(failed).await;
        assert!(run(&url).await.unwrap_err().contains("not ok"));
        server.abort();
    }

    async fn serve(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{address}"), server)
    }
}
