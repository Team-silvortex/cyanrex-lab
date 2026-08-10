use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use cyanrex_engine::{
    build_router, build_state,
    models::{
        runner_agent::RunnerAgentIsolation,
        runner_job::{RunnerCompileReport, RunnerJobState},
    },
    services::{
        runner_agent_client::{RunnerAgentClient, RunnerAgentClientConfig},
        runner_agent_executor::RunnerCompileExecutorConfig,
    },
};
use uuid::Uuid;

const TOKEN: &str = "runner-agent-client-integration-token-32-bytes";

#[tokio::test]
#[ignore = "requires a loopback TCP listener and Linux Clang with the BPF target"]
async fn runner_agent_client_completes_signed_probe_and_compile_jobs_over_http() {
    if !cfg!(target_os = "linux") {
        return;
    }
    std::env::set_var("CYANREX_RUNNER_AGENT_TOKEN", TOKEN);
    let state = build_state();
    std::env::remove_var("CYANREX_RUNNER_AGENT_TOKEN");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_router(state.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut labels = BTreeMap::new();
    labels.insert("test".to_string(), "integration".to_string());
    let work_root = std::env::temp_dir().join(format!(
        "cyanrex-agent-integration-{}",
        Uuid::new_v4().simple()
    ));
    let config = RunnerAgentClientConfig {
        engine_url: format!("http://{address}"),
        bootstrap_token: TOKEN.to_string(),
        agent_id: "client-test-agent".to_string(),
        isolation: RunnerAgentIsolation::Container,
        max_concurrent: 1,
        capabilities: vec!["control_probe".to_string(), "clang_check".to_string()],
        compile_check: Some(
            RunnerCompileExecutorConfig::new(PathBuf::from("/usr/bin/clang"), work_root.clone())
                .unwrap(),
        ),
        labels,
        poll_interval: Duration::from_millis(10),
        request_timeout: Duration::from_secs(5),
        run_once: true,
    };
    let mut client = RunnerAgentClient::new(config).unwrap();
    let agent = client.register().await.unwrap();
    assert_eq!(agent.agent_id, "client-test-agent");

    let submitted = state
        .runner_job_queue
        .submit_probe(
            Some("client-test-agent".to_string()),
            "integration ping".to_string(),
            Some(30),
        )
        .unwrap();
    let completed = client.poll_once().await.unwrap().unwrap();
    assert_eq!(completed.job_id, submitted.job_id);
    assert_eq!(completed.state, RunnerJobState::Succeeded);
    assert!(completed
        .output
        .as_deref()
        .is_some_and(|output| output.contains("integration ping")));

    let submitted_compile = state
        .runner_job_queue
        .submit_user_compile_check(
            "integration-user".to_string(),
            "client-test-agent".to_string(),
            "int integration_compile(void) { return 0; }\n".to_string(),
            Some("integration-compile".to_string()),
            Some(20),
        )
        .unwrap();
    let compiled = client.poll_once().await.unwrap().unwrap();
    assert_eq!(compiled.job_id, submitted_compile.job_id);
    assert_eq!(compiled.state, RunnerJobState::Succeeded);
    let report: RunnerCompileReport =
        serde_json::from_str(compiled.output.as_deref().unwrap()).unwrap();
    assert!(report.success);
    assert!(report.object_bytes.is_some_and(|size| size > 0));
    assert!(report.object_sha256.is_some_and(|hash| hash.len() == 64));

    let inventory = state.runner_job_queue.inventory();
    assert!(inventory
        .jobs
        .iter()
        .all(|job| job.state == RunnerJobState::Succeeded));
    server.abort();
    std::fs::remove_dir(work_root).unwrap();
}
