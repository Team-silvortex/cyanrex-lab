use super::fixture::{command, complete, server, success, text};
use axum::http::StatusCode;
use std::time::Instant;
use std::{sync::atomic::Ordering, time::Duration};

#[test]
fn wait_health_check_only_accepts_lan_without_issuing_requests() {
    for url in [
        "http://192.0.2.20:8080",
        "http://classroom.invalid:8080",
        "https://teacher.invalid",
        "http://[::1]:8080",
    ] {
        let output =
            complete(command().args(["smoke", "wait-health", "--engine-url", url, "--check-only"]));
        success(&output);
        assert!(text(&output).contains("no request"));
    }
    for args in [&["--help"][..], &["smoke", "--help"][..]] {
        assert!(text(&complete(command().args(args))).contains("wait-health"));
    }
}

#[test]
fn wait_health_rejects_unsafe_urls_and_invalid_budgets_before_networking() {
    for url in [
        "http://@host.invalid",
        "http://user:secret@host.invalid",
        "file:///tmp/file",
        "http://host.invalid/path",
        "http://host.invalid?query",
        "http://host.invalid#fragment",
        "http://host.invalid:0",
        " http://host.invalid",
    ] {
        let output =
            complete(command().args(["smoke", "wait-health", "--engine-url", url, "--check-only"]));
        assert!(!output.status.success());
        assert!(text(&output).contains("health URL"), "{}", text(&output));
    }
    for (key, values) in [
        (
            "--timeout-seconds",
            vec![
                "0",
                "-1",
                "3601",
                "bad",
                "--help",
                "-h",
                "99999999999999999999999",
            ],
        ),
        ("--poll-seconds", vec!["0", "61", "bad"]),
    ] {
        for value in values {
            let output = complete(command().args([
                "smoke",
                "wait-health",
                "--engine-url",
                "http://192.0.2.20:8080",
                key,
                value,
                "--check-only",
            ]));
            assert!(!output.status.success());
            assert!(text(&output).contains(key), "{}", text(&output));
        }
    }
    let output = complete(command().args(["smoke", "wait-health", "--check-only"]));
    assert!(!output.status.success());
    assert!(text(&output).contains("--engine-url"));
}

#[test]
fn original_health_smoke_keeps_its_plaintext_lan_rejection() {
    let output =
        complete(command().args(["smoke", "health", "--engine-url", "http://192.0.2.20:8080"]));
    assert!(!output.status.success());
    assert!(text(&output).contains("plaintext Engine smoke URLs must use a loopback host"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_health_caps_body_without_echoing_server_contents() {
    let body =
        Box::leak(format!("SYNTHETIC_PRIVATE_SERVER_BODY{}", "x".repeat(4096)).into_boxed_str());
    let (url, requests, handle) = server(body, StatusCode::OK, Duration::ZERO).await;
    let output = complete(command().args([
        "smoke",
        "wait-health",
        "--engine-url",
        &url,
        "--timeout-seconds",
        "1",
    ]));
    handle.abort();
    assert!(!output.status.success());
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    assert!(text(&output).contains("health response too large"));
    assert!(!text(&output).contains("SYNTHETIC_PRIVATE_SERVER_BODY"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_health_total_deadline_includes_a_stalled_chunked_body_after_headers() {
    let (url, requests, handle) = super::fixture::streaming_server(vec![
        ("{\"status\":\"", Duration::ZERO),
        ("ok\"}", Duration::from_secs(4)),
    ])
    .await;
    let started = Instant::now();
    let output = complete(command().args([
        "smoke",
        "wait-health",
        "--engine-url",
        &url,
        "--timeout-seconds",
        "1",
    ]));
    handle.abort();
    assert!(!output.status.success());
    assert!(text(&output).contains("deadline exceeded"));
    assert!(started.elapsed() < Duration::from_millis(2500));
    assert_eq!(requests.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_health_caps_accumulated_frames_without_content_length() {
    let first = Box::leak("a".repeat(3000).into_boxed_str());
    let second = Box::leak("b".repeat(2000).into_boxed_str());
    let (url, requests, handle) = super::fixture::streaming_server(vec![
        (first, Duration::ZERO),
        (second, Duration::from_millis(10)),
    ])
    .await;
    let output = complete(command().args([
        "smoke",
        "wait-health",
        "--engine-url",
        &url,
        "--timeout-seconds",
        "1",
    ]));
    handle.abort();
    assert!(!output.status.success());
    assert!(
        text(&output).contains("health response too large"),
        "{}",
        text(&output)
    );
    assert_eq!(requests.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_health_retries_degraded_to_ok_without_resetting_the_total_budget() {
    for (delay, succeeds) in [(Duration::ZERO, true), (Duration::from_millis(1400), false)] {
        let (url, requests, handle) = super::fixture::recovering_server(delay).await;
        let started = Instant::now();
        let output = complete(command().args([
            "smoke",
            "wait-health",
            "--engine-url",
            &url,
            "--timeout-seconds",
            "2",
            "--poll-seconds",
            "1",
        ]));
        handle.abort();
        assert_eq!(output.status.success(), succeeds, "{}", text(&output));
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "retry reset the deadline"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_health_requires_typed_ok_without_redirects_or_proxy_credentials() {
    for (body, status, expected) in [
        ("{\"status\":\"ok\"}", StatusCode::OK, true),
        ("<html>ok</html>", StatusCode::OK, false),
        ("{\"status\":\"degraded\"}", StatusCode::OK, false),
        ("{\"status\":\"ok\"}", StatusCode::FOUND, false),
        (
            "{\"status\":\"ok\",\"status\":\"ok\"}",
            StatusCode::OK,
            false,
        ),
    ] {
        let (url, requests, handle) = server(body, status, Duration::ZERO).await;
        let output = complete(
            command()
                .args([
                    "smoke",
                    "wait-health",
                    "--engine-url",
                    &url,
                    "--timeout-seconds",
                    "1",
                    "--poll-seconds",
                    "1",
                ])
                .env("http_proxy", "http://127.0.0.1:1")
                .env("ALL_PROXY", "http://127.0.0.1:1"),
        );
        handle.abort();
        assert_eq!(output.status.success(), expected, "{}", text(&output));
        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }
}
