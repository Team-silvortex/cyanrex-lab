use super::fixture::{server, success, text, Fixture};
use axum::http::StatusCode;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

#[test]
fn generated_deploy_rejects_health_configuration_before_docker_up() {
    let fixture = Fixture::new();
    for (key, value) in [
        ("CYANREX_DEPLOY_WAIT_FOR_HEALTH", "bad"),
        ("CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS", "0"),
        ("CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS", "3601"),
        ("CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS", "--help"),
        ("CYANREX_DEPLOY_HEALTH_POLL_SECONDS", "-h"),
        ("CYANREX_DEPLOY_HEALTH_POLL_SECONDS", "0"),
        ("CYANREX_DEPLOY_HEALTH_POLL_SECONDS", "bad"),
        ("CYANREX_ENGINE_PORT", "8080@other.invalid"),
        ("CYANREX_BIND_ADDRESS", "user@other.invalid"),
    ] {
        let output = fixture.run(&["up"], &[(key, value.into())]);
        assert!(
            !output.status.success(),
            "accepted {key}={value}: {}",
            text(&output)
        );
        assert!(
            !fixture.calls().contains(" up -d"),
            "invalid config started services"
        );
        assert!(!text(&output).contains("Service endpoints"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generated_deploy_success_checks_real_typed_health_and_preserves_up_arguments() {
    let fixture = Fixture::new();
    let (url, requests, handle) =
        server("{\"status\":\"ok\"}", StatusCode::OK, Duration::ZERO).await;
    let port = url.rsplit(':').next().unwrap().to_string();
    let output = fixture.run(
        &["up", "--pull", "never"],
        &[
            ("CYANREX_ENGINE_PORT", port),
            ("CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS", "2".into()),
        ],
    );
    handle.abort();
    success(&output);
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    assert!(fixture.calls().contains("up -d --pull never"));
    assert!(!fixture.calls().contains(" down"));
    assert!(text(&output).contains("Service endpoints"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generated_deploy_failed_health_preserves_state_and_explains_inspection_without_ready() {
    let fixture = Fixture::new();
    let (url, requests, handle) =
        server("{\"status\":\"degraded\"}", StatusCode::OK, Duration::ZERO).await;
    let output = fixture.run(
        &["up"],
        &[
            (
                "CYANREX_ENGINE_PORT",
                url.rsplit(':').next().unwrap().into(),
            ),
            ("CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS", "1".into()),
            ("CYANREX_DEPLOY_HEALTH_POLL_SECONDS", "1".into()),
        ],
    );
    handle.abort();
    assert!(!output.status.success(), "{}", text(&output));
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    assert!(!text(&output).contains("Service endpoints"));
    assert!(
        text(&output).contains("status")
            && text(&output).contains("logs engine")
            && text(&output).contains("no automatic rollback")
    );
    assert!(!fixture.calls().contains(" down"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generated_deploy_actual_hanging_response_respects_one_total_health_deadline() {
    let fixture = Fixture::new();
    let (url, requests, handle) = server(
        "not healthy",
        StatusCode::SERVICE_UNAVAILABLE,
        Duration::from_secs(4),
    )
    .await;
    let started = Instant::now();
    let output = fixture.run(
        &["up"],
        &[
            (
                "CYANREX_ENGINE_PORT",
                url.rsplit(':').next().unwrap().into(),
            ),
            ("CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS", "1".into()),
            ("CYANREX_DEPLOY_HEALTH_POLL_SECONDS", "1".into()),
            ("TEST_REAL_HTTP", "1".into()),
        ],
    );
    handle.abort();
    assert!(!output.status.success());
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "health deadline excluded request time"
    );
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    assert!(!fixture.calls().contains(" down"));
}

#[test]
fn generated_deploy_can_skip_health_and_keeps_status_logs_and_manual_down_compatible() {
    let fixture = Fixture::new();
    let output = fixture.run(
        &["up", "postgres"],
        &[
            ("CYANREX_DEPLOY_WAIT_FOR_HEALTH", "0".into()),
            ("CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS", "bad".into()),
        ],
    );
    success(&output);
    assert!(text(&output).contains("readiness unverified"));
    for args in [
        &["status"][..],
        &["logs", "engine"][..],
        &["down", "--volumes", "--remove-orphans"][..],
    ] {
        success(&fixture.run(args, &[]));
    }
    let calls = fixture.calls();
    assert!(calls.contains("up -d postgres"));
    assert!(calls.contains("logs -f engine"));
    assert!(calls.contains("--profile runner-agent down --volumes --remove-orphans"));
}

#[test]
fn generated_deploy_fails_closed_for_missing_native_and_failed_compose_without_rollback() {
    let missing = Fixture::new();
    std::fs::remove_file(missing.package.join("cyanrex-release")).unwrap();
    let output = missing.run(&["up"], &[]);
    assert!(!output.status.success());
    assert!(text(&output).contains("matching packaged cyanrex-release"));
    assert!(!missing.calls().contains(" up -d"));

    let failed = Fixture::new();
    let output = failed.run(&["up"], &[("TEST_UP_STATUS", "17".into())]);
    assert_eq!(output.status.code(), Some(17));
    assert!(!text(&output).contains("Service endpoints"));
    assert!(!failed.calls().contains(" down"));
}
